//! Typed references form a tree: a project can reference guidelines and other CretSpec
//! projects, and those projects bring their own references along. Every repository of the
//! tree is cloned once into the workspace root; pinned guidelines get one snapshot per commit.

use crate::{
    files, git,
    manifest::{
        self, GUIDELINE_INSTRUCTIONS, GuidelinesMode, Kind, Lock, Manifest, ROLES_DIRECTORY,
    },
    repository,
    workspace::{self, Paths, Progress},
};
use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

/// The directory below the spec's `.local/` that holds referenced projects' context files.
pub const CONTEXT_DIRECTORY: &str = "context";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    /// Every project in the tree. The first is the workspace's own project.
    pub projects: Vec<Project>,
    /// The references of every project, including each project's own guidelines.
    pub references: Vec<Reference>,
    /// Problems in referenced branches of the tree. The rest of the tree keeps working.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    /// The name of the project's spec repository, as `scope` refers to it.
    pub spec_name: String,
    /// The project's spec repository.
    pub path: PathBuf,
    pub source: String,
    /// The reference that brought this project into the tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via: Option<usize>,
    pub depth: usize,
    /// The generated context file of a referenced project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<PathBuf>,
    pub purpose: Option<String>,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    /// The declaring project.
    pub project: usize,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    /// The repository's folder in the workspace.
    pub path: PathBuf,
    /// The guidance that is read: a pinned snapshot or the working tree.
    pub active: PathBuf,
    pub source: String,
    /// The project's own top-level guidelines.
    pub primary: bool,
    pub priority: i32,
    /// The declaration order within the project; its own guidelines come first.
    pub position: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Vec<String>>,
    /// A skill directory: a code repository's declared one, or a guidelines repository's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<PathBuf>,
    /// The referenced project of a project reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child: Option<usize>,
    pub purpose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

impl Tree {
    /// References declared by a project, in precedence order: a higher priority first,
    /// then declaration order. The project's own guidelines are declared first.
    pub fn ordered(&self, project: usize) -> Vec<usize> {
        let mut result: Vec<usize> = (0..self.references.len())
            .filter(|&i| self.references[i].project == project)
            .collect();
        result.sort_by_key(|&i| {
            (
                Reverse(self.references[i].priority),
                self.references[i].position,
            )
        });
        result
    }

    /// Whether the tree contains anything beyond the workspace's own guidelines and code.
    pub fn layered(&self) -> bool {
        self.references
            .iter()
            .any(|r| r.project != 0 || (!r.primary && r.kind != Kind::Code))
    }

    /// The folders of a project: its spec, its references and its referenced projects.
    pub fn folders(&self, project: usize) -> BTreeSet<PathBuf> {
        let mut result = BTreeSet::from([self.projects[project].path.clone()]);
        for reference in self.references.iter().filter(|r| r.project == project) {
            result.insert(reference.path.clone());
            if let Some(child) = reference.child {
                // A project referenced again deeper in the tree is not expanded again.
                if self.projects[child].depth > self.projects[project].depth {
                    result.extend(self.folders(child));
                }
            }
        }
        result
    }

    /// The folders where a reference's guidance applies. `None` means everywhere its
    /// declaring project applies.
    pub fn scoped_folders(&self, index: usize) -> Option<BTreeSet<PathBuf>> {
        let reference = &self.references[index];
        if let Some(child) = reference.child {
            return Some(self.folders(child));
        }
        if reference.scope.is_empty() {
            return None;
        }
        let project = &self.projects[reference.project];
        let mut result = BTreeSet::from([reference.path.clone()]);
        for name in &reference.scope {
            match self
                .references
                .iter()
                .find(|r| r.project == reference.project && &r.name == name && !r.primary)
            {
                Some(target) => match target.child {
                    Some(child) => result.extend(self.folders(child)),
                    None => {
                        result.insert(target.path.clone());
                    }
                },
                None if *name == project.spec_name => {
                    result.insert(project.path.clone());
                }
                // A repository that was skipped, for example because it is missing.
                None => {}
            }
        }
        Some(result)
    }

    /// The instruction files of a guidelines reference that exist, in reading order.
    pub fn instruction_files(&self, index: usize, active: &Path) -> Vec<PathBuf> {
        let reference = &self.references[index];
        let explicit = reference.instructions.as_ref().map(|files| {
            files
                .iter()
                .map(|file| join(active, file))
                .collect::<Vec<_>>()
        });
        explicit
            .unwrap_or_else(|| {
                GUIDELINE_INSTRUCTIONS
                    .iter()
                    .map(|file| join(active, file))
                    .chain(
                        self.projects[reference.project]
                            .profiles
                            .iter()
                            .map(|profile| active.join("profiles").join(format!("{profile}.md"))),
                    )
                    .collect()
            })
            .into_iter()
            .filter(|path| path.is_file())
            .collect()
    }
}

fn join(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// A declared entry of a project definition, normalized across schema versions.
struct Entry {
    name: String,
    kind: Kind,
    url: String,
    directory: String,
    primary: bool,
    priority: i32,
    scope: Vec<String>,
    instructions: Option<Vec<String>>,
    skills: Option<String>,
    purpose: Option<String>,
    profile: Option<String>,
    reference: Option<String>,
    commit: Option<String>,
    working_tree: bool,
}

fn entries(manifest: &Manifest, lock: Option<&Lock>) -> Vec<Entry> {
    let mode = manifest.guidelines_mode(lock);
    let mut result = vec![Entry {
        name: "guidelines".into(),
        kind: Kind::Guidelines,
        url: manifest.guidelines.repository.clone(),
        directory: String::new(),
        primary: true,
        priority: 0,
        scope: Vec::new(),
        instructions: None,
        skills: Some("skills".into()),
        purpose: None,
        profile: None,
        reference: Some(manifest.guidelines.reference.clone()),
        commit: lock.map(|l| l.commit.clone()),
        working_tree: mode == GuidelinesMode::WorkingTree,
    }];
    if let Some(code) = &manifest.code {
        result.push(Entry {
            name: manifest::LEGACY_CODE_NAME.into(),
            kind: Kind::Code,
            url: code.repository.clone(),
            directory: manifest.name.clone(),
            primary: false,
            priority: 0,
            scope: Vec::new(),
            instructions: None,
            skills: None,
            purpose: None,
            profile: manifest.profile.clone(),
            reference: None,
            commit: None,
            working_tree: false,
        });
    }
    for declared in manifest.repositories.iter().flatten() {
        let kind = declared.kind();
        result.push(Entry {
            name: declared.name.clone(),
            kind,
            url: declared.url.clone(),
            directory: declared.directory().to_owned(),
            primary: false,
            priority: declared.priority.unwrap_or(0),
            scope: declared.scope.clone().unwrap_or_default(),
            instructions: declared.instructions.clone(),
            skills: match kind {
                Kind::Guidelines => Some(declared.skills.clone().unwrap_or("skills".into())),
                Kind::Code => declared.skills.clone(),
                Kind::Project => None,
            },
            purpose: declared.purpose.clone(),
            profile: declared.profile.clone(),
            reference: declared.reference.clone(),
            commit: declared.commit.clone(),
            working_tree: declared.commit.is_none(),
        });
    }
    result
}

struct Pending {
    reference: usize,
    path: PathBuf,
    source: String,
    depth: usize,
}

struct Resolver<'a> {
    spec: &'a Path,
    root: &'a Path,
    create: bool,
    clone: bool,
    progress: Progress<'a>,
    tree: Tree,
    /// Repository identity to its folder.
    clones: BTreeMap<String, PathBuf>,
    /// Lowercase folder names in use, with the identity that uses them.
    used: BTreeMap<String, String>,
    /// The branch of each working-tree guidelines clone, with the project that chose it.
    branches: BTreeMap<String, (String, String)>,
    /// Project identity to its tree index.
    projects: BTreeMap<String, usize>,
}

impl Resolver<'_> {
    fn warn(&mut self, message: String) {
        self.tree.warnings.push(message);
    }

    /// Resolves a nested repository to its folder, cloning it when requested. Returns
    /// `None`, with a warning, when the branch cannot be used.
    /// A folder name that another repository already uses gets the declaring project's
    /// folder name as a prefix.
    fn folder(
        &mut self,
        entry: &Entry,
        source: &str,
        identity: &str,
        project: &str,
        prefix: &str,
    ) -> Option<PathBuf> {
        if let Some(path) = self.clones.get(identity) {
            return Some(path.clone());
        }
        let declared = if entry.directory.is_empty() {
            repository::directory_name(source).ok()?
        } else {
            entry.directory.clone()
        };
        // Prefer a free candidate that already holds this repository, then a free and
        // empty one, so the choice does not depend on the order of references.
        let candidates: Vec<String> = [declared.clone(), format!("{prefix}-{declared}")]
            .into_iter()
            .filter(|d| files::portable_name(d) && !self.used.contains_key(&d.to_ascii_lowercase()))
            .collect();
        let holds = |directory: &String| {
            repository::origin(&self.root.join(directory))
                .and_then(|origin| repository::identity(&origin))
                .is_ok_and(|found| found == identity)
                && git::root(&self.root.join(directory))
                    .is_ok_and(|root| files::same(&root, &self.root.join(directory)))
        };
        let directory = candidates
            .iter()
            .find(|d| holds(d))
            .or_else(|| {
                candidates
                    .iter()
                    .find(|d| !files::exists(&self.root.join(d)).unwrap_or(true))
            })
            .cloned();
        let Some(directory) = directory else {
            self.warn(format!(
                "Reference {} of project {project} needs a folder, but {declared} and {prefix}-{declared} are already used in the workspace root. It was skipped.",
                entry.name
            ));
            return None;
        };
        let path = self.root.join(&directory);
        let exists = files::exists(&path).unwrap_or(false);
        if !exists {
            if !self.clone {
                self.warn(format!(
                    "Reference {} of project {project} is missing: {}\nExpected source: {source}\nRun cspec project attach --clone-missing to clone it.",
                    entry.name,
                    path.display()
                ));
                return None;
            }
            (self.progress)(&format!("Fetching {} of {project}", entry.name));
            if let Err(error) = git::clone(source, &path) {
                self.warn(format!(
                    "Reference {} of project {project} could not be cloned from {source}: {error:#}",
                    entry.name
                ));
                return None;
            }
            if entry.kind == Kind::Guidelines
                && entry.working_tree
                && let Some(branch) = &entry.reference
                && let Err(error) = git::run(&path, ["switch", branch])
            {
                self.warn(format!(
                    "Guidelines {} of project {project} has no branch {branch}: {error:#}",
                    entry.name
                ));
            }
        }
        if let Err(error) = workspace::verify_repository(&path, source, &entry.name) {
            self.warn(format!("{error:#}"));
            return None;
        }
        self.used
            .insert(directory.to_ascii_lowercase(), identity.to_owned());
        self.clones.insert(identity.to_owned(), path.clone());
        Some(path)
    }

    /// The guidance that a guidelines reference reads.
    fn active(
        &mut self,
        entry: &Entry,
        path: &Path,
        identity: &str,
        project: &str,
    ) -> Result<PathBuf> {
        if !entry.working_tree {
            let lock = Lock {
                schema_version: 1,
                reference: entry.reference.clone().unwrap_or_default(),
                commit: entry.commit.clone().unwrap_or_default(),
            };
            return workspace::pinned_snapshot(self.spec, path, &lock, self.create, self.progress);
        }
        let branch = entry.reference.clone().unwrap_or_default();
        match self.branches.get(identity) {
            Some((chosen, by)) if *chosen != branch => self.warn(format!(
                "Guidelines {} of project {project} uses branch {branch}, but one working tree serves the whole workspace; the reference closest to the root, in {by}, decides: {chosen}.",
                entry.name
            )),
            Some(_) => {}
            None => {
                self.branches
                    .insert(identity.to_owned(), (branch, project.to_owned()));
            }
        }
        Ok(path.to_owned())
    }

    fn push(
        &mut self,
        project: usize,
        position: usize,
        entry: &Entry,
        path: PathBuf,
        active: PathBuf,
        source: String,
    ) -> usize {
        self.tree.references.push(Reference {
            project,
            name: entry.name.clone(),
            kind: entry.kind,
            skills: entry.skills.as_ref().map(|skills| {
                join(
                    if entry.kind == Kind::Guidelines {
                        &active
                    } else {
                        &path
                    },
                    skills,
                )
            }),
            path,
            active,
            source,
            primary: entry.primary,
            priority: entry.priority,
            position,
            scope: entry.scope.clone(),
            instructions: entry.instructions.clone(),
            child: None,
            purpose: entry.purpose.clone(),
            profile: entry.profile.clone(),
            reference: entry.reference.clone(),
            commit: entry.commit.clone(),
        });
        self.tree.references.len() - 1
    }
}

/// Rejects a cycle among projects, including one that a shorter path to a project hides
/// from the breadth-first resolution.
fn check_cycles(tree: &Tree) -> Result<()> {
    let edges = |project: usize| {
        tree.references
            .iter()
            .filter(move |r| r.project == project)
            .filter_map(|r| r.child)
    };
    // 0: unvisited, 1: on the current path, 2: done.
    let mut state = vec![0u8; tree.projects.len()];
    let mut path = Vec::new();
    fn visit(
        project: usize,
        tree: &Tree,
        edges: &dyn Fn(usize) -> Vec<usize>,
        state: &mut [u8],
        path: &mut Vec<usize>,
    ) -> Result<()> {
        state[project] = 1;
        path.push(project);
        for child in edges(project) {
            if state[child] == 1 {
                let start = path.iter().position(|&p| p == child).unwrap_or(0);
                let names: Vec<_> = path[start..]
                    .iter()
                    .chain([&child])
                    .map(|&p| tree.projects[p].name.as_str())
                    .collect();
                bail!(
                    "Reference cycle: {}. A project cannot reference itself, directly or through other projects.",
                    names.join(" -> ")
                );
            }
            if state[child] == 0 {
                visit(child, tree, edges, state, path)?;
            }
        }
        path.pop();
        state[project] = 2;
        Ok(())
    }
    let edges = |project: usize| edges(project).collect::<Vec<_>>();
    for project in 0..tree.projects.len() {
        if state[project] == 0 {
            visit(project, tree, &edges, &mut state, &mut path)?;
        }
    }
    Ok(())
}

/// Builds the reference tree. The workspace's own references come from the verified
/// layout. Problems below them become warnings, except a cycle, which stops resolution.
#[allow(clippy::too_many_arguments)]
pub fn resolve(
    spec: &Path,
    manifest: &Manifest,
    lock: Option<&Lock>,
    source: &str,
    layout: &Paths,
    active_guidelines: &Path,
    create: bool,
    clone: bool,
    progress: Progress<'_>,
) -> Result<Tree> {
    let mut resolver = Resolver {
        spec,
        root: &layout.root,
        create,
        clone,
        progress,
        tree: Tree::default(),
        clones: BTreeMap::new(),
        used: BTreeMap::from([(ROLES_DIRECTORY.to_owned(), String::new())]),
        branches: BTreeMap::new(),
        projects: BTreeMap::new(),
    };
    let identity = repository::identity(source)?;
    resolver.tree.projects.push(Project {
        name: manifest.name.clone(),
        spec_name: manifest.spec_name().to_owned(),
        path: spec.to_owned(),
        source: source.to_owned(),
        via: None,
        depth: 0,
        context: None,
        purpose: manifest
            .spec_repository
            .as_ref()
            .and_then(|s| s.purpose.clone()),
        profiles: manifest.profiles().into_iter().map(Into::into).collect(),
    });
    resolver.projects.insert(identity.clone(), 0);
    let mut folders = vec![
        (spec.to_owned(), source.to_owned()),
        (layout.guidelines.clone(), layout.guidelines_source.clone()),
    ];
    folders.extend(
        layout
            .repositories
            .iter()
            .map(|r| (r.path.clone(), r.source.clone())),
    );
    for (path, origin) in folders {
        let id = repository::identity(&origin)?;
        if let Some(name) = path.file_name() {
            resolver
                .used
                .insert(name.to_string_lossy().to_ascii_lowercase(), id.clone());
        }
        resolver.clones.insert(id, path);
    }
    let mut queue = VecDeque::new();
    for (position, entry) in entries(manifest, lock).iter().enumerate() {
        let (path, origin) = if entry.primary {
            (layout.guidelines.clone(), layout.guidelines_source.clone())
        } else {
            let declared = layout
                .repositories
                .iter()
                .find(|r| r.name == entry.name)
                .expect("declared repositories are part of the layout");
            (declared.path.clone(), declared.source.clone())
        };
        let id = repository::identity(&origin)?;
        let active = if entry.primary {
            active_guidelines.to_owned()
        } else if entry.kind == Kind::Guidelines {
            resolver.active(entry, &path, &id, &manifest.name)?
        } else {
            path.clone()
        };
        if entry.primary && entry.working_tree {
            resolver.branches.insert(
                id.clone(),
                (
                    entry.reference.clone().unwrap_or_default(),
                    manifest.name.clone(),
                ),
            );
        }
        let index = resolver.push(0, position, entry, path.clone(), active, origin.clone());
        if entry.kind == Kind::Project {
            queue.push_back(Pending {
                reference: index,
                path,
                source: origin,
                depth: 1,
            });
        }
    }
    while let Some(pending) = queue.pop_front() {
        let parent = resolver.tree.references[pending.reference].project;
        let parent_name = resolver.tree.projects[parent].name.clone();
        let read = manifest::read(&pending.path);
        let (child, child_lock) = match read {
            Ok(value) => value,
            Err(error) if pending.depth == 1 => {
                bail!(
                    "Referenced project {} is not a valid CretSpec project: {}\n{error:#}",
                    resolver.tree.references[pending.reference].name,
                    pending.path.display()
                )
            }
            Err(error) => {
                resolver.warn(format!(
                    "Referenced project {} of {parent_name} is not a valid CretSpec project and was skipped: {error:#}",
                    resolver.tree.references[pending.reference].name
                ));
                continue;
            }
        };
        let identity = repository::identity(&pending.source)?;
        if let Some(&existing) = resolver.projects.get(&identity) {
            resolver.tree.references[pending.reference].child = Some(existing);
            continue;
        }
        let directory = pending
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let index = resolver.tree.projects.len();
        resolver.tree.projects.push(Project {
            name: child.name.clone(),
            spec_name: child.spec_name().to_owned(),
            path: pending.path.clone(),
            source: pending.source.clone(),
            via: Some(pending.reference),
            depth: pending.depth,
            context: Some(
                spec.join(".local")
                    .join(CONTEXT_DIRECTORY)
                    .join(format!("{directory}.md")),
            ),
            purpose: child
                .spec_repository
                .as_ref()
                .and_then(|s| s.purpose.clone()),
            profiles: child.profiles().into_iter().map(Into::into).collect(),
        });
        resolver.projects.insert(identity.clone(), index);
        resolver.tree.references[pending.reference].child = Some(index);
        for (position, entry) in entries(&child, child_lock.as_ref()).iter().enumerate() {
            let origin = match repository::resolve(&entry.url, &pending.source) {
                Ok(origin) => origin,
                Err(error) => {
                    resolver.warn(format!(
                        "Reference {} of project {} has an unusable source and was skipped: {error:#}",
                        entry.name, child.name
                    ));
                    continue;
                }
            };
            let id = repository::identity(&origin)?;
            let Some(path) = resolver.folder(entry, &origin, &id, &child.name, &directory) else {
                continue;
            };
            let active = if entry.kind == Kind::Guidelines {
                match resolver.active(entry, &path, &id, &child.name) {
                    Ok(active) => active,
                    Err(error) => {
                        resolver.warn(format!(
                            "Guidelines {} of project {} cannot be used: {error:#}",
                            entry.name, child.name
                        ));
                        continue;
                    }
                }
            } else {
                path.clone()
            };
            let reference =
                resolver.push(index, position, entry, path.clone(), active, origin.clone());
            if entry.kind == Kind::Project {
                queue.push_back(Pending {
                    reference,
                    path,
                    source: origin,
                    depth: pending.depth + 1,
                });
            }
        }
    }
    check_cycles(&resolver.tree)?;
    Ok(resolver.tree)
}

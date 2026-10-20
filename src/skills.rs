use crate::{files, git, manifest::Kind, tree::Tree, workspace::ProjectInfo};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const BUILTIN_NAME: &str = "cspec-workspace";
const BUILTIN: &str = include_str!("../assets/cspec-workspace/SKILL.md");
const MAX_BUNDLE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    Shared,
    Repository,
    #[value(skip)]
    Builtin,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub scope: Scope,
    /// The declaring repository of a repository skill, or the guidelines reference of a
    /// shared skill that does not come from the project's own guidelines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// The referenced project that provides the skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub source: Option<PathBuf>,
}

impl Skill {
    pub fn origin(&self) -> String {
        let base = match (self.scope, &self.repository) {
            (Scope::Repository, Some(repository)) => format!("repository `{repository}`"),
            (Scope::Shared, Some(repository)) => format!("guidelines `{repository}`"),
            (Scope::Project, _) => "the project source".into(),
            (Scope::Shared, _) => "the active shared source".into(),
            _ => "the built-in source".into(),
        };
        match &self.project {
            Some(project) => format!("{base} of referenced project `{project}`"),
            None => base,
        }
    }
}

#[derive(Clone)]
pub struct Resource {
    pub bytes: Vec<u8>,
    pub executable: bool,
}

pub struct Bundle {
    pub skill: Skill,
    pub files: BTreeMap<String, Resource>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub schema_version: u32,
    pub active: Vec<Skill>,
    pub shared_drafts: Vec<Skill>,
    pub draft_errors: Vec<String>,
    /// Skills that a source with higher precedence overrides, and skipped skill sources of
    /// referenced projects.
    pub warnings: Vec<String>,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && files::portable_name(name)
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

pub fn description_valid(description: &str) -> bool {
    !description.trim().is_empty() && description.chars().count() <= 1024
}

/// Checks every directory from `base` down to `path` without following links.
/// Returns false when a directory on the way does not exist yet.
fn source_present(base: &Path, path: &Path) -> Result<bool> {
    files::directory(base)?;
    let relative = path
        .strip_prefix(base)
        .context("Skill directory escapes its repository")?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if !files::exists(&current)? {
            return Ok(false);
        }
        files::directory(&current)?;
    }
    Ok(true)
}

/// Splits a Markdown document into its YAML frontmatter and body.
pub fn frontmatter(bytes: &[u8], document: &str) -> Result<(String, String)> {
    let text = std::str::from_utf8(bytes).with_context(|| format!("{document} must be UTF-8"))?;
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    text.strip_prefix("---\n")
        .and_then(|s| {
            s.split_once("\n---")
                .filter(|(_, rest)| rest.is_empty() || rest.starts_with('\n'))
                .map(|(yaml, body)| (yaml.to_owned(), body.trim_start_matches('\n').to_owned()))
        })
        .with_context(|| format!("{document} needs YAML frontmatter between --- lines"))
}

/// Parses frontmatter YAML and explains errors with the line in the Markdown file, the
/// field and, for the common unquoted colon, how to fix it.
pub fn parse_frontmatter<T: serde::de::DeserializeOwned>(
    yaml: &str,
    document: &str,
    fields: &str,
) -> Result<T> {
    serde_yaml_ng::from_str(yaml).map_err(|error| {
        let mut message = error.to_string();
        if let Some(index) = message.rfind(" at line ") {
            message.truncate(index);
        }
        let Some(location) = error.location() else {
            return anyhow::anyhow!("Invalid {document} frontmatter: {message}. {fields}");
        };
        // The opening --- line precedes the YAML.
        let line = yaml.lines().nth(location.line().saturating_sub(1)).unwrap_or("");
        let field = line
            .split_once(':')
            .map(|(key, _)| key.trim())
            .filter(|key| !key.is_empty() && key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'))
            .map(|key| format!(", field `{key}`"))
            .unwrap_or_default();
        let hint = if line.split_once(':').is_some_and(|(_, value)| value.contains(": ")) {
            " Quote a value that contains a colon followed by a space, for example description: \"Run IT: Microsoft 365\"."
        } else {
            ""
        };
        anyhow::anyhow!(
            "Invalid {document} frontmatter at line {}{field}: {message}. {fields}{hint}",
            location.line() + 1
        )
    })
}

fn header(bytes: &[u8], expected: &str, scope: Scope, source: Option<PathBuf>) -> Result<Skill> {
    let (yaml, _) = frontmatter(bytes, "SKILL.md")?;
    #[derive(Deserialize)]
    struct Header {
        name: String,
        description: String,
    }
    let parsed: Header = parse_frontmatter(&yaml, "skill", "Use name and description.")?;
    if !valid_name(&parsed.name) || parsed.name != expected {
        bail!(
            "Skill name must match its directory and use 1-64 lowercase letters, digits or single hyphens: {expected}"
        );
    }
    if !description_valid(&parsed.description) {
        bail!("Skill {expected} needs a nonempty description of at most 1024 characters.");
    }
    Ok(Skill {
        name: parsed.name,
        description: parsed.description,
        scope,
        repository: None,
        project: None,
        source,
    })
}

pub fn executable(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn collect(
    base: &Path,
    current: &Path,
    result: &mut BTreeMap<String, Resource>,
    size: &mut usize,
) -> Result<()> {
    files::directory(current)?;
    if current.strip_prefix(base)?.components().count() > 8 {
        bail!("Skill directory depth exceeds 8: {}", current.display());
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("Skill paths must be UTF-8"))?;
        if !files::portable_name(&name) {
            bail!(
                "Skill resources need portable, non-hidden names: {}",
                entry.path().display()
            );
        }
        if !names.insert(name.to_ascii_lowercase()) {
            bail!(
                "Skill resource names must be distinct ignoring case: {}",
                current.display()
            );
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(base, &entry.path(), result, size)?;
        } else if kind.is_file() {
            let metadata = entry.metadata()?;
            if metadata.len() > MAX_BUNDLE as u64 || result.len() >= 256 {
                bail!(
                    "Skill exceeds the 16 MiB / 256 file limit: {}",
                    base.display()
                );
            }
            let bytes = fs::read(entry.path())?;
            *size += bytes.len();
            if *size > MAX_BUNDLE {
                bail!("Skill exceeds 16 MiB: {}", base.display());
            }
            let relative = entry
                .path()
                .strip_prefix(base)?
                .to_string_lossy()
                .replace('\\', "/");
            result.insert(
                relative,
                Resource {
                    bytes,
                    executable: executable(&metadata),
                },
            );
        } else {
            bail!(
                "Skill resources must be regular files or directories, not links: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn scan(root: &Path, scope: Scope) -> Result<Vec<Bundle>> {
    if !files::exists(root)? {
        return Ok(Vec::new());
    }
    files::directory(root)?;
    let mut bundles = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.file_name() == "README.md" {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("Skill names must be UTF-8"))?;
        if !valid_name(&name) || name == BUILTIN_NAME {
            bail!(
                "Invalid or reserved skill directory: {}",
                entry.path().display()
            );
        }
        let mut content = BTreeMap::new();
        collect(&entry.path(), &entry.path(), &mut content, &mut 0)?;
        let document = content
            .get("SKILL.md")
            .context("Every skill directory needs SKILL.md")?;
        if document.bytes.len() > 128 * 1024 {
            bail!("SKILL.md exceeds 128 KiB");
        }
        let skill = header(&document.bytes, &name, scope, Some(entry.path()))
            .with_context(|| format!("Invalid skill at {}", entry.path().display()))?;
        bundles.push(Bundle {
            skill,
            files: content,
        });
        if bundles.len() > 128 {
            bail!("A skill source supports at most 128 skills");
        }
    }
    bundles.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    Ok(bundles)
}

/// A skill directory with its rank in the precedence order. Collisions inside the
/// workspace's own sources stop synchronization; any other collision resolves to the
/// source with the highest precedence.
struct SkillSource {
    rank: usize,
    own: bool,
    directory: PathBuf,
    /// The repository folder the directory lies in, checked without following links.
    base: PathBuf,
    scope: Scope,
    repository: Option<String>,
    project: Option<String>,
    /// A pinned snapshot, whose skills must all be tracked by Git.
    pinned: bool,
    /// A source of a referenced project: its problems are warnings, not errors.
    nested: bool,
}

/// The order of layers: a project's own sources, then its references by precedence,
/// with each referenced project expanded in place the first time it is reached.
fn layers(
    tree: &Tree,
    project: usize,
    visited: &mut BTreeSet<usize>,
    result: &mut Vec<(usize, Option<usize>)>,
) {
    if !visited.insert(project) {
        return;
    }
    result.push((project, None));
    for index in tree.ordered(project) {
        let reference = &tree.references[index];
        match (reference.kind, reference.child) {
            (Kind::Guidelines, _) => result.push((project, Some(index))),
            (Kind::Project, Some(child)) => layers(tree, child, visited, result),
            _ => {}
        }
    }
}

fn skill_sources(info: &ProjectInfo) -> Vec<SkillSource> {
    let tree = &info.tree;
    let mut order = Vec::new();
    layers(tree, 0, &mut BTreeSet::new(), &mut order);
    let mut result = Vec::new();
    for (rank, (project, layer)) in order.into_iter().enumerate() {
        let name = (project != 0).then(|| tree.projects[project].name.clone());
        match layer {
            None => {
                let spec = if project == 0 {
                    info.spec.clone()
                } else {
                    tree.projects[project].path.clone()
                };
                result.push(SkillSource {
                    rank,
                    own: project == 0,
                    directory: spec.join("spec/skills"),
                    base: spec,
                    scope: Scope::Project,
                    repository: None,
                    project: name.clone(),
                    pinned: false,
                    nested: project != 0,
                });
                let code = tree
                    .references
                    .iter()
                    .filter(|r| r.project == project && r.kind == Kind::Code);
                for reference in code {
                    let Some(directory) = &reference.skills else {
                        continue;
                    };
                    result.push(SkillSource {
                        rank,
                        own: project == 0,
                        directory: directory.clone(),
                        base: reference.path.clone(),
                        scope: Scope::Repository,
                        repository: Some(reference.name.clone()),
                        project: name.clone(),
                        pinned: false,
                        nested: project != 0,
                    });
                }
            }
            Some(index) => {
                let reference = &tree.references[index];
                // The workspace's own guidelines may be a proposed version during adoption.
                let (active, pinned) = if project == 0 && reference.primary {
                    (info.active_guidelines.clone(), info.lock.is_some())
                } else {
                    (reference.active.clone(), reference.commit.is_some())
                };
                let directory = if project == 0 && reference.primary {
                    active.join("skills")
                } else {
                    reference
                        .skills
                        .clone()
                        .unwrap_or_else(|| active.join("skills"))
                };
                result.push(SkillSource {
                    rank,
                    own: project == 0 && reference.primary,
                    directory,
                    base: active,
                    scope: Scope::Shared,
                    repository: (!reference.primary).then(|| reference.name.clone()),
                    project: name.clone(),
                    pinned,
                    nested: project != 0,
                });
            }
        }
    }
    result
}

/// Rejects files in a pinned snapshot's skills that Git does not track.
fn check_tracked(base: &Path, directory: &Path, found: &[Bundle]) -> Result<()> {
    let relative = directory
        .strip_prefix(base)
        .context("Skill directory escapes its repository")?
        .to_string_lossy()
        .replace('\\', "/");
    let tracked = git::run(base, ["ls-files", "-z", "--", &relative])?;
    let tracked: BTreeSet<_> = tracked.split('\0').collect();
    for bundle in found {
        for name in bundle.files.keys() {
            let path = format!("{relative}/{}/{name}", bundle.skill.name);
            if !tracked.contains(path.as_str()) {
                bail!(
                    "Adopted skill contains a file outside the locked Git snapshot: {path}. Preserve and remove that local addition before retrying."
                );
            }
        }
    }
    Ok(())
}

pub fn active(info: &ProjectInfo) -> Result<Vec<Bundle>> {
    Ok(resolve(info)?.0)
}

/// The active skills, and warnings for skills that a source with higher precedence overrides
/// and for skipped skill sources of referenced projects.
pub fn resolve(info: &ProjectInfo) -> Result<(Vec<Bundle>, Vec<String>)> {
    // Check the source parent as well, so a replaced spec/ directory is never followed.
    files::directory(&info.spec.join("spec"))?;
    let builtin = Bundle {
        skill: header(BUILTIN.as_bytes(), BUILTIN_NAME, Scope::Builtin, None)?,
        files: BTreeMap::from([(
            "SKILL.md".into(),
            Resource {
                bytes: BUILTIN.as_bytes().to_vec(),
                executable: false,
            },
        )]),
    };
    let mut ranked = vec![(0, true, builtin)];
    let mut warnings = Vec::new();
    let mut seen = BTreeSet::new();
    for source in skill_sources(info) {
        // A directory that several projects share is read once, at its highest precedence.
        if !seen.insert(source.directory.clone()) {
            continue;
        }
        let read = (|| -> Result<Option<Vec<Bundle>>> {
            // A repository that does not exist yet, or has no skill directory yet, has no skills.
            if !files::exists(&source.base)? || !source_present(&source.base, &source.directory)? {
                return Ok(None);
            }
            let found = scan(&source.directory, source.scope)?;
            if source.pinned {
                check_tracked(&source.base, &source.directory, &found)?;
            }
            Ok(Some(found))
        })();
        let mut found = match read {
            Ok(Some(found)) => found,
            Ok(None) => continue,
            Err(error) if source.nested => {
                warnings.push(format!(
                    "Skills in {} of referenced project `{}` were skipped: {error:#}",
                    source.directory.display(),
                    source.project.as_deref().unwrap_or_default()
                ));
                continue;
            }
            Err(error) => return Err(error),
        };
        for bundle in &mut found {
            bundle.skill.repository = source.repository.clone();
            bundle.skill.project = source.project.clone();
        }
        ranked.extend(found.into_iter().map(|b| (source.rank, source.own, b)));
    }
    // Own sources keep the order of earlier versions in conflict messages.
    let order = |scope: Scope| match scope {
        Scope::Builtin => 0,
        Scope::Project => 1,
        Scope::Shared => 2,
        Scope::Repository => 3,
    };
    ranked.sort_by(|a, b| a.2.skill.name.cmp(&b.2.skill.name));
    let mut bundles: Vec<Bundle> = Vec::new();
    let mut overridden = warnings;
    let mut group: Vec<(usize, bool, Bundle)> = Vec::new();
    let mut finish = |group: &mut Vec<(usize, bool, Bundle)>| -> Result<()> {
        if group.is_empty() {
            return Ok(());
        }
        let own: Vec<_> = group.iter().filter(|g| g.1).collect();
        if own.len() > 1 {
            let mut own = own;
            own.sort_by_key(|g| order(g.2.skill.scope));
            bail!(
                "Skill '{}' exists in both {} and {}. Rename one source skill before synchronizing.",
                own[0].2.skill.name,
                own[0].2.skill.origin(),
                own[1].2.skill.origin()
            );
        }
        let mut members = std::mem::take(group);
        members.sort_by_key(|g| (g.0, order(g.2.skill.scope)));
        let mut members = members.into_iter();
        let (_, _, winner) = members.next().expect("a group has members");
        let same = |a: &Bundle, b: &Bundle| {
            a.files.len() == b.files.len()
                && a.files.iter().zip(&b.files).all(|((m, x), (n, y))| {
                    m == n && x.bytes == y.bytes && x.executable == y.executable
                })
        };
        let mut reported: Vec<Bundle> = Vec::new();
        for (_, _, loser) in members {
            // Identical copies, such as one repository at two places in the tree, are
            // silent or reported once.
            if same(&loser, &winner) || reported.iter().any(|r| same(r, &loser)) {
                continue;
            }
            overridden.push(format!(
                "Skill '{}' from {} overrides the one from {}",
                winner.skill.name,
                winner.skill.origin(),
                loser.skill.origin()
            ));
            reported.push(loser);
        }
        bundles.push(winner);
        Ok(())
    };
    for entry in ranked {
        if group
            .last()
            .is_some_and(|g| g.2.skill.name != entry.2.skill.name)
        {
            finish(&mut group)?;
        }
        group.push(entry);
    }
    finish(&mut group)?;
    let size: usize = bundles
        .iter()
        .flat_map(|b| b.files.values())
        .map(|r| r.bytes.len())
        .sum();
    if size > 64 * 1024 * 1024 {
        bail!("Active skill resources exceed 64 MiB");
    }
    Ok((bundles, overridden))
}

/// Creates the missing directories from `base` down to `path` without following links.
fn create_directories(base: &Path, path: &Path) -> Result<()> {
    files::directory(base)?;
    let relative = path
        .strip_prefix(base)
        .context("Skill directory escapes its repository")?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if files::exists(&current)? {
            files::directory(&current)?;
        } else {
            fs::create_dir(&current)?;
        }
    }
    Ok(())
}

pub fn inventory(info: &ProjectInfo) -> Result<Inventory> {
    let (active, warnings) = resolve(info)?;
    let active = active.into_iter().map(|b| b.skill).collect();
    let (shared_drafts, draft_errors) = if info.lock.is_none() {
        (vec![], vec![])
    } else {
        match scan(&info.editable_guidelines.join("skills"), Scope::Shared) {
            Ok(bundles) => (bundles.into_iter().map(|b| b.skill).collect(), vec![]),
            Err(error) => (vec![], vec![format!("{error:#}")]),
        }
    };
    Ok(Inventory {
        schema_version: 1,
        active,
        shared_drafts,
        draft_errors,
        warnings,
    })
}

pub fn create(
    info: &ProjectInfo,
    name: &str,
    scope: Scope,
    repository: Option<&str>,
    description: &str,
) -> Result<PathBuf> {
    if !valid_name(name) || name == BUILTIN_NAME || !description_valid(description) {
        bail!(
            "Use a portable skill name (excluding cspec-workspace) and a nonempty description of at most 1024 characters."
        );
    }
    if repository.is_some() != (scope == Scope::Repository) {
        bail!("Name a repository with --repository exactly when the scope is repository.");
    }
    if active(info)?.iter().any(|b| b.skill.name == name) {
        bail!("Skill '{name}' is already active. Edit its source or choose a distinct name.");
    }
    let root = match scope {
        Scope::Project => info.spec.join("spec/skills"),
        Scope::Shared => info.editable_guidelines.join("skills"),
        Scope::Repository => {
            let name = repository.unwrap_or_default();
            let declared = info
                .repositories
                .iter()
                .find(|r| r.name == name)
                .with_context(|| format!("Unknown repository: {name}"))?;
            let directory = declared.skills.clone().with_context(|| {
                format!(
                    "Repository {name} declares no skill directory. Add \"skills\", for example \"spec/skills\", to its entry in project.json."
                )
            })?;
            create_directories(&declared.path, &directory)?;
            directory
        }
        Scope::Builtin => {
            bail!(
                "Built-in skills are maintained with CretSpec. Choose project, shared or repository."
            )
        }
    };
    let parent = root.parent().context("Skill directory has no parent")?;
    if scope != Scope::Repository {
        files::directory(parent)?;
        if files::exists(&root)? {
            files::directory(&root)?;
        } else {
            fs::create_dir(&root)?;
        }
    }
    // create_dir is exclusive: a concurrent creator cannot be overwritten.
    let destination = root.join(name);
    fs::create_dir(&destination).with_context(|| {
        format!(
            "Skill already exists or cannot be created: {}",
            destination.display()
        )
    })?;
    let description = serde_json::to_string(description)?; // JSON strings are valid YAML scalars.
    let text = format!(
        "---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\nDescribe the workflow, its expected result and any supporting resources here.\n"
    );
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination.join("SKILL.md"))?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(destination)
}

use crate::{
    files, git,
    manifest::{self, GuidelinesMode, LEGACY_CODE_NAME, Lock, Manifest, ROLES_DIRECTORY},
    repository,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
};

pub type Progress<'a> = &'a dyn Fn(&str);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub project_root: PathBuf,
    pub spec: PathBuf,
    /// The code repository of a schema version 1 definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<PathBuf>,
    pub spec_repository: SpecRepositoryInfo,
    /// Declared repositories other than the spec repository.
    pub repositories: Vec<RepositoryInfo>,
    pub editable_guidelines: PathBuf,
    pub active_guidelines: PathBuf,
    pub manifest: Manifest,
    pub lock: Option<Lock>,
    pub guidelines: GuidelinesState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecRepositoryInfo {
    pub name: String,
    pub path: PathBuf,
    pub purpose: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub purpose: Option<String>,
    pub profile: Option<String>,
    /// The repository's own skill directory, when the definition declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<PathBuf>,
    /// The stable ownership anchor for generated files in this repository.
    #[serde(skip)]
    pub anchor: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuidelinesState {
    pub mode: GuidelinesMode,
    pub commit: String,
    pub branch: Option<String>,
    pub dirty: bool,
    pub upstream: Option<String>,
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
}

pub fn guidelines_state(active: &Path, mode: GuidelinesMode) -> Result<GuidelinesState> {
    let commit = git::run(active, ["rev-parse", "HEAD"])?;
    let branch = git::run(active, ["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();
    let dirty = !git::run(active, ["status", "--porcelain", "--untracked-files=all"])?.is_empty();
    let upstream = git::run(
        active,
        [
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .ok();
    let counts = upstream.as_ref().and_then(|_| {
        git::run(
            active,
            ["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )
        .ok()
    });
    let counts = counts
        .as_deref()
        .and_then(|s| s.split_once(char::is_whitespace));
    let (ahead, behind) = counts
        .map(|(a, b)| (a.trim().parse().ok(), b.trim().parse().ok()))
        .unwrap_or((None, None));
    Ok(GuidelinesState {
        mode,
        commit,
        branch,
        dirty,
        upstream,
        ahead,
        behind,
    })
}

pub fn validate_profile(active: &Path, profile: &str) -> Result<()> {
    files::directory(&active.join("profiles"))?;
    let path = active.join("profiles").join(format!("{profile}.md"));
    if !fs::symlink_metadata(&path)
        .with_context(|| format!("Selected profile does not exist: {}", path.display()))?
        .file_type()
        .is_file()
    {
        bail!("The selected profile must be a regular file.");
    }
    Ok(())
}

pub fn validate_profiles(active: &Path, manifest: &Manifest) -> Result<()> {
    for profile in manifest.profiles() {
        validate_profile(active, profile)?;
    }
    Ok(())
}

pub fn validate_branch(directory: &Path, branch: &str) -> Result<()> {
    git::run(
        directory,
        ["check-ref-format", &format!("refs/heads/{branch}")],
    )
    .context("Working-tree guidelines need a valid branch name")?;
    let local = format!("refs/heads/{branch}");
    let remote = format!("refs/remotes/origin/{branch}");
    if git::run(directory, ["show-ref", "--verify", &local]).is_err()
        && git::run(directory, ["show-ref", "--verify", &remote]).is_err()
    {
        bail!(
            "Working-tree guidelines need an available branch, not a tag or commit: {branch}. Fetch the branch with Git or restore a valid branch definition before pinning a version with cspec guidelines update <ref>."
        );
    }
    Ok(())
}

pub struct Paths {
    pub root: PathBuf,
    pub repositories: Vec<RepositoryInfo>,
    pub guidelines: PathBuf,
    pub guidelines_source: String,
}

pub fn paths(spec: &Path, manifest: &Manifest, source: &str) -> Result<Paths> {
    let root = spec
        .parent()
        .context("Spec has no parent directory")?
        .to_owned();
    let mut repositories = Vec::new();
    if let Some(code) = &manifest.code {
        repositories.push(RepositoryInfo {
            name: LEGACY_CODE_NAME.into(),
            path: root.join(&manifest.name),
            source: repository::resolve(&code.repository, source)?,
            purpose: None,
            profile: manifest.profile.clone(),
            skills: None,
            anchor: "code".into(),
        });
    }
    for declared in manifest.repositories.iter().flatten() {
        repositories.push(RepositoryInfo {
            name: declared.name.clone(),
            path: root.join(declared.directory()),
            source: repository::resolve(&declared.url, source)?,
            purpose: declared.purpose.clone(),
            profile: declared.profile.clone(),
            skills: declared.skills.as_ref().map(|skills| {
                // Join each segment, so the path uses the platform's separators.
                skills
                    .split('/')
                    .fold(root.join(declared.directory()), |path, segment| {
                        path.join(segment)
                    })
            }),
            anchor: format!("repo:{}", declared.directory()),
        });
    }
    let guidelines_source = repository::resolve(&manifest.guidelines.repository, source)?;
    let guidelines = root.join(repository::directory_name(&guidelines_source)?);
    let directories = [spec, guidelines.as_path()]
        .into_iter()
        .chain(repositories.iter().map(|r| r.path.as_path()))
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        })
        .collect::<Vec<_>>();
    let mut distinct = directories.clone();
    distinct.sort();
    distinct.dedup();
    if distinct.len() != directories.len() {
        bail!(if manifest.code.is_some() {
            "Spec, code and guidelines need distinct directory names inside the project root."
        } else {
            "The spec, guidelines and repositories need distinct directory names inside the project root."
        });
    }
    if manifest.schema_version >= 2 {
        if directories.iter().any(|d| d == ROLES_DIRECTORY) {
            bail!("The {ROLES_DIRECTORY} directory is reserved for generated role entry points.");
        }
        let mut identities = vec![
            repository::identity(source)?,
            repository::identity(&guidelines_source)?,
        ];
        for repository in &repositories {
            let identity = repository::identity(&repository.source)?;
            if identities.contains(&identity) {
                bail!(
                    "Repository {} refers to a source that is already part of the workspace: {}",
                    repository.name,
                    repository.source
                );
            }
            identities.push(identity);
        }
    }
    Ok(Paths {
        root,
        repositories,
        guidelines,
        guidelines_source,
    })
}

/// Verifies every declared repository and reports all problems together.
fn verify_repositories(paths: &Paths, legacy: bool) -> Result<()> {
    if legacy {
        for repository in &paths.repositories {
            verify_repository(&repository.path, &repository.source, &repository.name)?;
        }
        return Ok(());
    }
    let mut problems = Vec::new();
    for repository in &paths.repositories {
        if !files::exists(&repository.path)? {
            problems.push(format!(
                "The {} repository is missing: {}\nExpected source: {}",
                repository.name,
                repository.path.display(),
                repository.source
            ));
        } else if let Err(error) =
            verify_repository(&repository.path, &repository.source, &repository.name)
        {
            problems.push(format!("{error:#}"));
        }
    }
    if !problems.is_empty() {
        bail!(
            "{}\nClone missing repositories with Git at their declared paths, or run cspec project attach <spec> --clone-missing.",
            problems.join("\n")
        );
    }
    Ok(())
}

pub fn local_directory(spec: &Path, create: bool) -> Result<PathBuf> {
    if !git::run(spec, ["ls-files", "--", ".local"])?.is_empty() {
        bail!(
            "The spec tracks .local/ files. Untrack those files before generating local context."
        );
    }
    let local = spec.join(".local");
    if files::exists(&local)? {
        files::directory(&local)?;
    } else if create {
        fs::create_dir(&local)?;
    } else {
        bail!("Local project context is missing. Run cspec project info to prepare it.");
    }
    if create
        && git::run(
            spec,
            ["check-ignore", "--quiet", "--", ".local/cspec-probe"],
        )
        .is_err()
    {
        let exclude = files::absolute(
            spec.join(git::run(spec, ["rev-parse", "--git-path", "info/exclude"])?),
        )?;
        fs::create_dir_all(exclude.parent().context("Git exclude path has no parent")?)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(exclude)?;
        file.write_all(b"\n# Generated local files\n/.local/\n")?;
    }
    Ok(local)
}

pub fn verify_repository(directory: &Path, expected: &str, label: &str) -> Result<()> {
    git::root(directory)?;
    if repository::identity(&repository::origin(directory)?)? != repository::identity(expected)? {
        bail!(
            "The {label} repository does not match its source in the spec: {}\nExpected source: {expected}\nUse the correct clone; the existing repository has not been changed.",
            directory.display()
        );
    }
    Ok(())
}

pub fn verify_revision(directory: &Path, lock: &Lock) -> Result<()> {
    if git::run(directory, ["cat-file", "-t", &lock.commit])
        .ok()
        .as_deref()
        != Some("commit")
    {
        bail!(
            "The project guidelines repository does not contain {}.\nRun git fetch --tags in {}.",
            lock.commit,
            directory.display()
        );
    }
    let revision = git::run(
        directory,
        [
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", lock.reference),
        ],
    )?;
    if revision != lock.commit {
        bail!("The guidelines ref does not match the lock. No version was adopted.");
    }
    Ok(())
}

pub fn snapshot(
    spec: &Path,
    manifest: &Manifest,
    lock: Option<&Lock>,
    create: bool,
    progress: Progress<'_>,
) -> Result<(PathBuf, PathBuf)> {
    let layout = paths(spec, manifest, &repository::origin(spec)?)?;
    let new_clone = !files::exists(&layout.guidelines)?;
    if new_clone {
        if !create {
            bail!(
                "The editable guidelines clone is missing. Run cspec project info to prepare it."
            );
        }
        progress("Fetching project guidelines");
        git::clone(&layout.guidelines_source, &layout.guidelines)?;
    }
    verify_repository(&layout.guidelines, &layout.guidelines_source, "guidelines")?;
    if manifest.guidelines_mode(lock) == GuidelinesMode::WorkingTree {
        validate_branch(&layout.guidelines, &manifest.guidelines.reference)?;
        if new_clone {
            git::run(
                &layout.guidelines,
                ["switch", &manifest.guidelines.reference],
            )?;
        }
        validate_profiles(&layout.guidelines, manifest)?;
        if create {
            local_directory(spec, true)?;
        }
        return Ok((layout.guidelines.clone(), layout.guidelines));
    }
    let lock = lock.context("Pinned guidelines require a lock")?;
    verify_revision(&layout.guidelines, lock)?;
    let cache = local_directory(spec, create)?.join("guidelines");
    if files::exists(&cache)? {
        files::directory(&cache)?;
    } else if create {
        fs::create_dir(&cache)?;
    }
    let active = cache.join(&lock.commit);
    if !files::exists(&active)? {
        if !create {
            bail!(
                "The pinned guidelines snapshot is missing. Run cspec project info to prepare it."
            );
        }
        progress("Fetching pinned guidelines");
        git::clone(&layout.guidelines.to_string_lossy(), &active)?;
        verify_revision(&active, lock)?;
        git::run(&active, ["checkout", "--detach", &lock.commit])?;
    }
    git::root(&active)?;
    let head = git::run(&active, ["rev-parse", "HEAD"])?;
    let dirty = git::run(&active, ["status", "--porcelain", "--untracked-files=all"])?;
    let attached = git::run(&active, ["symbolic-ref", "--quiet", "HEAD"]).is_ok();
    if head != lock.commit
        || !dirty.is_empty()
        || attached
        || verify_revision(&active, lock).is_err()
    {
        bail!(
            "The cached guidelines differ from the lock or contain local changes. Preserve any work and remove that snapshot before retrying."
        );
    }
    validate_profiles(&active, manifest)?;
    Ok((layout.guidelines, active))
}

fn is_spec(path: &Path) -> bool {
    // Inaccessible directories, such as other users' temporary folders, are not specs.
    files::exists(&path.join("project.json")).unwrap_or(false)
        && files::exists(&path.join("spec")).unwrap_or(false)
}

pub fn find_spec(start: &Path) -> Result<PathBuf> {
    let spec = discover_spec(start)?;
    manifest::read(&spec)?;
    Ok(spec)
}

pub fn discover_spec(start: &Path) -> Result<PathBuf> {
    let mut current = files::absolute(start)?;
    if fs::metadata(&current)?.is_file() {
        current.pop();
    }
    loop {
        if is_spec(&current) {
            let spec = git::root(&current)?;
            return Ok(spec);
        }
        let enclosing = git::enclosing(&current);
        if let Some(root) = &enclosing {
            if !files::same(root, &current) {
                current = root.clone();
                continue;
            }
        } else {
            let mut candidates = Vec::new();
            for entry in fs::read_dir(&current).into_iter().flatten().flatten() {
                if !entry.file_type().is_ok_and(|kind| kind.is_dir())
                    || entry.file_name().to_string_lossy().starts_with('.')
                {
                    continue;
                }
                if is_spec(&entry.path()) {
                    candidates.push(entry.path());
                }
            }
            if candidates.len() > 1 {
                bail!("Multiple project specs found. Pass the intended spec directory explicitly.");
            }
            if let Some(candidate) = candidates.first() {
                let spec = git::root(candidate)?;
                return Ok(spec);
            }
        }
        if !current.pop() {
            bail!(
                "No project spec found. Run this command inside a project root, code or spec repository, or pass a project directory."
            );
        }
    }
}

pub fn info(start: &Path, create: bool, progress: Progress<'_>) -> Result<ProjectInfo> {
    let spec = find_spec(start)?;
    let (manifest, lock) = manifest::read(&spec)?;
    let source = repository::origin(&spec)?;
    let layout = paths(&spec, &manifest, &source)?;
    verify_repositories(&layout, manifest.code.is_some())?;
    let (editable_guidelines, active_guidelines) =
        snapshot(&spec, &manifest, lock.as_ref(), create, progress)?;
    let guidelines = guidelines_state(&active_guidelines, manifest.guidelines_mode(lock.as_ref()))?;
    let declared = manifest.spec_repository.clone().unwrap_or_default();
    Ok(ProjectInfo {
        project_root: layout.root,
        code: manifest
            .code
            .as_ref()
            .and_then(|_| layout.repositories.first())
            .map(|r| r.path.clone()),
        spec_repository: SpecRepositoryInfo {
            name: manifest.spec_name().into(),
            path: spec.clone(),
            purpose: declared.purpose,
            profile: declared.profile,
        },
        repositories: layout.repositories,
        spec,
        editable_guidelines,
        active_guidelines,
        manifest,
        lock,
        guidelines,
    })
}

fn prepare_parent(directory: &Path) -> Result<PathBuf> {
    let target = files::absolute(directory)?;
    let mut parent = target.clone();
    while !files::exists(&parent)? {
        if !parent.pop() {
            bail!("Cannot locate an existing parent directory.");
        }
    }
    if git::enclosing(&parent).is_some() {
        bail!("Create the project outside existing Git repositories.");
    }
    fs::create_dir_all(&target)?;
    Ok(target)
}

pub fn clone_project(
    input: &str,
    destination: Option<&Path>,
    namespace: Option<&str>,
    cwd: &Path,
    progress: Progress<'_>,
) -> Result<ProjectInfo> {
    let source = repository::spec_source(input, namespace, cwd)?;
    let explicit = destination
        .map(|d| files::absolute(cwd.join(d)))
        .transpose()?;
    if let Some(target) = &explicit
        && files::exists(target)?
    {
        bail!(
            "Destination already exists; nothing overwritten: {}",
            target.display()
        );
    }
    let parent = prepare_parent(explicit.as_deref().and_then(Path::parent).unwrap_or(cwd))?;
    let staging = tempfile::Builder::new()
        .prefix(".cspec-clone-")
        .tempdir_in(&parent)?
        .keep();
    let mut preserved = staging.clone();
    let result = (|| -> Result<ProjectInfo> {
        progress("Fetching project spec");
        git::clone(&source, &staging)?;
        let (manifest, _) = manifest::read(&staging)?;
        let root = explicit.unwrap_or_else(|| parent.join(&manifest.name));
        if files::exists(&root)? {
            bail!(
                "Destination already exists; nothing overwritten: {}",
                root.display()
            );
        }
        let spec = root.join(repository::directory_name(&source)?);
        let layout = paths(&spec, &manifest, &source)?;
        fs::create_dir(&root)?;
        fs::rename(&staging, &spec).with_context(|| {
            format!(
                "Could not move the cloned spec into {}. Staging remains at {}",
                spec.display(),
                staging.display()
            )
        })?;
        preserved = root;
        for repository in &layout.repositories {
            if manifest.code.is_some() {
                progress("Fetching code");
            } else {
                progress(&format!("Fetching {}", repository.name));
            }
            git::clone(&repository.source, &repository.path)?;
        }
        let project = info(&spec, true, progress)?;
        crate::agents::sync(&project)?;
        Ok(project)
    })();
    result.with_context(|| format!("Project preparation failed. New directories preserved for inspection: {}\nExisting repositories have not been modified",preserved.display()))
}

/// Includes existing clones in place. With a code path, the legacy single-repository form
/// verifies that path; otherwise every declared repository is checked at its declared path.
pub fn attach(
    code: Option<&Path>,
    spec: &Path,
    clone_missing: bool,
    progress: Progress<'_>,
) -> Result<ProjectInfo> {
    let spec = git::root(&files::absolute(spec)?)?;
    let (manifest, _) = manifest::read(&spec)?;
    let layout = paths(&spec, &manifest, &repository::origin(&spec)?)?;
    if let Some(code) = code {
        let code = git::root(&files::absolute(code)?)?;
        let expected = match (&manifest.code, layout.repositories.first()) {
            (Some(_), Some(repository)) => repository.path.clone(),
            _ => bail!(
                "This definition declares its repositories by name. Run cspec project attach <spec>; existing clones are found at their declared paths."
            ),
        };
        if !files::same(&code, &expected) {
            bail!(
                "The spec expects its code in the sibling directory {}. No separate project bindings are stored.",
                expected.display()
            );
        }
    } else if let Some(enclosing) = git::enclosing(&layout.root) {
        bail!(
            "The project root is inside the Git repository {}. Place the spec and repositories as siblings in an ordinary directory.",
            enclosing.display()
        );
    }
    let mut missing = Vec::new();
    let mut problems = Vec::new();
    for repository in &layout.repositories {
        if !files::exists(&repository.path)? {
            missing.push(repository);
        } else if let Err(error) =
            verify_repository(&repository.path, &repository.source, &repository.name)
        {
            problems.push(format!("{error:#}"));
        }
    }
    if !problems.is_empty() {
        bail!(
            "{}\nNo repository or generated file was changed.",
            problems.join("\n")
        );
    }
    if !missing.is_empty() && !clone_missing {
        let list = missing
            .iter()
            .map(|r| format!("- {}: {} ({})", r.name, r.path.display(), r.source))
            .collect::<Vec<_>>()
            .join("\n");
        bail!(
            "Declared repositories are missing:\n{list}\nClone them with Git at those paths, or run cspec project attach <spec> --clone-missing. Nothing was changed."
        );
    }
    for repository in missing {
        progress(&format!("Fetching {}", repository.name));
        git::clone(&repository.source, &repository.path)?;
    }
    let project = info(&spec, true, progress)?;
    crate::agents::sync(&project)?;
    Ok(project)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryState {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
}

/// Reports local branch and change state without modifying any repository.
pub fn repository_states(info: &ProjectInfo) -> Result<Vec<RepositoryState>> {
    std::iter::once((&info.spec_repository.name, &info.spec))
        .chain(info.repositories.iter().map(|r| (&r.name, &r.path)))
        .map(|(name, path)| {
            Ok(RepositoryState {
                name: name.clone(),
                path: path.clone(),
                branch: git::run(path, ["symbolic-ref", "--quiet", "--short", "HEAD"]).ok(),
                head: git::run(path, ["rev-parse", "--verify", "--quiet", "HEAD"]).ok(),
                dirty: !git::run(path, ["status", "--porcelain"])?.is_empty(),
            })
        })
        .collect()
}

/// Lists Git repositories in the project root that the definition does not declare.
pub fn undeclared_repositories(info: &ProjectInfo) -> Result<Vec<PathBuf>> {
    let known: Vec<&Path> = [info.spec.as_path(), info.editable_guidelines.as_path()]
        .into_iter()
        .chain(info.repositories.iter().map(|r| r.path.as_path()))
        .collect();
    let mut result = Vec::new();
    for entry in fs::read_dir(&info.project_root)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir()
            || entry.file_name().to_string_lossy().starts_with('.')
            || entry.file_name() == ROLES_DIRECTORY
            || known.iter().any(|k| files::same(k, &path))
        {
            continue;
        }
        if files::exists(&path.join(".git"))? {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}

pub fn editor_workspace(start: &Path) -> Result<PathBuf> {
    let info = info(start, true, &|_| {})?;
    crate::agents::sync(&info)?;
    let file = local_directory(&info.spec, true)?.join("project.code-workspace");
    let mut value: Value = if files::exists(&file)? {
        files::read_json(&file)?
    } else {
        json!({})
    };
    let settings = value
        .as_object_mut()
        .context("Editor workspace must be a JSON object")?;
    let mut folders = Vec::new();
    let repositories = info.repositories.iter().map(|r| &r.path);
    for folder in repositories.chain([&info.spec, &info.editable_guidelines]) {
        let name = folder
            .file_name()
            .context("Project folder has no name")?
            .to_string_lossy();
        let relative = if folder == &info.spec {
            "..".into()
        } else {
            format!("../../{name}")
        };
        folders.push(json!({"name":name,"path":relative}));
    }
    settings.insert("folders".into(), json!(folders));
    files::write_json(&file, &value)?;
    Ok(file)
}

pub fn open_path(path: &Path) -> Result<()> {
    let path = files::absolute(path)?;
    fs::metadata(&path)?;
    let opener = if cfg!(windows) {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    git::command(opener)
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| {
            format!(
                "Open the path manually; {opener} could not be started: {}",
                path.display()
            )
        })?;
    Ok(())
}

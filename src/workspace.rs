use crate::{
    files, git,
    manifest::{self, GuidelinesMode, Lock, Manifest},
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
    pub code: PathBuf,
    pub editable_guidelines: PathBuf,
    pub active_guidelines: PathBuf,
    pub manifest: Manifest,
    pub lock: Option<Lock>,
    pub guidelines: GuidelinesState,
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
    pub code: PathBuf,
    pub guidelines: PathBuf,
    pub guidelines_source: String,
}

pub fn paths(spec: &Path, manifest: &Manifest, source: &str) -> Result<Paths> {
    let root = spec
        .parent()
        .context("Spec has no parent directory")?
        .to_owned();
    let code = root.join(&manifest.name);
    let guidelines_source = repository::resolve(&manifest.guidelines.repository, source)?;
    let guidelines = root.join(repository::directory_name(&guidelines_source)?);
    let mut names = [spec, code.as_path(), guidelines.as_path()]
        .iter()
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    if names.len() != 3 {
        bail!("Spec, code and guidelines need distinct directory names inside the project root.");
    }
    Ok(Paths {
        root,
        code,
        guidelines,
        guidelines_source,
    })
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
        validate_profile(&layout.guidelines, &manifest.profile)?;
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
    validate_profile(&active, &manifest.profile)?;
    Ok((layout.guidelines, active))
}

fn is_spec(path: &Path) -> Result<bool> {
    Ok(files::exists(&path.join("project.json"))? && files::exists(&path.join("spec"))?)
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
        if is_spec(&current)? {
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
            for entry in fs::read_dir(&current)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir()
                    || entry.file_name().to_string_lossy().starts_with('.')
                {
                    continue;
                }
                if is_spec(&entry.path())? {
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
    verify_repository(
        &layout.code,
        &repository::resolve(&manifest.code.repository, &source)?,
        "code",
    )?;
    let (editable_guidelines, active_guidelines) =
        snapshot(&spec, &manifest, lock.as_ref(), create, progress)?;
    let guidelines = guidelines_state(&active_guidelines, manifest.guidelines_mode(lock.as_ref()))?;
    Ok(ProjectInfo {
        project_root: layout.root,
        spec,
        code: layout.code,
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
        progress("Fetching code");
        git::clone(
            &repository::resolve(&manifest.code.repository, &source)?,
            &layout.code,
        )?;
        let project = info(&spec, true, progress)?;
        crate::agents::sync(&project)?;
        Ok(project)
    })();
    result.with_context(|| format!("Project preparation failed. New directories preserved for inspection: {}\nExisting repositories have not been modified",preserved.display()))
}

pub fn attach(code: &Path, spec: &Path, progress: Progress<'_>) -> Result<ProjectInfo> {
    let code = git::root(&files::absolute(code)?)?;
    let spec = git::root(&files::absolute(spec)?)?;
    let (manifest, _) = manifest::read(&spec)?;
    let expected = spec
        .parent()
        .context("Spec has no parent")?
        .join(&manifest.name);
    if !files::same(&code, &expected) {
        bail!(
            "The spec expects its code in the sibling directory {}. No separate project bindings are stored.",
            expected.display()
        );
    }
    let project = info(&spec, true, progress)?;
    crate::agents::sync(&project)?;
    Ok(project)
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
    for folder in [&info.code, &info.spec, &info.editable_guidelines] {
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

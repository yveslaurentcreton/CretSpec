use crate::{
    files, git,
    manifest::{self, Guidelines, GuidelinesMode, Lock, Manifest, Source},
    repository, workspace,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub struct InitOptions<'a> {
    pub directory: &'a Path,
    pub name: &'a str,
    pub code: &'a str,
    pub guidelines: &'a str,
    pub reference: Option<&'a str>,
    pub pinned: bool,
    pub profile: &'a str,
}

pub fn initialize(options: InitOptions<'_>) -> Result<PathBuf> {
    let spec = git::root(&files::absolute(options.directory)?)?;
    for name in ["project.json", "guidelines.lock.json", "spec"] {
        if files::exists(&spec.join(name))? {
            bail!(
                "Initialization would overwrite {}. Use an uninitialized spec repository.",
                spec.join(name).display()
            );
        }
    }
    if options.pinned && options.reference.is_none() {
        bail!("Pinned initialization requires --ref.");
    }
    let mut definition = Manifest {
        schema_version: 1,
        name: options.name.into(),
        code: Source {
            repository: options.code.into(),
        },
        guidelines: Guidelines {
            repository: options.guidelines.into(),
            reference: options.reference.unwrap_or("HEAD").into(),
            mode: Some(if options.pinned {
                GuidelinesMode::Pinned
            } else {
                GuidelinesMode::WorkingTree
            }),
        },
        profile: options.profile.into(),
        agents: manifest::default_agents(),
    };
    // Validate user inputs before fetching any repository.
    let mut lock = options.pinned.then(|| Lock {
        schema_version: 1,
        reference: options.reference.unwrap().into(),
        commit: "0".repeat(40),
    });
    manifest::validate(&definition, lock.as_ref())?;
    let source = repository::origin(&spec)?;
    let layout = workspace::paths(&spec, &definition, &source)?;
    repository::resolve(options.code, &source)?;
    let temporary = tempfile::Builder::new().prefix("cspec-init-").tempdir()?;
    let guidance = temporary.path().join("guidelines");
    git::clone(&layout.guidelines_source, &guidance)?;
    if let Some(lock) = &mut lock {
        lock.commit = git::run(
            &guidance,
            [
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", lock.reference),
            ],
        )?;
        git::run(&guidance, ["checkout", "--detach", &lock.commit])?;
    } else {
        definition.guidelines.reference = match options.reference {
            Some(branch) => branch.to_owned(),
            None => git::run(&guidance, ["symbolic-ref", "--short", "HEAD"])
                .context("The guidelines source needs a default branch or an explicit --ref")?,
        };
        workspace::validate_branch(&guidance, &definition.guidelines.reference)?;
        git::run(&guidance, ["switch", &definition.guidelines.reference])?;
    }
    manifest::validate(&definition, lock.as_ref())?;
    workspace::validate_profile(&guidance, options.profile)?;
    let template = guidance.join("templates/spec/spec");
    let mut content = Vec::new();
    collect_template(&template, &template, &mut content)?;
    if content.is_empty() {
        bail!("The selected guidelines contain no specification templates.");
    }
    for required in [
        "vision.md",
        "scope.md",
        "requirements.md",
        "architecture.md",
        "decisions.md",
        "roadmap.md",
        "acceptance.md",
        "project-rules.md",
    ] {
        if !content.iter().any(|(path, _)| path == Path::new(required)) {
            bail!("Missing required specification template: {required}");
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(".cspec-init-")
        .tempdir_in(&spec)?;
    let staged_spec = staging.path().join("spec");
    fs::create_dir(&staged_spec)?;
    for (relative, bytes) in content {
        let target = staged_spec.join(relative);
        fs::create_dir_all(target.parent().context("Template has no parent")?)?;
        fs::write(target, bytes)?;
    }
    // All source material is validated before creating any definition file.
    fs::rename(&staged_spec, spec.join("spec"))?;
    let result = (|| -> Result<()> {
        write_new_json(&spec.join("project.json"), &definition)?;
        if let Some(lock) = &lock {
            write_new_json(&spec.join("guidelines.lock.json"), lock)?;
        }
        workspace::local_directory(&spec, true)?;
        Ok(())
    })();
    result.context("Spec initialization was interrupted. Newly created files are preserved; inspect them before retrying. Existing files were not overwritten")?;
    Ok(spec)
}

fn collect_template(
    base: &Path,
    current: &Path,
    content: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<()> {
    files::directory(current)?;
    if current.strip_prefix(base)?.components().count() > 8 {
        bail!("Specification templates exceed the supported directory depth.");
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            bail!("Hidden files are not allowed in specification content templates.");
        }
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_template(base, &path, content)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "md") {
            if entry.metadata()?.len() > 128 * 1024 || content.len() >= 128 {
                bail!("Specification templates exceed the supported size.");
            }
            let bytes = fs::read(&path)?;
            std::str::from_utf8(&bytes).context("Specification templates must be UTF-8")?;
            content.push((path.strip_prefix(base)?.to_owned(), bytes));
        } else {
            bail!(
                "Specification templates must be regular Markdown files or directories: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if bytes.len() >= 128 * 1024 {
        bail!("Configuration exceeds the supported size.");
    }
    let mut file = tempfile::NamedTempFile::new_in(path.parent().context("Missing file parent")?)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).with_context(|| {
        format!(
            "File already exists or cannot be created: {}",
            path.display()
        )
    })?;
    Ok(())
}

#[derive(Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

pub fn diagnose(start: &Path) -> Vec<Check> {
    let mut checks = Vec::new();
    match git::run(start, ["--version"]) {
        Ok(version) => checks.push(Check {
            name: "Git".into(),
            ok: true,
            detail: version,
        }),
        Err(error) => {
            checks.push(Check {
                name: "Git".into(),
                ok: false,
                detail: format!("{error:#}"),
            });
            return checks;
        }
    }
    match workspace::info(start, false, &|_| {}) {
        Ok(info) => {
            checks.push(Check {
                name: "Project".into(),
                ok: true,
                detail: format!(
                    "{}; guidance {} ({})",
                    info.manifest.name,
                    info.manifest.guidelines.reference,
                    &info.guidelines.commit[..12]
                ),
            });
            let (ok, detail) = match crate::agents::check(&info) {
                Ok(detail) => (true, detail),
                Err(error) => (false, format!("{error:#}")),
            };
            checks.push(Check {
                name: "Agent integrations".into(),
                ok,
                detail,
            });
            if info.guidelines.mode == GuidelinesMode::WorkingTree {
                let state = &info.guidelines;
                let mut notices = Vec::new();
                if state.dirty {
                    notices.push(
                        "local changes are active; HEAD alone does not identify these contents"
                            .to_owned(),
                    );
                }
                match &state.branch {
                    None => notices.push("detached HEAD; use Git to select a branch".into()),
                    Some(branch) if branch != &info.manifest.guidelines.reference => {
                        notices.push(format!(
                            "active branch {branch} differs from configured {}",
                            info.manifest.guidelines.reference
                        ))
                    }
                    _ => {}
                }
                if state.upstream.is_none() {
                    notices.push("no upstream configured".into());
                }
                if state.ahead.is_some_and(|n| n > 0) {
                    notices.push(format!(
                        "{} commits ahead of cached upstream",
                        state.ahead.unwrap()
                    ));
                }
                if state.behind.is_some_and(|n| n > 0) {
                    notices.push(format!(
                        "{} commits behind cached upstream",
                        state.behind.unwrap()
                    ));
                }
                checks.push(Check {
                    name: "Guidelines working tree".into(),
                    ok: true,
                    detail: if notices.is_empty() {
                        "Local working tree is active. Remote freshness was not checked; use Git to fetch/pull.".into()
                    } else {
                        format!("Warning: {}. No Git state was changed or fetched.", notices.join("; "))
                    },
                });
            }
        }
        Err(error) => checks.push(Check {
            name: "Project".into(),
            ok: false,
            detail: format!("{error:#}"),
        }),
    }
    checks
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub previous: workspace::GuidelinesState,
    pub selected: Lock,
    pub applied: bool,
    pub changes: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    old_manifest: String,
    old_lock: Option<String>,
    new_manifest: Manifest,
    new_lock: Option<Lock>,
}

pub fn update(start: &Path, reference: &str, preview: bool, fetch: bool) -> Result<UpdateResult> {
    if !manifest::valid_ref(reference) {
        bail!("Invalid guidelines ref.");
    }
    let info = workspace::info(start, true, &|_| {})?;
    if fetch {
        git::run(&info.editable_guidelines, ["fetch", "--tags", "origin"])?;
    }
    let hash = git::run(
        &info.editable_guidelines,
        ["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )
    .context("Requested guidelines version is unavailable. Fetch it with Git or use --fetch")?;
    let mut definition = info.manifest.clone();
    definition.guidelines.reference = reference.into();
    definition.guidelines.mode = Some(GuidelinesMode::Pinned);
    let selected = Lock {
        schema_version: 1,
        reference: reference.into(),
        commit: hash,
    };
    manifest::validate(&definition, Some(&selected))?;
    let (_, active_guidelines) =
        workspace::snapshot(&info.spec, &definition, Some(&selected), true, &|_| {})?;
    let mut proposed = info.clone();
    proposed.manifest = definition.clone();
    proposed.lock = Some(selected.clone());
    proposed.guidelines = workspace::guidelines_state(&active_guidelines, GuidelinesMode::Pinned)?;
    proposed.active_guidelines = active_guidelines;
    crate::skills::active(&proposed).context("Requested guidelines contain invalid or conflicting skills. The definition was not changed")?;
    let changes = git::run(
        &info.editable_guidelines,
        [
            "diff",
            "--stat",
            &info.guidelines.commit,
            &selected.commit,
            "--",
        ],
    )?;
    if !preview {
        save_definition(&info, definition, Some(selected.clone()))?;
        refresh_agents(&info.spec)?;
    }
    Ok(UpdateResult {
        previous: info.guidelines,
        selected,
        applied: !preview,
        changes,
    })
}

pub fn unlock(start: &Path, preview: bool) -> Result<workspace::ProjectInfo> {
    let info = workspace::info(start, true, &|_| {})?;
    let state =
        workspace::guidelines_state(&info.editable_guidelines, GuidelinesMode::WorkingTree)?;
    let branch = state.branch.as_ref().context("Select a branch in the editable guidelines repository with Git before unlocking. No checkout was changed")?;
    let mut definition = info.manifest.clone();
    definition.guidelines.reference = branch.clone();
    definition.guidelines.mode = Some(GuidelinesMode::WorkingTree);
    manifest::validate(&definition, None)?;
    workspace::validate_branch(&info.editable_guidelines, branch)?;
    workspace::validate_profile(&info.editable_guidelines, &definition.profile)?;
    let mut proposed = info.clone();
    proposed.manifest = definition.clone();
    proposed.lock = None;
    proposed.active_guidelines = info.editable_guidelines.clone();
    proposed.guidelines = state;
    crate::skills::active(&proposed).context("Working-tree guidelines contain invalid or conflicting skills. The definition was not changed")?;
    if !preview {
        save_definition(&info, definition, None)?;
        refresh_agents(&info.spec)?;
    }
    Ok(proposed)
}

fn refresh_agents(spec: &Path) -> Result<()> {
    let current = workspace::info(spec, false, &|_| {})?;
    crate::agents::sync(&current).context("The guidelines definition is saved, but agent synchronization failed. Preserve and reconcile the reported files, then run cspec sync")?;
    Ok(())
}

fn read_optional_text(path: &Path) -> Result<Option<String>> {
    if files::exists(path)? {
        let _: serde_json::Value = files::read_json(path)?;
        Ok(Some(fs::read_to_string(path)?))
    } else {
        Ok(None)
    }
}

fn write_optional_lock(spec: &Path, lock: Option<&Lock>) -> Result<()> {
    let path = spec.join("guidelines.lock.json");
    match lock {
        Some(lock) => files::write_json(&path, lock),
        None => {
            if files::exists(&path)? {
                let _: Lock = files::read_json(&path)?;
                fs::remove_file(&path)?;
            }
            Ok(())
        }
    }
}

fn save_definition(
    info: &workspace::ProjectInfo,
    definition: Manifest,
    lock: Option<Lock>,
) -> Result<()> {
    manifest::validate(&definition, lock.as_ref())?;
    let journal = Journal {
        old_manifest: fs::read_to_string(info.spec.join("project.json"))?,
        old_lock: read_optional_text(&info.spec.join("guidelines.lock.json"))?,
        new_manifest: definition,
        new_lock: lock,
    };
    let original_manifest: Manifest =
        serde_json::from_str(journal.old_manifest.trim_start_matches('\u{feff}'))?;
    let original_lock: Option<Lock> = journal
        .old_lock
        .as_deref()
        .map(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')))
        .transpose()?;
    if serde_json::to_value(&original_manifest)? != serde_json::to_value(&info.manifest)?
        || serde_json::to_value(&original_lock)? != serde_json::to_value(&info.lock)?
    {
        bail!("The project definition changed during the update. Retry after reviewing it.");
    }
    if serde_json::to_value(&original_manifest)? == serde_json::to_value(&journal.new_manifest)?
        && serde_json::to_value(&original_lock)? == serde_json::to_value(&journal.new_lock)?
    {
        return Ok(());
    }
    let journal_path = workspace::local_directory(&info.spec, true)?.join("definition-update.json");
    write_new_json(&journal_path, &journal)?;
    if fs::read_to_string(info.spec.join("project.json"))? != journal.old_manifest
        || read_optional_text(&info.spec.join("guidelines.lock.json"))? != journal.old_lock
    {
        fs::remove_file(&journal_path)?;
        bail!(
            "The project definition changed during the update. No definition files were written."
        );
    }
    let result = (|| -> Result<()> {
        files::write_json(&info.spec.join("project.json"), &journal.new_manifest)?;
        write_optional_lock(&info.spec, journal.new_lock.as_ref())?;
        fs::remove_file(&journal_path)?;
        Ok(())
    })();
    if let Err(error) = result {
        match recover(&info.spec) {
            Ok(_) => {
                return Err(error)
                    .context("Guidelines update failed; the original definition was restored");
            }
            Err(recovery) => bail!(
                "Guidelines update failed: {error:#}. Recovery is required: {recovery:#}. Run cspec guidelines recover {}",
                info.spec.display()
            ),
        }
    }
    Ok(())
}

pub fn recover(start: &Path) -> Result<PathBuf> {
    let spec = workspace::discover_spec(start)?;
    let journal_path = workspace::local_directory(&spec, false)?.join("definition-update.json");
    let journal: Journal =
        files::read_json(&journal_path).context("No readable update journal is available")?;
    let old_manifest: Manifest =
        serde_json::from_str(journal.old_manifest.trim_start_matches('\u{feff}'))?;
    let old_lock: Option<Lock> = journal
        .old_lock
        .as_deref()
        .map(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')))
        .transpose()?;
    manifest::validate(&old_manifest, old_lock.as_ref())?;
    manifest::validate(&journal.new_manifest, journal.new_lock.as_ref())?;
    let current_manifest: serde_json::Value = files::read_json(&spec.join("project.json"))?;
    let current_lock = manifest::read_lock(&spec)?;
    if (current_manifest != serde_json::to_value(&old_manifest)?
        && current_manifest != serde_json::to_value(&journal.new_manifest)?)
        || (serde_json::to_value(&current_lock)? != serde_json::to_value(&old_lock)?
            && serde_json::to_value(&current_lock)? != serde_json::to_value(&journal.new_lock)?)
    {
        bail!(
            "The definition was changed after the interrupted update. Preserve and reconcile those changes before recovery; nothing was overwritten."
        );
    }
    write_text(&spec.join("project.json"), &journal.old_manifest)?;
    if let Some(text) = &journal.old_lock {
        write_text(&spec.join("guidelines.lock.json"), text)?;
    } else {
        write_optional_lock(&spec, None)?;
    }
    fs::remove_file(journal_path)?;
    Ok(spec)
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if files::exists(path)? && !fs::symlink_metadata(path)?.file_type().is_file() {
        bail!("Refusing to replace a non-regular configuration file.");
    }
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().context("Missing parent")?)?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

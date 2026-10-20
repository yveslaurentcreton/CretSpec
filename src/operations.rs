use crate::{
    files, git,
    manifest::{
        self, Guidelines, GuidelinesMode, Lock, Manifest, Repository, Source, SpecRepository,
    },
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
    /// A single code repository selects schema version 1.
    pub code: Option<&'a str>,
    pub guidelines: &'a str,
    pub reference: Option<&'a str>,
    pub pinned: bool,
    pub profile: Option<&'a str>,
    /// Schema version 2 repositories as `<path>` (an existing sibling clone) or `<path>=<url>`.
    pub repositories: &'a [String],
    /// Keep an existing spec/ directory and write only the definition.
    pub existing_spec: bool,
}

fn declared_repository(spec: &Path, value: &str) -> Result<Repository> {
    let (path, url) = match value.split_once('=') {
        Some((path, url)) => (path, Some(url)),
        None => (value, None),
    };
    let name = path.to_ascii_lowercase();
    if !crate::skills::valid_name(&name) {
        bail!(
            "Repository directory {path} needs a name of 1-64 letters, digits or single hyphens. Rename it, or edit project.json after initialization."
        );
    }
    let url = match url {
        Some(url) => repository::without_user(url)?,
        None => {
            let sibling = spec.parent().context("Spec has no parent")?.join(path);
            git::root(&sibling).with_context(|| {
                format!(
                    "No existing clone at {}. Pass {path}=<url> to declare a repository that is not cloned yet",
                    sibling.display()
                )
            })?;
            let origin =
                git::run(&sibling, ["remote", "get-url", "origin"]).with_context(|| {
                    format!(
                        "{} has no origin remote. Pass {path}=<url>",
                        sibling.display()
                    )
                })?;
            repository::without_user(&origin)?
        }
    };
    Ok(Repository {
        path: (path != name).then(|| path.to_owned()),
        name,
        url,
        ..Repository::default()
    })
}

pub fn initialize(options: InitOptions<'_>) -> Result<PathBuf> {
    let spec = git::root(&files::absolute(options.directory)?)?;
    for name in ["project.json", "guidelines.lock.json", "spec"] {
        if name == "spec" && options.existing_spec {
            files::directory(&spec.join(name))
                .context("--existing-spec requires an existing spec/ directory")?;
        } else if files::exists(&spec.join(name))? {
            bail!(
                "Initialization would overwrite {}. Use an uninitialized spec repository{}.",
                spec.join(name).display(),
                if name == "spec" {
                    ", or keep the existing specification with --existing-spec"
                } else {
                    ""
                }
            );
        }
    }
    if options.pinned && options.reference.is_none() {
        bail!("Pinned initialization requires --ref.");
    }
    let (schema_version, code, profile, spec_repository, repositories) = match options.code {
        Some(code) => {
            if !options.repositories.is_empty() {
                bail!(
                    "Use either --code for one code repository or --repository for named repositories."
                );
            }
            let profile = options
                .profile
                .context("A single code repository requires --profile.")?;
            (
                1,
                Some(Source {
                    repository: code.into(),
                }),
                Some(profile.into()),
                None,
                None,
            )
        }
        None => {
            let repositories = options
                .repositories
                .iter()
                .map(|value| declared_repository(&spec, value))
                .collect::<Result<Vec<_>>>()?;
            let directory = spec
                .file_name()
                .context("Spec has no directory name")?
                .to_string_lossy()
                .to_ascii_lowercase();
            let spec_repository = SpecRepository {
                name: crate::skills::valid_name(&directory).then_some(directory),
                purpose: None,
                profile: options.profile.map(Into::into),
            };
            (2, None, None, Some(spec_repository), Some(repositories))
        }
    };
    let mut definition = Manifest {
        schema_version,
        name: options.name.into(),
        code,
        spec_repository,
        repositories,
        guidelines: Guidelines {
            repository: options.guidelines.into(),
            reference: options.reference.unwrap_or("HEAD").into(),
            mode: Some(if options.pinned {
                GuidelinesMode::Pinned
            } else {
                GuidelinesMode::WorkingTree
            }),
        },
        profile,
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
    workspace::validate_profiles(&guidance, &definition)?;
    let mut content = Vec::new();
    if !options.existing_spec {
        let template = guidance.join("templates/spec/spec");
        collect_template(&template, &template, &mut content)?;
    }
    if content.is_empty() && !options.existing_spec {
        bail!("The selected guidelines contain no specification templates.");
    }
    for required in (!options.existing_spec)
        .then_some([
            "vision.md",
            "scope.md",
            "requirements.md",
            "architecture.md",
            "decisions.md",
            "roadmap.md",
            "acceptance.md",
            "project-rules.md",
        ])
        .into_iter()
        .flatten()
    {
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
    if !options.existing_spec {
        fs::rename(&staged_spec, spec.join("spec"))?;
    }
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
            checks.push(repositories_check(&info));
            let warnings = crate::agents::size_warnings(&info);
            if !warnings.is_empty() {
                checks.push(Check {
                    name: "Agent instruction size".into(),
                    ok: true,
                    detail: format!("Warning: {}", warnings.join("; ")),
                });
            }
            if info.tree.layered() || !info.tree.warnings.is_empty() {
                checks.push(references_check(&info));
            }
            let warnings = crate::agents::claude_warnings(&info);
            if !warnings.is_empty() {
                checks.push(Check {
                    name: "Claude Code entry points".into(),
                    ok: true,
                    detail: format!("Warning: {}", warnings.join("; ")),
                });
            }
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

fn references_check(info: &workspace::ProjectInfo) -> Check {
    let tree = &info.tree;
    let projects = tree.projects.len() - 1;
    let guidelines = tree
        .references
        .iter()
        .filter(|r| r.kind == manifest::Kind::Guidelines)
        .count();
    let mut detail = format!(
        "{projects} referenced {} and {guidelines} guidelines {} resolved",
        if projects == 1 { "project" } else { "projects" },
        if guidelines == 1 { "source" } else { "sources" }
    );
    let warnings = crate::agents::reference_warnings(info);
    if !warnings.is_empty() {
        detail.push_str(&format!(". Warning: {}", warnings.join("; ")));
    }
    Check {
        name: "References".into(),
        ok: true,
        detail,
    }
}

fn repositories_check(info: &workspace::ProjectInfo) -> Check {
    let declared = format!(
        "{} declared {} present with matching origins",
        info.repositories.len(),
        if info.repositories.len() == 1 {
            "repository is"
        } else {
            "repositories are"
        }
    );
    match workspace::undeclared_repositories(info) {
        Ok(undeclared) if undeclared.is_empty() => Check {
            name: "Repositories".into(),
            ok: true,
            detail: declared,
        },
        Ok(undeclared) => Check {
            name: "Repositories".into(),
            ok: true,
            detail: format!(
                "{declared}. Warning: undeclared Git repositories in the project root: {}. Declare them in project.json if they belong to this workspace.",
                undeclared
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        Err(error) => Check {
            name: "Repositories".into(),
            ok: false,
            detail: format!("{error:#}"),
        },
    }
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
    workspace::validate_profiles(&info.editable_guidelines, &definition)?;
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

/// Converts a schema version 2 definition to the current schema. The generated context
/// stays the same; the new schema only allows typed references.
pub fn migrate(start: &Path, preview: bool) -> Result<(u32, u32)> {
    let info = workspace::info(start, false, &|_| {})?;
    let from = info.manifest.schema_version;
    let to = manifest::CURRENT_SCHEMA;
    match from {
        1 => bail!(
            "Schema version 1 names one code repository by its location next to the spec. Declare it by name first: set schemaVersion 2, move code.repository to repositories (name, url and path {}) and the profile to that repository, then run cspec sync and cspec project migrate.",
            info.manifest.name
        ),
        version if version == to => return Ok((from, to)),
        _ => {}
    }
    let mut definition = info.manifest.clone();
    definition.schema_version = to;
    manifest::validate(&definition, info.lock.as_ref())?;
    if !preview {
        save_definition(&info, definition, info.lock.clone())?;
        refresh_agents(&info.spec)?;
    }
    Ok((from, to))
}

pub fn recover(start: &Path) -> Result<PathBuf> {
    // A referenced project's spec is recovered itself, not the workspace around it.
    let given = git::root(&files::absolute(start)?).ok();
    let spec = match given {
        Some(root) if files::exists(&root.join(".local/definition-update.json"))? => root,
        _ => workspace::discover_spec(start)?,
    };
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

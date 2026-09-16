use crate::{
    files, git,
    manifest::{self, Guidelines, Lock, Manifest, Source},
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
    pub reference: &'a str,
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
    let definition = Manifest {
        schema_version: 1,
        name: options.name.into(),
        code: Source {
            repository: options.code.into(),
        },
        guidelines: Guidelines {
            repository: options.guidelines.into(),
            reference: options.reference.into(),
        },
        profile: options.profile.into(),
    };
    // Validate user inputs before fetching any repository.
    let mut lock = Lock {
        schema_version: 1,
        reference: options.reference.into(),
        commit: "0".repeat(40),
    };
    manifest::validate(&definition, &lock)?;
    let source = repository::origin(&spec)?;
    let layout = workspace::paths(&spec, &definition, &source)?;
    repository::resolve(options.code, &source)?;
    let temporary = tempfile::Builder::new().prefix("cspec-init-").tempdir()?;
    let guidance = temporary.path().join("guidelines");
    git::clone(&layout.guidelines_source, &guidance)?;
    lock.commit = git::run(
        &guidance,
        [
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", lock.reference),
        ],
    )?;
    manifest::validate(&definition, &lock)?;
    git::run(&guidance, ["checkout", "--detach", &lock.commit])?;
    let profile = guidance
        .join("profiles")
        .join(format!("{}.md", options.profile));
    if !fs::symlink_metadata(&profile)
        .with_context(|| {
            format!(
                "Profile '{}' does not exist in {}",
                options.profile, options.reference
            )
        })?
        .file_type()
        .is_file()
    {
        bail!("Profile must be a regular file.");
    }
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
        write_new_json(&spec.join("guidelines.lock.json"), &lock)?;
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
        Ok(info) => checks.push(Check {
            name: "Project".into(),
            ok: true,
            detail: format!(
                "{}; guidance {} ({})",
                info.manifest.name,
                info.lock.reference,
                &info.lock.commit[..12]
            ),
        }),
        Err(error) => checks.push(Check {
            name: "Project".into(),
            ok: false,
            detail: format!("{error:#}"),
        }),
    }
    checks
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub previous: Lock,
    pub selected: Lock,
    pub applied: bool,
    pub changes: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    old_manifest: String,
    old_lock: String,
    new_manifest: Manifest,
    new_lock: Lock,
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
    .context("Requested guidelines version is unavailable. Fetch tags first or use --fetch")?;
    let mut definition = info.manifest.clone();
    definition.guidelines.reference = reference.into();
    let selected = Lock {
        schema_version: 1,
        reference: reference.into(),
        commit: hash,
    };
    manifest::validate(&definition, &selected)?;
    workspace::snapshot(&info.spec, &definition, &selected, true, &|_| {})?;
    let changes = git::run(
        &info.editable_guidelines,
        ["diff", "--stat", &info.lock.commit, &selected.commit, "--"],
    )?;
    if !preview
        && (selected.commit != info.lock.commit || selected.reference != info.lock.reference)
    {
        let journal = Journal {
            old_manifest: fs::read_to_string(info.spec.join("project.json"))?,
            old_lock: fs::read_to_string(info.spec.join("guidelines.lock.json"))?,
            new_manifest: definition,
            new_lock: selected.clone(),
        };
        let journal_path =
            workspace::local_directory(&info.spec, true)?.join("definition-update.json");
        let original_manifest: Manifest =
            serde_json::from_str(journal.old_manifest.trim_start_matches('\u{feff}'))?;
        let original_lock: Lock =
            serde_json::from_str(journal.old_lock.trim_start_matches('\u{feff}'))?;
        if serde_json::to_value(&original_manifest)? != serde_json::to_value(&info.manifest)?
            || serde_json::to_value(&original_lock)? != serde_json::to_value(&info.lock)?
        {
            bail!("The project definition changed during the update. Retry after reviewing it.");
        }
        write_new_json(&journal_path, &journal)?;
        if fs::read_to_string(info.spec.join("project.json"))? != journal.old_manifest
            || fs::read_to_string(info.spec.join("guidelines.lock.json"))? != journal.old_lock
        {
            fs::remove_file(&journal_path)?;
            bail!(
                "The project definition changed during the update. No definition files were written."
            );
        }
        let result = (|| -> Result<()> {
            files::write_json(&info.spec.join("project.json"), &journal.new_manifest)?;
            files::write_json(&info.spec.join("guidelines.lock.json"), &journal.new_lock)?;
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
    }
    Ok(UpdateResult {
        previous: info.lock,
        selected,
        applied: !preview,
        changes,
    })
}

pub fn recover(start: &Path) -> Result<PathBuf> {
    let spec = workspace::discover_spec(start)?;
    let journal_path = workspace::local_directory(&spec, false)?.join("definition-update.json");
    let journal: Journal =
        files::read_json(&journal_path).context("No readable update journal is available")?;
    let old_manifest: Manifest =
        serde_json::from_str(journal.old_manifest.trim_start_matches('\u{feff}'))?;
    let old_lock: Lock = serde_json::from_str(journal.old_lock.trim_start_matches('\u{feff}'))?;
    manifest::validate(&old_manifest, &old_lock)?;
    manifest::validate(&journal.new_manifest, &journal.new_lock)?;
    for (name, old, new) in [
        (
            "project.json",
            serde_json::to_value(&old_manifest)?,
            serde_json::to_value(&journal.new_manifest)?,
        ),
        (
            "guidelines.lock.json",
            serde_json::to_value(&old_lock)?,
            serde_json::to_value(&journal.new_lock)?,
        ),
    ] {
        let current: serde_json::Value = files::read_json(&spec.join(name))?;
        if current != old && current != new {
            bail!(
                "{} was changed after the interrupted update. Preserve and reconcile those changes before recovery; nothing was overwritten.",
                name
            );
        }
    }
    write_text(&spec.join("project.json"), &journal.old_manifest)?;
    write_text(&spec.join("guidelines.lock.json"), &journal.old_lock)?;
    fs::remove_file(journal_path)?;
    Ok(spec)
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        bail!("Refusing to replace a non-regular configuration file.");
    }
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().context("Missing parent")?)?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

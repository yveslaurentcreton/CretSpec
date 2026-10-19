use crate::{files, git, workspace::ProjectInfo};
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
    /// The declaring repository of a repository skill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    pub source: Option<PathBuf>,
}

impl Skill {
    fn origin(&self) -> String {
        match (self.scope, &self.repository) {
            (Scope::Repository, Some(repository)) => format!("repository `{repository}`"),
            (Scope::Project, _) => "the project source".into(),
            (Scope::Shared, _) => "the active shared source".into(),
            _ => "the built-in source".into(),
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

pub fn active(info: &ProjectInfo) -> Result<Vec<Bundle>> {
    // Check the source parent as well, so a replaced spec/ directory is never followed.
    files::directory(&info.spec.join("spec"))?;
    let mut bundles = vec![Bundle {
        skill: header(BUILTIN.as_bytes(), BUILTIN_NAME, Scope::Builtin, None)?,
        files: BTreeMap::from([(
            "SKILL.md".into(),
            Resource {
                bytes: BUILTIN.as_bytes().to_vec(),
                executable: false,
            },
        )]),
    }];
    bundles.extend(scan(&info.spec.join("spec/skills"), Scope::Project)?);
    let adopted = scan(&info.active_guidelines.join("skills"), Scope::Shared)?;
    if info.lock.is_some() {
        let tracked = git::run(&info.active_guidelines, ["ls-files", "-z", "--", "skills"])?;
        let tracked: BTreeSet<_> = tracked.split('\0').collect();
        for bundle in &adopted {
            for name in bundle.files.keys() {
                let path = format!("skills/{}/{name}", bundle.skill.name);
                if !tracked.contains(path.as_str()) {
                    bail!(
                        "Adopted skill contains a file outside the locked Git snapshot: {path}. Preserve and remove that local addition before retrying."
                    );
                }
            }
        }
    }
    bundles.extend(adopted);
    for repository in &info.repositories {
        let Some(directory) = &repository.skills else {
            continue;
        };
        // A repository that does not exist yet, or has no skill directory yet, has no skills.
        if !files::exists(&repository.path)? || !source_present(&repository.path, directory)? {
            continue;
        }
        let mut found = scan(directory, Scope::Repository)?;
        for bundle in &mut found {
            bundle.skill.repository = Some(repository.name.clone());
        }
        bundles.extend(found);
    }
    bundles.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    for pair in bundles.windows(2) {
        if pair[0].skill.name == pair[1].skill.name {
            bail!(
                "Skill '{}' exists in both {} and {}. Rename one source skill before synchronizing.",
                pair[0].skill.name,
                pair[0].skill.origin(),
                pair[1].skill.origin()
            );
        }
    }
    let size: usize = bundles
        .iter()
        .flat_map(|b| b.files.values())
        .map(|r| r.bytes.len())
        .sum();
    if size > 64 * 1024 * 1024 {
        bail!("Active skill resources exceed 64 MiB");
    }
    Ok(bundles)
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
    let active = active(info)?.into_iter().map(|b| b.skill).collect();
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

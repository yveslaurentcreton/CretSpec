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
    #[value(skip)]
    Builtin,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub scope: Scope,
    pub source: Option<PathBuf>,
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

fn description_valid(description: &str) -> bool {
    !description.trim().is_empty() && description.chars().count() <= 1024
}

fn header(bytes: &[u8], expected: &str, scope: Scope, source: Option<PathBuf>) -> Result<Skill> {
    let text = std::str::from_utf8(bytes).context("SKILL.md must be UTF-8")?;
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let yaml = text
        .strip_prefix("---\n")
        .and_then(|s| {
            s.split_once("\n---")
                .filter(|(_, rest)| rest.is_empty() || rest.starts_with('\n'))
                .map(|(y, _)| y)
        })
        .context("SKILL.md needs YAML frontmatter between --- lines")?;
    #[derive(Deserialize)]
    struct Header {
        name: String,
        description: String,
    }
    let parsed: Header = serde_yaml_ng::from_str(yaml).context("Invalid skill frontmatter")?;
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
    bundles.extend(adopted);
    bundles.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    for pair in bundles.windows(2) {
        if pair[0].skill.name == pair[1].skill.name {
            bail!(
                "Skill '{}' exists in both project and adopted shared sources. Rename one source skill before synchronizing.",
                pair[0].skill.name
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

pub fn inventory(info: &ProjectInfo) -> Result<Inventory> {
    let active = active(info)?.into_iter().map(|b| b.skill).collect();
    let (shared_drafts, draft_errors) =
        match scan(&info.editable_guidelines.join("skills"), Scope::Shared) {
            Ok(bundles) => (bundles.into_iter().map(|b| b.skill).collect(), vec![]),
            Err(error) => (vec![], vec![format!("{error:#}")]),
        };
    Ok(Inventory {
        schema_version: 1,
        active,
        shared_drafts,
        draft_errors,
    })
}

pub fn create(info: &ProjectInfo, name: &str, scope: Scope, description: &str) -> Result<PathBuf> {
    if !valid_name(name) || name == BUILTIN_NAME || !description_valid(description) {
        bail!(
            "Use a portable skill name (excluding cspec-workspace) and a nonempty description of at most 1024 characters."
        );
    }
    if active(info)?.iter().any(|b| b.skill.name == name) {
        bail!("Skill '{name}' is already active. Edit its source or choose a distinct name.");
    }
    let parent = match scope {
        Scope::Project => info.spec.join("spec"),
        Scope::Shared => info.editable_guidelines.clone(),
        Scope::Builtin => {
            bail!("Built-in skills are maintained with CretSpec. Choose project or shared.")
        }
    };
    files::directory(&parent)?;
    let root = parent.join("skills");
    if files::exists(&root)? {
        files::directory(&root)?;
    } else {
        fs::create_dir(&root)?;
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

use crate::{files, repository};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A project definition. Schema version 1 names one code repository and a profile;
/// schema version 2 names any number of repositories, each with an optional profile.
/// Schema version 3 adds typed references to further guidelines and other projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_repository: Option<SpecRepository>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repositories: Option<Vec<Repository>>,
    pub guidelines: Guidelines,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default = "default_agents")]
    pub agents: Vec<Agent>,
}

/// Describes the repository containing the definition, which may also contain product code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecRepository {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub name: String,
    /// What the reference contains. Omitted means code; other types need schema version 3.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The repository's own skill directory, relative to the repository root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<String>,
    /// Guidelines: the branch for a working tree, or the selected version when pinned.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Guidelines: the exact commit that pins this reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Guidelines: the repositories of this definition where the guidance applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    /// Guidelines and projects: a higher priority comes earlier in the precedence order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    /// Guidelines: files to read, relative to the repository. Defaults to the CretAI layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<Vec<String>>,
}

impl Repository {
    pub fn directory(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }

    pub fn kind(&self) -> Kind {
        self.kind.unwrap_or(Kind::Code)
    }
}

/// The type of a repository reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Code,
    Guidelines,
    Project,
}

/// The newest supported definition schema.
pub const CURRENT_SCHEMA: u32 = 3;
/// The default instruction files of a guidelines repository.
pub const GUIDELINE_INSTRUCTIONS: [&str; 3] = [
    "README.md",
    "guidelines/principles.md",
    "guidelines/specification.md",
];

/// The directory inside the project root that contains generated role entry points.
pub const ROLES_DIRECTORY: &str = "roles";
/// The repository name of the spec when a definition does not name it.
pub const DEFAULT_SPEC_NAME: &str = "spec";
/// The repository name of the code repository in schema version 1 definitions.
pub const LEGACY_CODE_NAME: &str = "code";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
    Copilot,
    Cursor,
}

pub fn default_agents() -> Vec<Agent> {
    vec![Agent::Codex, Agent::Claude, Agent::Copilot, Agent::Cursor]
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guidelines {
    pub repository: String,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<GuidelinesMode>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuidelinesMode {
    WorkingTree,
    Pinned,
}

impl Manifest {
    pub fn spec_name(&self) -> &str {
        self.spec_repository
            .as_ref()
            .and_then(|spec| spec.name.as_deref())
            .unwrap_or(DEFAULT_SPEC_NAME)
    }

    /// Every distinct profile selected by the definition, in declaration order.
    pub fn profiles(&self) -> Vec<&str> {
        let mut result = Vec::new();
        let declared = self.profile.as_deref().into_iter().chain(
            self.spec_repository
                .as_ref()
                .and_then(|spec| spec.profile.as_deref()),
        );
        let repositories = self
            .repositories
            .iter()
            .flatten()
            .filter_map(|repository| repository.profile.as_deref());
        for profile in declared.chain(repositories) {
            if !result.contains(&profile) {
                result.push(profile);
            }
        }
        result
    }

    pub fn guidelines_mode(&self, lock: Option<&Lock>) -> GuidelinesMode {
        self.guidelines.mode.unwrap_or(if lock.is_some() {
            GuidelinesMode::Pinned
        } else {
            GuidelinesMode::WorkingTree
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Lock {
    pub schema_version: u32,
    #[serde(rename = "ref")]
    pub reference: String,
    pub commit: String,
}

pub fn valid_ref(reference: &str) -> bool {
    reference
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && reference
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._/-".contains(&c))
}

pub fn read(spec: &Path) -> Result<(Manifest, Option<Lock>)> {
    if files::exists(&spec.join(".local/definition-update.json"))? {
        bail!(
            "A guidelines update was interrupted. Run cspec guidelines recover with this project directory before continuing."
        );
    }
    let manifest: Manifest = files::read_json(&spec.join("project.json"))?;
    let lock = read_lock(spec)?;
    validate(&manifest, lock.as_ref())?;
    files::directory(&spec.join("spec"))?;
    Ok((manifest, lock))
}

pub fn read_lock(spec: &Path) -> Result<Option<Lock>> {
    let path = spec.join("guidelines.lock.json");
    if files::exists(&path)? {
        Ok(Some(files::read_json(&path)?))
    } else {
        Ok(None)
    }
}

fn valid_profile(profile: &str) -> bool {
    profile
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
}

fn validate_purpose(purpose: Option<&str>, owner: &str) -> Result<()> {
    if let Some(purpose) = purpose
        && (purpose.trim().is_empty()
            || purpose.chars().count() > 500
            || purpose.chars().any(char::is_control))
    {
        bail!("The purpose of {owner} must be one nonempty line of at most 500 characters.");
    }
    Ok(())
}

/// A relative path of portable, non-hidden names separated by `/`, such as a skill
/// directory or a guideline instruction file.
pub fn valid_relative_directory(path: &str) -> bool {
    let segments: Vec<_> = path.split('/').collect();
    path.len() <= 255
        && segments.len() <= 8
        && segments
            .iter()
            .all(|segment| files::portable_name(segment) && !segment.starts_with('.'))
}

fn valid_commit(commit: &str) -> bool {
    [40, 64].contains(&commit.len())
        && commit
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Checks the fields that belong to one reference type.
fn validate_reference(
    repository: &Repository,
    schema_version: u32,
    names: &[String],
) -> Result<()> {
    let name = &repository.name;
    let typed = repository.kind.is_some()
        || repository.reference.is_some()
        || repository.commit.is_some()
        || repository.scope.is_some()
        || repository.priority.is_some()
        || repository.instructions.is_some();
    if typed && schema_version < 3 {
        bail!(
            "Repository {name} uses typed reference fields (type, ref, commit, scope, priority or instructions), which require schemaVersion 3. Run cspec project migrate first."
        );
    }
    let kind = repository.kind();
    let only = |present: bool, field: &str, allowed: &str| -> Result<()> {
        if present {
            bail!("Repository {name}: {field} is only valid for {allowed} references.");
        }
        Ok(())
    };
    only(
        kind != Kind::Code && repository.profile.is_some(),
        "profile",
        "code",
    )?;
    only(
        kind == Kind::Project && repository.skills.is_some(),
        "skills",
        "code and guidelines",
    )?;
    for (present, field) in [
        (repository.reference.is_some(), "ref"),
        (repository.commit.is_some(), "commit"),
        (repository.scope.is_some(), "scope"),
        (repository.instructions.is_some(), "instructions"),
    ] {
        only(kind != Kind::Guidelines && present, field, "guidelines")?;
    }
    only(
        kind == Kind::Code && repository.priority.is_some(),
        "priority",
        "guidelines and project",
    )?;
    if kind == Kind::Guidelines {
        if !repository.reference.as_deref().is_some_and(valid_ref) {
            bail!(
                "Guidelines reference {name} needs a valid ref: a branch, or the selected version when it is pinned."
            );
        }
        if repository
            .commit
            .as_deref()
            .is_some_and(|c| !valid_commit(c))
        {
            bail!("Guidelines reference {name} needs a full exact commit ID to pin it.");
        }
        if let Some(scope) = &repository.scope {
            if scope.is_empty() || scope.len() > 64 {
                bail!("The scope of {name} must name 1-64 repositories.");
            }
            for (index, target) in scope.iter().enumerate() {
                if target == name || !names.contains(target) || scope[..index].contains(target) {
                    bail!(
                        "The scope of {name} must name distinct other repositories of this definition: {target}"
                    );
                }
            }
        }
        if let Some(instructions) = &repository.instructions {
            if instructions.is_empty() || instructions.len() > 32 {
                bail!("Guidelines reference {name} lists 1-32 instruction files.");
            }
            for file in instructions {
                if !valid_relative_directory(file) {
                    bail!(
                        "Instruction files of {name} must be relative paths of portable, non-hidden names: {file}"
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_repositories(manifest: &Manifest) -> Result<()> {
    let repositories = manifest
        .repositories
        .as_ref()
        .context("Schema version 2 requires a repositories list; use [] when the spec repository contains all code.")?;
    if repositories.len() > 64 {
        bail!("A project supports at most 64 repositories.");
    }
    let spec = manifest.spec_repository.clone().unwrap_or_default();
    let mut names = vec![manifest.spec_name().to_owned()];
    let mut directories = vec![ROLES_DIRECTORY.to_owned()];
    validate_purpose(spec.purpose.as_deref(), "the spec repository")?;
    let declared: Vec<String> = names
        .iter()
        .cloned()
        .chain(repositories.iter().map(|r| r.name.clone()))
        .collect();
    for repository in repositories {
        validate_reference(repository, manifest.schema_version, &declared)?;
        validate_purpose(repository.purpose.as_deref(), &repository.name)?;
        repository::validate(&repository.url)?;
        let directory = repository.directory();
        if !files::portable_name(directory) || directory.starts_with('.') {
            bail!("Repository path must be one portable, non-hidden directory name: {directory}");
        }
        if let Some(skills) = &repository.skills
            && !valid_relative_directory(skills)
        {
            bail!(
                "The skill directory of {} must be a relative path of at most 8 portable, non-hidden directory names separated by /: {skills}",
                repository.name
            );
        }
        names.push(repository.name.clone());
        directories.push(directory.to_ascii_lowercase());
    }
    for (index, name) in names.iter().enumerate() {
        if !crate::skills::valid_name(name) {
            bail!("Repository names use 1-64 lowercase letters, digits or single hyphens: {name}");
        }
        if names[..index].contains(name) {
            bail!("Repository names must be distinct: {name}");
        }
    }
    for (index, directory) in directories.iter().enumerate() {
        if directories[..index].contains(directory) {
            bail!(
                "Repository paths must be distinct ignoring case and must not use the reserved {ROLES_DIRECTORY} directory: {directory}"
            );
        }
    }
    Ok(())
}

pub fn validate(manifest: &Manifest, lock: Option<&Lock>) -> Result<()> {
    for (index, agent) in manifest.agents.iter().enumerate() {
        if manifest.agents[..index].contains(agent) {
            bail!("Agent integrations must not contain duplicates.");
        }
    }
    if lock.is_some_and(|lock| lock.schema_version != 1) {
        bail!("Only guidelines lock schemaVersion 1 is supported.");
    }
    match manifest.schema_version {
        1 => {
            if manifest.repositories.is_some() || manifest.spec_repository.is_some() {
                bail!("repositories and specRepository require schemaVersion 2.");
            }
            let code = manifest
                .code
                .as_ref()
                .context("Schema version 1 requires code.repository.")?;
            if !manifest.profile.as_deref().is_some_and(valid_profile) {
                bail!("Invalid profile.");
            }
            repository::validate(&code.repository)?;
        }
        2 | 3 => {
            if manifest.code.is_some() || manifest.profile.is_some() {
                bail!(
                    "Schema version {} declares repositories instead of code, and profiles per repository instead of a top-level profile.",
                    manifest.schema_version
                );
            }
            validate_repositories(manifest)?;
            if manifest.profiles().iter().any(|p| !valid_profile(p)) {
                bail!("Invalid profile.");
            }
        }
        _ => bail!(
            "Only schemaVersion 1, 2 and {CURRENT_SCHEMA} are supported. A newer definition needs a newer CretSpec."
        ),
    }
    if !files::portable_name(&manifest.name) || !manifest.name.as_bytes()[0].is_ascii_alphabetic() {
        bail!("Invalid or non-portable project name.");
    }
    repository::validate(&manifest.guidelines.repository)?;
    if !valid_ref(&manifest.guidelines.reference) {
        bail!("Invalid guidelines ref.");
    }
    match (manifest.guidelines_mode(lock), lock) {
        (GuidelinesMode::Pinned, None) => bail!(
            "Pinned guidelines require guidelines.lock.json. Restore the lock; use cspec guidelines unlock to switch modes explicitly."
        ),
        (GuidelinesMode::WorkingTree, Some(_)) => bail!(
            "Working-tree guidelines must not have a lock. Restore the original matching manifest and lock, then use cspec guidelines unlock to switch modes."
        ),
        _ => {}
    }
    if let Some(lock) = lock
        && (lock.reference != manifest.guidelines.reference || !valid_commit(&lock.commit))
    {
        bail!("The lock must contain the same ref and a full exact commit ID.");
    }
    Ok(())
}

use crate::{
    files,
    manifest::ROLES_DIRECTORY,
    skills::{self, BUILTIN_NAME},
    workspace::ProjectInfo,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_ROLE: u64 = 128 * 1024;
const MAX_ROLES: usize = 64;

/// A project role: shared project context with a specific focus and responsibilities.
/// Roles are behavioral agreements for agents, not a security boundary.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub name: String,
    pub description: String,
    /// Focus repositories, by repository name, in priority order.
    pub repositories: Vec<String>,
    /// Selected skills; `None` selects every active skill.
    pub skills: Option<Vec<String>>,
    pub instructions: String,
    pub source: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    name: String,
    description: String,
    #[serde(default)]
    repositories: Vec<String>,
    #[serde(default)]
    skills: Option<Vec<String>>,
}

pub fn source_directory(info: &ProjectInfo) -> PathBuf {
    info.spec.join("spec/roles")
}

/// The generated entry folder of a role inside the project root.
pub fn entry_point(info: &ProjectInfo, name: &str) -> PathBuf {
    info.project_root.join(ROLES_DIRECTORY).join(name)
}

/// Names of every repository a role may reference: the spec and each declared repository.
pub fn repository_names(info: &ProjectInfo) -> Vec<&str> {
    std::iter::once(info.spec_repository.name.as_str())
        .chain(info.repositories.iter().map(|r| r.name.as_str()))
        .collect()
}

/// The workspace path of a repository referenced by name.
pub fn repository_path<'a>(info: &'a ProjectInfo, name: &str) -> Option<&'a Path> {
    if info.spec_repository.name == name {
        return Some(&info.spec);
    }
    info.repositories
        .iter()
        .find(|r| r.name == name)
        .map(|r| r.path.as_path())
}

fn parse(info: &ProjectInfo, path: &Path, expected: &str) -> Result<Role> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_ROLE {
        bail!("Expected a regular role file of at most 128 KiB");
    }
    let (yaml, body) = skills::frontmatter(&fs::read(path)?, "A role file")?;
    let header: Header = skills::parse_frontmatter(
        &yaml,
        "role",
        "Use name, description, optional repositories and optional skills.",
    )?;
    if !skills::valid_name(&header.name) || header.name != expected {
        bail!(
            "Role name must match its file name and use 1-64 lowercase letters, digits or single hyphens: {expected}"
        );
    }
    if !skills::description_valid(&header.description) {
        bail!("Role {expected} needs a nonempty description of at most 1024 characters.");
    }
    if body.trim().is_empty() {
        bail!("Role {expected} needs Markdown responsibilities after its frontmatter.");
    }
    let known = repository_names(info);
    for (index, repository) in header.repositories.iter().enumerate() {
        if !known.contains(&repository.as_str()) {
            bail!(
                "Role {expected} refers to an unknown repository: {repository}. Known repositories: {}",
                known.join(", ")
            );
        }
        if header.repositories[..index].contains(repository) {
            bail!("Role {expected} lists repository {repository} more than once.");
        }
    }
    for (index, skill) in header.skills.iter().flatten().enumerate() {
        if !skills::valid_name(skill) {
            bail!("Role {expected} lists an invalid skill name: {skill}");
        }
        if header.skills.as_ref().unwrap()[..index].contains(skill) {
            bail!("Role {expected} lists skill {skill} more than once.");
        }
    }
    Ok(Role {
        name: header.name,
        description: header.description.trim().to_owned(),
        repositories: header.repositories,
        skills: header.skills,
        instructions: body.trim_end().to_owned(),
        source: path.to_owned(),
    })
}

/// Reads every role source from the spec's working tree.
pub fn load(info: &ProjectInfo) -> Result<Vec<Role>> {
    files::directory(&info.spec.join("spec"))?;
    let directory = source_directory(info);
    if !files::exists(&directory)? {
        return Ok(Vec::new());
    }
    files::directory(&directory)?;
    let mut roles = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let file = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("Role file names must be UTF-8"))?;
        if file.starts_with('.') {
            continue;
        }
        let name = file.strip_suffix(".md").with_context(|| {
            format!(
                "Role sources must be <name>.md files: {}",
                entry.path().display()
            )
        })?;
        let role = parse(info, &entry.path(), name)
            .with_context(|| format!("Invalid role at {}", entry.path().display()))?;
        roles.push(role);
        if roles.len() > MAX_ROLES {
            bail!("A project supports at most {MAX_ROLES} roles");
        }
    }
    roles.sort_by(|a, b| a.name.cmp(&b.name));
    if !roles.is_empty() {
        let reserved = [&info.spec, &info.editable_guidelines]
            .into_iter()
            .chain(info.repositories.iter().map(|r| &r.path))
            .any(|path| {
                path.file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(ROLES_DIRECTORY))
            });
        if reserved {
            bail!(
                "Role entry points use the {ROLES_DIRECTORY} directory in the project root, which is already a repository directory."
            );
        }
    }
    Ok(roles)
}

/// Selects the role's skills from the active bundles. The workflow skill is always included.
pub fn selected<'a>(role: &Role, bundles: &'a [skills::Bundle]) -> Result<Vec<&'a skills::Bundle>> {
    let Some(names) = &role.skills else {
        return Ok(bundles.iter().collect());
    };
    for name in names {
        if !bundles.iter().any(|b| &b.skill.name == name) {
            bail!(
                "Role {} selects a skill that is not active: {name}. Create or adopt the skill first, or remove it from the role.",
                role.name
            );
        }
    }
    Ok(bundles
        .iter()
        .filter(|b| b.skill.name == BUILTIN_NAME || names.contains(&b.skill.name))
        .collect())
}

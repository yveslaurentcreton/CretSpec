use crate::{
    files, git,
    manifest::{Agent, ROLES_DIRECTORY},
    roles::{self, Role},
    skills::{self, Resource},
    workspace::{self, ProjectInfo},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const STATE: &str = "agents-state.json";
const PENDING: &str = "agents-pending.json";
const STATE_LIMIT: u64 = 16 * 1024 * 1024;

struct SyncLock(fs::File);

impl Drop for SyncLock {
    fn drop(&mut self) {
        // A concurrently forked process can briefly retain a duplicate descriptor.
        // Unlock explicitly instead of waiting for every duplicate to close.
        let _ = self.0.unlock();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    sha256: String,
    executable: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    schema_version: u32,
    files: BTreeMap<String, Vec<Stamp>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub schema_version: u32,
    pub written: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Generated paths that Git now tracks with other content, for example after a
    /// branch switch. CretSpec stopped owning them without changing them.
    pub released: Vec<PathBuf>,
    pub compatibility_notice: Option<String>,
}

fn stamp(resource: &Resource) -> Stamp {
    Stamp {
        sha256: Sha256::digest(&resource.bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        executable: resource.executable,
    }
}

/// A location that receives generated entry points. Shared anchors are role-neutral;
/// role anchors are generated folders inside the project root.
struct Anchor {
    name: String,
    base: PathBuf,
    role: Option<Role>,
}

fn anchors(info: &ProjectInfo, roles: Vec<Role>) -> Vec<Anchor> {
    let shared = [
        ("root".to_owned(), info.project_root.clone()),
        ("spec".into(), info.spec.clone()),
        ("guidelines".into(), info.editable_guidelines.clone()),
    ]
    .into_iter()
    .chain(
        info.repositories
            .iter()
            .map(|r| (r.anchor.clone(), r.path.clone())),
    )
    .map(|(name, base)| Anchor {
        name,
        base,
        role: None,
    });
    let roles = roles.into_iter().map(|role| Anchor {
        name: format!("role:{}", role.name),
        base: roles::entry_point(info, &role.name),
        role: Some(role),
    });
    shared.chain(roles).collect()
}

struct Target {
    base: PathBuf,
    path: PathBuf,
    git: bool,
    role: bool,
}

/// Resolves an ownership key. Keys of removed repositories and roles still resolve,
/// so their generated files can be removed after verification.
fn target(info: &ProjectInfo, key: &str) -> Result<Target> {
    let (anchor, relative) = key.split_once('/').context("Invalid generated file key")?;
    let (base, git, role) = match anchor {
        "root" => (info.project_root.clone(), false, false),
        "spec" => (info.spec.clone(), true, false),
        "guidelines" => (info.editable_guidelines.clone(), true, false),
        // Schema version 1 places the code repository at the project name.
        "code" => (info.project_root.join(&info.manifest.name), true, false),
        _ => {
            if let Some(directory) = anchor.strip_prefix("repo:")
                && files::portable_name(directory)
                && !directory.starts_with('.')
            {
                (info.project_root.join(directory), true, false)
            } else if let Some(name) = anchor.strip_prefix("role:")
                && skills::valid_name(name)
            {
                (roles::entry_point(info, name), false, true)
            } else {
                bail!("Invalid generated file anchor")
            }
        }
    };
    // Earlier versions generated CLAUDE.md and CLAUDE.local.md. Their keys still resolve,
    // so sync can remove them.
    let instruction = matches!(
        relative,
        "AGENTS.md"
            | "AGENTS.override.md"
            | "CLAUDE.md"
            | "CLAUDE.local.md"
            | ".github/instructions/cspec.instructions.md"
            | ".cursor/rules/cspec.mdc"
    );
    let skill = [".agents/skills/", ".claude/skills/"].iter().any(|prefix| {
        relative.strip_prefix(prefix).is_some_and(|rest| {
            rest.split_once('/').is_some_and(|(name, file)| {
                skills::valid_name(name) && file.split('/').all(files::portable_name)
            })
        })
    });
    if !instruction && !skill {
        bail!("Invalid generated file path: {key}");
    }
    Ok(Target {
        path: base.join(relative),
        base,
        git,
        role,
    })
}

/// Checks that a target's base can be used. Missing role folders are created by sync;
/// a missing repository base means there is nothing to inspect or remove.
fn base_present(info: &ProjectInfo, target: &Target) -> Result<bool> {
    if target.role {
        let roles = info.project_root.join(ROLES_DIRECTORY);
        if files::exists(&roles)? {
            files::directory(&roles)?;
        }
    }
    if files::exists(&target.base)? {
        files::directory(&target.base)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn check_parents(base: &Path, path: &Path) -> Result<()> {
    files::directory(base)?;
    let mut parent = path.parent().context("Generated file has no parent")?;
    while parent != base {
        if !parent.starts_with(base) {
            bail!("Generated path escapes the workspace");
        }
        if files::exists(parent)? {
            files::directory(parent)?;
        }
        parent = parent
            .parent()
            .context("Generated path has no workspace parent")?;
    }
    Ok(())
}

fn read_resource(path: &Path) -> Result<Resource> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > 16 * 1024 * 1024 {
        bail!(
            "Expected a regular file of at most 16 MiB: {}",
            path.display()
        );
    }
    Ok(Resource {
        bytes: fs::read(path)?,
        executable: skills::executable(&metadata),
    })
}

fn read_state(info: &ProjectInfo, name: &str) -> Result<State> {
    let path = workspace::local_directory(&info.spec, false)?.join(name);
    if !files::exists(&path)? {
        return Ok(State {
            schema_version: 1,
            files: BTreeMap::new(),
        });
    }
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() || metadata.len() > STATE_LIMIT {
        bail!("Invalid agent ownership file: {}", path.display());
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)?
        .take(STATE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    let state: State = serde_json::from_slice(&bytes).context(
        "Invalid agent ownership metadata; preserve it and any edited outputs before rebuilding",
    )?;
    if state.schema_version != 1 {
        bail!("Unsupported agent ownership format");
    }
    for (key, stamps) in &state.files {
        target(info, key)?;
        if stamps.is_empty()
            || stamps.len() > 128
            || stamps.iter().any(|s| {
                s.sha256.len() != 64
                    || !s
                        .sha256
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
        {
            bail!("Invalid generated file fingerprint: {key}");
        }
    }
    Ok(state)
}

/// Ownership without generated paths that Git now tracks with other content. A branch
/// switch can replace an excluded generated file with a tracked one, such as AGENTS.md;
/// that file now belongs to the repository. A tracked file that still has generated
/// content was added to Git by mistake and remains a conflict.
fn effective_ownership(info: &ProjectInfo) -> Result<(State, Vec<PathBuf>)> {
    let mut state = ownership(info)?;
    let tracked = tracked_paths(info, state.files.keys())?;
    let allowed = owned_paths(info, &state)?;
    let mut released = BTreeSet::new();
    let mut keys = Vec::new();
    for key in state.files.keys() {
        let target = target(info, key)?;
        if !tracked.contains(&target.path) || !base_present(info, &target)? {
            continue;
        }
        check_parents(&target.base, &target.path)?;
        if files::exists(&target.path)?
            && !allowed[&target.path].contains(&stamp(&read_resource(&target.path)?))
        {
            keys.push(key.clone());
            released.insert(target.path);
        }
    }
    for key in keys {
        state.files.remove(&key);
    }
    Ok((state, released.into_iter().collect()))
}

fn ownership(info: &ProjectInfo) -> Result<State> {
    let mut state = read_state(info, STATE)?;
    for (key, stamps) in read_state(info, PENDING)?.files {
        for stamp in stamps {
            let allowed = state.files.entry(key.clone()).or_default();
            if !allowed.contains(&stamp) {
                allowed.push(stamp);
            }
        }
    }
    Ok(state)
}

/// A path relative to `base`, where both lie inside the project root.
fn relative(info: &ProjectInfo, base: &Path, path: &Path) -> Result<String> {
    let depth = base.strip_prefix(&info.project_root)?.components().count();
    let relative = path
        .strip_prefix(&info.project_root)?
        .to_str()
        .context("Workspace paths must be UTF-8")?
        .replace('\\', "/");
    Ok(format!("{}{relative}", "../".repeat(depth)))
}

pub fn instruction_sources(info: &ProjectInfo) -> Vec<PathBuf> {
    [
        info.active_guidelines.join("README.md"),
        info.active_guidelines.join("guidelines/principles.md"),
        info.active_guidelines.join("guidelines/specification.md"),
    ]
    .into_iter()
    .chain(info.manifest.profiles().into_iter().map(|profile| {
        info.active_guidelines
            .join(format!("profiles/{profile}.md"))
    }))
    .chain([info.spec.join("spec/project-rules.md")])
    .filter(|path| path.is_file())
    .collect()
}

fn described(purpose: Option<&str>, profile: Option<&str>) -> String {
    let mut text = match purpose {
        Some(purpose) => format!(": {purpose}"),
        None => ": purpose not described".into(),
    };
    if let Some(profile) = profile {
        text.push_str(&format!(" (profile `{profile}`)"));
    }
    text
}

fn repository_map(info: &ProjectInfo, base: &Path) -> Result<String> {
    let mut lines = Vec::new();
    if info.manifest.code.is_some() {
        lines.push(format!(
            "- Project definition and specification: `{}`",
            relative(info, base, &info.spec)?
        ));
        for repository in &info.repositories {
            lines.push(format!(
                "- Product code: `{}`",
                relative(info, base, &repository.path)?
            ));
        }
    } else {
        let spec = &info.spec_repository;
        lines.push(format!(
            "- Project definition and specification, repository `{}`: `{}`{}",
            spec.name,
            relative(info, base, &info.spec)?,
            described(spec.purpose.as_deref(), spec.profile.as_deref())
        ));
        for repository in &info.repositories {
            lines.push(format!(
                "- Repository `{}`: `{}`{}",
                repository.name,
                relative(info, base, &repository.path)?,
                described(repository.purpose.as_deref(), repository.profile.as_deref())
            ));
        }
    }
    lines.push(format!(
        "- Editable shared AI guidelines: `{}`",
        relative(info, base, &info.editable_guidelines)?
    ));
    Ok(lines.join("\n"))
}

fn bootstrap(
    info: &ProjectInfo,
    base: &Path,
    roles: &[Role],
    role: Option<&Role>,
) -> Result<String> {
    let sources = instruction_sources(info)
        .iter()
        .map(|p| Ok(format!("- `{}`", relative(info, base, p)?)))
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    let (selection, shared_policy) = match &info.lock {
        Some(lock) => (
            format!(
                "Pinned guidelines: `{}` (commit `{}`)",
                lock.reference, lock.commit
            ),
            "Shared drafts require explicit version adoption before they become active.",
        ),
        None => (
            format!(
                "Guidelines mode: working tree (configured branch `{}`)",
                info.manifest.guidelines.reference
            ),
            "Local guideline edits are active immediately. Run `cspec sync` after changing skills to refresh their generated copies.",
        ),
    };
    let location = if role.is_some() {
        "role folder"
    } else {
        "repository/workspace root"
    };
    let roles_text = if roles.is_empty() || role.is_some() {
        String::new()
    } else {
        format!(
            "\n\nThis context is role-neutral. For a role-specific focus, start the agent in that role's folder or run `cspec context --role <name>`. Roles: {}.",
            roles
                .iter()
                .map(|r| Ok(format!(
                    "`{}` (`{}`)",
                    r.name,
                    relative(info, base, &roles::entry_point(info, &r.name))?
                )))
                .collect::<Result<Vec<_>>>()?
                .join(", ")
        )
    };
    let repository_skills = info
        .repositories
        .iter()
        .filter_map(|r| {
            let path = r.skills.as_ref()?;
            Some(relative(info, base, path).map(|path| format!("`{}` (`{path}`)", r.name)))
        })
        .collect::<Result<Vec<_>>>()?;
    let repository_policy = if repository_skills.is_empty() {
        String::new()
    } else {
        format!(
            " Skills that belong to one repository's own code or method belong in that repository's skill directory and are committed with it: {}.",
            repository_skills.join(", ")
        )
    };
    let mut text = format!(
        "# CretSpec workspace\n\nGenerated by CretSpec {}. Edit the source repositories; run `cspec sync` to refresh this file. Paths below are relative to this {location}.\n\n{}\n- {selection}\n\nBefore project work, read the relevant active guidance and project rules. Project rules take precedence over shared guidance:\n\n{sources}\n\nUse `cspec context --json` to inspect sources and `cspec skill list --json` to find skills. Follow the active specification method when changing requirements, stories, decisions or verification evidence. Use the `cspec-workspace` skill for workflow and CLI help.\n\nWhen asked to remember an approach or create a skill, infer whether it is project-specific or reusable. Ask one scope question only when intent is unclear. Project skills belong in the spec's `spec/skills/`; shared skills belong in the editable guidelines repository's `skills/`.{repository_policy} {shared_policy} Use ordinary Git fetch/pull/commit/push in the guidelines repository to exchange changes; CretSpec does not do this automatically. Agreements belong in guidelines or project rules rather than automatically becoming skills.\n\nKeep internal context out of product commits. Generated instructions and skill copies are local and replaceable. Do not edit snapshots or generated outputs. Use the source paths returned by the CLI. These instructions do not change the host's permissions; it may need access to sibling repositories.{roles_text}\n",
        env!("CARGO_PKG_VERSION"),
        repository_map(info, base)?,
    );
    if let Some(role) = role {
        text.push('\n');
        text.push_str(&role_section(info, base, role)?);
    }
    Ok(text)
}

fn role_section(info: &ProjectInfo, base: &Path, role: &Role) -> Result<String> {
    let mut focus = Vec::new();
    for name in &role.repositories {
        let path = roles::repository_path(info, name).context("Unknown role repository")?;
        focus.push(format!("- `{name}`: `{}`", relative(info, base, path)?));
    }
    let focus = if focus.is_empty() {
        String::new()
    } else {
        format!(
            "Focus repositories, in priority order:\n\n{}\n\n",
            focus.join("\n")
        )
    };
    Ok(format!(
        "## Role: {}\n\nGenerated from `{}`. {}\n\n{focus}Role instructions are working agreements, not a security boundary: agents running as the same operating-system user share file and credential access. A host that only allows writes inside this folder needs the focus repositories as additional directories.\n\n{}\n",
        role.name,
        relative(info, base, &role.source)?,
        role.description,
        role.instructions
    ))
}

fn text(value: String) -> Resource {
    Resource {
        bytes: value.into_bytes(),
        executable: false,
    }
}

fn unmanaged(anchor: &Anchor, name: &str, owned: &BTreeMap<PathBuf, Vec<Stamp>>) -> Result<bool> {
    let path = anchor.base.join(name);
    Ok(files::exists(&path)? && !owned.contains_key(&path))
}

/// Allowed fingerprints per generated path. Several keys can name one path, for example
/// after a schema version 1 code repository is declared by name in schema version 2.
fn owned_paths(info: &ProjectInfo, state: &State) -> Result<BTreeMap<PathBuf, Vec<Stamp>>> {
    let mut result: BTreeMap<PathBuf, Vec<Stamp>> = BTreeMap::new();
    for (key, stamps) in &state.files {
        let allowed = result.entry(target(info, key)?.path).or_default();
        for stamp in stamps {
            if !allowed.contains(stamp) {
                allowed.push(stamp.clone());
            }
        }
    }
    Ok(result)
}

fn desired(info: &ProjectInfo, owned: &State) -> Result<BTreeMap<String, Resource>> {
    let mut result = BTreeMap::new();
    if info.manifest.agents.is_empty() {
        return Ok(result);
    }
    let bundles = skills::active(info)?;
    let roles = roles::load(info)?;
    let owned = owned_paths(info, owned)?;
    let has = |agent| info.manifest.agents.contains(&agent);
    let mut directories = Vec::new();
    if has(Agent::Codex) || (!has(Agent::Claude) && (has(Agent::Copilot) || has(Agent::Cursor))) {
        directories.push(".agents/skills");
    }
    if has(Agent::Claude) {
        directories.push(".claude/skills");
    }
    for anchor in anchors(info, roles.clone()) {
        let key = |name: &str| format!("{}/{name}", anchor.name);
        let guidance = bootstrap(info, &anchor.base, &roles, anchor.role.as_ref())?;
        let selected = match &anchor.role {
            Some(role) => roles::selected(role, &bundles)?,
            None => bundles.iter().collect(),
        };
        if anchor.role.is_some() {
            result.insert(key("AGENTS.md"), text(guidance.clone()));
        } else {
            let agents_name = if unmanaged(&anchor, "AGENTS.md", &owned)? {
                "AGENTS.override.md"
            } else {
                "AGENTS.md"
            };
            if has(Agent::Codex) && unmanaged(&anchor, "AGENTS.override.md", &owned)? {
                bail!(
                    "Existing AGENTS.override.md would hide generated instructions: {}. Preserve it and reconcile the entry point before cspec sync.",
                    anchor.base.display()
                );
            }
            let instructions = if agents_name == "AGENTS.override.md" {
                let existing = read_resource(&anchor.base.join("AGENTS.md"))?;
                if existing.bytes.len() > 24 * 1024 {
                    bail!(
                        "Existing AGENTS.md is too large to combine with workspace context: {}",
                        anchor.base.display()
                    );
                }
                format!(
                    "{}\n\n{guidance}",
                    std::str::from_utf8(&existing.bytes)
                        .context("Existing AGENTS.md must be UTF-8")?
                )
            } else {
                guidance.clone()
            };
            result.insert(key(agents_name), text(instructions));
        }
        if has(Agent::Copilot) {
            result.insert(
                key(".github/instructions/cspec.instructions.md"),
                text(format!("---\napplyTo: '**'\n---\n\n{guidance}")),
            );
        }
        if has(Agent::Cursor) {
            result.insert(key(".cursor/rules/cspec.mdc"), text(format!("---\ndescription: CretSpec workspace sources and workflow\nalwaysApply: true\n---\n\n{guidance}")));
        }
        for directory in &directories {
            for bundle in &selected {
                for (name, resource) in &bundle.files {
                    result.insert(
                        key(&format!("{directory}/{}/{name}", bundle.skill.name)),
                        resource.clone(),
                    );
                }
            }
        }
    }
    Ok(result)
}

const TRACKED_NAMES: [&str; 8] = [
    "AGENTS.md",
    "AGENTS.override.md",
    "CLAUDE.md",
    "CLAUDE.local.md",
    ".agents/skills",
    ".claude/skills",
    ".github/instructions/cspec.instructions.md",
    ".cursor/rules/cspec.mdc",
];

fn tracked_paths<'a>(
    info: &ProjectInfo,
    keys: impl Iterator<Item = &'a String>,
) -> Result<BTreeSet<PathBuf>> {
    let mut bases = BTreeSet::new();
    for key in keys {
        let target = target(info, key)?;
        if target.git && files::exists(&target.base)? {
            bases.insert(target.base);
        }
    }
    let mut result = BTreeSet::new();
    for base in bases {
        let output = git::run(
            &base,
            ["ls-files", "-z", "--"].into_iter().chain(TRACKED_NAMES),
        )?;
        for path in output.split('\0').filter(|s| !s.is_empty()) {
            result.insert(base.join(path));
        }
    }
    Ok(result)
}

fn preflight(info: &ProjectInfo, owned: &State, next: &BTreeMap<String, Resource>) -> Result<()> {
    let keys = owned
        .files
        .keys()
        .chain(next.keys())
        .collect::<BTreeSet<_>>();
    let tracked = tracked_paths(info, keys.iter().copied())?;
    let allowed = owned_paths(info, owned)?;
    for key in keys {
        let target = target(info, key)?;
        if !base_present(info, &target)? {
            if next.contains_key(key) && !target.role {
                bail!("Missing workspace directory: {}", target.base.display());
            }
            continue;
        }
        check_parents(&target.base, &target.path)?;
        if tracked.contains(&target.path) {
            bail!(
                "Generated path is tracked by Git: {}. Untrack it before cspec sync; nothing was overwritten.",
                target.path.display()
            );
        }
        if files::exists(&target.path)? {
            let current = stamp(&read_resource(&target.path)?);
            if !allowed
                .get(&target.path)
                .is_some_and(|allowed| allowed.contains(&current))
            {
                bail!(
                    "Unmanaged or locally edited generated file: {}. Preserve your changes in the source, then remove only this generated file and run cspec sync. Nothing was overwritten.",
                    target.path.display()
                );
            }
        }
    }
    Ok(())
}

fn excludes(info: &ProjectInfo, keys: impl Iterator<Item = String>, write: bool) -> Result<()> {
    let mut patterns: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut paths: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for key in keys {
        let target = target(info, &key)?;
        if !target.git {
            continue;
        }
        let (_, relative) = key.split_once('/').context("Invalid generated key")?;
        paths
            .entry(target.base.clone())
            .or_default()
            .push(relative.into());
        let pattern = if let Some((parent, rest)) = relative.split_once("/skills/") {
            let name = rest.split('/').next().unwrap_or_default();
            format!("/{parent}/skills/{name}/")
        } else {
            format!("/{relative}")
        };
        patterns.entry(target.base).or_default().insert(pattern);
    }
    for (base, patterns) in patterns {
        let path = files::absolute(base.join(git::run(
            &base,
            ["rev-parse", "--git-path", "info/exclude"],
        )?))?;
        let previous = if files::exists(&path)? {
            String::from_utf8(read_resource(&path)?.bytes)?
        } else {
            String::new()
        };
        let missing: Vec<_> = patterns
            .into_iter()
            .filter(|p| !previous.lines().any(|l| l == p))
            .collect();
        if !missing.is_empty() {
            if !write {
                bail!(
                    "Local Git exclusions are missing in {}. Run cspec sync.",
                    base.display()
                );
            }
            fs::create_dir_all(path.parent().context("Exclude file has no parent")?)?;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            write!(
                file,
                "\n# CretSpec local agent integration\n{}\n",
                missing.join("\n")
            )?;
            file.sync_all()?;
        }
        let input = format!("{}\0", paths[&base].join("\0"));
        let ignored = git::run_input(
            &base,
            &[
                "check-ignore",
                "--stdin",
                "-z",
                "--verbose",
                "--non-matching",
                "--no-index",
            ],
            input.as_bytes(),
        )?;
        let fields: Vec<_> = ignored.trim_end_matches('\0').split('\0').collect();
        if fields.len() != paths[&base].len() * 4 {
            bail!(
                "Git did not report every generated path in {}",
                base.display()
            );
        }
        for record in fields.chunks_exact(4) {
            if record[2].is_empty() || record[2].starts_with('!') {
                bail!(
                    "Git ignore rules expose generated file {}. Reconcile the conflicting ignore rule before cspec sync; no generated outputs were written.",
                    base.join(record[3]).display()
                );
            }
        }
    }
    Ok(())
}

fn write_resource(path: &Path, resource: &Resource) -> Result<()> {
    let parent = path.parent().context("Generated file has no parent")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&resource.bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(if resource.executable {
                0o755
            } else {
                0o644
            }))?;
    }
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

/// Creates a role folder below the project root without following links.
fn prepare_role_base(info: &ProjectInfo, base: &Path) -> Result<()> {
    let roles = info.project_root.join(ROLES_DIRECTORY);
    for directory in [&roles, &base.to_path_buf()] {
        if files::exists(directory)? {
            files::directory(directory)?;
        } else {
            fs::create_dir(directory)?;
        }
    }
    Ok(())
}

/// Removes directories of a removed role output once they are empty.
fn remove_empty_role_directories(info: &ProjectInfo, path: &Path) {
    let roles = info.project_root.join(ROLES_DIRECTORY);
    let mut current = path.parent();
    while let Some(directory) = current {
        if !directory.starts_with(&roles) || fs::remove_dir(directory).is_err() {
            break;
        }
        current = directory.parent();
    }
}

pub fn sync(info: &ProjectInfo) -> Result<SyncResult> {
    let local = workspace::local_directory(&info.spec, false)?;
    let lock_path = local.join("agents.lock");
    if files::exists(&lock_path)? && !fs::symlink_metadata(&lock_path)?.is_file() {
        bail!("Agent lock must be a regular file");
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.try_lock()
        .context("Another agent synchronization is running. Retry after it finishes")?;
    let _lock = SyncLock(lock);
    let (owned, released) = effective_ownership(info)?;
    let next = desired(info, &owned)?;
    preflight(info, &owned, &next)?;
    // Journal both old and planned fingerprints before any generated output changes.
    let mut pending = owned.clone();
    let mut committed = State {
        schema_version: 1,
        files: BTreeMap::new(),
    };
    for (key, resource) in &next {
        let stamp = stamp(resource);
        committed.files.insert(key.clone(), vec![stamp.clone()]);
        let allowed = pending.files.entry(key.clone()).or_default();
        if !allowed.contains(&stamp) {
            allowed.push(stamp);
        }
    }
    if serde_json::to_vec(&pending)?.len() as u64 > STATE_LIMIT {
        bail!("Agent ownership metadata exceeds 16 MiB");
    }
    files::write_json(&local.join(PENDING), &pending)?;
    excludes(info, next.keys().cloned(), true)?;
    let mut result = SyncResult {
        schema_version: 1,
        written: 0,
        removed: 0,
        unchanged: 0,
        released,
        compatibility_notice: compatibility_notice(info),
    };
    let allowed = owned_paths(info, &pending)?;
    let mut planned = BTreeSet::new();
    for (key, resource) in &next {
        let target = target(info, key)?;
        if target.role {
            prepare_role_base(info, &target.base)?;
        }
        check_parents(&target.base, &target.path)?;
        planned.insert(target.path.clone());
        if files::exists(&target.path)? {
            let current = stamp(&read_resource(&target.path)?);
            if !allowed[&target.path].contains(&current) {
                bail!(
                    "Generated file changed during sync: {}. Preserve it before retrying.",
                    target.path.display()
                );
            }
            if current == stamp(resource) {
                result.unchanged += 1;
                continue;
            }
        }
        write_resource(&target.path, resource)?;
        result.written += 1;
    }
    let previous = owned_paths(info, &owned)?;
    for key in owned.files.keys().filter(|key| !next.contains_key(*key)) {
        let target = target(info, key)?;
        if planned.contains(&target.path) || !base_present(info, &target)? {
            continue;
        }
        check_parents(&target.base, &target.path)?;
        if files::exists(&target.path)? {
            if !previous[&target.path].contains(&stamp(&read_resource(&target.path)?)) {
                bail!(
                    "Generated file changed during sync: {}",
                    target.path.display()
                );
            }
            fs::remove_file(&target.path)?;
            result.removed += 1;
            if target.role {
                remove_empty_role_directories(info, &target.path);
            }
        }
    }
    files::write_json(&local.join(STATE), &committed)?;
    fs::remove_file(local.join(PENDING))?;
    Ok(result)
}

pub fn compatibility_notice(info: &ProjectInfo) -> Option<String> {
    let agents = &info.manifest.agents;
    (agents.contains(&Agent::Codex) && agents.contains(&Agent::Claude)
        && (agents.contains(&Agent::Copilot) || agents.contains(&Agent::Cursor)))
        .then(|| "Both .agents/skills and .claude/skills are prepared. Compatibility readers may show duplicate skills; select integrations in project.json if needed.".into())
}

/// Explains why a Codex override copy is stale when the tracked AGENTS.md changed.
fn stale_cause(path: &Path) -> String {
    if path
        .file_name()
        .is_none_or(|name| name != "AGENTS.override.md")
    {
        return String::new();
    }
    let tracked = path.with_file_name("AGENTS.md");
    match (fs::read(&tracked), fs::read(path)) {
        (Ok(tracked), Ok(copy)) if !copy.starts_with(&tracked) => format!(
            " The repository's tracked {} changed since the last sync; Codex reads the outdated copy until then.",
            tracked_name(path)
        ),
        _ => String::new(),
    }
}

fn tracked_name(path: &Path) -> String {
    path.with_file_name("AGENTS.md").display().to_string()
}

pub fn check(info: &ProjectInfo) -> Result<String> {
    if files::exists(&workspace::local_directory(&info.spec, false)?.join(PENDING))? {
        bail!("Agent synchronization was interrupted. Run cspec sync to finish it.");
    }
    let (owned, released) = effective_ownership(info)?;
    if let Some(path) = released.first() {
        bail!(
            "Git now tracks {} with other content, for example after a branch switch. Run cspec sync to stop generating it; the tracked file is kept.",
            path.display()
        );
    }
    let next = desired(info, &owned)?;
    preflight(info, &owned, &next)?;
    for (key, resource) in &next {
        let path = target(info, key)?.path;
        if !files::exists(&path)? || stamp(&read_resource(&path)?) != stamp(resource) {
            bail!(
                "Agent integration is missing or stale: {}.{} Run cspec sync.",
                path.display(),
                stale_cause(&path)
            );
        }
    }
    if owned.files.keys().ne(next.keys()) {
        bail!("Agent integrations are stale or missing. Run cspec sync.");
    }
    excludes(info, next.keys().cloned(), false)?;
    Ok(format!(
        "{} generated files match the selected integrations and active skills",
        next.len()
    ))
}

/// Files that make Claude Code read CLAUDE.md instead of AGENTS.md.
const CLAUDE_ENTRY_POINTS: [&str; 3] = ["CLAUDE.md", "CLAUDE.local.md", ".claude/CLAUDE.md"];

/// Reports CLAUDE.md files that stop Claude Code from reading the generated AGENTS.md.
/// Claude Code reads AGENTS.md only when no such file exists in its working directory
/// or above it. Files that CretSpec still owns are removed by the next sync.
pub fn claude_warnings(info: &ProjectInfo) -> Vec<String> {
    if !info.manifest.agents.contains(&Agent::Claude) {
        return Vec::new();
    }
    let Ok(owned) = effective_ownership(info).and_then(|(state, _)| owned_paths(info, &state))
    else {
        return Vec::new();
    };
    let roles = roles::load(info).unwrap_or_default();
    let mut bases = info
        .project_root
        .ancestors()
        .skip(1)
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    bases.extend(anchors(info, roles).into_iter().map(|anchor| anchor.base));
    let mut warnings: Vec<(PathBuf, PathBuf)> = Vec::new();
    for base in bases {
        for name in CLAUDE_ENTRY_POINTS {
            let path = base.join(name);
            if path.is_file()
                && !owned.contains_key(&path)
                && !warnings.iter().any(|(_, known)| *known == path)
            {
                warnings.push((base.clone(), path));
            }
        }
    }
    warnings
        .into_iter()
        .map(|(base, path)| {
            format!(
                "{} makes Claude Code read CLAUDE.md files instead of AGENTS.md in {} and below",
                path.display(),
                base.display()
            )
        })
        .collect()
}

/// Codex reads at most 32 KiB of project instructions by default (`project_doc_max_bytes`).
const CODEX_INSTRUCTION_LIMIT: usize = 32 * 1024;

/// Reports generated Codex entry points that exceed the default instruction size.
pub fn size_warnings(info: &ProjectInfo) -> Vec<String> {
    let Ok(next) = effective_ownership(info).and_then(|(owned, _)| desired(info, &owned)) else {
        return Vec::new();
    };
    next.iter()
        .filter(|(key, resource)| {
            (key.ends_with("/AGENTS.md") || key.ends_with("/AGENTS.override.md"))
                && resource.bytes.len() > CODEX_INSTRUCTION_LIMIT
        })
        .filter_map(|(key, resource)| {
            let path = target(info, key).ok()?.path;
            Some(format!(
                "{} is {} KiB; Codex reads at most 32 KiB of project instructions by default",
                path.display(),
                resource.bytes.len().div_ceil(1024)
            ))
        })
        .collect()
}

fn role_details(info: &ProjectInfo, role: &Role) -> Result<Value> {
    let bundles = skills::active(info)?;
    let skills: Vec<_> = roles::selected(role, &bundles)?
        .into_iter()
        .map(|b| b.skill.name.clone())
        .collect();
    let mut repositories = Vec::new();
    let mut directories = Vec::new();
    for name in &role.repositories {
        let path = roles::repository_path(info, name).context("Unknown role repository")?;
        let purpose = if info.spec_repository.name == *name {
            info.spec_repository.purpose.clone()
        } else {
            info.repositories
                .iter()
                .find(|r| &r.name == name)
                .and_then(|r| r.purpose.clone())
        };
        repositories.push(json!({"name":name,"path":path,"purpose":purpose}));
        directories.push(path.to_owned());
    }
    Ok(json!({
        "name":role.name, "description":role.description, "source":role.source,
        "entryPoint":roles::entry_point(info, &role.name), "repositories":repositories,
        "additionalDirectories":directories, "skills":skills, "instructions":role.instructions,
    }))
}

pub fn context(info: &ProjectInfo, role: Option<&str>) -> Result<Value> {
    let status = match check(info) {
        Ok(detail) => json!({"ready":true,"detail":detail}),
        Err(error) => json!({"ready":false,"detail":format!("{error:#}")}),
    };
    let (roles, role_errors) = match roles::load(info) {
        Ok(roles) => (roles, Vec::new()),
        Err(error) => (Vec::new(), vec![format!("{error:#}")]),
    };
    let mut value = json!({
        "schemaVersion":1, "cliVersion":env!("CARGO_PKG_VERSION"), "project":info,
        "instructionSources":instruction_sources(info),
        "skillSources":{"project":info.spec.join("spec/skills"), "shared":info.editable_guidelines.join("skills"), "activeShared":info.active_guidelines.join("skills"),
            "repositories":info.repositories.iter().filter_map(|r| Some((r.name.clone(), json!(r.skills.as_ref()?)))).collect::<serde_json::Map<_,_>>()},
        "skills":skills::inventory(info)?, "integration":status, "compatibilityNotice":compatibility_notice(info),
        "roleSource":roles::source_directory(info),
        "roles":roles.iter().map(|r| json!({"name":r.name,"description":r.description,"entryPoint":roles::entry_point(info, &r.name)})).collect::<Vec<_>>(),
        "roleErrors":role_errors,
        "repositoryStates":workspace::repository_states(info)?,
        "undeclaredRepositories":workspace::undeclared_repositories(info)?,
    });
    if let Some(name) = role {
        let role = roles::load(info)?
            .into_iter()
            .find(|r| r.name == name)
            .with_context(|| {
                format!(
                    "Unknown role: {name}. Roles are defined in {}",
                    roles::source_directory(info).display()
                )
            })?;
        value["role"] = role_details(info, &role)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_lock_releases_with_a_duplicate_handle_still_open() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let lock = file.reopen().unwrap();
        lock.try_lock().unwrap();
        let duplicate = lock.try_clone().unwrap();
        let contender = file.reopen().unwrap();
        let operation = || -> Result<()> {
            let _lock = SyncLock(lock);
            assert!(matches!(
                contender.try_lock(),
                Err(fs::TryLockError::WouldBlock)
            ));
            bail!("Simulated synchronization failure")
        };
        assert!(operation().is_err());
        contender.try_lock().unwrap();
        contender.unlock().unwrap();
        drop(duplicate);
    }
}

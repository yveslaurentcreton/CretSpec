use crate::{
    files, git,
    manifest::Agent,
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

fn anchors(info: &ProjectInfo) -> [(&str, &Path); 4] {
    [
        ("root", &info.project_root),
        ("code", &info.code),
        ("spec", &info.spec),
        ("guidelines", &info.editable_guidelines),
    ]
}

fn target<'a>(info: &'a ProjectInfo, key: &str) -> Result<(&'a Path, String, PathBuf)> {
    let (anchor, relative) = key.split_once('/').context("Invalid generated file key")?;
    let base = anchors(info)
        .into_iter()
        .find(|(name, _)| *name == anchor)
        .map(|(_, path)| path)
        .context("Invalid generated file anchor")?;
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
    Ok((base, relative.into(), base.join(relative)))
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

fn relative(info: &ProjectInfo, anchor: &str, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(&info.project_root)?
        .to_str()
        .context("Workspace paths must be UTF-8")?
        .replace('\\', "/");
    Ok(format!(
        "{}{relative}",
        if anchor == "root" { "" } else { "../" }
    ))
}

pub fn instruction_sources(info: &ProjectInfo) -> Vec<PathBuf> {
    [
        info.active_guidelines.join("README.md"),
        info.active_guidelines.join("guidelines/principles.md"),
        info.active_guidelines.join("guidelines/specification.md"),
        info.active_guidelines
            .join(format!("profiles/{}.md", info.manifest.profile)),
        info.spec.join("spec/project-rules.md"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .collect()
}

fn bootstrap(info: &ProjectInfo, anchor: &str) -> Result<String> {
    let sources = instruction_sources(info)
        .iter()
        .map(|p| Ok(format!("- `{}`", relative(info, anchor, p)?)))
        .collect::<Result<Vec<_>>>()?
        .join("\n");
    Ok(format!(
        "# CretSpec workspace\n\nGenerated by CretSpec {}. Edit the source repositories; run `cspec sync` to refresh this file. Paths below are relative to this repository/workspace root.\n\n- Project definition and specification: `{}`\n- Product code: `{}`\n- Editable shared AI guidelines: `{}`\n- Adopted guidelines: `{}` (commit `{}`)\n\nBefore project work, read the relevant adopted guidance and project rules:\n\n{sources}\n\nUse `cspec context --json` to inspect sources and `cspec skill list --json` to find skills. Follow the adopted specification method when changing requirements, stories, decisions or verification evidence. Use the `cspec-workspace` skill for workflow and CLI help.\n\nWhen asked to remember an approach or create a skill, infer whether it is project-specific or reusable. Ask one scope question only when intent is unclear. Project skills belong in the spec's `spec/skills/`; shared skills belong in the editable guidelines repository's `skills/`. Shared drafts require explicit version adoption before they become active. Agreements belong in guidelines or project rules rather than automatically becoming skills.\n\nKeep internal context out of product commits. Generated instructions and skill copies are local and replaceable. Do not edit snapshots or generated outputs. Use the source paths returned by the CLI. These instructions do not change the host's permissions; it may need access to sibling repositories.\n",
        env!("CARGO_PKG_VERSION"),
        relative(info, anchor, &info.spec)?,
        relative(info, anchor, &info.code)?,
        relative(info, anchor, &info.editable_guidelines)?,
        info.lock.reference,
        info.lock.commit
    ))
}

fn text(value: String) -> Resource {
    Resource {
        bytes: value.into_bytes(),
        executable: false,
    }
}

fn unmanaged(base: &Path, anchor: &str, name: &str, owned: &State) -> Result<bool> {
    Ok(files::exists(&base.join(name))? && !owned.files.contains_key(&format!("{anchor}/{name}")))
}

fn desired(info: &ProjectInfo, owned: &State) -> Result<BTreeMap<String, Resource>> {
    let mut result = BTreeMap::new();
    if info.manifest.agents.is_empty() {
        return Ok(result);
    }
    let bundles = skills::active(info)?;
    let has = |agent| info.manifest.agents.contains(&agent);
    let mut directories = Vec::new();
    if has(Agent::Codex) || (!has(Agent::Claude) && (has(Agent::Copilot) || has(Agent::Cursor))) {
        directories.push(".agents/skills");
    }
    if has(Agent::Claude) {
        directories.push(".claude/skills");
    }
    for (anchor, base) in anchors(info) {
        let guidance = bootstrap(info, anchor)?;
        let agents_name = if unmanaged(base, anchor, "AGENTS.md", owned)? {
            "AGENTS.override.md"
        } else {
            "AGENTS.md"
        };
        if has(Agent::Codex) && unmanaged(base, anchor, "AGENTS.override.md", owned)? {
            bail!(
                "Existing AGENTS.override.md would hide generated instructions: {}. Preserve it and reconcile the entry point before cspec sync.",
                base.display()
            );
        }
        let instructions = if agents_name == "AGENTS.override.md" {
            let existing = read_resource(&base.join("AGENTS.md"))?;
            if existing.bytes.len() > 24 * 1024 {
                bail!(
                    "Existing AGENTS.md is too large to combine with workspace context: {}",
                    base.display()
                );
            }
            format!(
                "{}\n\n{guidance}",
                std::str::from_utf8(&existing.bytes).context("Existing AGENTS.md must be UTF-8")?
            )
        } else {
            guidance.clone()
        };
        result.insert(format!("{anchor}/{agents_name}"), text(instructions));
        if has(Agent::Claude) {
            let name = if unmanaged(base, anchor, "CLAUDE.md", owned)? {
                "CLAUDE.local.md"
            } else {
                "CLAUDE.md"
            };
            result.insert(
                format!("{anchor}/{name}"),
                text(format!("@{agents_name}\n")),
            );
        }
        if has(Agent::Copilot) {
            result.insert(
                format!("{anchor}/.github/instructions/cspec.instructions.md"),
                text(format!("---\napplyTo: '**'\n---\n\n{guidance}")),
            );
        }
        if has(Agent::Cursor) {
            result.insert(format!("{anchor}/.cursor/rules/cspec.mdc"), text(format!("---\ndescription: CretSpec workspace sources and workflow\nalwaysApply: true\n---\n\n{guidance}")));
        }
        for directory in &directories {
            for bundle in &bundles {
                for (name, resource) in &bundle.files {
                    result.insert(
                        format!("{anchor}/{directory}/{}/{name}", bundle.skill.name),
                        resource.clone(),
                    );
                }
            }
        }
    }
    Ok(result)
}

fn tracked_paths(info: &ProjectInfo) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for (anchor, base) in anchors(info)
        .into_iter()
        .filter(|(name, _)| *name != "root")
    {
        let output = git::run(
            base,
            [
                "ls-files",
                "-z",
                "--",
                "AGENTS.md",
                "AGENTS.override.md",
                "CLAUDE.md",
                "CLAUDE.local.md",
                ".agents/skills",
                ".claude/skills",
                ".github/instructions/cspec.instructions.md",
                ".cursor/rules/cspec.mdc",
            ],
        )?;
        for path in output.split('\0').filter(|s| !s.is_empty()) {
            result.insert(format!("{anchor}/{path}"));
        }
    }
    Ok(result)
}

fn preflight(info: &ProjectInfo, owned: &State, next: &BTreeMap<String, Resource>) -> Result<()> {
    let tracked = tracked_paths(info)?;
    for key in owned
        .files
        .keys()
        .chain(next.keys())
        .collect::<BTreeSet<_>>()
    {
        let (base, _, path) = target(info, key)?;
        check_parents(base, &path)?;
        if tracked.contains(key) {
            bail!(
                "Generated path is tracked by Git: {}. Untrack it before cspec sync; nothing was overwritten.",
                path.display()
            );
        }
        if files::exists(&path)? {
            let current = stamp(&read_resource(&path)?);
            if !owned
                .files
                .get(key)
                .is_some_and(|allowed| allowed.contains(&current))
            {
                bail!(
                    "Unmanaged or locally edited generated file: {}. Preserve your changes in the source, then remove only this generated file and run cspec sync. Nothing was overwritten.",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn excludes(info: &ProjectInfo, keys: impl Iterator<Item = String>, write: bool) -> Result<()> {
    let mut patterns: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut paths: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in keys {
        let (anchor, relative) = key.split_once('/').context("Invalid generated key")?;
        if anchor == "root" {
            continue;
        }
        paths
            .entry(anchor.into())
            .or_default()
            .push(relative.into());
        let pattern = if let Some((parent, _)) = relative.split_once("/skills/") {
            let name = relative
                .split("/skills/")
                .nth(1)
                .unwrap()
                .split('/')
                .next()
                .unwrap();
            format!("/{parent}/skills/{name}/")
        } else {
            format!("/{relative}")
        };
        patterns.entry(anchor.into()).or_default().insert(pattern);
    }
    for (anchor, patterns) in patterns {
        let base = anchors(info)
            .into_iter()
            .find(|(a, _)| *a == anchor)
            .unwrap()
            .1;
        let path = files::absolute(
            base.join(git::run(base, ["rev-parse", "--git-path", "info/exclude"])?),
        )?;
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
        let input = format!("{}\0", paths[&anchor].join("\0"));
        let ignored = git::run_input(
            base,
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
        if fields.len() != paths[&anchor].len() * 4 {
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
    let owned = ownership(info)?;
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
        compatibility_notice: compatibility_notice(info),
    };
    for (key, resource) in &next {
        let (base, _, path) = target(info, key)?;
        check_parents(base, &path)?;
        if files::exists(&path)? {
            let current = stamp(&read_resource(&path)?);
            if !pending.files[key].contains(&current) {
                bail!(
                    "Generated file changed during sync: {}. Preserve it before retrying.",
                    path.display()
                );
            }
            if current == stamp(resource) {
                result.unchanged += 1;
                continue;
            }
        }
        write_resource(&path, resource)?;
        result.written += 1;
    }
    for key in owned.files.keys().filter(|key| !next.contains_key(*key)) {
        let (base, _, path) = target(info, key)?;
        check_parents(base, &path)?;
        if files::exists(&path)? {
            if !owned.files[key].contains(&stamp(&read_resource(&path)?)) {
                bail!("Generated file changed during sync: {}", path.display());
            }
            fs::remove_file(&path)?;
            result.removed += 1;
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

pub fn check(info: &ProjectInfo) -> Result<String> {
    if files::exists(&workspace::local_directory(&info.spec, false)?.join(PENDING))? {
        bail!("Agent synchronization was interrupted. Run cspec sync to finish it.");
    }
    let owned = ownership(info)?;
    let next = desired(info, &owned)?;
    preflight(info, &owned, &next)?;
    if owned.files.len() != next.len() {
        bail!("Agent integrations are stale or missing. Run cspec sync.");
    }
    for (key, resource) in &next {
        let (_, _, path) = target(info, key)?;
        if !files::exists(&path)? || stamp(&read_resource(&path)?) != stamp(resource) {
            bail!(
                "Agent integration is missing or stale: {}. Run cspec sync.",
                path.display()
            );
        }
    }
    excludes(info, next.keys().cloned(), false)?;
    Ok(format!(
        "{} generated files match the selected integrations and active skills",
        next.len()
    ))
}

pub fn context(info: &ProjectInfo) -> Result<Value> {
    let status = match check(info) {
        Ok(detail) => json!({"ready":true,"detail":detail}),
        Err(error) => json!({"ready":false,"detail":format!("{error:#}")}),
    };
    Ok(json!({
        "schemaVersion":1, "cliVersion":env!("CARGO_PKG_VERSION"), "project":info,
        "instructionSources":instruction_sources(info),
        "skillSources":{"project":info.spec.join("spec/skills"), "shared":info.editable_guidelines.join("skills"), "activeShared":info.active_guidelines.join("skills")},
        "skills":skills::inventory(info)?, "integration":status, "compatibilityNotice":compatibility_notice(info),
    }))
}

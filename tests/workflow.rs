use cretspec::{
    agents, files, git,
    manifest::{Guidelines, Lock, Manifest, Source},
    operations, repository, skills, workspace,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    source: PathBuf,
    code: PathBuf,
    spec: PathBuf,
    guidelines: PathBuf,
    destination: PathBuf,
    manifest: Manifest,
    lock: Lock,
}

fn commit(directory: &Path, message: &str) -> String {
    git::run(directory, ["add", "."]).unwrap();
    git::run(
        directory,
        [
            "-c",
            "user.name=Test Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            message,
        ],
    )
    .unwrap();
    git::run(directory, ["rev-parse", "HEAD"]).unwrap()
}

fn repo(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    git::run(directory, ["init", "-b", "main"]).unwrap();
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("cspec-test-")
            .tempdir()
            .unwrap();
        let root = if cfg!(unix) {
            fs::canonicalize(temp.path()).unwrap()
        } else {
            temp.path().to_owned()
        };
        let source = root.join("source repositories");
        let code = source.join("Sample");
        let spec = source.join("Sample-spec");
        let guidelines = source.join("CretAI");
        let destination = root.join("destination with spaces");
        repo(&guidelines);
        fs::create_dir_all(guidelines.join("profiles")).unwrap();
        fs::create_dir_all(guidelines.join("guidelines")).unwrap();
        fs::write(guidelines.join("profiles/rust.md"), "# Rust\n").unwrap();
        fs::write(guidelines.join("guidelines/principles.md"), "# Rules\n").unwrap();
        let hash = commit(&guidelines, "Fixture");
        git::run(&guidelines, ["tag", "v0.1.0"]).unwrap();
        repo(&code);
        fs::write(code.join("README.md"), "# Code\n").unwrap();
        commit(&code, "Fixture");
        repo(&spec);
        fs::create_dir(spec.join("spec")).unwrap();
        fs::write(spec.join("spec/vision.md"), "# Spec\n").unwrap();
        let manifest = Manifest {
            schema_version: 1,
            name: "Sample".into(),
            code: Source {
                repository: "../Sample".into(),
            },
            guidelines: Guidelines {
                repository: "../CretAI".into(),
                reference: "v0.1.0".into(),
                mode: None,
            },
            profile: "rust".into(),
            agents: cretspec::manifest::default_agents(),
        };
        let lock = Lock {
            schema_version: 1,
            reference: "v0.1.0".into(),
            commit: hash,
        };
        files::write_json(&spec.join("project.json"), &manifest).unwrap();
        files::write_json(&spec.join("guidelines.lock.json"), &lock).unwrap();
        commit(&spec, "Fixture");
        fs::create_dir(&destination).unwrap();
        Self {
            _temp: temp,
            root,
            source,
            code,
            spec,
            guidelines,
            destination,
            manifest,
            lock,
        }
    }
    fn clone(&self) -> workspace::ProjectInfo {
        workspace::clone_project(
            &self.spec.to_string_lossy(),
            None,
            None,
            &self.destination,
            &|_| {},
        )
        .unwrap()
    }
    fn cli(&self, args: &[&str]) -> Output {
        self.binary_cli(Path::new(env!("CARGO_BIN_EXE_cspec")), args)
    }
    fn binary_cli(&self, binary: &Path, args: &[&str]) -> Output {
        Command::new(binary)
            .args(args)
            .current_dir(&self.destination)
            .env("CRETSPEC_HOME", self.root.join("personal config"))
            .output()
            .unwrap()
    }
}

fn ok(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn children(path: &Path) -> Vec<String> {
    let mut result = fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn skill_source(root: &Path, name: &str, instructions: &str) -> PathBuf {
    let directory = root.join(name);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("SKILL.md"), format!("---\nname: {name}\ndescription: >\n  Use when validating a fixture workflow.\n---\n\n{instructions}\n")).unwrap();
    directory
}

#[test]
fn repository_ignore_exceptions_cannot_expose_private_generated_files() {
    let f = Fixture::new();
    let info = f.clone();
    let before = fs::read(info.code.join("AGENTS.md")).unwrap();
    fs::write(info.code.join(".gitignore"), "!AGENTS.md\n").unwrap();
    assert!(agents::check(&info).is_err());
    let error = agents::sync(&info).unwrap_err();
    assert!(format!("{error:#}").contains("expose generated file"));
    assert_eq!(fs::read(info.code.join("AGENTS.md")).unwrap(), before);
    fs::write(info.code.join(".gitignore"), "# Resolved\n").unwrap();
    agents::sync(&info).unwrap();
    agents::check(&info).unwrap();
}

#[test]
fn ignored_snapshot_additions_are_not_exposed_as_adopted_skills() {
    let f = Fixture::new();
    let info = f.clone();
    let exclude = info.active_guidelines.join(".git/info/exclude");
    fs::write(&exclude, "/skills/\n").unwrap();
    skill_source(
        &info.active_guidelines.join("skills"),
        "uncommitted",
        "This is not adopted",
    );
    assert_eq!(
        git::run(&info.active_guidelines, ["status", "--porcelain"]).unwrap(),
        ""
    );
    let error = agents::sync(&info).unwrap_err();
    assert!(format!("{error:#}").contains("outside the locked Git snapshot"));
    assert!(!info.code.join(".agents/skills/uncommitted").exists());
}

#[test]
fn retry_recognizes_a_new_output_written_before_interruption() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let info = f.clone();
    let source = skill_source(&info.spec.join("spec/skills"), "recover-check", "New skill");
    let bytes = fs::read(source.join("SKILL.md")).unwrap();
    let fingerprint: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut pending: Value = files::read_json(&info.spec.join(".local/agents-state.json")).unwrap();
    pending["files"]["code/.agents/skills/recover-check/SKILL.md"] =
        json!([{"sha256":fingerprint,"executable":false}]);
    files::write_json(&info.spec.join(".local/agents-pending.json"), &pending).unwrap();
    let output = info.code.join(".agents/skills/recover-check/SKILL.md");
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(&output, bytes).unwrap();
    agents::sync(&info).unwrap();
    assert!(
        info.spec
            .join(".claude/skills/recover-check/SKILL.md")
            .is_file()
    );
    assert!(!info.spec.join(".local/agents-pending.json").exists());
    agents::check(&info).unwrap();
}

#[test]
fn invalid_proposed_shared_skill_does_not_change_the_definition() {
    let f = Fixture::new();
    let info = f.clone();
    let source = skill_source(
        &info.editable_guidelines.join("skills"),
        "invalid-skill",
        "Draft",
    );
    fs::write(source.join("SKILL.md"), "Invalid frontmatter").unwrap();
    commit(&info.editable_guidelines, "Add malformed skill");
    git::run(&info.editable_guidelines, ["tag", "v0.2.0"]).unwrap();
    let before = fs::read(info.spec.join("project.json")).unwrap();
    assert!(operations::update(&info.spec, "v0.2.0", false, false).is_err());
    assert_eq!(fs::read(info.spec.join("project.json")).unwrap(), before);
    agents::check(&info).unwrap();
    let inventory = skills::inventory(&info).unwrap();
    assert_eq!(inventory.draft_errors.len(), 1);
}

#[cfg(windows)]
#[test]
fn windows_junction_cannot_redirect_generated_resources() {
    let f = Fixture::new();
    let info = f.clone();
    let outside = f.root.join("outside resources");
    fs::create_dir(&outside).unwrap();
    let output = info.code.join(".agents/skills/cspec-workspace");
    fs::rename(&output, info.code.join("preserved-skill")).unwrap();
    let result = git::command("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&output)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(agents::sync(&info).is_err());
    assert!(children(&outside).is_empty());
}

#[test]
fn agent_entry_points_are_discoverable_excluded_and_relocatable() {
    let f = Fixture::new();
    let info = f.clone();
    for base in [
        &info.project_root,
        &info.code,
        &info.spec,
        &info.editable_guidelines,
    ] {
        for relative in [
            "AGENTS.md",
            "CLAUDE.md",
            ".agents/skills/cspec-workspace/SKILL.md",
            ".claude/skills/cspec-workspace/SKILL.md",
            ".github/instructions/cspec.instructions.md",
            ".cursor/rules/cspec.mdc",
        ] {
            let path = base.join(relative);
            assert!(fs::symlink_metadata(&path).unwrap().is_file());
            if base != &info.project_root {
                git::run(base, ["check-ignore", "--quiet", "--", relative]).unwrap();
            }
        }
        assert!(
            !fs::read_to_string(base.join("AGENTS.md"))
                .unwrap()
                .contains(&info.project_root.to_string_lossy().to_string())
        );
    }
    let result = agents::sync(&info).unwrap();
    assert_eq!(result.written, 0);
    assert_eq!(result.removed, 0);
    assert!(result.unchanged > 0);
    let moved = f.root.join("new parent with spaces");
    fs::rename(&info.project_root, &moved).unwrap();
    let info = workspace::info(&moved, false, &|_| {}).unwrap();
    agents::check(&info).unwrap();
    for base in [
        &info.project_root,
        &info.code,
        &info.spec,
        &info.editable_guidelines,
    ] {
        let output = ok(f.cli(&["context", &base.to_string_lossy(), "--json"]));
        let context: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(context["schemaVersion"], 1);
        assert_eq!(context["integration"]["ready"], true);
        assert_eq!(context["project"]["lock"]["commit"], f.lock.commit);
    }
}

#[test]
fn skills_keep_project_sources_and_shared_drafts_separate() {
    let f = Fixture::new();
    let info = f.clone();
    let output = ok(f.cli(&[
        "skill",
        "create",
        "query-validation",
        "--scope",
        "project",
        "--description",
        "Validate query inputs",
        "--location",
        &info.code.to_string_lossy(),
        "--json",
    ]));
    let created: Value = serde_json::from_str(&output).unwrap();
    let source = PathBuf::from(created["source"].as_str().unwrap());
    assert_eq!(
        fs::canonicalize(&source).unwrap(),
        fs::canonicalize(info.spec.join("spec/skills/query-validation")).unwrap()
    );
    fs::create_dir(source.join("references")).unwrap();
    fs::write(
        source.join("references/example.json"),
        b"{\"fixture\":true}",
    )
    .unwrap();
    assert!(agents::check(&info).is_err());
    let synced = ok(f.cli(&["sync", &info.code.to_string_lossy(), "--json"]));
    assert!(
        serde_json::from_str::<Value>(&synced).unwrap()["written"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        fs::read(
            info.code
                .join(".agents/skills/query-validation/references/example.json")
        )
        .unwrap(),
        b"{\"fixture\":true}"
    );
    assert!(
        skills::create(
            &info,
            "query-validation",
            skills::Scope::Project,
            "Another description"
        )
        .is_err()
    );
    let shared = skills::create(
        &info,
        "release-check",
        skills::Scope::Shared,
        "Check a release",
    )
    .unwrap();
    assert_eq!(
        shared,
        info.editable_guidelines.join("skills/release-check")
    );
    agents::sync(&info).unwrap();
    assert!(!info.code.join(".agents/skills/release-check").exists());
    let inventory = skills::inventory(&info).unwrap();
    assert!(
        inventory
            .active
            .iter()
            .any(|s| s.name == "query-validation")
    );
    assert!(!inventory.active.iter().any(|s| s.name == "release-check"));
    assert!(
        inventory
            .shared_drafts
            .iter()
            .any(|s| s.name == "release-check")
    );
    commit(&info.editable_guidelines, "Add a shared skill");
    git::run(&info.editable_guidelines, ["tag", "v0.2.0"]).unwrap();
    operations::update(&info.spec, "v0.2.0", false, false).unwrap();
    let updated = workspace::info(&info.spec, false, &|_| {}).unwrap();
    agents::check(&updated).unwrap();
    assert!(
        updated
            .code
            .join(".agents/skills/release-check/SKILL.md")
            .is_file()
    );
    assert_eq!(updated.lock.as_ref().unwrap().reference, "v0.2.0");
    assert_eq!(info.lock.as_ref().unwrap().reference, "v0.1.0");
}

#[test]
fn synchronization_preflights_conflicts_and_preserves_unrelated_resources() {
    let f = Fixture::new();
    let info = f.clone();
    let source = skill_source(
        &info.spec.join("spec/skills"),
        "check-data",
        "Original instructions",
    );
    agents::sync(&info).unwrap();
    let first = info.project_root.join(".agents/skills/check-data/SKILL.md");
    let before = fs::read(&first).unwrap();
    let edited = info.code.join(".claude/skills/check-data/SKILL.md");
    fs::write(&edited, "User work").unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: check-data\ndescription: Updated check\n---\nNew instructions\n",
    )
    .unwrap();
    assert!(
        agents::sync(&info)
            .unwrap_err()
            .to_string()
            .contains("locally edited")
    );
    assert_eq!(fs::read(&first).unwrap(), before);
    assert_eq!(fs::read(&edited).unwrap(), b"User work");
    fs::remove_file(&edited).unwrap();
    agents::sync(&info).unwrap();
    let keep = info.code.join(".agents/skills/check-data/user-notes.txt");
    fs::write(&keep, "Unmanaged notes").unwrap();
    fs::remove_dir_all(&source).unwrap();
    agents::sync(&info).unwrap();
    assert!(!first.exists());
    assert_eq!(fs::read(&keep).unwrap(), b"Unmanaged notes");
    agents::check(&info).unwrap();
}

#[test]
fn existing_repository_instructions_are_preserved_and_included() {
    let f = Fixture::new();
    fs::write(
        f.code.join("AGENTS.md"),
        "# Existing\nRun the existing checks.\n",
    )
    .unwrap();
    fs::write(f.code.join("CLAUDE.md"), "# Existing Claude instructions\n").unwrap();
    commit(&f.code, "Add instructions");
    let info = f.clone();
    assert_eq!(
        fs::read_to_string(info.code.join("AGENTS.md"))
            .unwrap()
            .replace("\r\n", "\n"),
        "# Existing\nRun the existing checks.\n"
    );
    assert!(
        fs::read_to_string(info.code.join("AGENTS.override.md"))
            .unwrap()
            .contains("Run the existing checks.")
    );
    assert_eq!(
        fs::read_to_string(info.code.join("CLAUDE.local.md")).unwrap(),
        "@AGENTS.override.md\n"
    );
    assert_eq!(git::run(&info.code, ["status", "--porcelain"]).unwrap(), "");
    fs::write(info.code.join("AGENTS.md"), "Updated tracked guidance\n").unwrap();
    assert!(agents::check(&info).is_err());
    agents::sync(&info).unwrap();
    assert!(
        fs::read_to_string(info.code.join("AGENTS.override.md"))
            .unwrap()
            .contains("Updated tracked guidance")
    );
}

#[test]
fn tracked_generated_files_and_existing_overrides_are_not_replaced() {
    let f = Fixture::new();
    let info = f.clone();
    git::run(&info.code, ["add", "--force", "AGENTS.md"]).unwrap();
    let before = fs::read(info.code.join("AGENTS.md")).unwrap();
    assert!(
        agents::sync(&info)
            .unwrap_err()
            .to_string()
            .contains("tracked")
    );
    assert_eq!(fs::read(info.code.join("AGENTS.md")).unwrap(), before);
    git::run(&info.code, ["reset", "--", "AGENTS.md"]).unwrap();
    fs::write(info.code.join("AGENTS.override.md"), "Owner override").unwrap();
    assert!(agents::sync(&info).is_err());
    assert_eq!(
        fs::read(info.code.join("AGENTS.override.md")).unwrap(),
        b"Owner override"
    );
}

#[test]
fn invalid_and_colliding_skills_fail_before_changing_integrations() {
    let f = Fixture::new();
    skill_source(&f.guidelines.join("skills"), "same-name", "Shared skill");
    let hash = commit(&f.guidelines, "Add shared skill");
    git::run(&f.guidelines, ["tag", "v0.2.0"]).unwrap();
    let mut manifest = f.manifest.clone();
    manifest.guidelines.reference = "v0.2.0".into();
    let lock = Lock {
        schema_version: 1,
        reference: "v0.2.0".into(),
        commit: hash,
    };
    files::write_json(&f.spec.join("project.json"), &manifest).unwrap();
    files::write_json(&f.spec.join("guidelines.lock.json"), &lock).unwrap();
    commit(&f.spec, "Adopt shared skill");
    let info = f.clone();
    let output = info.code.join(".agents/skills/same-name/SKILL.md");
    let before = fs::read(&output).unwrap();
    let source = skill_source(
        &info.spec.join("spec/skills"),
        "same-name",
        "Project collision",
    );
    assert!(
        agents::sync(&info)
            .unwrap_err()
            .to_string()
            .contains("both project")
    );
    assert_eq!(fs::read(&output).unwrap(), before);
    fs::remove_dir_all(source).unwrap();
    let invalid = skill_source(&info.spec.join("spec/skills"), "invalid", "Original");
    fs::write(
        invalid.join("SKILL.md"),
        "---\nname: wrong-name\ndescription: Example\n---\n",
    )
    .unwrap();
    assert!(agents::sync(&info).is_err());
    assert_eq!(fs::read(&output).unwrap(), before);
    assert!(skills::create(&info, "../outside", skills::Scope::Project, "Example").is_err());
    assert!(skills::create(&info, "cspec-workspace", skills::Scope::Project, "Example").is_err());
    assert!(skills::create(&info, "con", skills::Scope::Project, "Example").is_err());
}

#[test]
fn integration_selection_removes_only_owned_files_and_can_be_disabled() {
    let f = Fixture::new();
    let info = f.clone();
    let mut definition = info.manifest.clone();
    definition.agents = vec![cretspec::manifest::Agent::Copilot];
    files::write_json(&info.spec.join("project.json"), &definition).unwrap();
    let changed = workspace::info(&info.spec, false, &|_| {}).unwrap();
    agents::sync(&changed).unwrap();
    assert!(
        info.code
            .join(".agents/skills/cspec-workspace/SKILL.md")
            .is_file()
    );
    assert!(
        !info
            .code
            .join(".claude/skills/cspec-workspace/SKILL.md")
            .exists()
    );
    assert!(!info.code.join("CLAUDE.md").exists());
    definition.agents.clear();
    files::write_json(&info.spec.join("project.json"), &definition).unwrap();
    let changed = workspace::info(&info.spec, false, &|_| {}).unwrap();
    agents::sync(&changed).unwrap();
    assert!(!info.code.join("AGENTS.md").exists());
    assert!(
        !info
            .code
            .join(".agents/skills/cspec-workspace/SKILL.md")
            .exists()
    );
    agents::check(&changed).unwrap();
    let mut old = serde_json::to_value(&f.manifest).unwrap();
    old.as_object_mut().unwrap().remove("agents");
    let old: Manifest = serde_json::from_value(old).unwrap();
    assert_eq!(old.agents, cretspec::manifest::default_agents());
    definition.agents = vec![
        cretspec::manifest::Agent::Codex,
        cretspec::manifest::Agent::Codex,
    ];
    assert!(cretspec::manifest::validate(&definition, Some(&f.lock)).is_err());
}

#[test]
fn pending_sync_is_diagnosed_and_can_be_retried_without_losing_user_work() {
    let f = Fixture::new();
    let info = f.clone();
    let local = info.spec.join(".local");
    let state = fs::read(local.join("agents-state.json")).unwrap();
    fs::write(local.join("agents-pending.json"), &state).unwrap();
    fs::remove_file(info.code.join("CLAUDE.md")).unwrap();
    let before = fs::read(local.join("agents-pending.json")).unwrap();
    let report = operations::diagnose(&info.spec);
    assert!(
        report
            .iter()
            .any(|check| !check.ok && check.detail.contains("interrupted"))
    );
    assert_eq!(fs::read(local.join("agents-pending.json")).unwrap(), before);
    assert!(!info.code.join("CLAUDE.md").exists());
    agents::sync(&info).unwrap();
    assert!(!local.join("agents-pending.json").exists());
    assert!(info.code.join("CLAUDE.md").exists());
    agents::check(&info).unwrap();
    fs::write(local.join("agents-pending.json"), &state).unwrap();
    fs::write(
        info.code.join("CLAUDE.md"),
        "User changed this after interruption",
    )
    .unwrap();
    assert!(agents::sync(&info).is_err());
    assert_eq!(
        fs::read(info.code.join("CLAUDE.md")).unwrap(),
        b"User changed this after interruption"
    );
}

#[test]
fn simultaneous_sync_and_malicious_ownership_paths_are_rejected() {
    let f = Fixture::new();
    let info = f.clone();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(info.spec.join(".local/agents.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(
        agents::sync(&info)
            .unwrap_err()
            .to_string()
            .contains("Another")
    );
    lock.unlock().unwrap();
    drop(lock);
    agents::sync(&info).unwrap();
    let state_path = info.spec.join(".local/agents-state.json");
    let mut state: Value = files::read_json(&state_path).unwrap();
    state["files"]["code/../../outside"] = state["files"]["code/AGENTS.md"].clone();
    files::write_json(&state_path, &state).unwrap();
    assert!(agents::sync(&info).is_err());
    assert!(!f.root.join("outside").exists());
}

#[test]
fn adopted_definition_is_reported_when_agent_refresh_conflicts() {
    let f = Fixture::new();
    let info = f.clone();
    fs::write(
        info.editable_guidelines.join("profiles/rust.md"),
        "# Updated Rust\n",
    )
    .unwrap();
    commit(&info.editable_guidelines, "Update guidance");
    git::run(&info.editable_guidelines, ["tag", "v0.2.0"]).unwrap();
    fs::write(info.code.join("AGENTS.md"), "Local user edit").unwrap();
    let error = operations::update(&info.spec, "v0.2.0", false, false).unwrap_err();
    assert!(error.to_string().contains("definition is saved"));
    let (manifest, _) = cretspec::manifest::read(&info.spec).unwrap();
    assert_eq!(manifest.guidelines.reference, "v0.2.0");
    assert_eq!(
        fs::read(info.code.join("AGENTS.md")).unwrap(),
        b"Local user edit"
    );
    fs::remove_file(info.code.join("AGENTS.md")).unwrap();
    let updated = workspace::info(&info.spec, false, &|_| {}).unwrap();
    agents::sync(&updated).unwrap();
    agents::check(&updated).unwrap();
}

#[cfg(unix)]
#[test]
fn linked_sources_and_targets_are_rejected_and_executable_resources_survive() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = Fixture::new();
    let info = f.clone();
    let source = skill_source(
        &info.spec.join("spec/skills"),
        "run-check",
        "Use scripts/check.sh when asked",
    );
    fs::create_dir(source.join("scripts")).unwrap();
    let script = source.join("scripts/check.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    agents::sync(&info).unwrap();
    let destination = info.code.join(".agents/skills/run-check/scripts/check.sh");
    assert_ne!(
        fs::metadata(destination).unwrap().permissions().mode() & 0o111,
        0
    );
    symlink(f.root.join("outside"), source.join("escape")).unwrap();
    assert!(agents::sync(&info).is_err());
    fs::remove_file(source.join("escape")).unwrap();
    let generated = info.code.join("CLAUDE.md");
    fs::remove_file(&generated).unwrap();
    symlink(f.root.join("outside"), &generated).unwrap();
    assert!(agents::sync(&info).is_err());
    assert!(!f.root.join("outside").exists());
}

#[test]
fn references_work_across_transports_and_reject_credentials() {
    assert_eq!(
        repository::resolve("../Code", "https://github.com/owner/Code-spec.git").unwrap(),
        "https://github.com/owner/Code"
    );
    assert_eq!(
        repository::resolve(
            "../CretAI.git",
            "ssh://git@example.test:2222/group/Code-spec.git"
        )
        .unwrap(),
        "ssh://git@example.test:2222/group/CretAI.git"
    );
    assert_eq!(
        repository::resolve("../Code", "git@example.test:group/Code-spec.git").unwrap(),
        "git@example.test:group/Code"
    );
    let source = files::absolute("source with spaces/Code-spec").unwrap();
    assert_eq!(
        repository::resolve("../Code", &source.to_string_lossy()).unwrap(),
        source.parent().unwrap().join("Code").to_string_lossy()
    );
    assert!(repository::validate("ext::untrusted command").is_err());
    assert!(repository::validate("https://token@example.test/repo").is_err());
    assert!(repository::resolve(&source.to_string_lossy(), "https://example.test/spec").is_err());
    assert_eq!(
        repository::identity("git@github.com:owner/repo.git").unwrap(),
        repository::identity("https://github.com/owner/repo").unwrap()
    );
}

#[test]
fn short_names_and_explicit_paths_have_distinct_meaning() {
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        repository::namespace("owner", &cwd).unwrap(),
        "git@github.com:owner"
    );
    assert_eq!(
        repository::spec_source("Sample-spec", Some("owner"), &cwd).unwrap(),
        "git@github.com:owner/Sample-spec"
    );
    assert_eq!(
        repository::spec_source("./Sample-spec", Some("owner"), &cwd).unwrap(),
        cwd.join("Sample-spec").to_string_lossy()
    );
    assert!(repository::spec_source("Sample-spec", None, &cwd).is_err());
    for name in ["CON", "Lpt9.txt", "trailing.", "../outside", ""] {
        assert!(!files::portable_name(name));
    }
}

#[test]
fn clone_creates_three_repositories_local_integrations_and_an_exact_snapshot() {
    let f = Fixture::new();
    fs::write(f.guidelines.join("draft.md"), "Uncommitted work").unwrap();
    let info = f.clone();
    assert_eq!(children(&f.destination), ["Sample"]);
    assert_eq!(
        children(&info.project_root)
            .into_iter()
            .filter(|name| info.project_root.join(name).join(".git").is_dir())
            .collect::<Vec<_>>(),
        ["CretAI", "Sample", "Sample-spec"]
    );
    assert_eq!(
        git::run(&info.active_guidelines, ["rev-parse", "HEAD"]).unwrap(),
        f.lock.commit
    );
    assert_eq!(
        git::run(&info.editable_guidelines, ["branch", "--show-current"]).unwrap(),
        "main"
    );
    assert!(!info.active_guidelines.join("draft.md").exists());
    assert!(!info.spec.join(".local/project.code-workspace").exists());
    for repo in [&info.spec, &info.code] {
        assert_eq!(git::run(repo, ["status", "--porcelain"]).unwrap(), "");
    }
    assert!(f.guidelines.join("draft.md").exists());
}

#[test]
fn editor_regeneration_preserves_settings_and_follows_the_spec() {
    let f = Fixture::new();
    let info = f.clone();
    let file = workspace::editor_workspace(&info.spec.join("spec")).unwrap();
    let mut editor: Value = files::read_json(&file).unwrap();
    editor["settings"] = json!({"editor.tabSize":2});
    files::write_json(&file, &editor).unwrap();
    fs::rename(&info.code, info.project_root.join("RenamedSample")).unwrap();
    let mut manifest = f.manifest.clone();
    manifest.name = "RenamedSample".into();
    files::write_json(&info.spec.join("project.json"), &manifest).unwrap();
    workspace::editor_workspace(&info.spec).unwrap();
    let editor: Value = files::read_json(&file).unwrap();
    assert_eq!(editor["settings"]["editor.tabSize"], 2);
    assert_eq!(editor["folders"][0]["path"], "../../RenamedSample");
    fs::remove_file(&file).unwrap();
    assert_eq!(
        fs::canonicalize(workspace::editor_workspace(&info.spec).unwrap()).unwrap(),
        fs::canonicalize(file).unwrap()
    );
}

#[test]
fn project_can_move_without_a_registry_or_original_guidelines_source() {
    let f = Fixture::new();
    let info = f.clone();
    let moved = f.root.join("relocated project");
    fs::rename(&info.project_root, &moved).unwrap();
    fs::rename(
        &f.guidelines,
        f.root.join("unavailable original guidelines"),
    )
    .unwrap();
    let current = workspace::info(&moved, true, &|_| {}).unwrap();
    assert_eq!(current.code, moved.join("Sample"));
    for start in [
        &moved,
        &current.code,
        &current.spec,
        &current.editable_guidelines,
    ] {
        assert_eq!(
            fs::canonicalize(workspace::find_spec(start).unwrap()).unwrap(),
            fs::canonicalize(&current.spec).unwrap()
        );
    }
}

#[test]
fn destinations_and_enclosing_repositories_are_preserved() {
    let f = Fixture::new();
    let target = f.destination.join("existing");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), "Keep me").unwrap();
    assert!(
        workspace::clone_project(
            &f.spec.to_string_lossy(),
            Some(&target),
            None,
            &f.destination,
            &|_| {}
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(target.join("keep.txt")).unwrap(),
        "Keep me"
    );
    assert!(
        workspace::clone_project(
            &f.spec.to_string_lossy(),
            Some(&f.code.join("private-spec")),
            None,
            &f.destination,
            &|_| {}
        )
        .is_err()
    );
}

#[test]
fn attaching_preserves_uncommitted_code_and_rejects_wrong_placement() {
    let f = Fixture::new();
    let spec = f.destination.join("Sample-spec");
    let code = f.destination.join("Sample");
    git::clone(&f.spec.to_string_lossy(), &spec).unwrap();
    git::clone(&f.code.to_string_lossy(), &code).unwrap();
    fs::write(code.join("README.md"), "User draft").unwrap();
    workspace::attach(&code, &spec, &|_| {}).unwrap();
    assert_eq!(
        fs::read_to_string(code.join("README.md")).unwrap(),
        "User draft"
    );
    assert!(workspace::attach(&f.code, &spec, &|_| {}).is_err());
}

#[test]
fn changed_cache_and_moved_tag_cannot_substitute_guidelines() {
    let f = Fixture::new();
    let info = f.clone();
    fs::write(
        info.active_guidelines.join("guidelines/principles.md"),
        "Changed cache",
    )
    .unwrap();
    assert!(workspace::info(&info.spec, true, &|_| {}).is_err());
    fs::write(f.guidelines.join("new.md"), "New guidance").unwrap();
    commit(&f.guidelines, "Changed rules");
    git::run(&f.guidelines, ["tag", "-f", "v0.1.0"]).unwrap();
    assert!(
        workspace::clone_project(
            &f.spec.to_string_lossy(),
            Some(&f.root.join("retry")),
            None,
            &f.destination,
            &|_| {}
        )
        .is_err()
    );
}

#[test]
fn unknown_manifest_fields_fail_before_code_is_cloned() {
    let f = Fixture::new();
    let mut manifest = serde_json::to_value(&f.manifest).unwrap();
    manifest["run"] = json!("untrusted command");
    files::write_json(&f.spec.join("project.json"), &manifest).unwrap();
    commit(&f.spec, "Invalid fixture");
    assert!(
        workspace::clone_project(
            &f.spec.to_string_lossy(),
            None,
            None,
            &f.destination,
            &|_| {}
        )
        .is_err()
    );
    assert!(!f.destination.join("Sample").exists());
}

#[test]
fn help_and_version_need_no_configuration() {
    let f = Fixture::new();
    assert!(ok(f.cli(&["--help"])).contains("Usage:"));
    assert_eq!(
        ok(f.cli(&["--version"])),
        format!("cspec {}", env!("CARGO_PKG_VERSION"))
    );
    let output = f.cli(&["project", "clone", "Sample-spec"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Configure a repository namespace"));
    assert!(
        !f.cli(&["project", "open", "--unsupported"])
            .status
            .success()
    );
}

#[test]
fn standalone_executable_clones_and_discovers_without_a_source_checkout() {
    let f = Fixture::new();
    let binary = f.root.join(if cfg!(windows) {
        "installed-cspec.exe"
    } else {
        "installed-cspec"
    });
    fs::copy(env!("CARGO_BIN_EXE_cspec"), &binary).unwrap();
    ok(f.binary_cli(
        &binary,
        &["config", "namespace", &f.source.to_string_lossy()],
    ));
    ok(f.binary_cli(&binary, &["project", "clone", "Sample-spec"]));
    fs::remove_file(f.root.join("personal config/config.json")).unwrap();
    let root = f.destination.join("Sample");
    let output = ok(f.binary_cli(&binary, &["project", "info", &root.to_string_lossy()]));
    let info: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        Path::new(info["code"].as_str().unwrap()),
        root.join("Sample")
    );
    assert_eq!(
        ok(f.binary_cli(
            &binary,
            &["guidelines", "edit", &root.to_string_lossy(), "--print"]
        )),
        root.join("CretAI").to_string_lossy()
    );
}

#[test]
fn namespace_migration_preserves_preferences_even_when_the_old_clone_moved() {
    let f = Fixture::new();
    ok(f.cli(&["config", "namespace", "owner"]));
    let file = f.root.join("personal config/config.json");
    let before = fs::read(&file).unwrap();
    assert!(
        !f.cli(&["config", "namespace", "https://token@example.test/owner"])
            .status
            .success()
    );
    assert_eq!(fs::read(&file).unwrap(), before);
    files::write_json(
        &file,
        &json!({"schemaVersion":1,"guidelinesRoot":f.guidelines,"preference":"keep"}),
    )
    .unwrap();
    assert_eq!(
        ok(f.cli(&["config", "namespace"])),
        f.source.to_string_lossy()
    );
    fs::rename(&f.guidelines, f.root.join("moved guidelines")).unwrap();
    ok(f.cli(&["config", "namespace", "owner"]));
    let value: Value = files::read_json(&file).unwrap();
    assert_eq!(
        value,
        json!({"schemaVersion":2,"repositoryNamespace":"git@github.com:owner","preference":"keep"})
    );
}

#[test]
fn tracked_local_files_and_unrelated_guidelines_are_rejected() {
    let f = Fixture::new();
    let info = f.clone();
    git::run(
        &info.editable_guidelines,
        [
            "remote",
            "set-url",
            "origin",
            "https://example.test/unrelated",
        ],
    )
    .unwrap();
    assert!(workspace::info(&info.spec, true, &|_| {}).is_err());
    fs::create_dir(f.spec.join(".local")).unwrap();
    fs::write(f.spec.join(".local/keep.txt"), "Tracked file").unwrap();
    commit(&f.spec, "Tracked local file");
    assert!(workspace::attach(&f.code, &f.spec, &|_| {}).is_err());
    assert_eq!(
        fs::read_to_string(f.spec.join(".local/keep.txt")).unwrap(),
        "Tracked file"
    );
}

#[test]
fn unrelated_code_at_the_expected_path_is_not_changed() {
    let f = Fixture::new();
    let spec = f.destination.join("Sample-spec");
    let code = f.destination.join("Sample");
    git::clone(&f.spec.to_string_lossy(), &spec).unwrap();
    repo(&code);
    fs::write(code.join("README.md"), "Independent product").unwrap();
    let head = commit(&code, "Independent");
    assert!(workspace::info(&spec, true, &|_| {}).is_err());
    assert_eq!(git::run(&code, ["rev-parse", "HEAD"]).unwrap(), head);
    assert!(!spec.join(".local").exists());
}

#[test]
fn editing_one_project_does_not_change_another_or_either_snapshot() {
    let f = Fixture::new();
    let first = f.clone();
    let second = workspace::clone_project(
        &f.spec.to_string_lossy(),
        Some(&f.root.join("Second")),
        None,
        &f.destination,
        &|_| {},
    )
    .unwrap();
    fs::write(
        first.editable_guidelines.join("guidelines/principles.md"),
        "Local proposal",
    )
    .unwrap();
    for directory in [
        &second.editable_guidelines,
        &first.active_guidelines,
        &second.active_guidelines,
    ] {
        assert_eq!(
            fs::read_to_string(directory.join("guidelines/principles.md"))
                .unwrap()
                .trim(),
            "# Rules"
        );
    }
    workspace::info(&first.code, true, &|_| {}).unwrap();
}

#[test]
fn ambiguous_discovery_requires_an_explicit_spec() {
    let f = Fixture::new();
    let info = f.clone();
    git::clone(
        &f.spec.to_string_lossy(),
        &info.project_root.join("Another-spec"),
    )
    .unwrap();
    assert!(workspace::find_spec(&info.project_root).is_err());
    assert_eq!(workspace::find_spec(&info.spec).unwrap(), info.spec);
}

#[test]
fn inspection_without_preparation_does_not_create_local_context() {
    let f = Fixture::new();
    assert!(workspace::info(&f.spec, false, &|_| {}).is_err());
    assert!(!f.spec.join(".local").exists());
}

#[test]
fn malformed_editor_settings_are_preserved_on_failure() {
    let f = Fixture::new();
    let info = f.clone();
    let file = info.spec.join(".local/project.code-workspace");
    fs::write(&file, "[1,2,3]\n").unwrap();
    assert!(workspace::editor_workspace(&info.spec).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "[1,2,3]\n");
}

#[test]
fn initialize_from_pinned_templates_preserves_existing_files_and_can_be_cloned() {
    let f = Fixture::new();
    let templates = f.guidelines.join("templates/spec/spec");
    fs::create_dir_all(&templates).unwrap();
    for name in [
        "vision",
        "scope",
        "requirements",
        "architecture",
        "decisions",
        "roadmap",
        "acceptance",
        "project-rules",
    ] {
        fs::write(templates.join(format!("{name}.md")), format!("# {name}\n")).unwrap();
    }
    let hash = commit(&f.guidelines, "Add templates");
    git::run(&f.guidelines, ["tag", "v0.2.0"]).unwrap();
    let spec = f.source.join("Fresh-spec");
    repo(&spec);
    fs::write(spec.join("README.md"), "Existing readme").unwrap();
    operations::initialize(operations::InitOptions {
        directory: &spec,
        name: "Fresh",
        code: "../Sample",
        guidelines: "../CretAI",
        reference: Some("v0.2.0"),
        pinned: true,
        profile: "rust",
    })
    .unwrap();
    assert_eq!(
        fs::read_to_string(spec.join("README.md")).unwrap(),
        "Existing readme"
    );
    let lock: Lock = files::read_json(&spec.join("guidelines.lock.json")).unwrap();
    assert_eq!(lock.commit, hash);
    assert_eq!(children(&spec.join("spec")).len(), 8);
    assert!(
        operations::initialize(operations::InitOptions {
            directory: &spec,
            name: "Fresh",
            code: "../Sample",
            guidelines: "../CretAI",
            reference: Some("v0.2.0"),
            pinned: true,
            profile: "rust",
        })
        .is_err()
    );
    commit(&spec, "Define project");
    let info =
        workspace::clone_project(&spec.to_string_lossy(), None, None, &f.destination, &|_| {})
            .unwrap();
    assert_eq!(info.manifest.name, "Fresh");
    assert_eq!(info.lock.as_ref().unwrap().commit, hash);
}

#[test]
fn initialization_validates_all_templates_before_writing() {
    let f = Fixture::new();
    let spec = f.source.join("Fresh-spec");
    repo(&spec);
    assert!(
        operations::initialize(operations::InitOptions {
            directory: &spec,
            name: "Fresh",
            code: "../Sample",
            guidelines: "../CretAI",
            reference: Some("v0.1.0"),
            pinned: true,
            profile: "rust",
        })
        .is_err()
    );
    assert!(!spec.join("project.json").exists());
    assert!(!spec.join("spec").exists());
}

#[test]
fn guideline_preview_and_adoption_preserve_drafts_and_other_projects() {
    let f = Fixture::new();
    let info = f.clone();
    let manifest_before = fs::read(info.spec.join("project.json")).unwrap();
    let lock_before = fs::read(info.spec.join("guidelines.lock.json")).unwrap();
    fs::write(
        info.editable_guidelines.join("draft.md"),
        "Work in progress",
    )
    .unwrap();
    let head_before = git::run(&info.editable_guidelines, ["rev-parse", "HEAD"]).unwrap();
    fs::write(
        f.guidelines.join("guidelines/principles.md"),
        "# Updated rules\n",
    )
    .unwrap();
    let next = commit(&f.guidelines, "Update rules");
    git::run(&f.guidelines, ["tag", "v0.2.0"]).unwrap();
    let preview = operations::update(&info.code, "v0.2.0", true, true).unwrap();
    assert!(!preview.applied);
    assert_eq!(preview.selected.commit, next);
    assert_eq!(
        fs::read(info.spec.join("project.json")).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read(info.spec.join("guidelines.lock.json")).unwrap(),
        lock_before
    );
    assert!(operations::update(&info.spec, "missing", false, false).is_err());
    let applied = operations::update(&info.spec, "v0.2.0", false, false).unwrap();
    assert!(applied.applied);
    let updated = workspace::info(&info.spec, false, &|_| {}).unwrap();
    assert_eq!(updated.lock.as_ref().unwrap().commit, next);
    assert_eq!(updated.manifest.guidelines.reference, "v0.2.0");
    assert_eq!(
        git::run(&info.editable_guidelines, ["rev-parse", "HEAD"]).unwrap(),
        head_before
    );
    assert_eq!(
        fs::read_to_string(info.editable_guidelines.join("draft.md")).unwrap(),
        "Work in progress"
    );
    let original: Lock = files::read_json(&f.spec.join("guidelines.lock.json")).unwrap();
    assert_eq!(original.commit, f.lock.commit);
    assert!(!info.spec.join(".local/definition-update.json").exists());
}

#[test]
fn interrupted_adoption_can_be_recovered_without_overwriting_unrelated_edits() {
    let f = Fixture::new();
    let info = f.clone();
    let old_manifest = fs::read_to_string(info.spec.join("project.json")).unwrap();
    let old_lock = fs::read_to_string(info.spec.join("guidelines.lock.json")).unwrap();
    let mut new_manifest = info.manifest.clone();
    new_manifest.guidelines.reference = "v0.2.0".into();
    let mut new_lock = info.lock.clone().unwrap();
    new_lock.reference = "v0.2.0".into();
    let journal = info.spec.join(".local/definition-update.json");
    files::write_json(
        &journal,
        &json!({"oldManifest": old_manifest, "oldLock": old_lock,
        "newManifest": new_manifest, "newLock": new_lock}),
    )
    .unwrap();
    files::write_json(&info.spec.join("project.json"), &new_manifest).unwrap();
    assert!(workspace::info(&info.code, true, &|_| {}).is_err());
    new_manifest.name = "UserEdit".into();
    files::write_json(&info.spec.join("project.json"), &new_manifest).unwrap();
    assert!(operations::recover(&info.code).is_err());
    assert!(journal.exists());
    new_manifest.name = info.manifest.name;
    files::write_json(&info.spec.join("project.json"), &new_manifest).unwrap();
    operations::recover(&info.code).unwrap();
    assert_eq!(
        fs::read_to_string(info.spec.join("project.json")).unwrap(),
        old_manifest
    );
    assert_eq!(
        fs::read_to_string(info.spec.join("guidelines.lock.json")).unwrap(),
        old_lock
    );
    assert!(!journal.exists());
}

#[test]
fn doctor_reports_failure_without_creating_context_and_success_after_clone() {
    let f = Fixture::new();
    let output = f.cli(&["project", "doctor", &f.spec.to_string_lossy(), "--json"]);
    assert!(!output.status.success());
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(checks.iter().any(|c| c["ok"] == false));
    assert!(!f.spec.join(".local").exists());
    let info = f.clone();
    let checks: Vec<Value> = serde_json::from_str(&ok(f.cli(&[
        "project",
        "doctor",
        &info.code.to_string_lossy(),
        "--json",
    ])))
    .unwrap();
    assert!(checks.iter().all(|c| c["ok"] == true));
    assert!(f.cli(&[]).status.success());
}

fn working_fixture() -> Fixture {
    let mut f = Fixture::new();
    f.manifest.guidelines.reference = "main".into();
    // A legacy manifest without a lock defaults to the working tree too.
    files::write_json(&f.spec.join("project.json"), &f.manifest).unwrap();
    fs::remove_file(f.spec.join("guidelines.lock.json")).unwrap();
    commit(&f.spec, "Use editable guidelines");
    f
}

#[test]
fn working_tree_clone_reads_local_rules_and_uncommitted_shared_skills() {
    let f = working_fixture();
    let info = f.clone();
    assert!(info.lock.is_none());
    assert_eq!(info.active_guidelines, info.editable_guidelines);
    assert!(!info.spec.join(".local/guidelines").exists());
    assert!(!info.spec.join("guidelines.lock.json").exists());
    assert_eq!(info.guidelines.branch.as_deref(), Some("main"));
    let rules = info.editable_guidelines.join("guidelines/principles.md");
    fs::write(&rules, "# Locally edited rules\n").unwrap();
    assert!(agents::instruction_sources(&info).contains(&rules));
    let output: Value = serde_json::from_str(&ok(f.cli(&[
        "skill",
        "create",
        "shared-check",
        "--scope",
        "shared",
        "--description",
        "Check a reusable workflow",
        "--location",
        &info.code.to_string_lossy(),
        "--json",
    ])))
    .unwrap();
    assert!(output["nextStep"].as_str().unwrap().contains("cspec sync"));
    let source = info
        .editable_guidelines
        .join("skills/shared-check/SKILL.md");
    let refreshed = workspace::info(&info.code, false, &|_| {}).unwrap();
    assert!(refreshed.guidelines.dirty);
    let inventory = skills::inventory(&refreshed).unwrap();
    assert!(inventory.shared_drafts.is_empty());
    assert!(inventory.active.iter().any(|s| s.name == "shared-check"));
    assert!(agents::check(&refreshed).is_err());
    ok(f.cli(&["sync", &info.code.to_string_lossy()]));
    assert_eq!(
        fs::read(&source).unwrap(),
        fs::read(info.code.join(".agents/skills/shared-check/SKILL.md")).unwrap()
    );
    let bootstrap = fs::read_to_string(info.code.join("AGENTS.md")).unwrap();
    assert!(bootstrap.contains("Local guideline edits are active immediately"));
    assert!(!bootstrap.contains("Shared drafts require"));
    let context: Value = serde_json::from_str(&ok(f.cli(&[
        "context",
        &info.code.to_string_lossy(),
        "--json",
    ])))
    .unwrap();
    assert_eq!(context["project"]["lock"], Value::Null);
    assert_eq!(context["project"]["guidelines"]["mode"], "workingTree");
    assert_eq!(context["project"]["guidelines"]["dirty"], true);
    assert_eq!(context["integration"]["ready"], true);
    let checks = operations::diagnose(&info.code);
    assert!(checks.iter().all(|c| c.ok));
    assert!(
        checks
            .iter()
            .any(|c| c.detail.contains("local changes are active"))
    );
}

#[test]
fn working_tree_commands_never_fetch_pull_or_switch_and_manual_git_updates_skills() {
    let f = working_fixture();
    let info = f.clone();
    let before = info.guidelines.commit.clone();
    skill_source(
        &f.guidelines.join("skills"),
        "remote-check",
        "Published shared workflow",
    );
    let remote = commit(&f.guidelines, "Publish shared skill");
    git::run(&info.editable_guidelines, ["switch", "-c", "local-work"]).unwrap();
    fs::write(info.editable_guidelines.join("draft.md"), "Local draft").unwrap();
    for args in [
        vec!["project", "info"],
        vec!["project", "open"],
        vec!["context"],
        vec!["sync"],
        vec!["project", "doctor"],
    ] {
        let path = info.code.to_string_lossy();
        let mut args = args;
        args.push(&path);
        if args[1] == "open" {
            args.push("--print");
        }
        ok(f.cli(&args));
    }
    assert_eq!(
        git::run(&info.editable_guidelines, ["rev-parse", "origin/main"]).unwrap(),
        before
    );
    assert_eq!(
        git::run(&info.editable_guidelines, ["rev-parse", "HEAD"]).unwrap(),
        before
    );
    assert_eq!(
        git::run(&info.editable_guidelines, ["branch", "--show-current"]).unwrap(),
        "local-work"
    );
    assert_eq!(
        fs::read_to_string(info.editable_guidelines.join("draft.md")).unwrap(),
        "Local draft"
    );
    assert!(!info.code.join(".agents/skills/remote-check").exists());
    assert!(
        operations::diagnose(&info.code)
            .iter()
            .any(|c| c.detail.contains("differs from configured"))
    );
    git::run(&info.editable_guidelines, ["switch", "main"]).unwrap();
    git::run(&info.editable_guidelines, ["fetch", "origin"]).unwrap();
    let fetched = workspace::info(&info.code, false, &|_| {}).unwrap();
    assert_eq!(fetched.guidelines.behind, Some(1));
    assert_eq!(fetched.guidelines.commit, before);
    git::run(&info.editable_guidelines, ["pull", "--ff-only"]).unwrap();
    ok(f.cli(&["sync", &info.code.to_string_lossy()]));
    assert!(
        info.code
            .join(".agents/skills/remote-check/SKILL.md")
            .is_file()
    );
    git::run(&info.editable_guidelines, ["checkout", "--detach", &remote]).unwrap();
    let detached = workspace::info(&info.code, true, &|_| {}).unwrap();
    assert!(detached.guidelines.branch.is_none());
    assert!(
        operations::diagnose(&info.code)
            .iter()
            .any(|c| c.detail.contains("detached HEAD"))
    );
    assert!(operations::unlock(&info.code, false).is_err());
    assert!(detached.lock.is_none());
}

#[test]
fn new_initialization_uses_remote_default_branch_without_a_lock() {
    let f = Fixture::new();
    let templates = f.guidelines.join("templates/spec/spec");
    fs::create_dir_all(&templates).unwrap();
    for name in [
        "vision",
        "scope",
        "requirements",
        "architecture",
        "decisions",
        "roadmap",
        "acceptance",
        "project-rules",
    ] {
        fs::write(templates.join(format!("{name}.md")), format!("# {name}\n")).unwrap();
    }
    commit(&f.guidelines, "Add templates");
    git::run(&f.guidelines, ["branch", "-m", "trunk"]).unwrap();
    let spec = f.source.join("Fresh-spec");
    repo(&spec);
    ok(f.cli(&[
        "project",
        "init",
        &spec.to_string_lossy(),
        "--name",
        "Fresh",
        "--code",
        "../Sample",
        "--guidelines",
        "../CretAI",
        "--profile",
        "rust",
    ]));
    let (manifest, lock) = cretspec::manifest::read(&spec).unwrap();
    assert!(lock.is_none());
    assert_eq!(manifest.guidelines.reference, "trunk");
    assert_eq!(
        manifest.guidelines.mode,
        Some(cretspec::manifest::GuidelinesMode::WorkingTree)
    );
    commit(&spec, "Define project");
    let info =
        workspace::clone_project(&spec.to_string_lossy(), None, None, &f.destination, &|_| {})
            .unwrap();
    assert_eq!(info.guidelines.branch.as_deref(), Some("trunk"));
    assert_eq!(info.active_guidelines, info.editable_guidelines);
    let other = f.source.join("Other-spec");
    repo(&other);
    let failure = f.cli(&[
        "project",
        "init",
        &other.to_string_lossy(),
        "--name",
        "Other",
        "--code",
        "../Sample",
        "--guidelines",
        "../CretAI",
        "--profile",
        "rust",
        "--lock",
    ]);
    assert!(!failure.status.success());
    assert!(!other.join("project.json").exists());
}

#[test]
fn working_tree_clone_selects_configured_branch_and_rejects_tags() {
    let mut f = working_fixture();
    git::run(&f.guidelines, ["switch", "-c", "development"]).unwrap();
    fs::write(f.guidelines.join("profiles/rust.md"), "# Development rules").unwrap();
    commit(&f.guidelines, "Change branch rules");
    git::run(&f.guidelines, ["switch", "main"]).unwrap();
    f.manifest.guidelines.reference = "development".into();
    files::write_json(&f.spec.join("project.json"), &f.manifest).unwrap();
    commit(&f.spec, "Select guidelines branch");
    let info = f.clone();
    assert_eq!(info.guidelines.branch.as_deref(), Some("development"));
    assert_eq!(
        fs::read_to_string(info.active_guidelines.join("profiles/rust.md")).unwrap(),
        "# Development rules"
    );
    let mut invalid = info.manifest.clone();
    invalid.guidelines.reference = "v0.1.0".into();
    files::write_json(&info.spec.join("project.json"), &invalid).unwrap();
    assert!(workspace::info(&info.code, true, &|_| {}).is_err());
    assert_eq!(
        git::run(&info.editable_guidelines, ["branch", "--show-current"]).unwrap(),
        "development"
    );
}

#[test]
fn optional_pinning_and_unlock_preserve_drafts_snapshots_and_preview_definition() {
    let f = working_fixture();
    let info = f.clone();
    skill_source(
        &info.editable_guidelines.join("skills"),
        "local-check",
        "Uncommitted shared skill",
    );
    agents::sync(&info).unwrap();
    let original = fs::read(info.spec.join("project.json")).unwrap();
    operations::update(&info.code, "v0.1.0", true, false).unwrap();
    assert_eq!(fs::read(info.spec.join("project.json")).unwrap(), original);
    assert!(!info.spec.join("guidelines.lock.json").exists());
    operations::update(&info.code, "v0.1.0", false, false).unwrap();
    let pinned = workspace::info(&info.code, false, &|_| {}).unwrap();
    assert_eq!(pinned.lock.as_ref().unwrap().commit, f.lock.commit);
    assert!(
        !pinned
            .code
            .join(".agents/skills/local-check/SKILL.md")
            .exists()
    );
    assert!(
        pinned
            .editable_guidelines
            .join("skills/local-check/SKILL.md")
            .exists()
    );
    let definition = fs::read(pinned.spec.join("project.json")).unwrap();
    let lock = fs::read(pinned.spec.join("guidelines.lock.json")).unwrap();
    ok(f.cli(&[
        "guidelines",
        "unlock",
        &info.code.to_string_lossy(),
        "--preview",
    ]));
    assert_eq!(
        fs::read(pinned.spec.join("project.json")).unwrap(),
        definition
    );
    assert_eq!(
        fs::read(pinned.spec.join("guidelines.lock.json")).unwrap(),
        lock
    );
    // An explicit pin cannot silently downgrade when its lock is lost.
    fs::remove_file(pinned.spec.join("guidelines.lock.json")).unwrap();
    assert!(workspace::info(&pinned.code, false, &|_| {}).is_err());
    fs::write(pinned.spec.join("guidelines.lock.json"), lock).unwrap();
    ok(f.cli(&["guidelines", "unlock", &info.code.to_string_lossy()]));
    let unlocked = workspace::info(&info.code, false, &|_| {}).unwrap();
    assert!(unlocked.lock.is_none());
    assert!(unlocked.guidelines.dirty);
    assert_eq!(
        fs::canonicalize(&unlocked.active_guidelines).unwrap(),
        fs::canonicalize(&info.editable_guidelines).unwrap()
    );
    assert!(pinned.active_guidelines.is_dir());
    assert!(
        info.code
            .join(".agents/skills/local-check/SKILL.md")
            .exists()
    );
    assert!(!info.spec.join(".local/definition-update.json").exists());
}

#[test]
fn interrupted_pin_and_unlock_restore_lock_presence_and_exact_original_text() {
    let f = working_fixture();
    let info = f.clone();
    for unlocking in [false, true] {
        if unlocking {
            operations::update(&info.code, "v0.1.0", false, false).unwrap();
        }
        let current = workspace::info(&info.code, false, &|_| {}).unwrap();
        let old_manifest = fs::read_to_string(current.spec.join("project.json")).unwrap();
        let old_lock = fs::read_to_string(current.spec.join("guidelines.lock.json")).ok();
        let mut next_manifest = current.manifest.clone();
        let next_lock = if unlocking {
            next_manifest.guidelines.mode = Some(cretspec::manifest::GuidelinesMode::WorkingTree);
            next_manifest.guidelines.reference = "main".into();
            None
        } else {
            next_manifest.guidelines.mode = Some(cretspec::manifest::GuidelinesMode::Pinned);
            next_manifest.guidelines.reference = f.lock.reference.clone();
            Some(f.lock.clone())
        };
        let journal = current.spec.join(".local/definition-update.json");
        files::write_json(&journal, &json!({"oldManifest":old_manifest,"oldLock":old_lock,"newManifest":next_manifest,"newLock":next_lock})).unwrap();
        files::write_json(&current.spec.join("project.json"), &next_manifest).unwrap();
        if let Some(lock) = next_lock {
            files::write_json(&current.spec.join("guidelines.lock.json"), &lock).unwrap();
        } else {
            fs::remove_file(current.spec.join("guidelines.lock.json")).unwrap();
        }
        assert!(workspace::info(&current.code, false, &|_| {}).is_err());
        operations::recover(&current.code).unwrap();
        assert_eq!(
            fs::read_to_string(current.spec.join("project.json")).unwrap(),
            old_manifest
        );
        assert_eq!(
            fs::read_to_string(current.spec.join("guidelines.lock.json")).ok(),
            old_lock
        );
        assert!(!journal.exists());
    }
}

#[test]
fn unlocking_validates_active_skills_before_changing_definition() {
    let f = Fixture::new();
    let info = f.clone();
    skill_source(
        &info.spec.join("spec/skills"),
        "same-name",
        "Project workflow",
    );
    skill_source(
        &info.editable_guidelines.join("skills"),
        "same-name",
        "Shared workflow",
    );
    let before = fs::read(info.spec.join("project.json")).unwrap();
    assert!(operations::unlock(&info.code, false).is_err());
    assert_eq!(fs::read(info.spec.join("project.json")).unwrap(), before);
    assert!(info.spec.join("guidelines.lock.json").exists());
}

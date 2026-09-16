use cretspec::{
    files, git,
    manifest::{Guidelines, Lock, Manifest, Source},
    operations, repository, workspace,
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
            },
            profile: "rust".into(),
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
fn clone_creates_only_three_repositories_and_an_exact_snapshot() {
    let f = Fixture::new();
    fs::write(f.guidelines.join("draft.md"), "Uncommitted work").unwrap();
    let info = f.clone();
    assert_eq!(children(&f.destination), ["Sample"]);
    assert_eq!(
        children(&info.project_root),
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
        reference: "v0.2.0",
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
            reference: "v0.2.0",
            profile: "rust",
        })
        .is_err()
    );
    commit(&spec, "Define project");
    let info =
        workspace::clone_project(&spec.to_string_lossy(), None, None, &f.destination, &|_| {})
            .unwrap();
    assert_eq!(info.manifest.name, "Fresh");
    assert_eq!(info.lock.commit, hash);
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
            reference: "v0.1.0",
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
    assert_eq!(updated.lock.commit, next);
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
    let mut new_lock = info.lock.clone();
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

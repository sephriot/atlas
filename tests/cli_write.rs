use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("atlas-{prefix}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir should be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn atlas(storage: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_atlas"));
    cmd.arg("--storage")
        .arg(storage)
        .arg("--org")
        .arg("acme")
        .arg("--project")
        .arg("atlas")
        .arg("--format")
        .arg("json");
    cmd
}

fn run_json(mut cmd: Command) -> Value {
    let output = cmd.output().expect("command should run");
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn create_with_same_title_creates_distinct_atoms() {
    let temp = TempDir::new("same-title-create");

    let mut first = atlas(temp.path());
    first.args([
        "create",
        "--title",
        "Shared title",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "First atom",
    ]);
    let first = run_json(first);

    let mut second = atlas(temp.path());
    second.args([
        "create",
        "--title",
        "Shared title",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Second atom",
    ]);
    let second = run_json(second);

    assert_eq!(first["created"], true);
    assert_eq!(second["created"], true);
    assert_ne!(first["id"], second["id"]);
}

#[test]
fn create_with_summary_does_not_wait_for_open_stdin() {
    let temp = TempDir::new("create-open-stdin");

    let mut create = atlas(temp.path());
    create
        .args([
            "create",
            "--title",
            "Open stdin",
            "--type",
            "note",
            "--confidence",
            "high",
            "--summary",
            "Summary is provided",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = create.spawn().expect("command should spawn");
    let stdin = child.stdin.take().expect("stdin should be piped");
    let deadline = Instant::now() + Duration::from_secs(1);

    loop {
        if child
            .try_wait()
            .expect("child status should be readable")
            .is_some()
        {
            drop(stdin);
            let output = child.wait_with_output().expect("output should be readable");
            assert_success(&output);
            let created: Value =
                serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
            assert_eq!(created["created"], true);
            return;
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            drop(stdin);
            let output = child.wait_with_output().expect("output should be readable");
            panic!(
                "atlas create did not exit while stdin stayed open\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn update_uses_positional_id_and_preserves_omitted_fields() {
    let temp = TempDir::new("update-by-id");

    let mut create = atlas(temp.path());
    create.args([
        "create",
        "--title",
        "Original",
        "--type",
        "note",
        "--confidence",
        "medium",
        "--summary",
        "Original summary",
        "--tag",
        "original",
        "--source",
        "src/original.rs",
    ]);
    let created = run_json(create);
    let id = created["id"].as_str().expect("id should be a string");

    let mut target = atlas(temp.path());
    target.args([
        "create",
        "--title",
        "Link target",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Target summary",
    ]);
    let target = run_json(target);
    let target_id = target["id"].as_str().expect("id should be a string");

    let mut link = atlas(temp.path());
    link.args(["link", id, target_id]);
    run_json(link);

    let mut update = atlas(temp.path());
    update.args(["update", id, "--summary", "Updated summary"]);
    let updated = run_json(update);
    assert_eq!(updated["created"], false);
    assert_eq!(updated["id"], id);

    let mut get = atlas(temp.path());
    get.args(["get", id]);
    let atom = run_json(get);
    assert_eq!(atom["title"], "Original");
    assert_eq!(atom["summary"], "Updated summary");
    assert_eq!(atom["type"], "note");
    assert_eq!(atom["tags"], serde_json::json!(["original"]));
    assert_eq!(atom["sources"], serde_json::json!(["src/original.rs"]));
    let target_link = target_id
        .rsplit('/')
        .next()
        .expect("target ID should include an atom ID");
    assert_eq!(atom["links"], serde_json::json!([target_link]));
}

#[test]
fn update_clears_multiple_fields_with_clear_values() {
    let temp = TempDir::new("update-clear-values");

    let mut create = atlas(temp.path());
    create.args([
        "create",
        "--title",
        "Clearable",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Summary",
        "--details",
        "Details",
        "--tag",
        "one",
        "--source",
        "src/one.rs",
    ]);
    let created = run_json(create);
    let id = created["id"].as_str().expect("id should be a string");

    let mut update = atlas(temp.path());
    update.args([
        "update", "--clear", "details", "--clear", "tags", "--clear", "sources", id,
    ]);
    run_json(update);

    let mut get = atlas(temp.path());
    get.args(["get", id]);
    let atom = run_json(get);
    assert!(atom.get("details").is_none());
    assert!(atom.get("tags").is_none());
    assert!(atom.get("sources").is_none());
}

#[test]
fn enable_local_uses_normal_context_and_a_command_specific_root() {
    let temp = TempDir::new("enable-local-context");
    let root = temp.path().join("repo");
    std::fs::create_dir_all(&root).expect("project root should be created");

    let mut enable = atlas(temp.path());
    enable.args([
        "enable-local",
        "--root",
        root.to_str().expect("project root should be UTF-8"),
    ]);
    let result = run_json(enable);

    assert_eq!(result["path"], root.join(".atlas").to_str().unwrap());
    assert!(root.join(".atlas/index.yaml").exists());
}

#[test]
fn create_rejects_fallback_context() {
    let storage = TempDir::new("fallback-storage");
    let cwd = TempDir::new("fallback-cwd");
    let output = Command::new(env!("CARGO_BIN_EXE_atlas"))
        .args([
            "--storage",
            storage
                .path()
                .to_str()
                .expect("storage path should be UTF-8"),
            "create",
            "--title",
            "Fallback write",
            "--type",
            "note",
            "--confidence",
            "high",
            "--summary",
            "Must require explicit context",
        ])
        .current_dir(cwd.path())
        .output()
        .expect("command should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fallback"));
}

#[test]
fn search_defaults_to_the_current_project() {
    let temp = TempDir::new("project-search");

    let mut current = atlas(temp.path());
    current.args([
        "create",
        "--title",
        "Shared query current",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Shared query",
    ]);
    run_json(current);

    let mut other = Command::new(env!("CARGO_BIN_EXE_atlas"));
    other.args([
        "--storage",
        temp.path().to_str().expect("storage path should be UTF-8"),
        "--org",
        "acme",
        "--project",
        "other",
        "--format",
        "json",
        "create",
        "--title",
        "Shared query other",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Shared query",
    ]);
    run_json(other);

    let mut search = atlas(temp.path());
    search.args(["search", "Shared query"]);
    let results = run_json(search);

    assert_eq!(results["total"], 1);
    assert_eq!(results["results"][0]["id"], "acme/atlas/K-000001");
}

#[test]
fn delete_rejects_atoms_with_inbound_links_without_force() {
    let temp = TempDir::new("delete-inbound-links");

    let mut source = atlas(temp.path());
    source.args([
        "create",
        "--title",
        "Source",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Source summary",
    ]);
    let source = run_json(source);

    let mut target = atlas(temp.path());
    target.args([
        "create",
        "--title",
        "Target",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Target summary",
    ]);
    let target = run_json(target);

    let source_id = source["id"].as_str().expect("source ID should be a string");
    let target_id = target["id"].as_str().expect("target ID should be a string");
    let mut link = atlas(temp.path());
    link.args(["link", source_id, target_id]);
    run_json(link);

    let mut delete = atlas(temp.path());
    delete.args(["delete", target_id]);
    let output = delete.output().expect("command should run");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inbound links"));

    let mut force_delete = atlas(temp.path());
    force_delete.args(["delete", "--force", target_id]);
    let result = run_json(force_delete);
    assert_eq!(result["deleted"], true);
}

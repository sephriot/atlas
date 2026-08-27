use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn atlas_in(storage: &Path, project: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_atlas"));
    cmd.arg("--storage")
        .arg(storage)
        .arg("--org")
        .arg("acme")
        .arg("--project")
        .arg(project);
    cmd
}

fn run(cmd: Command) -> Output {
    let mut cmd = cmd;
    cmd.output().expect("command should run")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn note(storage: &Path, project: &str, title: &str) {
    let mut create = atlas_in(storage, project);
    create.args([
        "--format",
        "json",
        "create",
        "--title",
        title,
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        title,
    ]);
    let output = run(create);
    assert_success(&output);
}

fn index_yaml(storage: &Path, project: &str) -> PathBuf {
    storage
        .join("orgs")
        .join("acme")
        .join(project)
        .join("index.yaml")
}

#[test]
fn index_prints_text_lines_for_the_current_project() {
    let temp = TempDir::new("index-text");
    note(temp.path(), "atlas", "per-tenant token bucket");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index"]);
    let output = run(index);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "K-000001 — per-tenant token bucket (note, high)\n"
    );
}

#[test]
fn index_prints_json_array_when_format_is_json() {
    let temp = TempDir::new("index-json");
    note(temp.path(), "atlas", "per-tenant token bucket");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index", "--format", "json"]);
    let output = run(index);
    assert_success(&output);

    let listed: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(
        listed,
        serde_json::json!([{
            "id": "K-000001",
            "title": "per-tenant token bucket",
            "type": "note",
            "confidence": "high",
        }])
    );
}

#[test]
fn index_org_scope_prefixes_project_on_cross_project_lines() {
    let temp = TempDir::new("index-org-scope");
    note(temp.path(), "atlas", "Local inventory");
    note(temp.path(), "other", "Sibling inventory");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index", "--scope", "acme"]);
    let output = run(index);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "atlas/K-000001 — Local inventory (note, high)\nother/K-000001 — Sibling inventory (note, high)\n"
    );
}

#[test]
fn index_project_scope_stays_unprefixed() {
    let temp = TempDir::new("index-project-scope");
    note(temp.path(), "atlas", "Local inventory");
    note(temp.path(), "other", "Sibling inventory");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index", "--scope", "acme/other"]);
    let output = run(index);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "K-000001 — Sibling inventory (note, high)\n"
    );
}

#[test]
fn index_empty_scope_exits_zero_with_no_output() {
    let temp = TempDir::new("index-empty");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index", "--scope", "acme/missing"]);
    let output = run(index);
    assert_success(&output);
    assert!(
        output.stdout.is_empty(),
        "empty scope should print nothing, got:\n{}",
        stdout(&output)
    );
}

#[test]
fn index_skips_unreadable_indexes() {
    let temp = TempDir::new("index-orphan");
    note(temp.path(), "atlas", "Readable inventory");
    note(temp.path(), "other", "Orphan inventory");
    std::fs::write(index_yaml(temp.path(), "other"), "not: yaml: [[[")
        .expect("corrupt index should be writable");

    let mut index = atlas_in(temp.path(), "atlas");
    index.args(["index", "--scope", "acme"]);
    let output = run(index);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "atlas/K-000001 — Readable inventory (note, high)\n"
    );
}

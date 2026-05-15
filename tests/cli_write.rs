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
fn update_requires_id_and_updates_that_atom() {
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
    ]);
    let created = run_json(create);
    let id = created["id"].as_str().expect("id should be a string");

    let mut missing_id = atlas(temp.path());
    missing_id.args([
        "update",
        "--title",
        "No ID",
        "--type",
        "note",
        "--confidence",
        "medium",
        "--summary",
        "Should fail",
    ]);
    let output = missing_id.output().expect("command should run");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--id"));

    let mut update = atlas(temp.path());
    update.args([
        "update",
        "--id",
        id,
        "--title",
        "Updated",
        "--type",
        "gotcha",
        "--confidence",
        "high",
        "--summary",
        "Updated summary",
    ]);
    let updated = run_json(update);
    assert_eq!(updated["created"], false);
    assert_eq!(updated["id"], id);

    let mut get = atlas(temp.path());
    get.args(["get", id]);
    let atom = run_json(get);
    assert_eq!(atom["title"], "Updated");
    assert_eq!(atom["summary"], "Updated summary");
    assert_eq!(atom["type"], "gotcha");
}

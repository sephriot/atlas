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
    atlas_in(storage, "atlas")
}

fn atlas_in(storage: &Path, project: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_atlas"));
    cmd.arg("--storage")
        .arg(storage)
        .arg("--org")
        .arg("acme")
        .arg("--project")
        .arg(project)
        .arg("--format")
        .arg("json");
    cmd
}

fn note(storage: &Path, project: &str, title: &str) -> String {
    let mut create = atlas_in(storage, project);
    create.args([
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
    run_json(create)["id"]
        .as_str()
        .expect("id should be a string")
        .to_string()
}

fn links_of(storage: &Path, id: &str) -> Value {
    let mut get = atlas(storage);
    get.args(["get", id]);
    run_json(get)
        .get("links")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]))
}

/// Reproduce a pre-symmetry edge by dropping one atom's half of it.
fn drop_links(storage: &Path, id: &str) {
    let parts: Vec<&str> = id.split('/').collect();
    let path = storage
        .join("orgs")
        .join(parts[0])
        .join(parts[1])
        .join("atoms")
        .join(format!("{}.yaml", parts[2]));
    let text = std::fs::read_to_string(&path).expect("atom file should be readable");
    let mut atom: serde_yaml::Value = serde_yaml::from_str(&text).expect("atom should be YAML");
    atom.as_mapping_mut()
        .expect("atom should be a mapping")
        .remove("links");
    std::fs::write(
        &path,
        serde_yaml::to_string(&atom).expect("atom should serialize"),
    )
    .expect("atom file should be writable");
}

fn run_json(mut cmd: Command) -> Value {
    let output = cmd.output().expect("command should run");
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

fn run_failing(mut cmd: Command) -> Output {
    let output = cmd.output().expect("command should run");
    assert!(
        !output.status.success(),
        "command should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
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
fn search_spans_the_org_and_ranks_the_current_project_first() {
    let temp = TempDir::new("project-search");
    note(temp.path(), "atlas", "Shared query current");
    note(temp.path(), "other", "Shared query other");

    let mut search = atlas(temp.path());
    search.args(["search", "Shared query"]);
    let results = run_json(search);

    assert_eq!(results["total"], 2);
    assert_eq!(results["results"][0]["id"], "acme/atlas/K-000001");
    assert_eq!(results["results"][1]["id"], "acme/other/K-000001");
    let local = results["results"][0]["score"].as_f64().expect("score");
    let sibling = results["results"][1]["score"].as_f64().expect("score");
    assert!(
        sibling < local,
        "sibling project should rank below the current one: {sibling} vs {local}"
    );
}

#[test]
fn search_narrows_to_one_project_when_the_scope_names_it() {
    let temp = TempDir::new("project-search-scoped");
    note(temp.path(), "atlas", "Shared query current");
    note(temp.path(), "other", "Shared query other");

    let mut search = atlas(temp.path());
    search.args(["search", "Shared query", "--scope", "acme/other"]);
    let results = run_json(search);

    assert_eq!(results["total"], 1);
    assert_eq!(results["results"][0]["id"], "acme/other/K-000001");
}

#[test]
fn atoms_lists_the_named_org_rather_than_the_current_project() {
    let temp = TempDir::new("atoms-org-scope");
    note(temp.path(), "atlas", "Local inventory");
    note(temp.path(), "other", "Sibling inventory");

    let mut local = atlas(temp.path());
    local.args(["atoms"]);
    assert_eq!(
        run_json(local),
        serde_json::json!([{
            "id": "acme/atlas/K-000001",
            "title": "Local inventory",
            "type": "note",
            "confidence": "high",
            "tags": [],
        }])
    );

    let mut org = atlas(temp.path());
    org.args(["atoms", "--scope", "acme"]);
    let listed = run_json(org);
    let ids: Vec<&str> = listed
        .as_array()
        .expect("listing should be an array")
        .iter()
        .map(|a| a["id"].as_str().expect("id should be a string"))
        .collect();
    assert_eq!(ids, vec!["acme/atlas/K-000001", "acme/other/K-000001"]);
}

#[test]
fn create_links_the_new_atom_to_every_named_peer() {
    let temp = TempDir::new("create-with-links");
    let local = note(temp.path(), "atlas", "Existing local");
    let sibling = note(temp.path(), "backend", "Existing sibling");

    let mut create = atlas_in(temp.path(), "atlas");
    create.args([
        "create",
        "--title",
        "Recorded with its edges",
        "--type",
        "decision",
        "--confidence",
        "high",
        "--summary",
        "Linked as it was written",
        "--link",
        &local,
        "--link",
        "backend/K-000001",
    ]);
    let created = run_json(create);
    assert_eq!(
        created["links"],
        serde_json::json!([local.clone(), sibling.clone()])
    );

    let new_id = created["id"].as_str().expect("id should be a string");
    assert_eq!(
        links_of(temp.path(), new_id),
        serde_json::json!(["K-000001", "backend/K-000001"])
    );
    assert_eq!(
        links_of(temp.path(), &local),
        serde_json::json!(["K-000002"])
    );
    assert_eq!(
        links_of(temp.path(), &sibling),
        serde_json::json!(["atlas/K-000002"])
    );
}

#[test]
fn update_reports_the_edges_the_rewritten_atom_still_holds() {
    let temp = TempDir::new("update-echoes-edges");
    let subject = note(temp.path(), "atlas", "Subject");
    let local = note(temp.path(), "atlas", "Local peer");
    let sibling = note(temp.path(), "backend", "Sibling peer");

    for peer in [&local, &sibling] {
        let mut link = atlas(temp.path());
        link.args(["link", &subject, peer]);
        run_json(link);
    }

    let mut update = atlas(temp.path());
    update.args(["update", &subject, "--summary", "Means something else now"]);
    let updated = run_json(update);
    assert_eq!(updated["created"], false);
    assert_eq!(
        updated["links"],
        serde_json::json!([local.clone(), sibling.clone()]),
        "an update must surface the edges that may no longer fit the new text"
    );

    let mut unlinked = atlas(temp.path());
    unlinked.args(["unlink", &subject, &sibling]);
    run_json(unlinked);

    let mut again = atlas(temp.path());
    again.args(["update", &subject, "--summary", "Narrower still"]);
    assert_eq!(run_json(again)["links"], serde_json::json!([local]));
}

#[test]
fn update_of_an_unlinked_atom_reports_no_edges() {
    let temp = TempDir::new("update-no-edges");
    let subject = note(temp.path(), "atlas", "Subject");

    let mut update = atlas(temp.path());
    update.args(["update", &subject, "--summary", "Still standing alone"]);
    assert!(run_json(update).get("links").is_none());
}

#[test]
fn create_writes_nothing_when_a_named_peer_does_not_exist() {
    let temp = TempDir::new("create-bad-link");
    note(temp.path(), "atlas", "Existing local");

    let mut create = atlas_in(temp.path(), "atlas");
    create.args([
        "create",
        "--title",
        "Should not survive",
        "--type",
        "note",
        "--confidence",
        "high",
        "--summary",
        "Names a peer that is not there",
        "--link",
        "K-000404",
    ]);
    let output = create.output().expect("command should run");
    assert!(!output.status.success());

    let mut list = atlas(temp.path());
    list.args(["atoms"]);
    let listed = run_json(list);
    assert_eq!(
        listed.as_array().expect("listing should be an array").len(),
        1,
        "the failed create must not leave an atom behind"
    );
}

#[test]
fn link_writes_both_halves_within_one_project() {
    let temp = TempDir::new("link-same-project");
    let source = note(temp.path(), "atlas", "Source");
    let target = note(temp.path(), "atlas", "Target");

    let mut link = atlas(temp.path());
    link.args(["link", &source, &target]);
    assert_eq!(run_json(link)["created"], true);

    assert_eq!(
        links_of(temp.path(), &source),
        serde_json::json!(["K-000002"])
    );
    assert_eq!(
        links_of(temp.path(), &target),
        serde_json::json!(["K-000001"])
    );
}

#[test]
fn link_writes_both_halves_across_projects() {
    let temp = TempDir::new("link-cross-project");
    let source = note(temp.path(), "atlas", "Source");
    let target = note(temp.path(), "backend", "Target");

    let mut link = atlas(temp.path());
    link.args(["link", &source, &target]);
    run_json(link);

    assert_eq!(
        links_of(temp.path(), &source),
        serde_json::json!(["backend/K-000001"])
    );
    assert_eq!(
        links_of(temp.path(), &target),
        serde_json::json!(["atlas/K-000001"])
    );
}

#[test]
fn link_is_idempotent_and_restores_a_missing_half() {
    let temp = TempDir::new("link-idempotent");
    let source = note(temp.path(), "atlas", "Source");
    let target = note(temp.path(), "atlas", "Target");

    let mut first = atlas(temp.path());
    first.args(["link", &source, &target]);
    run_json(first);

    let mut again = atlas(temp.path());
    again.args(["link", &source, &target]);
    assert_eq!(run_json(again)["created"], false);
    assert_eq!(
        links_of(temp.path(), &source),
        serde_json::json!(["K-000002"])
    );

    drop_links(temp.path(), &target);
    let mut heal = atlas(temp.path());
    heal.args(["link", &source, &target]);
    assert_eq!(run_json(heal)["created"], true);
    assert_eq!(
        links_of(temp.path(), &target),
        serde_json::json!(["K-000001"])
    );
}

#[test]
fn unlink_clears_both_halves() {
    let temp = TempDir::new("unlink-both-halves");
    let source = note(temp.path(), "atlas", "Source");
    let target = note(temp.path(), "backend", "Target");

    let mut link = atlas(temp.path());
    link.args(["link", &source, &target]);
    run_json(link);

    let mut unlink = atlas(temp.path());
    unlink.args(["unlink", &source, &target]);
    assert_eq!(run_json(unlink)["removed"], true);

    assert_eq!(links_of(temp.path(), &source), serde_json::json!([]));
    assert_eq!(links_of(temp.path(), &target), serde_json::json!([]));
}

#[test]
fn link_rejects_an_atom_linked_to_itself() {
    let temp = TempDir::new("link-self");
    let source = note(temp.path(), "atlas", "Source");

    let mut link = atlas(temp.path());
    link.args(["link", &source, &source]);
    let output = link.output().expect("command should run");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("itself"));
}

#[test]
fn delete_detaches_every_atom_that_referenced_it() {
    let temp = TempDir::new("delete-detaches-peers");
    let neighbour = note(temp.path(), "atlas", "Neighbour");
    let elsewhere = note(temp.path(), "backend", "Elsewhere");
    let doomed = note(temp.path(), "atlas", "Doomed");

    for peer in [&neighbour, &elsewhere] {
        let mut link = atlas(temp.path());
        link.args(["link", peer, &doomed]);
        run_json(link);
    }

    let mut delete = atlas(temp.path());
    delete.args(["delete", &doomed]);
    let result = run_json(delete);
    assert_eq!(result["deleted"], true);
    assert_eq!(
        result["detached"],
        serde_json::json!([neighbour.clone(), elsewhere.clone()])
    );

    assert_eq!(links_of(temp.path(), &neighbour), serde_json::json!([]));
    assert_eq!(links_of(temp.path(), &elsewhere), serde_json::json!([]));
}

#[test]
fn telemetry_is_enabled_by_default_and_configuration_is_idempotent() {
    let temp = TempDir::new("telemetry-configuration");

    let mut status = atlas(temp.path());
    status.args(["telemetry", "status"]);
    let status = run_json(status);
    assert_eq!(status["enabled"], true);
    assert_eq!(
        status["events_path"],
        temp.path().join("telemetry/events.jsonl").to_str().unwrap()
    );

    for _ in 0..2 {
        let mut disable = atlas(temp.path());
        disable.args(["telemetry", "disable"]);
        assert_eq!(run_json(disable)["enabled"], false);
    }

    for _ in 0..2 {
        let mut enable = atlas(temp.path());
        enable.args(["telemetry", "enable"]);
        assert_eq!(run_json(enable)["enabled"], true);
    }
}

#[test]
fn search_and_feedback_create_redacted_local_events() {
    let temp = TempDir::new("telemetry-feedback");
    let id = note(temp.path(), "atlas", "Telemetry result");

    let mut search = atlas(temp.path());
    search.args(["search", "private search phrase"]);
    let search = run_json(search);
    let search_id = search["search_id"]
        .as_str()
        .expect("search should expose a correlation ID");

    let mut feedback = atlas(temp.path());
    feedback.args([
        "feedback",
        search_id,
        "--result",
        &id,
        "--verdict",
        "helpful",
    ]);
    assert_eq!(run_json(feedback)["recorded"], true);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains("search"));
    assert!(events.contains("feedback"));
    assert!(events.contains(search_id));
    assert!(events.contains(&id));
    assert!(!events.contains("private search phrase"));
}

#[test]
fn get_records_the_atom_reference_without_its_contents() {
    let temp = TempDir::new("telemetry-get");
    let id = note(temp.path(), "atlas", "Sensitive title");

    let mut get = atlas(temp.path());
    get.args(["get", &id]);
    run_json(get);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains("get"));
    assert!(events.contains(&id));
    assert!(!events.contains("Sensitive title"));
}

#[test]
fn telemetry_clear_removes_the_local_journal() {
    let temp = TempDir::new("telemetry-clear");
    note(temp.path(), "atlas", "A searchable atom");

    let mut search = atlas(temp.path());
    search.args(["search", "searchable"]);
    run_json(search);
    assert!(temp.path().join("telemetry/events.jsonl").exists());

    let mut clear = atlas(temp.path());
    clear.args(["telemetry", "clear"]);
    assert_eq!(run_json(clear)["cleared"], true);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());

    let mut clear_again = atlas(temp.path());
    clear_again.args(["telemetry", "clear"]);
    assert_eq!(run_json(clear_again)["cleared"], false);
}

#[test]
fn disabled_telemetry_does_not_record_retrieval_or_feedback() {
    let temp = TempDir::new("telemetry-disabled");

    let mut disable = atlas(temp.path());
    disable.args(["telemetry", "disable"]);
    run_json(disable);

    let id = note(temp.path(), "atlas", "Searchable atom");

    let mut search = atlas(temp.path());
    search.args(["search", "searchable"]);
    let search = run_json(search);
    assert!(search.get("search_id").is_none());

    let mut get = atlas(temp.path());
    get.args(["get", &id]);
    run_json(get);

    let mut feedback = atlas(temp.path());
    feedback.args([
        "feedback",
        "S-local",
        "--result",
        &id,
        "--verdict",
        "helpful",
    ]);
    assert_eq!(run_json(feedback)["recorded"], false);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());
}

#[test]
fn feedback_accepts_a_result_without_a_search_id() {
    let temp = TempDir::new("telemetry-feedback-without-search");
    let id = note(temp.path(), "atlas", "Feedback target");

    let mut feedback = atlas(temp.path());
    feedback.args(["feedback", "--result", &id, "--verdict", "misleading"]);
    assert_eq!(run_json(feedback)["recorded"], true);
}

#[test]
fn telemetry_metrics_accumulate_and_clear_with_the_journal() {
    let temp = TempDir::new("telemetry-metrics");
    let id = note(temp.path(), "atlas", "Telemetry metric result");

    let mut search = atlas(temp.path());
    search.args(["search", "telemetry"]);
    run_json(search);

    let mut get = atlas(temp.path());
    get.args(["get", &id]);
    run_json(get);

    let mut feedback = atlas(temp.path());
    feedback.args(["feedback", "--result", &id, "--verdict", "helpful"]);
    run_json(feedback);

    let mut metrics = atlas(temp.path());
    metrics.args(["telemetry", "metrics"]);
    let metrics = run_json(metrics);
    assert_eq!(metrics["all_time"]["searches"], 1);
    assert_eq!(metrics["all_time"]["results_matched"], 1);
    assert_eq!(metrics["all_time"]["gets"], 1);
    assert_eq!(metrics["all_time"]["helpful_feedback"], 1);

    let mut clear = atlas(temp.path());
    clear.args(["telemetry", "clear"]);
    run_json(clear);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());
    assert!(!temp.path().join("telemetry/metrics.yaml").exists());
}

#[test]
fn index_and_context_record_command_events() {
    let temp = TempDir::new("telemetry-command-index-context");
    note(temp.path(), "atlas", "Indexable atom");

    let mut index = atlas(temp.path());
    index.args(["index"]);
    run_json(index);

    let mut context = atlas(temp.path());
    context.args(["context"]);
    run_json(context);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains(r#""kind":"command""#));
    assert!(events.contains(r#""command":"index""#));
    assert!(events.contains(r#""command":"context""#));
    assert!(events.contains(r#""ok":true"#));

    let mut metrics = atlas(temp.path());
    metrics.args(["telemetry", "metrics"]);
    let metrics = run_json(metrics);
    assert_eq!(metrics["all_time"]["commands"]["index"], 1);
    assert_eq!(metrics["all_time"]["commands"]["context"], 1);
}

#[test]
fn failed_command_records_error_kind_without_message() {
    let temp = TempDir::new("telemetry-command-error");
    note(temp.path(), "atlas", "Existing atom");

    let mut get = atlas(temp.path());
    get.args(["get", "K-999999"]);
    run_failing(get);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains(r#""ok":false"#));
    assert!(events.contains(r#""error_kind":"not_found""#));
    assert!(!events.contains("Not found:"));
    assert!(!events.contains("IO error"));

    let mut metrics = atlas(temp.path());
    metrics.args(["telemetry", "metrics"]);
    let metrics = run_json(metrics);
    assert_eq!(metrics["all_time"]["command_errors"], 1);
}

#[test]
fn command_event_includes_sanitized_source() {
    let temp = TempDir::new("telemetry-command-source");

    let mut context = atlas(temp.path());
    context.env("ATLAS_TELEMETRY_SOURCE", "hook:atlas-index");
    context.env("ATLAS_TELEMETRY_HOOK_EVENT", "SessionStart");
    context.args(["context"]);
    run_json(context);

    let mut context_invalid = atlas(temp.path());
    context_invalid.env("ATLAS_TELEMETRY_SOURCE", "bad source");
    context_invalid.args(["context"]);
    run_json(context_invalid);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains(r#""source":"hook:atlas-index""#));
    assert!(events.contains(r#""hook_event":"SessionStart""#));
    assert!(!events.contains("bad source"));
    assert_eq!(
        events.matches(r#""command":"context""#).count(),
        2,
        "both context invocations should record a command event"
    );
}

#[test]
fn telemetry_subcommands_do_not_write_command_events() {
    let temp = TempDir::new("telemetry-no-self-observe");

    let mut status = atlas(temp.path());
    status.args(["telemetry", "status"]);
    run_json(status);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());

    let mut search = atlas(temp.path());
    search.args(["search", "anything"]);
    run_json(search);
    assert!(temp.path().join("telemetry/events.jsonl").exists());

    let mut clear = atlas(temp.path());
    clear.args(["telemetry", "clear"]);
    run_json(clear);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());
    assert!(!temp.path().join("telemetry/metrics.yaml").exists());
}

#[test]
fn telemetry_hook_records_a_hook_event() {
    let temp = TempDir::new("telemetry-hook-event");

    let mut hook = atlas(temp.path());
    hook.args([
        "telemetry",
        "hook",
        "--name",
        "atlas-index",
        "--event",
        "SessionStart",
        "--outcome",
        "skipped",
        "--reason",
        "already_done",
    ]);
    assert_eq!(run_json(hook)["recorded"], true);

    let events = std::fs::read_to_string(temp.path().join("telemetry/events.jsonl"))
        .expect("telemetry journal should exist");
    assert!(events.contains(r#""kind":"hook""#));
    assert!(events.contains(r#""name":"atlas-index""#));
    assert!(events.contains(r#""outcome":"skipped""#));
    assert!(events.contains(r#""reason":"already_done""#));

    let mut metrics = atlas(temp.path());
    metrics.args(["telemetry", "metrics"]);
    let metrics = run_json(metrics);
    assert_eq!(metrics["all_time"]["hooks"]["atlas-index"], 1);

    let mut disable = atlas(temp.path());
    disable.args(["telemetry", "disable"]);
    run_json(disable);
    std::fs::remove_file(temp.path().join("telemetry/events.jsonl"))
        .expect("journal should be removable for the disabled check");

    let mut hook_disabled = atlas(temp.path());
    hook_disabled.args([
        "telemetry",
        "hook",
        "--name",
        "atlas-index",
        "--event",
        "SessionStart",
        "--outcome",
        "skipped",
        "--reason",
        "already_done",
    ]);
    assert_eq!(run_json(hook_disabled)["recorded"], false);
    assert!(!temp.path().join("telemetry/events.jsonl").exists());
}

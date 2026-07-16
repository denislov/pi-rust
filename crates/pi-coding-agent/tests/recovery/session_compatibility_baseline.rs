//! Supported historical session compatibility and recovery behavior.

use std::path::{Path, PathBuf};

use pi_coding_agent::api::runtime::{CodingAgentSession, CodingAgentSessionOptions};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/architecture-baseline-v1"
);

#[tokio::test]
async fn event_v2_without_session_sequence_remains_openable() {
    let fixture = Path::new(FIXTURES).join("session-v2-no-sequence");
    let (temp, session_dir) = copy_fixture(&fixture);
    let session = CodingAgentSession::open(
        CodingAgentSessionOptions::new()
            .with_session_log_root(temp.path())
            .with_session_path(&session_dir),
    )
    .await
    .expect("event-v2 session without explicit sequences should remain readable");

    assert_eq!(session.view().session_id, "sess_v2_no_sequence");
    let events = read_jsonl(&session_dir.join("events.jsonl"));
    assert_eq!(
        events.len(),
        3,
        "a complete legacy operation is not recovered"
    );
    assert!(
        events
            .iter()
            .all(|event| event.get("session_sequence").is_none()),
        "opening a complete legacy session must not rewrite historical events"
    );
}

#[tokio::test]
async fn event_v2_incomplete_operation_gets_one_recovery_record() {
    let fixture = Path::new(FIXTURES).join("session-v2-incomplete");
    let (temp, session_dir) = copy_fixture(&fixture);
    let options = CodingAgentSessionOptions::new()
        .with_session_log_root(temp.path())
        .with_session_path(&session_dir);

    let first = CodingAgentSession::open(options.clone())
        .await
        .expect("incomplete event-v2 session should recover");
    assert_eq!(first.view().session_id, "sess_v2_incomplete");
    drop(first);

    let second = CodingAgentSession::open(options)
        .await
        .expect("reopening a recovered session should be idempotent");
    assert_eq!(second.view().session_id, "sess_v2_incomplete");

    let events = read_jsonl(&session_dir.join("events.jsonl"));
    let recovery: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "operation.recovered")
        .collect();
    assert_eq!(recovery.len(), 1, "startup recovery must be idempotent");
    assert_eq!(recovery[0]["operation_id"], "op_incomplete");
    assert_eq!(recovery[0]["session_sequence"], 3);
}

fn copy_fixture(source: &Path) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("create fixture tempdir");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(source.join("session.json")).expect("read fixture manifest"),
    )
    .expect("fixture manifest should be valid JSON");
    let destination = temp.path().join(
        manifest["session_id"]
            .as_str()
            .expect("fixture manifest should contain a session id"),
    );
    std::fs::create_dir(&destination).expect("create copied fixture directory");
    for file in ["session.json", "events.jsonl"] {
        std::fs::copy(source.join(file), destination.join(file))
            .unwrap_or_else(|error| panic!("copy fixture file {file}: {error}"));
    }
    (temp, destination)
}

fn read_jsonl(path: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .expect("read fixture event log")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("fixture event should be valid JSON"))
        .collect()
}

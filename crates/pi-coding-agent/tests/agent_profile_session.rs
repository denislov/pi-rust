use pi_coding_agent::api::{CodingAgentSession, CodingAgentSessionOptions, ProfileId};
use tempfile::tempdir;

#[tokio::test]
async fn default_agent_profile_is_persisted_and_restored_from_manifest() {
    let temp = tempdir().unwrap();
    let options = CodingAgentSessionOptions::new()
        .with_session_id("sess_profile_default")
        .with_session_log_root(temp.path())
        .with_default_agent_profile_id("coder");

    let session = CodingAgentSession::create(options.clone()).await.unwrap();

    assert_eq!(session.view().default_agent_profile_id.as_str(), "coder");
    let manifest = read_manifest(temp.path(), "sess_profile_default");
    assert_eq!(manifest["default_agent_profile_id"], "coder");

    let reopened = CodingAgentSession::open(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_profile_default")
            .with_session_log_root(temp.path()),
    )
    .await
    .unwrap();

    assert_eq!(reopened.view().default_agent_profile_id.as_str(), "coder");
}

#[tokio::test]
async fn set_default_agent_profile_updates_manifest_and_emits_event() {
    let temp = tempdir().unwrap();
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_session_id("sess_profile_switch")
            .with_session_log_root(temp.path()),
    )
    .await
    .unwrap();
    let mut events = session.subscribe_product_events_public();

    assert_eq!(
        session.view().default_agent_profile_id,
        ProfileId::from("default")
    );

    session.set_default_agent_profile_id("reviewer").unwrap();

    assert_eq!(session.view().default_agent_profile_id.as_str(), "reviewer");
    let manifest = read_manifest(temp.path(), "sess_profile_switch");
    assert_eq!(manifest["default_agent_profile_id"], "reviewer");
    let event = events
        .try_recv()
        .unwrap()
        .expect("profile change should emit a public product event");
    assert_eq!(event.family, "Profile");
    assert_eq!(event.kind, "Profile(DefaultChanged)");
}

fn read_manifest(root: &std::path::Path, session_id: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(root.join(session_id).join("session.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

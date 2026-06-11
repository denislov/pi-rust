use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_coding_agent::interactive::test_harness::run_scripted_interactive_with_session_dir;

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

#[tokio::test]
async fn interactive_mode_appends_to_session() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("saved")]);
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "persist me\r")
        .await
        .unwrap();
    assert!(result.session_file.exists());
    let contents = std::fs::read_to_string(result.session_file).unwrap();
    assert!(contents.contains("persist me"));
    assert!(contents.contains("saved"));
}

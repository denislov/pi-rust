use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_ai::types::StopReason;
use pi_coding_agent::interactive::test_harness::{
    run_scripted_interactive_with_session_dir, run_scripted_interactive_with_session_dir_and_waits,
};

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

#[tokio::test]
async fn interactive_footer_updates_to_created_session_id_after_prompt() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("saved")]);
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "persist me\r")
        .await
        .unwrap();
    let files = jsonl_files(temp.path());
    assert_eq!(files.len(), 1);
    let session_text = std::fs::read_to_string(&files[0]).unwrap();
    let header = session_text
        .lines()
        .next()
        .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .expect("session header should be valid JSON");
    let session_id = header["id"]
        .as_str()
        .expect("session id should be present in header");
    let visible_session_prefix = &session_id[..13];

    let final_frame = result.rendered_lines.join("\n");
    assert!(
        final_frame.contains(&format!("session: {visible_session_prefix}")),
        "{final_frame}"
    );
    assert!(!final_frame.contains("session: session"), "{final_frame}");
}

#[tokio::test]
async fn interactive_mode_continues_same_session_across_prompts() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("first saved", StopReason::Stop),
        FauxProvider::text_call("second saved", StopReason::Stop),
    ]);

    run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("first prompt\r", "first saved"),
            ("second prompt\r", "second saved"),
        ],
    )
    .await
    .unwrap();

    let files = jsonl_files(temp.path());
    assert_eq!(files.len(), 1);
    let contents = std::fs::read_to_string(&files[0]).unwrap();
    assert!(contents.contains("first prompt"));
    assert!(contents.contains("first saved"));
    assert!(contents.contains("second prompt"));
    assert!(contents.contains("second saved"));
}

fn jsonl_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_jsonl_files(root, &mut files);
    files.sort();
    files
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_jsonl_files(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
}

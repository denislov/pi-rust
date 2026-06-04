use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::bash::bash_execute;
use tempfile::tempdir;

fn text(b: &[ContentBlock]) -> String {
    b.iter()
        .filter_map(|x| match x {
            ContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn captures_stdout() {
    let d = tempdir().unwrap();
    let r = bash_execute(d.path(), serde_json::json!({"command":"echo hello"}))
        .await
        .unwrap();
    assert!(text(&r).contains("hello"));
}

#[tokio::test]
async fn captures_stderr() {
    let d = tempdir().unwrap();
    let r = bash_execute(d.path(), serde_json::json!({"command":"echo oops 1>&2"}))
        .await
        .unwrap();
    assert!(text(&r).contains("oops"));
}

#[tokio::test]
async fn nonzero_exit_is_error() {
    let d = tempdir().unwrap();
    let e = bash_execute(d.path(), serde_json::json!({"command":"echo bad; exit 3"}))
        .await
        .unwrap_err();
    assert!(e.contains("bad"));
    assert!(e.contains("Command exited with code 3"));
}

#[tokio::test]
async fn timeout_errors() {
    let d = tempdir().unwrap();
    let e = bash_execute(
        d.path(),
        serde_json::json!({"command":"sleep 5","timeout":1}),
    )
    .await
    .unwrap_err();
    assert!(e.contains("Command timed out after 1 seconds"));
}

#[tokio::test]
async fn truncates_tail_output() {
    let d = tempdir().unwrap();
    let r = bash_execute(d.path(), serde_json::json!({"command":"seq 1 2005"}))
        .await
        .unwrap();
    let t = text(&r);
    assert!(t.contains("2005"));
    assert!(
        t.contains("[Output truncated: showing last 2000 of 2005 lines (50KB/2000-line limit).]"),
        "{t}"
    );
}

#[tokio::test]
async fn missing_cwd_errors() {
    let e = bash_execute(
        std::path::Path::new("/no/such/dir/xyz"),
        serde_json::json!({"command":"echo hi"}),
    )
    .await
    .unwrap_err();
    assert!(e.contains("Working directory does not exist"));
}

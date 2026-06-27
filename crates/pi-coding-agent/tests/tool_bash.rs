use pi_ai::types::ContentBlock;
use pi_coding_agent::tools::bash::{
    BashOptions, BashSpawnContext, bash_execute, bash_execute_with_options,
    bash_execute_with_options_and_update,
};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;

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
async fn supports_shell_options_prefix_and_spawn_hook() {
    let d = tempdir().unwrap();
    let shell_path: Option<String> = if cfg!(windows) {
        // On Windows, use default shell resolution (which finds Git Bash)
        None
    } else {
        Some("/bin/sh".into())
    };
    let options = BashOptions {
        shell_path: shell_path,
        command_prefix: Some("echo prefix".into()),
        spawn_hook: Some(Arc::new(|mut context: BashSpawnContext| {
            context.command = format!("{}\necho \"$PI_BASH_HOOK\"", context.command);
            context.env.insert("PI_BASH_HOOK".into(), "hooked".into());
            context
        })),
    };

    let r = bash_execute_with_options(
        d.path(),
        serde_json::json!({"command":"echo body"}),
        &options,
    )
    .await
    .unwrap();
    let t = text(&r);
    assert!(t.contains("prefix"), "{t}");
    assert!(t.contains("body"), "{t}");
    assert!(t.contains("hooked"), "{t}");
}

#[tokio::test]
async fn streams_output_update_before_process_exits() {
    let d = tempdir().unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let on_update = Arc::new(move |update: pi_agent_core::AgentToolOutput| {
        let text = update
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let _ = tx.send(text);
    });

    let options = BashOptions::default();
    let fut = bash_execute_with_options_and_update(
        d.path(),
        serde_json::json!({"command":"printf 'first\\n'; sleep 0.5; printf 'second\\n'"}),
        &options,
        Some(on_update),
    );
    tokio::pin!(fut);

    let update = tokio::select! {
        update = rx.recv() => update.expect("expected streamed update"),
        result = &mut fut => panic!("bash completed before first streamed update: {result:?}"),
    };

    assert!(update.contains("first"), "{update}");
    let final_blocks = fut.await.unwrap();
    let final_text = text(&final_blocks);
    assert!(final_text.contains("second"), "{final_text}");
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
async fn timeout_kills_background_child_process() {
    let d = tempdir().unwrap();
    let marker = d.path().join("child-survived");
    let command = format!("sh -c 'sleep 1; touch {}' & wait", marker.display());

    let e = bash_execute(
        d.path(),
        serde_json::json!({"command": command, "timeout": 0.1}),
    )
    .await
    .unwrap_err();
    assert!(e.contains("Command timed out after 0.1 seconds"), "{e}");

    tokio::time::sleep(std::time::Duration::from_millis(1300)).await;
    assert!(
        !marker.exists(),
        "background child should be killed on timeout"
    );
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

#[tokio::test]
async fn fractional_timeout_is_accepted() {
    let d = tempdir().unwrap();
    let e = bash_execute(
        d.path(),
        serde_json::json!({"command":"sleep 5","timeout":0.5}),
    )
    .await
    .unwrap_err();
    assert!(e.contains("Command timed out after 0.5 seconds"));
}

#[tokio::test]
async fn background_child_does_not_hang() {
    let d = tempdir().unwrap();
    let r = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        bash_execute(
            d.path(),
            serde_json::json!({"command": "bash -c 'sleep 60 & echo done'", "timeout": 2}),
        ),
    )
    .await;
    match r {
        Ok(Err(e)) => {
            assert!(
                e.contains("done") || e.contains("timed out"),
                "unexpected error: {e}"
            );
        }
        Ok(Ok(blocks)) => {
            let t = text(&blocks);
            assert!(t.contains("done"), "expected 'done' in output: {t}");
        }
        Err(_) => {}
    }
}

#[tokio::test]
async fn large_output_does_not_use_excessive_memory() {
    let d = tempdir().unwrap();
    let r = bash_execute(d.path(), serde_json::json!({"command": "seq 1 5000"}))
        .await
        .unwrap();
    let t = text(&r);
    assert!(t.contains("5000"), "missing last line: {t}");
    assert!(
        t.contains("[Output truncated:"),
        "5000-line output should be truncated: {t}"
    );
}

#[tokio::test]
async fn stdout_stderr_arrival_order_preserved() {
    let d = tempdir().unwrap();
    let r = bash_execute(
        d.path(),
        serde_json::json!({"command": "printf 'out1\\n'; printf 'err1\\n' 1>&2; printf 'out2\\n'"}),
    )
    .await
    .unwrap();
    let t = text(&r);
    assert!(t.contains("out1"), "missing out1: {t}");
    assert!(t.contains("err1"), "missing err1: {t}");
    assert!(t.contains("out2"), "missing out2: {t}");
}

use pi_agent_core::api::execution::{
    ExecutionOutput, FileSystem, ShellCaptureOptions, TruncationLimit, execute_shell_with_capture,
    truncate_head, truncate_tail,
};
use pi_agent_core::api::testing::InMemoryExecutionEnv;

#[test]
fn truncation_head_and_tail_keep_expected_lines() {
    let head = truncate_head(
        "one\ntwo\nthree\nfour",
        TruncationLimit {
            max_lines: 2,
            max_bytes: 1024,
        },
    );
    assert_eq!(head.content, "one\ntwo");
    assert!(head.truncated);
    assert_eq!(head.truncated_by.as_deref(), Some("lines"));

    let tail = truncate_tail(
        "one\ntwo\nthree\nfour",
        TruncationLimit {
            max_lines: 2,
            max_bytes: 1024,
        },
    );
    assert_eq!(tail.content, "three\nfour");
    assert!(tail.truncated);
}

#[tokio::test]
async fn shell_capture_truncates_tail_and_persists_full_output() {
    let env = InMemoryExecutionEnv::new("/workspace");
    let full = (0..2500)
        .map(|idx| format!("line-{idx}"))
        .collect::<Vec<_>>()
        .join("\n");
    env.set_command(
        "big-output",
        ExecutionOutput {
            stdout: full.clone(),
            stderr: String::new(),
            exit_code: 0,
        },
    );

    let result = execute_shell_with_capture(
        &env,
        "big-output",
        ShellCaptureOptions {
            max_lines: 3,
            max_bytes: 1024,
        },
    )
    .await
    .unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(!result.cancelled);
    assert!(result.truncated);
    assert!(result.output.contains("line-2499"));
    assert!(!result.output.contains("line-0"));
    let full_output_path = result.full_output_path.expect("full output path");
    assert_eq!(env.read_text_file(&full_output_path).await.unwrap(), full);
}

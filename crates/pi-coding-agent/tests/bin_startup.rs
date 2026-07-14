use std::process::Command;

#[test]
fn binary_lists_models_without_stack_overflow() {
    let output = Command::new(env!("CARGO_BIN_EXE_pi-coding-agent"))
        .args(["--list-models", "--provider", "anthropic"])
        .output()
        .expect("pi-coding-agent binary should run");

    assert!(
        output.status.success(),
        "expected binary to exit successfully\nstatus: {}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("provider"));
    assert!(stdout.contains("anthropic"));
}

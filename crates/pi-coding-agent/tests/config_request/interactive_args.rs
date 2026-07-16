//! Interactive-mode routing behavior.

use pi_coding_agent::api::cli::runner::run_cli_with_options;
use pi_coding_agent::api::cli::runtime::CliRunOptions;

#[tokio::test]
async fn default_invocation_routes_to_interactive_instead_of_unsupported_mode() {
    let output = run_cli_with_options(Vec::<String>::new(), CliRunOptions::default()).await;
    assert_ne!(output.stderr, "unsupported mode: interactive\n");
    assert_eq!(output.exit_code, 1);
    assert_eq!(output.stderr, "interactive mode requires a TTY\n");
}

#[tokio::test]
async fn print_mode_still_requires_prompt() {
    let output = run_cli_with_options(vec!["-p".to_string()], CliRunOptions::default()).await;
    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("missing prompt"));
}

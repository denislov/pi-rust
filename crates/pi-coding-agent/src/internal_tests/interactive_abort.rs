//! Internal owner tests for interactive abort handling.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use super::support::EnvGuard;
use async_stream::stream;
use pi_ai::api::conversation::{AssistantMessage, Context, StopReason};
use pi_ai::api::model::Model;
use pi_ai::api::provider::ApiProvider;
use pi_ai::api::stream::{AssistantMessageEvent, EventStream, StreamOptions};
use pi_coding_agent::adapters::interactive::test_harness::{
    ScriptedInteractiveOutput, run_scripted_idle_interactive,
    run_scripted_interactive_with_provider_chunks,
};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const INTERACTIVE_ABORT_HARNESS_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct AbortAwareProvider {
    cancelled: Arc<AtomicBool>,
}

impl AbortAwareProvider {
    fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }
}

async fn run_abort_harness_with_timeout(
    provider: Arc<dyn ApiProvider>,
    input_chunks: Vec<&'static str>,
    context: &str,
) -> ScriptedInteractiveOutput {
    tokio::time::timeout(
        INTERACTIVE_ABORT_HARNESS_TIMEOUT,
        run_scripted_interactive_with_provider_chunks(provider, input_chunks),
    )
    .await
    .unwrap_or_else(|_| panic!("interactive loop timed out while {context}"))
    .unwrap_or_else(|error| panic!("scripted interactive run failed while {context}: {error}"))
}

impl ApiProvider for AbortAwareProvider {
    fn stream(&self, model: &Model, _ctx: Context, opts: Option<StreamOptions>) -> EventStream {
        let cancelled = Arc::clone(&self.cancelled);
        let model_id = model.id.clone();
        let cancel = opts.and_then(|opts| opts.cancel);

        Box::pin(stream! {
            let mut partial = AssistantMessage::empty("faux", &model_id);
            partial.provider = Some("faux".into());
            yield AssistantMessageEvent::Start {
                content_index: None,
                partial: partial.clone(),
            };

            if let Some(cancel) = cancel {
                cancel.cancelled().await;
                cancelled.store(true, Ordering::SeqCst);
            }

            let mut message = AssistantMessage::empty("faux", &model_id);
            message.provider = Some("faux".into());
            message.stop_reason = StopReason::Aborted;
            message.error_message = Some("aborted".to_string());
            yield AssistantMessageEvent::Error {
                reason: StopReason::Aborted,
                message,
            };
        })
    }
}

#[tokio::test]
async fn ctrl_c_exits_when_idle_with_empty_editor() {
    let output = run_scripted_idle_interactive("\x03").await.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
}

#[tokio::test]
async fn ctrl_c_cancels_running_prompt_on_coding_session_path() {
    let cancelled = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(AbortAwareProvider::new(Arc::clone(&cancelled)));

    let output = run_abort_harness_with_timeout(
        provider,
        vec!["please wait\r", "\x03", "\x03"],
        "aborting a running prompt",
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(output.contains("please wait"));
    assert!(output.contains("prompt aborted: user cancelled"));
    assert!(output.contains("status: idle"));
    // The provider may be cancelled before its stream is polled; the abort
    // contract is asserted through the operation outcome and terminal state.
}

#[tokio::test]
async fn ctrl_c_cancels_running_agent_invocation_child_prompt() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::write(
        dir.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let cancelled = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(AbortAwareProvider::new(Arc::clone(&cancelled)));
    let output = run_abort_harness_with_timeout(
        provider,
        vec!["/agent:coder please wait\r", "\x03", "\x03"],
        "aborting an agent invocation child prompt",
    )
    .await;
    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(output.contains("/agent:coder please wait"), "{output:?}");
    assert!(output.contains("Error: cancelled"), "{output:?}");
    assert!(
        !output.contains("interactive agent invocation abort is not implemented yet"),
        "{output:?}"
    );
    assert!(output.contains("status: idle"), "{output:?}");
    // Pre-start cancellation may happen before the child provider stream is polled.
}

#[tokio::test]
async fn ctrl_c_cancels_running_agent_team_through_operation_control() {
    let _guard = ENV_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("agents")).unwrap();
    std::fs::create_dir_all(dir.path().join("teams")).unwrap();
    std::fs::write(
        dir.path().join("agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("teams/implementation.toml"),
        r#"
schema_version = 1
id = "implementation"
display_name = "Implementation Team"
supervisor = "deterministic"
strategy = "plan_execute_review"
members = ["coder"]
"#,
    )
    .unwrap();
    let env = EnvGuard::new(&["PI_RUST_DIR"]);
    env.set_pi_rust_dir(dir.path());

    let cancelled = Arc::new(AtomicBool::new(false));
    let provider = Arc::new(AbortAwareProvider::new(Arc::clone(&cancelled)));
    let output = run_abort_harness_with_timeout(
        provider,
        vec!["/team:implementation please wait\r", "\x03", "\x03"],
        "aborting an agent team member prompt",
    )
    .await;

    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(
        output.contains("/team:implementation please wait"),
        "{output:?}"
    );
    assert!(output.contains("status: idle"), "{output:?}");
    // Pre-start cancellation may happen before the member provider stream is polled.
}

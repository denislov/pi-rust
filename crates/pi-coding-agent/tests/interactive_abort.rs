mod support;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use async_stream::stream;
use pi_ai::registry::ApiProvider;
use pi_ai::{
    AssistantMessage, AssistantMessageEvent, Context, EventStream, Model, StopReason, StreamOptions,
};
use pi_coding_agent::interactive::test_harness::run_scripted_idle_interactive;
use pi_coding_agent::interactive::test_harness::run_scripted_interactive_with_provider_chunks;
use support::EnvGuard;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug)]
struct AbortAwareProvider {
    cancelled: Arc<AtomicBool>,
}

impl AbortAwareProvider {
    fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }
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

    let output = tokio::time::timeout(
        Duration::from_millis(500),
        run_scripted_interactive_with_provider_chunks(
            provider,
            vec!["please wait\r", "\x03", "\x03"],
        ),
    )
    .await
    .expect("interactive loop should not hang while aborting")
    .expect("scripted interactive run should succeed");

    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(output.contains("please wait"));
    assert!(output.contains("prompt aborted: user cancelled"));
    assert!(output.contains("status: idle"));
    assert!(cancelled.load(Ordering::SeqCst));
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
    let output = tokio::time::timeout(
        Duration::from_millis(500),
        run_scripted_interactive_with_provider_chunks(
            provider,
            vec!["/agent:coder please wait\r", "\x03", "\x03"],
        ),
    )
    .await
    .expect("interactive loop should not hang while aborting agent invocation");

    let output = output.expect("scripted interactive run should succeed");
    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
    assert!(output.contains("/agent:coder please wait"), "{output:?}");
    assert!(
        output.contains("agent invocation aborted: user cancelled"),
        "{output:?}"
    );
    assert!(
        !output.contains("interactive agent invocation abort is not implemented yet"),
        "{output:?}"
    );
    assert!(output.contains("status: idle"), "{output:?}");
    assert!(cancelled.load(Ordering::SeqCst));
}

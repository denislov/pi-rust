use crate::support;

use std::sync::Arc;

use pi_agent_core::api::resources::AgentResources;
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::testing::FauxProvider;
use pi_coding_agent::api::cli::runtime::{PromptInvocation, PromptRunOptions};
use pi_coding_agent::api::client::{
    CodingAgentClientId, CodingAgentSubmittedOperationStatus, CodingAgentSubmittedTerminalAnchor,
};
use pi_coding_agent::api::event::{
    CodingAgentProductEventKind, CodingAgentSessionProductEvent,
    CodingAgentSubmittedEventDurability,
};
use pi_coding_agent::api::operation::{
    CodingAgentOperation, CodingAgentOperationOutcome, PromptTurnOptions,
};
use pi_coding_agent::api::runtime::{CodingAgentSession, CodingAgentSessionOptions};
use support::ProviderGuard;

fn model(api: &str) -> Model {
    Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

fn options(api: &str, invocation: PromptInvocation) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: match &invocation {
            PromptInvocation::Text(text) => text.clone(),
            _ => String::new(),
        },
        model: model(api),
        api_key: None,
        auth_diagnostics: Vec::new(),
        system_prompt: Some("test".into()),
        max_turns: Some(2),
        tools: Vec::new(),
        register_builtins: false,
        ai_client: None,
        session: None,
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation,
    })
}

async fn seeded_compaction_session(
    api: &str,
    session_id: &str,
    root: &std::path::Path,
    ai_client: pi_ai::api::client::AiClient,
) -> CodingAgentSession {
    let mut session = CodingAgentSession::create(
        CodingAgentSessionOptions::new()
            .with_ai_client(ai_client)
            .with_session_id(session_id)
            .with_session_log_root(root),
    )
    .await
    .unwrap();
    let outcome = session
        .run(CodingAgentOperation::Prompt(options(
            api,
            PromptInvocation::Text("seed question".into()),
        )))
        .await
        .unwrap();
    assert!(matches!(outcome, CodingAgentOperationOutcome::Prompt(_)));
    session
}

#[tokio::test]
async fn terminal_association_uses_the_exact_compact_root_event() {
    let api = "operation-association-compact";
    let _provider = ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![
            FauxProvider::text_call("seed answer", pi_ai::api::conversation::StopReason::Stop),
            FauxProvider::text_call(
                "compact summary",
                pi_ai::api::conversation::StopReason::Stop,
            ),
        ])),
    );
    let temp = tempfile::tempdir().unwrap();
    let mut session =
        seeded_compaction_session(api, "sess_association", temp.path(), _provider.ai_client())
            .await;
    let connection = session
        .connect(CodingAgentClientId::new("association-client"))
        .unwrap();
    let operation = CodingAgentOperation::Compact(options(
        api,
        PromptInvocation::Compact {
            custom_instructions: None,
        },
    ));
    let lease = connection
        .prepare_submission(
            &mut session,
            pi_coding_agent::api::client::CodingAgentDraftId("unused".into()),
            &operation,
        )
        .unwrap();

    assert!(matches!(
        session.run(operation).await.unwrap(),
        CodingAgentOperationOutcome::Compact(_)
    ));
    drop(lease);

    let submitted = connection
        .state()
        .unwrap()
        .submitted_operation
        .expect("compact terminal state");
    let sequence = match submitted.status {
        CodingAgentSubmittedOperationStatus::Terminal {
            anchor:
                CodingAgentSubmittedTerminalAnchor::ProductEvent {
                    sequence,
                    durability: CodingAgentSubmittedEventDurability::Durable,
                },
            ..
        } => sequence,
        other => panic!("unexpected compact terminal anchor: {other:?}"),
    };
    let pi_coding_agent::api::client::CodingAgentReconnect::Replayed { events, .. } =
        connection.reconnect(0).unwrap()
    else {
        panic!("compact events should be retained")
    };
    let matching = events
        .iter()
        .filter(|event| {
            event.sequence() == sequence
                && matches!(
                    event.event(),
                    CodingAgentProductEventKind::Session(
                        CodingAgentSessionProductEvent::CompactionCompleted { .. }
                    )
                )
        })
        .count();
    assert_eq!(matching, 1, "anchor must identify the one Compact root");
}

mod common;

use common::ProviderGuard;
use futures::StreamExt;
use pi_agent_core::{AgentHarness, AgentHarnessEvent, AgentHarnessHooks};
use pi_ai::providers::faux::{FauxCall, FauxProvider, FauxResponse};
use pi_ai::types::{Model, ModelCost, ModelInput, StopReason};
use std::sync::{Arc, Mutex};

fn faux_model(api: &str) -> Model {
    Model {
        id: "subscribe-faux".into(),
        name: "Subscribe Faux".into(),
        api: api.into(),
        provider: "faux".into(),
        base_url: "http://localhost".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 10_000,
        max_tokens: 1_000,
        headers: None,
        compat: None,
    }
}

fn register_simple_text(api: &str, text: &str) -> ProviderGuard {
    ProviderGuard::register(
        api,
        Arc::new(FauxProvider::with_call_queue(vec![FauxCall {
            responses: vec![FauxResponse {
                text_deltas: vec![text.into()],
                thinking_deltas: vec![],
                tool_calls: vec![],
            }],
            stop_reason: StopReason::Stop,
        }])),
    )
}

#[tokio::test]
async fn subscribe_observes_all_harness_events() {
    let api = "subscribe-all";
    let _provider_guard = register_simple_text(api, "hi");

    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    let captured: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_for_subscriber = captured.clone();
    let _guard = harness.subscribe(Arc::new(move |event: &AgentHarnessEvent| {
        let label: &'static str = match event {
            AgentHarnessEvent::BeforeAgentStart { .. } => "before_agent_start",
            AgentHarnessEvent::Context { .. } => "context",
            AgentHarnessEvent::BeforeProviderRequest { .. } => "before_provider_request",
            AgentHarnessEvent::Settled => "settled",
            _ => "other",
        };
        captured_for_subscriber.lock().unwrap().push(label);
    }));

    let _ = harness.prompt("hi").collect::<Vec<_>>().await;

    let labels = captured.lock().unwrap().clone();
    assert!(labels.contains(&"before_agent_start"));
    assert!(labels.contains(&"context"));
    assert!(labels.contains(&"before_provider_request"));
    assert!(labels.contains(&"settled"));
}

#[tokio::test]
async fn subscribe_guard_drop_removes_listener() {
    let api = "subscribe-drop";
    let _provider_guard = register_simple_text(api, "hi");

    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    let counter = Arc::new(Mutex::new(0usize));
    let counter_for_listener = counter.clone();
    let guard = harness.subscribe(Arc::new(move |_| {
        *counter_for_listener.lock().unwrap() += 1;
    }));
    drop(guard);

    let _ = harness.prompt("hi").collect::<Vec<_>>().await;
    assert_eq!(*counter.lock().unwrap(), 0);
}

#[tokio::test]
async fn on_context_appends_to_existing_hook_chain() {
    use pi_agent_core::on_kind::ContextKind;

    let api = "on-context";
    let _provider_guard = register_simple_text(api, "hi");

    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);

    let initial_seen = Arc::new(Mutex::new(false));
    let initial_seen_clone = initial_seen.clone();
    let hooks = AgentHarnessHooks {
        context: Some(Arc::new(move |ctx| {
            *initial_seen_clone.lock().unwrap() = true;
            Box::pin(async move { Ok(Some(ctx)) })
        })),
        ..Default::default()
    };
    let harness = AgentHarness::new(config).with_hooks(hooks);

    let on_seen = Arc::new(Mutex::new(false));
    let on_seen_clone = on_seen.clone();
    let _guard = harness.on::<ContextKind>(Arc::new(move |ctx| {
        *on_seen_clone.lock().unwrap() = true;
        Box::pin(async move { Ok(Some(ctx)) })
    }));

    let _ = harness.prompt("hi").collect::<Vec<_>>().await;

    assert!(*initial_seen.lock().unwrap(), "initial hook ran");
    assert!(*on_seen.lock().unwrap(), "on() handler ran");
}

#[tokio::test]
async fn phase_starts_idle_and_returns_to_idle_after_prompt() {
    use pi_agent_core::AgentHarnessPhase;

    let api = "phase-idle";
    let _provider_guard = register_simple_text(api, "ok");
    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    assert_eq!(harness.phase(), AgentHarnessPhase::Idle);
    let _events: Vec<_> = harness.prompt("hi").collect().await;
    assert_eq!(harness.phase(), AgentHarnessPhase::Idle);
}

#[tokio::test]
async fn phase_is_turn_during_active_prompt() {
    use pi_agent_core::AgentHarnessPhase;

    let api = "phase-turn";
    let _provider_guard = register_simple_text(api, "ok");
    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    let observed_phase: Arc<Mutex<Option<AgentHarnessPhase>>> = Arc::new(Mutex::new(None));
    let observed_for_hook = observed_phase.clone();
    let harness_clone = harness.clone();
    let _guard = harness.subscribe(Arc::new(move |event: &AgentHarnessEvent| {
        if matches!(event, AgentHarnessEvent::BeforeProviderRequest { .. }) {
            *observed_for_hook.lock().unwrap() = Some(harness_clone.phase());
        }
    }));

    let _events: Vec<_> = harness.prompt("hi").collect().await;
    assert_eq!(
        observed_phase.lock().unwrap().unwrap(),
        AgentHarnessPhase::Turn
    );
}

#[tokio::test]
async fn abort_returns_cleared_queue_messages() {
    use pi_agent_core::AbortResult;

    let api = "abort-queues";
    let _provider_guard = register_simple_text(api, "ok");
    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    harness.steer("steer1");
    harness.steer("steer2");
    harness.follow_up("followup1");

    let result: AbortResult = harness.abort();
    assert_eq!(result.cleared_steer.len(), 2);
    assert_eq!(result.cleared_follow_up.len(), 1);
    let texts: Vec<&str> = result
        .cleared_steer
        .iter()
        .filter_map(|m| match m {
            pi_agent_core::api::AgentMessage::UserText { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["steer1", "steer2"]);
}

#[tokio::test]
async fn abort_with_empty_queues_returns_empty_lists() {
    use pi_agent_core::AbortResult;

    let api = "abort-empty";
    let _provider_guard = register_simple_text(api, "ok");
    let mut config = common::agent_config(faux_model(api));
    config.max_turns = Some(1);
    let harness = AgentHarness::new(config);

    let result: AbortResult = harness.abort();
    assert!(result.cleared_steer.is_empty());
    assert!(result.cleared_follow_up.is_empty());
}

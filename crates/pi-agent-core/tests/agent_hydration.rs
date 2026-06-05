mod common;

use common::faux_model;
use futures::StreamExt;
use pi_agent_core::{Agent, AgentConfig, AgentEvent, AgentMessage};
use pi_ai::providers::faux::FauxProvider;
use pi_ai::registry;
use std::sync::Arc;

#[tokio::test]
async fn prompt_starts_after_hydrated_messages() {
    let api = "agent-hydration-history";
    registry::register(
        api,
        Arc::new(FauxProvider::new(vec![common::faux_text_turn(
            "second answer",
        )])),
    );
    let mut config = AgentConfig::new(faux_model(api));
    config.system_prompt = Some("system".into());
    config.max_turns = 5;
    let agent = Agent::with_messages(
        config,
        vec![AgentMessage::UserText {
            message_id: "entry001".into(),
            text: "first".into(),
        }],
    );
    let baseline = agent.messages().len();
    let mut stream = agent.prompt("second");
    while let Some(event) = stream.next().await {
        if matches!(event, AgentEvent::AgentError { .. }) {
            panic!("unexpected agent error");
        }
    }
    let messages = agent.messages();
    assert_eq!(baseline, 1);
    assert!(matches!(messages[0], AgentMessage::UserText { .. }));
    assert!(messages.len() >= 3);
    registry::unregister(api);
}

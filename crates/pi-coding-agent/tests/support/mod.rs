#![allow(deprecated)]

use std::ffi::{OsStr, OsString};
use std::sync::{Arc, Mutex, MutexGuard};

use pi_agent_core::api::resources::AgentResources;
use pi_agent_core::api::tool::AgentTool;
use pi_ai::api::client::AiClient;
use pi_ai::api::conversation::Usage;
use pi_ai::api::model::{Model, ModelCost, ModelInput};
use pi_ai::api::provider::ApiProvider;
use pi_coding_agent::api::event::{
    CodingAgentProductEvent, CodingAgentProductEventCheckOutput, CodingAgentProductEventDiagnostic,
    CodingAgentProductEventError, CodingAgentProductEventKind, CodingAgentProductEventReplacement,
    CodingAgentProductEventUsage,
};
use pi_coding_agent::api::operation::{
    PromptInvocation, PromptRunOptions, PromptTurnOptions, SelfHealingEditCheckOutput,
    SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use pi_coding_agent::api::runtime::{CodingSessionError, SessionRunOptions};
use serde_json::json;

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct EnvGuard<'a> {
    _lock: MutexGuard<'a, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

#[allow(dead_code)]
impl EnvGuard<'static> {
    pub fn new(names: &[&'static str]) -> Self {
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = names
            .iter()
            .map(|name| (*name, std::env::var_os(name)))
            .collect();
        Self { _lock: lock, saved }
    }

    pub fn with_pi_rust_dir<V: AsRef<OsStr>>(value: V) -> Self {
        let guard = Self::new(&["PI_RUST_DIR"]);
        guard.set_pi_rust_dir(value);
        guard
    }
}

#[allow(dead_code)]
impl EnvGuard<'_> {
    pub fn set<V: AsRef<OsStr>>(&self, name: &str, value: V) {
        unsafe {
            std::env::set_var(name, value);
        }
    }

    pub fn remove(&self, name: &str) {
        unsafe {
            std::env::remove_var(name);
        }
    }

    pub fn set_pi_rust_dir<V: AsRef<OsStr>>(&self, value: V) {
        self.set("PI_RUST_DIR", value);
    }
}

impl Drop for EnvGuard<'_> {
    fn drop(&mut self) {
        for (name, value) in self.saved.iter().rev() {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

pub struct ProviderGuard {
    ai_client: AiClient,
}

#[allow(dead_code)]
impl ProviderGuard {
    pub fn register(api: impl Into<String>, provider: Arc<dyn ApiProvider>) -> Self {
        Self::register_many(vec![(api.into(), provider)])
    }

    pub fn register_many(providers: Vec<(String, Arc<dyn ApiProvider>)>) -> Self {
        let ai_client = AiClient::new();
        for (api, provider) in providers {
            ai_client.register_provider(api, provider);
        }
        Self { ai_client }
    }

    pub fn ai_client(&self) -> AiClient {
        self.ai_client.clone()
    }
}

#[allow(dead_code)]
pub fn model(api: &str) -> Model {
    named_model("test-model", "Test Model", api)
}

#[allow(dead_code)]
pub fn fallback_model(api: &str) -> Model {
    named_model("fallback-model", "Fallback Model", api)
}

fn named_model(id: &str, name: &str, api: &str) -> Model {
    Model {
        id: id.into(),
        name: name.into(),
        api: api.into(),
        provider: "test".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}

#[allow(dead_code)]
pub fn prompt_options(
    cwd: &std::path::Path,
    api: &str,
    prompt: &str,
    tools: Vec<AgentTool>,
    max_turns: u32,
) -> PromptTurnOptions {
    PromptTurnOptions::from_prompt_run_options(PromptRunOptions {
        prompt: prompt.into(),
        model: fallback_model(api),
        api_key: None,
        auth_diagnostics: Vec::new(),
        system_prompt: Some("Runtime fallback instructions.".into()),
        max_turns: Some(max_turns),
        tools,
        register_builtins: false,
        ai_client: None,
        session: Some(SessionRunOptions::disabled(cwd.to_path_buf())),
        session_target: None,
        session_name: None,
        thinking_level: None,
        tool_execution: None,
        resources: AgentResources::default(),
        settings: None,
        invocation: PromptInvocation::Text(prompt.into()),
    })
}

#[allow(dead_code)]
pub fn product_event(event: CodingAgentProductEventKind) -> CodingAgentProductEvent {
    let delivery_class = match &event {
        CodingAgentProductEventKind::Workflow(
            pi_coding_agent::api::event::CodingAgentWorkflowProductEvent::OperationRecovered {
                ..
            },
        ) => "recovery",
        CodingAgentProductEventKind::Capability(_)
        | CodingAgentProductEventKind::Runtime(
            pi_coding_agent::api::event::CodingAgentRuntimeProductEvent::ShutDown,
        ) => "control",
        _ => "data",
    };
    serde_json::from_value(json!({
        "stream_id": "test-stream",
        "sequence": 1,
        "event": event,
        "operation_id": null,
        "terminal_status": null,
        "terminal_operation": null,
        "durability": { "state": "live_only" },
        "delivery_class": delivery_class,
    }))
    .expect("typed product-event fixture must deserialize")
}

#[allow(dead_code)]
pub fn product_usage(usage: Usage) -> CodingAgentProductEventUsage {
    usage.into()
}

#[allow(dead_code)]
pub fn product_error(error: CodingSessionError) -> CodingAgentProductEventError {
    error.into()
}

#[allow(dead_code)]
pub fn product_replacement(
    replacement: SelfHealingEditReplacement,
) -> CodingAgentProductEventReplacement {
    replacement.into()
}

#[allow(dead_code)]
pub fn product_diagnostic(
    diagnostic: SelfHealingEditDiagnostic,
) -> CodingAgentProductEventDiagnostic {
    diagnostic.into()
}

#[allow(dead_code)]
pub fn product_check_output(
    output: SelfHealingEditCheckOutput,
) -> CodingAgentProductEventCheckOutput {
    output.into()
}

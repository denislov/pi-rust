#![allow(deprecated)]

use std::ffi::{OsStr, OsString};
use std::sync::{Arc, Mutex, MutexGuard};

use pi_ai::api::AiClient;
use pi_ai::api::ApiProvider;
use pi_ai::api::Usage;
use pi_coding_agent::api::{
    CodingAgentProductEvent, CodingAgentProductEventCheckOutput, CodingAgentProductEventDiagnostic,
    CodingAgentProductEventError, CodingAgentProductEventKind, CodingAgentProductEventReplacement,
    CodingAgentProductEventUsage, CodingSessionError, SelfHealingEditCheckOutput,
    SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
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
pub fn product_event(event: CodingAgentProductEventKind) -> CodingAgentProductEvent {
    serde_json::from_value(json!({
        "sequence": 1,
        "event": event,
        "operation_id": null,
        "terminal_status": null,
        "terminal_operation": null,
        "durability": { "state": "live_only" },
    }))
    .expect("typed product-event fixture must deserialize")
}

#[allow(dead_code)]
pub fn product_usage(usage: Usage) -> CodingAgentProductEventUsage {
    CodingAgentProductEventUsage {
        input: usage.input,
        output: usage.output,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        total_tokens: usage.total_tokens,
        input_cost: usage.cost.input,
        output_cost: usage.cost.output,
        cache_read_cost: usage.cost.cache_read,
        cache_write_cost: usage.cost.cache_write,
    }
}

#[allow(dead_code)]
pub fn product_error(error: CodingSessionError) -> CodingAgentProductEventError {
    CodingAgentProductEventError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

#[allow(dead_code)]
pub fn product_replacement(
    replacement: SelfHealingEditReplacement,
) -> CodingAgentProductEventReplacement {
    CodingAgentProductEventReplacement {
        old_text: replacement.old_text,
        new_text: replacement.new_text,
    }
}

#[allow(dead_code)]
pub fn product_diagnostic(
    diagnostic: SelfHealingEditDiagnostic,
) -> CodingAgentProductEventDiagnostic {
    CodingAgentProductEventDiagnostic {
        message: diagnostic.message,
    }
}

#[allow(dead_code)]
pub fn product_check_output(
    output: SelfHealingEditCheckOutput,
) -> CodingAgentProductEventCheckOutput {
    CodingAgentProductEventCheckOutput {
        command: output.command,
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
    }
}

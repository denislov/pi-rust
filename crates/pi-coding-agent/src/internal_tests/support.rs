use pi_ai::api::conversation::Usage;
use serde_json::json;

use crate::api::event::{
    CodingAgentProductEvent, CodingAgentProductEventCheckOutput, CodingAgentProductEventDiagnostic,
    CodingAgentProductEventError, CodingAgentProductEventKind, CodingAgentProductEventReplacement,
    CodingAgentProductEventUsage,
};
use crate::api::operation::{
    SelfHealingEditCheckOutput, SelfHealingEditDiagnostic, SelfHealingEditReplacement,
};
use crate::api::runtime::CodingSessionError;

pub(crate) use crate::test_support::{EnvGuard, ProviderGuard};

pub(crate) fn product_event(event: CodingAgentProductEventKind) -> CodingAgentProductEvent {
    serde_json::from_value(json!({
        "stream_id": "internal-test-stream",
        "sequence": 1,
        "event": event,
        "operation_id": null,
        "terminal_status": null,
        "terminal_operation": null,
        "durability": { "state": "live_only" },
        "delivery_class": "data",
    }))
    .expect("typed product-event fixture must deserialize")
}

pub(crate) fn product_usage(usage: Usage) -> CodingAgentProductEventUsage {
    CodingAgentProductEventUsage {
        input: usage.input,
        output: usage.output,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        total_tokens: usage.total_tokens,
        cost_known: usage.cost.known,
        input_cost: usage.cost.input,
        output_cost: usage.cost.output,
        cache_read_cost: usage.cost.cache_read,
        cache_write_cost: usage.cost.cache_write,
    }
}

pub(crate) fn product_error(error: CodingSessionError) -> CodingAgentProductEventError {
    CodingAgentProductEventError {
        code: error.code().to_owned(),
        message: error.to_string(),
    }
}

pub(crate) fn product_replacement(
    replacement: SelfHealingEditReplacement,
) -> CodingAgentProductEventReplacement {
    CodingAgentProductEventReplacement {
        old_text: replacement.old_text,
        new_text: replacement.new_text,
    }
}

pub(crate) fn product_diagnostic(
    diagnostic: SelfHealingEditDiagnostic,
) -> CodingAgentProductEventDiagnostic {
    CodingAgentProductEventDiagnostic {
        message: diagnostic.message,
    }
}

pub(crate) fn product_check_output(
    output: SelfHealingEditCheckOutput,
) -> CodingAgentProductEventCheckOutput {
    CodingAgentProductEventCheckOutput {
        command: output.command,
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
    }
}

use serde_json::json;

use crate::api::event::{CodingAgentProductEvent, CodingAgentProductEventKind};

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

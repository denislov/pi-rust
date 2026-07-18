use serde::{Deserialize, Serialize};

use super::emission::ProductEventDraft;

pub(crate) const OUTBOX_SCHEMA: &str = "pi.coding-agent.product-event-outbox";
pub(crate) const OUTBOX_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DurableOutboxRecordKind {
    SessionWrite,
    OperationTerminal,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DurableOutboxRecord {
    pub(crate) schema: String,
    pub(crate) version: u32,
    pub(crate) record_id: String,
    pub(crate) session_id: String,
    pub(crate) operation_id: Option<String>,
    pub(crate) kind: DurableOutboxRecordKind,
    pub(crate) draft: ProductEventDraft,
}

impl DurableOutboxRecord {
    pub(crate) fn new(
        record_id: impl Into<String>,
        session_id: impl Into<String>,
        operation_id: Option<String>,
        kind: DurableOutboxRecordKind,
        draft: ProductEventDraft,
    ) -> Result<Self, &'static str> {
        let record_id = record_id.into();
        if record_id.trim().is_empty() {
            return Err("outbox record id must not be empty");
        }
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err("outbox session id must not be empty");
        }
        Ok(Self {
            schema: OUTBOX_SCHEMA.into(),
            version: OUTBOX_VERSION,
            record_id,
            session_id,
            operation_id,
            kind,
            draft,
        })
    }

    pub(crate) fn semantic_id(&self) -> &str {
        &self.record_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{CodingAgentProductEventDurability, CodingAgentProductEventKind};

    fn draft() -> ProductEventDraft {
        ProductEventDraft {
            event: CodingAgentProductEventKind::Diagnostic(
                crate::events::CodingAgentDiagnosticProductEvent::Diagnostic {
                    operation_id: Some("op-outbox".into()),
                    message: "outbox test".into(),
                },
            ),
            operation_id: Some("op-outbox".into()),
            session_id: Some("session-outbox".into()),
            terminal_status: None,
            durability: CodingAgentProductEventDurability::Durable {
                session_id: "session-outbox".into(),
            },
        }
    }

    #[test]
    fn record_has_stable_schema_and_semantic_identity() {
        let record = DurableOutboxRecord::new(
            "session-outbox/op-outbox/diagnostic/outbox-test",
            "session-outbox",
            Some("op-outbox".into()),
            DurableOutboxRecordKind::SessionWrite,
            draft(),
        )
        .unwrap();

        assert_eq!(record.schema, OUTBOX_SCHEMA);
        assert_eq!(record.version, OUTBOX_VERSION);
        assert_eq!(
            record.semantic_id(),
            "session-outbox/op-outbox/diagnostic/outbox-test"
        );
    }

    #[test]
    fn record_rejects_empty_identity_fields() {
        assert!(
            DurableOutboxRecord::new(
                "",
                "session-outbox",
                None,
                DurableOutboxRecordKind::Recovery,
                draft(),
            )
            .is_err()
        );
        assert!(
            DurableOutboxRecord::new(
                "record",
                "",
                None,
                DurableOutboxRecordKind::Recovery,
                draft(),
            )
            .is_err()
        );
    }

    #[test]
    fn record_round_trips_as_structured_json() {
        let record = DurableOutboxRecord::new(
            "record-1",
            "session-outbox",
            None,
            DurableOutboxRecordKind::Recovery,
            draft(),
        )
        .unwrap();
        let encoded = serde_json::to_string(&record).unwrap();
        let decoded: DurableOutboxRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, record);
    }
}

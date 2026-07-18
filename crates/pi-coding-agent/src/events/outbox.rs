use serde::{Deserialize, Serialize};

use super::emission::ProductEventDraft;

pub const OUTBOX_SCHEMA: &str = "pi.coding-agent.product-event-outbox";
pub const OUTBOX_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableOutboxRecordKind {
    SessionWrite,
    OperationTerminal,
    Recovery,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DurableOutboxIntent {
    pub(crate) record_id: String,
    pub(crate) kind: DurableOutboxRecordKind,
    pub(crate) draft: ProductEventDraft,
}

impl DurableOutboxIntent {
    pub(crate) fn new(
        record_id: impl Into<String>,
        kind: DurableOutboxRecordKind,
        draft: ProductEventDraft,
    ) -> Self {
        Self {
            record_id: record_id.into(),
            kind,
            draft,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DurableOutboxRecord {
    pub schema: String,
    pub version: u32,
    pub record_id: String,
    pub session_id: String,
    pub operation_id: Option<String>,
    pub source_event_ids: Vec<String>,
    pub kind: DurableOutboxRecordKind,
    pub draft: ProductEventDraft,
}

impl DurableOutboxRecord {
    pub fn new(
        record_id: impl Into<String>,
        session_id: impl Into<String>,
        operation_id: Option<String>,
        source_event_ids: Vec<String>,
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
        if source_event_ids.is_empty()
            || source_event_ids
                .iter()
                .any(|event_id| event_id.trim().is_empty())
        {
            return Err("outbox source event ids must not be empty");
        }
        Ok(Self {
            schema: OUTBOX_SCHEMA.into(),
            version: OUTBOX_VERSION,
            record_id,
            session_id,
            operation_id,
            source_event_ids,
            kind,
            draft,
        })
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
            vec!["evt-outbox".into()],
            DurableOutboxRecordKind::SessionWrite,
            draft(),
        )
        .unwrap();

        assert_eq!(record.schema, OUTBOX_SCHEMA);
        assert_eq!(record.version, OUTBOX_VERSION);
        assert_eq!(
            record.record_id,
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
                vec!["evt-outbox".into()],
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
                vec!["evt-outbox".into()],
                DurableOutboxRecordKind::Recovery,
                draft(),
            )
            .is_err()
        );
        assert!(
            DurableOutboxRecord::new(
                "record",
                "session-outbox",
                None,
                Vec::new(),
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
            vec!["evt-outbox".into()],
            DurableOutboxRecordKind::Recovery,
            draft(),
        )
        .unwrap();
        let encoded = serde_json::to_string(&record).unwrap();
        let decoded: DurableOutboxRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, record);
    }
}

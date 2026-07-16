use super::emission::ProductEventDraft;
use super::{
    CodingAgentProductEventDurability, CodingAgentProductEventKind,
    CodingAgentProductEventTerminalStatus, CodingAgentWorkflowProductEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryEvent {
    pub(crate) operation_id: String,
    pub(crate) recovery_id: String,
    pub(crate) reason: String,
    pub(crate) session_id: String,
}

impl RecoveryEvent {
    pub(crate) fn into_product_draft(self) -> ProductEventDraft {
        ProductEventDraft {
            event: CodingAgentProductEventKind::Workflow(
                CodingAgentWorkflowProductEvent::OperationRecovered {
                    operation_id: self.operation_id.clone(),
                    recovery_id: self.recovery_id.clone(),
                    reason: self.reason,
                },
            ),
            operation_id: Some(self.operation_id.clone()),
            session_id: Some(self.session_id.clone()),
            terminal_status: Some(CodingAgentProductEventTerminalStatus::Recovered),
            durability: CodingAgentProductEventDurability::DerivedFromSession {
                session_id: self.session_id,
                source_operation_id: self.operation_id,
                recovery_id: self.recovery_id,
            },
        }
    }
}

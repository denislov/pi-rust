use super::emission::ProductEventDraft;
use super::{
    CodingAgentDiagnosticProductEvent, CodingAgentProductEventDurability,
    CodingAgentProductEventKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiagnosticEvent {
    Diagnostic {
        operation_id: Option<String>,
        message: String,
    },
}

impl DiagnosticEvent {
    pub(crate) fn into_product_draft(self) -> ProductEventDraft {
        match self {
            Self::Diagnostic {
                operation_id,
                message,
            } => ProductEventDraft {
                event: CodingAgentProductEventKind::Diagnostic(
                    CodingAgentDiagnosticProductEvent::Diagnostic {
                        operation_id: operation_id.clone(),
                        message,
                    },
                ),
                operation_id,
                session_id: None,
                terminal_status: None,
                durability: CodingAgentProductEventDurability::LiveOnly,
            },
        }
    }
}

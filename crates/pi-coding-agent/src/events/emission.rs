use super::{
    CodingAgentProductEventDurability, CodingAgentProductEventKind,
    CodingAgentProductEventTerminalStatus,
};

#[derive(Debug, Clone)]
pub(crate) struct ProductEventDraft {
    pub(crate) event: CodingAgentProductEventKind,
    pub(crate) operation_id: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) terminal_status: Option<CodingAgentProductEventTerminalStatus>,
    pub(crate) durability: CodingAgentProductEventDurability,
}

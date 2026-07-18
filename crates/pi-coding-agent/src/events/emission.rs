use super::{
    CodingAgentProductEventDurability, CodingAgentProductEventKind,
    CodingAgentProductEventTerminalStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProductEventDraft {
    pub(crate) event: CodingAgentProductEventKind,
    pub(crate) operation_id: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) terminal_status: Option<CodingAgentProductEventTerminalStatus>,
    pub(crate) durability: CodingAgentProductEventDurability,
}

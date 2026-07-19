#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum PromptHookPoint {
    BeforePromptPrepare,
    AfterInputPrepared,
    AfterResourcesLoaded,
    BeforeAgentTurn,
    AfterAgentTurn,
    BeforeSessionCommit,
    AfterSessionCommit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct PromptHookContext {
    pub(crate) operation_id: String,
    pub(crate) turn_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) point: PromptHookPoint,
}

use super::CodingAgentCapabilities;

#[derive(Debug)]
pub(crate) struct CapabilityService {
    capabilities: CodingAgentCapabilities,
}

impl CapabilityService {
    pub(crate) fn new() -> Self {
        Self {
            capabilities: CodingAgentCapabilities::phase_1(),
        }
    }

    pub(crate) fn capabilities(&self) -> CodingAgentCapabilities {
        self.capabilities.clone()
    }
}

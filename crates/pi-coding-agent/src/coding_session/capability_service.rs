use super::CodingAgentCapabilities;

#[derive(Debug)]
pub(crate) struct CapabilityService;

impl CapabilityService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn capabilities(&self, active_operation: Option<&str>) -> CodingAgentCapabilities {
        CodingAgentCapabilities::phase_3(active_operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::CapabilityStatus;

    #[test]
    fn capabilities_report_prompt_available_when_idle() {
        let capabilities = CapabilityService::new().capabilities(None);

        assert_eq!(capabilities.prompt, CapabilityStatus::Available);
        assert_eq!(capabilities.tools, CapabilityStatus::Available);
        assert_eq!(capabilities.shell, CapabilityStatus::Available);
        assert!(matches!(
            capabilities.plugins,
            CapabilityStatus::Unsupported { .. }
        ));
    }

    #[test]
    fn capabilities_report_prompt_busy_for_active_operation() {
        let capabilities = CapabilityService::new().capabilities(Some("prompt"));

        assert_eq!(
            capabilities.prompt,
            CapabilityStatus::Busy {
                operation: "prompt".into(),
            }
        );
    }
}

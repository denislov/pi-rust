use super::CodingAgentCapabilities;
use crate::plugins::PluginCapabilities;

#[derive(Debug)]
pub(crate) struct CapabilityService;

impl CapabilityService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn capabilities(
        &self,
        active_operation: Option<&str>,
        plugin_capabilities: &PluginCapabilities,
        persistent_session: bool,
    ) -> CodingAgentCapabilities {
        CodingAgentCapabilities::phase_5(active_operation, plugin_capabilities, persistent_session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::CapabilityStatus;
    use crate::plugins::PluginCapabilities;

    #[test]
    fn capabilities_report_prompt_available_when_idle() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(None, &plugin_capabilities, true);

        assert_eq!(capabilities.prompt, CapabilityStatus::Available);
        assert_eq!(capabilities.branch_summary, CapabilityStatus::Available);
        assert_eq!(capabilities.plugin_reload, CapabilityStatus::Available);
        assert_eq!(capabilities.tools, CapabilityStatus::Available);
        assert_eq!(capabilities.shell, CapabilityStatus::Available);
        assert_eq!(capabilities.plugins, CapabilityStatus::Available);
    }

    #[test]
    fn capabilities_report_prompt_busy_for_active_operation() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities =
            CapabilityService::new().capabilities(Some("prompt"), &plugin_capabilities, true);

        assert_eq!(
            capabilities.prompt,
            CapabilityStatus::Busy {
                operation: "prompt".into(),
            }
        );
    }

    #[test]
    fn capabilities_report_persistent_workflows_busy_for_active_operation() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(
            Some("branch_summary"),
            &plugin_capabilities,
            true,
        );

        for capability in [
            capabilities.compact,
            capabilities.fork,
            capabilities.clone_session,
            capabilities.export,
            capabilities.branch_summary,
            capabilities.plugin_reload,
        ] {
            assert_eq!(
                capability,
                CapabilityStatus::Busy {
                    operation: "branch_summary".into(),
                }
            );
        }
    }

    #[test]
    fn capabilities_report_plugins_available_when_kernel_exists() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(None, &plugin_capabilities, true);

        assert_eq!(capabilities.plugins, CapabilityStatus::Available);
    }

    #[test]
    fn capabilities_disable_persistent_session_operations_without_persistence() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(None, &plugin_capabilities, false);

        assert_eq!(
            capabilities.export,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
        assert_eq!(
            capabilities.compact,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
        assert_eq!(
            capabilities.clone_session,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
        assert_eq!(
            capabilities.branch_summary,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
        assert_eq!(
            capabilities.plugin_reload,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
    }
}

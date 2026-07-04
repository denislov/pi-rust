use super::{CodingAgentCapabilities, OperationKind};
use crate::plugins::PluginCapabilities;

#[derive(Debug)]
pub(crate) struct CapabilityService;

impl CapabilityService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn capabilities(
        &self,
        active_operation: Option<OperationKind>,
        plugin_capabilities: &PluginCapabilities,
        persistent_session: bool,
    ) -> CodingAgentCapabilities {
        CodingAgentCapabilities::from_runtime_state(
            active_operation,
            plugin_capabilities,
            persistent_session,
        )
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
        assert_eq!(capabilities.agent_profiles, CapabilityStatus::Available);
        assert_eq!(capabilities.team_profiles, CapabilityStatus::Available);
        assert_eq!(capabilities.delegation, CapabilityStatus::Available);
        assert_eq!(capabilities.tools, CapabilityStatus::Available);
        assert_eq!(capabilities.shell, CapabilityStatus::Available);
        assert_eq!(capabilities.plugins, CapabilityStatus::Available);
    }

    #[test]
    fn capabilities_report_prompt_busy_for_active_operation() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(
            Some(OperationKind::Prompt),
            &plugin_capabilities,
            true,
        );

        assert_eq!(
            capabilities.prompt,
            CapabilityStatus::Busy {
                operation: "prompt".into(),
            }
        );
    }

    #[test]
    fn capabilities_disable_prompt_controls_when_no_prompt_is_running() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(None, &plugin_capabilities, true);

        for capability in [
            capabilities.abort,
            capabilities.steer,
            capabilities.follow_up,
        ] {
            assert_eq!(
                capability,
                CapabilityStatus::Disabled {
                    reason: "no prompt is running".into(),
                }
            );
        }
    }

    #[test]
    fn capabilities_enable_prompt_controls_only_for_running_prompt() {
        let plugin_capabilities = PluginCapabilities::new();
        let prompt_capabilities = CapabilityService::new().capabilities(
            Some(OperationKind::Prompt),
            &plugin_capabilities,
            true,
        );

        assert_eq!(prompt_capabilities.abort, CapabilityStatus::Available);
        assert_eq!(prompt_capabilities.steer, CapabilityStatus::Available);
        assert_eq!(prompt_capabilities.follow_up, CapabilityStatus::Available);

        let plugin_load_capabilities = CapabilityService::new().capabilities(
            Some(OperationKind::PluginLoad),
            &plugin_capabilities,
            true,
        );

        for capability in [
            plugin_load_capabilities.abort,
            plugin_load_capabilities.steer,
            plugin_load_capabilities.follow_up,
        ] {
            assert_eq!(
                capability,
                CapabilityStatus::Disabled {
                    reason: "no prompt is running".into(),
                }
            );
        }
    }

    #[test]
    fn capabilities_report_persistent_workflows_busy_for_active_operation() {
        let plugin_capabilities = PluginCapabilities::new();
        let capabilities = CapabilityService::new().capabilities(
            Some(OperationKind::BranchSummary),
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
            capabilities.self_healing_edit,
            capabilities.agent_profiles,
            capabilities.team_profiles,
            capabilities.delegation,
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
        assert_eq!(
            capabilities.self_healing_edit,
            CapabilityStatus::Disabled {
                reason: "requires persistent Rust-native session".into(),
            }
        );
    }

    #[test]
    fn self_healing_edit_operation_kind_reports_stable_name() {
        assert_eq!(OperationKind::SelfHealingEdit.as_str(), "self_healing_edit");
    }
}

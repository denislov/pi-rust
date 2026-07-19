use super::*;
use crate::session::id::{Clock, SystemClock};

impl CodingAgentSession {
    pub(crate) fn tool_authorization_control(
        &self,
    ) -> crate::services::authorization::AuthorizationService {
        self.runtime_host.authorization_service.clone()
    }

    pub fn pending_tool_authorizations(
        &self,
    ) -> Vec<crate::authorization::ToolAuthorizationRequest> {
        self.runtime_host.authorization_service.pending()
    }

    pub fn decide_tool_authorization(
        &self,
        authorization_id: &str,
        decision: crate::authorization::ToolAuthorizationDecision,
    ) -> Result<(), CodingSessionError> {
        self.runtime_host
            .authorization_service
            .decide(authorization_id, decision)
    }

    pub(in crate::runtime) fn default_agent_profile_id(&self) -> ProfileId {
        crate::operations::prompt::default_agent_profile_id(
            &self.runtime_host.session_coordinator.persistence,
        )
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::Capabilities,
        );
        let plugin_capabilities = self.runtime_host.plugin_service.capabilities();
        let persistent = matches!(
            self.runtime_host.session_coordinator.persistence,
            SessionPersistence::Persistent(_)
        );
        self.runtime_host.capability_service.capabilities(
            &self.runtime_host.operation_supervisor.control.activity(),
            &plugin_capabilities,
            persistent,
        )
    }

    pub fn view(&self) -> CodingAgentSessionView {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::SessionView,
        );
        let _ = (
            &self.runtime_host.runtime_service,
            &self.runtime_host.workflow_service,
            &self.runtime_host.plugin_service,
        );
        match &self.runtime_host.session_coordinator.persistence {
            SessionPersistence::Persistent(session_service) => session_service.view(),
            SessionPersistence::NonPersistent(state) => CodingAgentSessionView {
                session_id: state.runtime_id.clone(),
                default_agent_profile_id: state.default_agent_profile_id.clone(),
            },
        }
    }

    pub fn recovery_pending(&self) -> Result<Vec<CodingAgentRecoveryPending>, CodingSessionError> {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::SessionView,
        );
        let SessionPersistence::Persistent(service) =
            &self.runtime_host.session_coordinator.persistence
        else {
            return Ok(Vec::new());
        };
        Ok(service
            .inspect_recovery_pending()?
            .into_iter()
            .map(|pending| CodingAgentRecoveryPending {
                operation_id: pending.operation_id,
                recovery_id: pending.recovery_id,
                operation_kind: pending.operation_kind,
                record_version: pending.record_version,
                descriptor_revision: pending.descriptor_revision,
                capability_generation: pending.capability_generation,
                attempt_count: pending.attempt_count,
                last_attempt_at: pending.last_attempt_at,
                next_attempt_at: pending.next_attempt_at,
            })
            .collect())
    }

    pub fn agent_profiles(&self) -> Vec<AgentProfile> {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::AgentProfiles,
        );
        self.runtime_host
            .profile_registry
            .agents()
            .cloned()
            .collect()
    }

    pub fn team_profiles(&self) -> Vec<TeamProfile> {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::TeamProfiles,
        );
        self.runtime_host
            .profile_registry
            .teams()
            .cloned()
            .collect()
    }

    pub fn profile_diagnostics(&self) -> Vec<ProfileDiagnostic> {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::ProfileDiagnostics,
        );
        self.runtime_host.profile_registry.diagnostics().to_vec()
    }

    pub fn pending_delegation_confirmations(&self) -> Vec<PendingDelegationConfirmation> {
        IntentRouter::admit_query(
            &self.runtime_host.operation_supervisor.control,
            QueryIntent::PendingDelegationConfirmations,
        );
        let now = SystemClock.now_rfc3339();
        crate::operations::delegation::confirmation::active_views(
            &self
                .runtime_host
                .session_coordinator
                .pending_delegation_confirmations,
            &now,
        )
    }

    pub(crate) fn plugin_commands(&self) -> Vec<CommandDefinition> {
        self.runtime_host.plugin_service.collect_commands()
    }

    pub(crate) fn plugin_ui_actions(&self) -> Vec<UiActionDefinition> {
        self.runtime_host.plugin_service.collect_ui_actions()
    }

    pub(crate) fn plugin_ui_dialogs(&self) -> Vec<UiDialogDefinition> {
        self.runtime_host.plugin_service.collect_ui_dialogs()
    }

    pub(crate) fn plugin_keybindings(&self) -> Vec<KeybindDefinition> {
        self.runtime_host.plugin_service.collect_keybindings()
    }
}

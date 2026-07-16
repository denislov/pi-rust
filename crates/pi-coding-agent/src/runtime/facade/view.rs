use super::*;
use crate::session::id::{Clock, SystemClock};

impl CodingAgentSession {
    pub(crate) fn tool_authorization_control(
        &self,
    ) -> crate::services::authorization::AuthorizationService {
        self.authorization_service.clone()
    }

    pub fn pending_tool_authorizations(
        &self,
    ) -> Vec<crate::authorization::ToolAuthorizationRequest> {
        self.authorization_service.pending()
    }

    pub fn decide_tool_authorization(
        &self,
        authorization_id: &str,
        decision: crate::authorization::ToolAuthorizationDecision,
    ) -> Result<(), CodingSessionError> {
        self.authorization_service
            .decide(authorization_id, decision)
    }

    pub(in crate::runtime) fn default_agent_profile_id(&self) -> ProfileId {
        crate::operations::prompt::default_agent_profile_id(&self.persistence)
    }

    pub fn capabilities(&self) -> CodingAgentCapabilities {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::Capabilities);
        let plugin_capabilities = self.plugin_service.capabilities();
        let persistent = matches!(self.persistence, SessionPersistence::Persistent(_));
        self.capability_service.capabilities(
            &self.operation_control.activity(),
            &plugin_capabilities,
            persistent,
        )
    }

    pub fn view(&self) -> CodingAgentSessionView {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::SessionView);
        let _ = (
            &self.runtime_service,
            &self.flow_service,
            &self.plugin_service,
        );
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_service.view(),
            SessionPersistence::NonPersistent(state) => CodingAgentSessionView {
                session_id: state.runtime_id.clone(),
                default_agent_profile_id: state.default_agent_profile_id.clone(),
            },
        }
    }

    pub fn agent_profiles(&self) -> Vec<AgentProfile> {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::AgentProfiles);
        self.profile_registry.agents().cloned().collect()
    }

    pub fn team_profiles(&self) -> Vec<TeamProfile> {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::TeamProfiles);
        self.profile_registry.teams().cloned().collect()
    }

    pub fn profile_diagnostics(&self) -> Vec<ProfileDiagnostic> {
        IntentRouter::admit_query(&self.operation_control, QueryIntent::ProfileDiagnostics);
        self.profile_registry.diagnostics().to_vec()
    }

    pub fn pending_delegation_confirmations(&self) -> Vec<PendingDelegationConfirmation> {
        IntentRouter::admit_query(
            &self.operation_control,
            QueryIntent::PendingDelegationConfirmations,
        );
        let now = SystemClock.now_rfc3339();
        crate::operations::delegation::confirmation::active_views(
            &self.pending_delegation_confirmations,
            &now,
        )
    }

    pub(crate) fn plugin_commands(&self) -> Vec<CommandDefinition> {
        self.plugin_service.collect_commands()
    }

    pub(crate) fn plugin_ui_actions(&self) -> Vec<UiActionDefinition> {
        self.plugin_service.collect_ui_actions()
    }

    pub(crate) fn plugin_ui_dialogs(&self) -> Vec<UiDialogDefinition> {
        self.plugin_service.collect_ui_dialogs()
    }

    pub(crate) fn plugin_keybindings(&self) -> Vec<KeybindDefinition> {
        self.plugin_service.collect_keybindings()
    }
}

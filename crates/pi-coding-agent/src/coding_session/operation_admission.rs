use super::*;

impl CodingAgentSession {
    pub(super) fn resolve_operation_admission(
        &self,
        operation: &Operation,
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        let (kind, admitted_at) = match operation {
            Operation::ApproveDelegationConfirmation {
                operation_id,
                tool_call_id,
            } => {
                let now = SystemClock.now_rfc3339();
                let kind = self.delegation_approval_operation_kind(
                    operation_id.as_str(),
                    tool_call_id.as_str(),
                    &now,
                )?;
                (kind, Some(now))
            }
            _ => (
                operation.static_kind().ok_or_else(|| {
                    CodingSessionError::UnsupportedCapability {
                        capability: "dynamic operation requires async dispatcher".into(),
                    }
                })?,
                None,
            ),
        };
        let operation_id = self.next_operation_admission_id(operation);
        let snapshot = self
            .capability_snapshots
            .snapshot(self.snapshot_input_for_operation(operation_id, kind, operation));
        Ok(OperationAdmission::new(
            kind,
            metadata,
            admitted_at,
            snapshot,
        ))
    }

    fn next_operation_admission_id(&self, _operation: &Operation) -> String {
        let mut ids = SystemIdGenerator;
        ids.next_operation_id()
    }

    fn snapshot_input_for_operation(
        &self,
        operation_id: String,
        kind: OperationKind,
        operation: &Operation,
    ) -> CapabilitySnapshotInput {
        let plugin_capabilities = self.plugin_service.capabilities();
        let default_profile_id = self.default_agent_profile_id();
        let runtime_tools = self.operation_runtime_tool_names(operation);
        let profile_tools = match self.active_agent_profile() {
            Some(profile) if !profile.tools.is_empty() => profile.tools.clone(),
            _ => runtime_tools.clone(),
        };
        CapabilitySnapshotInput {
            operation_id,
            operation_kind: kind,
            actor: ActorId::Client,
            default_profile_id,
            plugin_capabilities,
            persistent_session: matches!(self.persistence, SessionPersistence::Persistent(_)),
            cwd: self.cwd(),
            runtime_tools,
            profile_tools,
        }
    }

    fn operation_runtime_tool_names(&self, operation: &Operation) -> Vec<String> {
        let mut names = self.current_runtime_tool_names();
        let options = match operation {
            Operation::Prompt(options)
            | Operation::ManualCompaction(options)
            | Operation::BranchSummary { options, .. } => Some(options),
            _ => None,
        };
        if let Some(options) = options
            && let Some(runtime) = options.runtime()
        {
            names.extend(runtime.tools().iter().map(|tool| tool.name.clone()));
        }
        names.extend(
            self.plugin_service
                .collect_tools()
                .into_iter()
                .map(|tool| tool.name),
        );
        if let Some(profile) = self.active_agent_profile() {
            names.extend(
                delegation::delegation_tools(Some(&profile.id), Some(&profile.delegation))
                    .into_iter()
                    .map(|tool| tool.name),
            );
        }
        names.sort();
        names.dedup();
        names
    }

    fn cwd(&self) -> Option<PathBuf> {
        match &self.persistence {
            SessionPersistence::Persistent(session_service) => session_cwd(session_service),
            SessionPersistence::NonPersistent(_) => None,
        }
    }

    fn active_agent_profile(&self) -> Option<&AgentProfile> {
        let id = self.default_agent_profile_id();
        self.profile_registry.agent(id.as_str())
    }

    fn current_runtime_tool_names(&self) -> Vec<String> {
        vec![
            "read".into(),
            "write".into(),
            "edit".into(),
            "bash".into(),
            "grep".into(),
            "find".into(),
            "ls".into(),
        ]
    }

    pub(super) fn delegation_approval_operation_kind(
        &self,
        operation_id: &str,
        tool_call_id: &str,
        now: &str,
    ) -> Result<OperationKind, CodingSessionError> {
        let pending = self.delegation_confirmation_service.active_pending(
            &self.pending_delegation_confirmations,
            operation_id,
            tool_call_id,
            now,
        )?;
        Ok(match pending.request.target_kind {
            ProfileKind::Agent => OperationKind::AgentInvocation,
            ProfileKind::Team => OperationKind::AgentTeam,
        })
    }
}

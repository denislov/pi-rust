use super::capability::OperationCapabilitySnapshot;
use super::control::{
    ChildOperationGuard, OperationCancellationHandle, OperationControl, OperationGuard,
    OperationKind, PromptControlHandle,
};
#[cfg(test)]
use super::operation::Operation;
#[cfg(test)]
use super::operation::OperationOrigin;
use super::operation::{OperationClass, OperationExecution};
use super::scheduler::OperationScheduler;
use crate::runtime::facade::CodingSessionError;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlIntent {
    PromptControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryIntent {
    Capabilities,
    SessionView,
    AgentProfiles,
    TeamProfiles,
    ProfileDiagnostics,
    PendingDelegationConfirmations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControlIntentMetadata {
    pub(crate) operation_kind: OperationKind,
    pub(crate) class: OperationClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryIntentMetadata {
    pub(crate) intent: QueryIntent,
    pub(crate) class: OperationClass,
}

#[derive(Debug)]
#[must_use = "dropping OperationPermit releases any guarded operation"]
pub(crate) struct OperationPermit {
    guard: Option<OperationGuard>,
    _child_guard: Option<ChildOperationGuard>,
    execution: OperationExecution,
    cancellation: Option<CancellationToken>,
    cancellation_handle: Option<OperationCancellationHandle>,
    #[cfg(test)]
    kind: OperationKind,
    #[cfg(test)]
    class: OperationClass,
}

impl ControlIntent {
    pub(crate) fn metadata(self) -> ControlIntentMetadata {
        match self {
            Self::PromptControl => ControlIntentMetadata {
                operation_kind: OperationKind::Prompt,
                class: OperationClass::Control,
            },
        }
    }
}

impl QueryIntent {
    pub(crate) fn metadata(self) -> QueryIntentMetadata {
        QueryIntentMetadata {
            intent: self,
            class: OperationClass::Query,
        }
    }
}

impl OperationPermit {
    pub(crate) fn guarded(
        kind: OperationKind,
        class: OperationClass,
        mut guard: OperationGuard,
        execution: OperationExecution,
    ) -> Self {
        guard.bind_capability_generation(execution.capability_generation);
        let cancellation = guard.cancellation_token();
        let cancellation_handle = Some(guard.cancellation_handle());
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: Some(guard),
            _child_guard: None,
            execution,
            cancellation,
            cancellation_handle,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    pub(crate) fn unguarded(
        kind: OperationKind,
        class: OperationClass,
        execution: OperationExecution,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: None,
            _child_guard: None,
            execution,
            cancellation: None,
            cancellation_handle: None,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    pub(crate) fn child(
        kind: OperationKind,
        execution: OperationExecution,
        mut guard: ChildOperationGuard,
    ) -> Self {
        guard.bind_capability_generation(execution.capability_generation);
        let cancellation = Some(guard.cancellation_token());
        let cancellation_handle = Some(guard.cancellation_handle());
        #[cfg(not(test))]
        let _ = kind;

        Self {
            guard: None,
            _child_guard: Some(guard),
            execution,
            cancellation,
            cancellation_handle,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class: OperationClass::Child,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn capability_snapshot(&self) -> &OperationCapabilitySnapshot {
        &self.execution.capability_snapshot
    }

    pub(crate) fn execution(&self) -> &OperationExecution {
        &self.execution
    }

    pub(crate) fn cancellation_token(&self) -> Option<CancellationToken> {
        self.cancellation.clone()
    }

    pub(crate) fn cancellation_handle(&self) -> Option<OperationCancellationHandle> {
        self.cancellation_handle.clone()
    }

    #[cfg(test)]
    pub(crate) fn kind(&self) -> OperationKind {
        self.kind
    }

    #[cfg(test)]
    pub(crate) fn class(&self) -> OperationClass {
        self.class
    }

    #[cfg(test)]
    pub(crate) fn is_guarded(&self) -> bool {
        self.guard.is_some() || self._child_guard.is_some()
    }
}

impl Drop for OperationPermit {
    fn drop(&mut self) {
        let _ = self.guard.is_some();
    }
}

pub(crate) struct IntentRouter;

impl IntentRouter {
    #[cfg(test)]
    pub(crate) fn static_admission(
        operation: &Operation,
    ) -> Result<OperationExecution, CodingSessionError> {
        if operation.static_kind().is_none() {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            });
        }
        let snapshot = OperationCapabilitySnapshot::permissive("op_static_admission");
        Ok(OperationExecution::root(
            operation.kind(),
            operation.descriptor(),
            OperationOrigin::ClientRoot,
            None,
            Some("static-test-session".into()),
            snapshot,
        ))
    }

    pub(crate) fn prompt_control_handle(
        control: &mut OperationControl,
        intent: ControlIntent,
    ) -> Result<PromptControlHandle, CodingSessionError> {
        let metadata = intent.metadata();
        debug_assert_eq!(metadata.class, OperationClass::Control);
        match metadata.operation_kind {
            OperationKind::Prompt => control.prompt_control_handle(),
            _ => unreachable!("unsupported control intent target"),
        }
    }

    pub(crate) fn admit_query(
        control: &OperationControl,
        intent: QueryIntent,
    ) -> QueryIntentMetadata {
        let metadata = OperationScheduler::admit_query(control, intent);
        debug_assert_eq!(metadata.class, OperationClass::Query);
        metadata
    }

    pub(crate) fn unsupported_dispatch(admission: &OperationExecution) -> CodingSessionError {
        CodingSessionError::UnsupportedCapability {
            capability: format!(
                "{} operation requires {} dispatcher",
                admission.kind.as_str(),
                admission.descriptor.dispatch_mode.dispatcher_label(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::export::runner::ExportOptions;
    use crate::runtime::control::{OperationKind, PromptControlCommand};
    use crate::runtime::operation::OperationDispatchMode;
    use crate::runtime::operation::{Operation, OperationClass};

    #[test]
    fn control_intent_prompt_control_is_classified_as_control() {
        let metadata = ControlIntent::PromptControl.metadata();

        assert_eq!(metadata.operation_kind, OperationKind::Prompt);
        assert_eq!(metadata.class, OperationClass::Control);
    }

    #[test]
    fn intent_router_prompt_control_handle_rejects_busy_or_pending_receiver() {
        let mut control = OperationControl::new();
        let guard = control
            .begin(OperationKind::PluginLoad, "op_test".into())
            .unwrap();

        assert_eq!(
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "plugin_load".into(),
            }
        );
        drop(guard);

        let _handle =
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap();
        assert_eq!(
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "prompt_control".into(),
            }
        );
    }

    #[test]
    fn intent_router_prompt_control_handle_preserves_prompt_control_commands() {
        let mut control = OperationControl::new();

        let handle =
            IntentRouter::prompt_control_handle(&mut control, ControlIntent::PromptControl)
                .unwrap();
        handle.abort("stop").unwrap();
        handle.steer("focus").unwrap();
        handle.follow_up("continue").unwrap();

        let mut receiver = control
            .take_prompt_control_receiver()
            .expect("router should leave receiver owned by operation control");
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Abort {
                reason: "stop".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::Steer {
                text: "focus".into(),
            }
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            PromptControlCommand::FollowUp {
                text: "continue".into(),
            }
        );
    }

    #[test]
    fn query_intents_are_classified_as_query() {
        for intent in [
            QueryIntent::Capabilities,
            QueryIntent::SessionView,
            QueryIntent::AgentProfiles,
            QueryIntent::TeamProfiles,
            QueryIntent::ProfileDiagnostics,
            QueryIntent::PendingDelegationConfirmations,
        ] {
            let metadata = intent.metadata();

            assert_eq!(metadata.intent, intent);
            assert_eq!(metadata.class, OperationClass::Query);
        }
    }

    #[test]
    fn intent_router_admits_queries_while_root_operation_is_busy() {
        let control = OperationControl::new();
        let guard = control
            .begin(OperationKind::Prompt, "op_test".into())
            .unwrap();

        let admission =
            IntentRouter::admit_query(&control, QueryIntent::PendingDelegationConfirmations);

        assert_eq!(
            admission.intent,
            QueryIntent::PendingDelegationConfirmations
        );
        assert_eq!(admission.class, OperationClass::Query);
        assert_eq!(control.active(), Some(OperationKind::Prompt));
        drop(guard);
        assert_eq!(control.active(), None);
    }

    #[test]
    fn read_only_admission_allows_export_while_root_operation_is_busy() {
        let operation = Operation::Export(ExportOptions::view());
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();
        let guard = control
            .begin(OperationKind::Prompt, "op_test".into())
            .unwrap();

        let permit =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncReadOnly)
                .map_err(|rejection| rejection.into_error())
                .unwrap();

        assert_eq!(permit.kind(), OperationKind::Export);
        assert_eq!(permit.class(), OperationClass::ReadOnly);
        assert!(!permit.is_guarded());
        assert_eq!(control.active(), Some(OperationKind::Prompt));
        drop(permit);
        assert_eq!(control.active(), Some(OperationKind::Prompt));
        drop(guard);
        assert_eq!(control.active(), None);
    }

    #[test]
    fn async_admission_keeps_plugin_command_guarded() {
        let operation = Operation::PluginCommand {
            command_id: "plugin.echo".into(),
            args: serde_json::json!({}),
        };
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();

        let permit = OperationScheduler::admit(&control, &admission, OperationDispatchMode::Async)
            .map_err(|rejection| rejection.into_error())
            .unwrap();

        assert_eq!(permit.kind(), OperationKind::PluginCommand);
        assert_eq!(permit.class(), OperationClass::NonSessionRoot);
        assert!(permit.is_guarded());
        assert_eq!(control.active(), Some(OperationKind::PluginCommand));
        drop(permit);
        assert_eq!(control.active(), None);
    }

    #[test]
    fn delegation_rejection_session_write_admission_is_busy_while_root_is_active() {
        let operation = Operation::RejectDelegationConfirmation {
            operation_id: "op_parent".into(),
            tool_call_id: "tool_delegate".into(),
            reason: "not needed".into(),
        };
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();
        let root = control
            .begin(OperationKind::Prompt, "op_prompt".into())
            .unwrap();

        let error =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncMutable)
                .map_err(|rejection| rejection.into_error())
                .unwrap_err();

        assert_eq!(
            error,
            CodingSessionError::Busy {
                operation: "prompt".into(),
            }
        );
        assert_eq!(
            admission.descriptor.admission_class(),
            OperationClass::SessionWriteRoot
        );
        assert_eq!(control.active(), Some(OperationKind::Prompt));
        drop(root);
        assert_eq!(control.active(), None);
    }

    #[test]
    fn operation_permit_exposes_the_frozen_snapshot_for_execution() {
        use crate::runtime::capability::{
            ActorId, CapabilityGeneration, OperationCapabilitySnapshot, PluginCapabilitySet,
            ToolCapabilitySet,
        };
        use crate::runtime::operation::OperationOrigin;

        let control = OperationControl::new();
        let descriptor = crate::runtime::outcome::descriptor_for_test_admission(
            OperationKind::Export,
            OperationClass::ReadOnly,
            OperationDispatchMode::SyncReadOnly,
        );
        let snapshot = OperationCapabilitySnapshot {
            generation: CapabilityGeneration::new(3),
            operation_id: "op_export".into(),
            actor: ActorId::Client,
            model: None,
            tools: ToolCapabilitySet::default(),
            commands: Default::default(),
            filesystem: None,
            shell: None,
            session_read: None,
            session_write: None,
            ui: None,
            plugin: PluginCapabilitySet::default(),
        };
        let admission = OperationExecution::root(
            OperationKind::Export,
            descriptor,
            OperationOrigin::ClientRoot,
            None,
            Some("intent-test-session".into()),
            snapshot.clone(),
        );

        let permit =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncReadOnly)
                .map_err(|rejection| rejection.into_error())
                .unwrap();

        assert_eq!(permit.capability_snapshot(), &snapshot);
    }

    #[test]
    fn operation_permit_exposes_frozen_snapshot_for_guarded_execution() {
        use crate::runtime::capability::{
            ActorId, CapabilityGeneration, OperationCapabilitySnapshot, PluginCapabilitySet,
            ToolCapabilitySet,
        };
        use crate::runtime::operation::OperationOrigin;

        let control = OperationControl::new();
        let descriptor = crate::runtime::outcome::descriptor_for_test_admission(
            OperationKind::PluginCommand,
            OperationClass::NonSessionRoot,
            OperationDispatchMode::Async,
        );
        let snapshot = OperationCapabilitySnapshot {
            generation: CapabilityGeneration::new(5),
            operation_id: "op_command".into(),
            actor: ActorId::Client,
            model: None,
            tools: ToolCapabilitySet::default(),
            commands: Default::default(),
            filesystem: None,
            shell: None,
            session_read: None,
            session_write: None,
            ui: None,
            plugin: PluginCapabilitySet::default(),
        };
        let admission = OperationExecution::root(
            OperationKind::PluginCommand,
            descriptor,
            OperationOrigin::ClientRoot,
            None,
            Some("intent-test-session".into()),
            snapshot.clone(),
        );

        let permit = OperationScheduler::admit(&control, &admission, OperationDispatchMode::Async)
            .map_err(|rejection| rejection.into_error())
            .unwrap();

        assert!(permit.is_guarded());
        assert_eq!(permit.capability_snapshot(), &snapshot);
    }

    #[test]
    fn session_query_facade_routes_through_query_admission() {
        let source = [include_str!("facade.rs"), include_str!("facade/view.rs")].concat();

        assert!(
            source.matches("IntentRouter::admit_query(").count() >= 6,
            "CodingAgentSession query facade should route query methods through query admission"
        );
        for expected in [
            "QueryIntent::Capabilities",
            "QueryIntent::SessionView",
            "QueryIntent::AgentProfiles",
            "QueryIntent::TeamProfiles",
            "QueryIntent::ProfileDiagnostics",
            "QueryIntent::PendingDelegationConfirmations",
        ] {
            assert!(
                source.contains(expected),
                "CodingAgentSession query facade should route through query admission: {expected}"
            );
        }
    }

    #[test]
    fn canonical_session_run_owns_mutation_dispatch() {
        let session_source = include_str!("facade.rs");
        let dispatch_source = include_str!("dispatch.rs");
        let submission_source = include_str!("submission.rs");
        let operation_source = include_str!("operation.rs");
        let outcome_source = include_str!("outcome.rs");

        assert!(
            !session_source.contains("fn set_default_agent_profile_id(")
                && submission_source
                    .contains("self.run_sync_mut_operation(operation, submission)?"),
            "default-profile mutation should be owned by the canonical run dispatcher"
        );
        assert!(
            submission_source.contains("OperationDispatchMode::SyncMutable => {")
                && submission_source
                    .contains("self.run_sync_mut_operation(operation, submission)?"),
            "public run should own sync-mutable operation dispatch"
        );
        assert!(
            session_source.matches("OperationScheduler::admit(").count()
                + dispatch_source
                    .matches("OperationScheduler::admit(")
                    .count()
                >= 3,
            "the three canonical dispatchers should admit through OperationScheduler"
        );
        for expected in [
            "Self::ForkSession { .. } => OperationContract::ForkSession",
            "Self::SwitchActiveLeaf { .. } => OperationContract::SwitchActiveLeaf",
            "Self::SetDefaultAgentProfile { .. } => OperationContract::SetDefaultAgentProfile",
        ] {
            assert!(
                outcome_source.contains(expected),
                "authoritative operation descriptor mapping should include {expected}"
            );
        }
        assert!(
            operation_source.contains("descriptor_for_internal_operation(self)"),
            "internal operation metadata should derive from the authoritative descriptor"
        );
        assert!(
            outcome_source
                .matches("OperationDispatchMode::SyncMutable,")
                .count()
                >= 3,
            "fork, active-leaf switch, and default-profile mutation should be sync-mutable"
        );
    }
}

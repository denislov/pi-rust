use super::CodingSessionError;
use super::capability_snapshot::OperationCapabilitySnapshot;
#[cfg(test)]
use super::operation::Operation;
use super::operation::{OperationAdmission, OperationClass};
use super::operation_control::{
    OperationControl, OperationGuard, OperationKind, PromptControlHandle,
};
use super::scheduler::OperationScheduler;
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
    capability_snapshot: OperationCapabilitySnapshot,
    cancellation: Option<CancellationToken>,
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
        guard: OperationGuard,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        let cancellation = guard.cancellation_token();
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: Some(guard),
            capability_snapshot,
            cancellation,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    pub(crate) fn unguarded(
        kind: OperationKind,
        class: OperationClass,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: None,
            capability_snapshot,
            cancellation: None,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn capability_snapshot(&self) -> &OperationCapabilitySnapshot {
        &self.capability_snapshot
    }

    pub(crate) fn cancellation_token(&self) -> Option<CancellationToken> {
        self.cancellation.clone()
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
        self.guard.is_some()
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
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        if operation.static_kind().is_none() {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            });
        }
        let snapshot = OperationCapabilitySnapshot::permissive("op_static_admission");
        Ok(OperationAdmission::new(
            operation.kind(),
            metadata,
            None,
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

    pub(crate) fn unsupported_dispatch(admission: &OperationAdmission) -> CodingSessionError {
        CodingSessionError::UnsupportedCapability {
            capability: format!(
                "{} operation requires {} dispatcher",
                admission.kind.as_str(),
                admission.metadata.dispatch_mode.dispatcher_label(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::export_flow::ExportOptions;
    use crate::coding_session::operation::OperationDispatchMode;
    use crate::coding_session::operation::{Operation, OperationClass};
    use crate::coding_session::operation_control::{OperationKind, PromptControlCommand};

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
    fn read_only_admission_keeps_plugin_command_guarded() {
        let operation = Operation::PluginCommand {
            command_id: "plugin.echo".into(),
            args: serde_json::json!({}),
        };
        let admission = IntentRouter::static_admission(&operation).unwrap();
        let control = OperationControl::new();

        let permit =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncReadOnly)
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
    fn operation_permit_exposes_the_frozen_snapshot_for_execution() {
        use crate::coding_session::capability_snapshot::{
            ActorId, CapabilityGeneration, OperationCapabilitySnapshot, PluginCapabilitySet,
            ToolCapabilitySet,
        };
        use crate::coding_session::operation::{OperationMetadata, OperationOrigin};

        let control = OperationControl::new();
        let metadata = OperationMetadata {
            static_kind: Some(OperationKind::Export),
            origin: OperationOrigin::ClientRoot,
            class: OperationClass::ReadOnly,
            dispatch_mode: OperationDispatchMode::SyncReadOnly,
        };
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
        let admission =
            OperationAdmission::new(OperationKind::Export, metadata, None, snapshot.clone());

        let permit =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncReadOnly)
                .map_err(|rejection| rejection.into_error())
                .unwrap();

        assert_eq!(permit.capability_snapshot(), &snapshot);
    }

    #[test]
    fn operation_permit_exposes_frozen_snapshot_for_guarded_execution() {
        use crate::coding_session::capability_snapshot::{
            ActorId, CapabilityGeneration, OperationCapabilitySnapshot, PluginCapabilitySet,
            ToolCapabilitySet,
        };
        use crate::coding_session::operation::{OperationMetadata, OperationOrigin};

        let control = OperationControl::new();
        let metadata = OperationMetadata {
            static_kind: Some(OperationKind::PluginCommand),
            origin: OperationOrigin::ClientRoot,
            class: OperationClass::NonSessionRoot,
            dispatch_mode: OperationDispatchMode::SyncReadOnly,
        };
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
        let admission = OperationAdmission::new(
            OperationKind::PluginCommand,
            metadata,
            None,
            snapshot.clone(),
        );

        let permit =
            OperationScheduler::admit(&control, &admission, OperationDispatchMode::SyncReadOnly)
                .map_err(|rejection| rejection.into_error())
                .unwrap();

        assert!(permit.is_guarded());
        assert_eq!(permit.capability_snapshot(), &snapshot);
    }

    #[test]
    fn session_query_facade_routes_through_query_admission() {
        let source = [include_str!("mod.rs"), include_str!("session_view.rs")].concat();

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
        let session_source = include_str!("mod.rs");
        let operation_source = include_str!("operation.rs");

        assert!(
            !session_source.contains("fn set_default_agent_profile_id(")
                && session_source.contains("self.run_sync_mut_operation(operation, submission)?"),
            "default-profile mutation should be owned by the canonical run dispatcher"
        );
        assert!(
            session_source.contains("OperationDispatchMode::SyncMutable => {")
                && session_source.contains("self.run_sync_mut_operation(operation, submission)?"),
            "public run should own sync-mutable operation dispatch"
        );
        assert!(
            session_source.matches("OperationScheduler::admit(").count() >= 3,
            "the three canonical dispatchers should admit through OperationScheduler"
        );
        for expected in [
            "Self::ForkSession { .. } => OperationMetadata::new(",
            "Self::SwitchActiveLeaf { .. } => OperationMetadata::new(",
            "Self::SetDefaultAgentProfile { .. } => OperationMetadata::new(",
        ] {
            assert!(
                operation_source.contains(expected),
                "sync-mutable operation metadata should include {expected}"
            );
        }
        assert!(
            operation_source
                .matches("OperationDispatchMode::SyncMutable,")
                .count()
                >= 3,
            "fork, active-leaf switch, and default-profile mutation should be sync-mutable"
        );
    }
}

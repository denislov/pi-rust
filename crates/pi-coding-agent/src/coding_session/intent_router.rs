use super::CodingSessionError;
use super::capability_snapshot::OperationCapabilitySnapshot;
#[cfg(test)]
use super::operation::Operation;
use super::operation::{OperationAdmission, OperationClass, OperationDispatchMode};
use super::operation_control::{
    OperationControl, OperationGuard, OperationKind, PromptControlHandle,
};

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
    fn guarded(
        kind: OperationKind,
        class: OperationClass,
        guard: OperationGuard,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: Some(guard),
            capability_snapshot,
            #[cfg(test)]
            kind,
            #[cfg(test)]
            class,
        }
    }

    fn unguarded(
        kind: OperationKind,
        class: OperationClass,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Self {
        #[cfg(not(test))]
        let _ = (kind, class);

        Self {
            guard: None,
            capability_snapshot,
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
        let snapshot = OperationCapabilitySnapshot::permissive_for_tests("op_static_admission");
        Ok(OperationAdmission::new(
            operation.kind(),
            metadata,
            None,
            snapshot,
        ))
    }

    #[cfg(test)]
    pub(crate) fn begin(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<OperationGuard, CodingSessionError> {
        Self::validate_dispatch_mode(admission, expected)?;

        control.begin(admission.kind)
    }

    pub(crate) fn admit_operation(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<OperationPermit, CodingSessionError> {
        Self::validate_dispatch_mode(admission, expected)?;

        if admission.metadata.class == OperationClass::ReadOnly {
            return Ok(OperationPermit::unguarded(
                admission.kind,
                admission.metadata.class,
                admission.capability_snapshot.clone(),
            ));
        }

        control.begin(admission.kind).map(|guard| {
            OperationPermit::guarded(
                admission.kind,
                admission.metadata.class,
                guard,
                admission.capability_snapshot.clone(),
            )
        })
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
        let metadata = intent.metadata();
        debug_assert_eq!(metadata.class, OperationClass::Query);
        let _ = control;
        metadata
    }

    fn validate_dispatch_mode(
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<(), CodingSessionError> {
        if admission.metadata.dispatch_mode != expected {
            return Err(Self::unsupported_dispatch(admission));
        }
        Ok(())
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
        let guard = control.begin(OperationKind::PluginLoad).unwrap();

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
        let guard = control.begin(OperationKind::Prompt).unwrap();

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
        let guard = control.begin(OperationKind::Prompt).unwrap();

        let permit = IntentRouter::admit_operation(
            &control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
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

        let permit = IntentRouter::admit_operation(
            &control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
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

        let permit = IntentRouter::admit_operation(
            &control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
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

        let permit = IntentRouter::admit_operation(
            &control,
            &admission,
            OperationDispatchMode::SyncReadOnly,
        )
        .unwrap();

        assert!(permit.is_guarded());
        assert_eq!(permit.capability_snapshot(), &snapshot);
    }

    #[test]
    fn session_query_facade_routes_through_query_admission() {
        let source = include_str!("mod.rs");

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
    fn session_mutation_facade_routes_through_intent_admission() {
        let source = include_str!("mod.rs");

        assert!(
            source.contains("run_sync_mut_operation(Operation::SetDefaultAgentProfile"),
            "set_default_agent_profile_id should route through run_sync_mut_operation"
        );
        assert!(
            source.contains("Operation::ForkSession"),
            "fork_current_session should construct a ForkSession operation"
        );
        // 3 dispatcher admit_operation calls + 1 fork_current_session direct call = 4
        assert!(
            source.matches("IntentRouter::admit_operation(").count() >= 4,
            "session mutation should admit through IntentRouter (dispatchers + fork)"
        );
    }
}

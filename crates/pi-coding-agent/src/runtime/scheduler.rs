use super::capability::{ActorId, OperationCapabilitySnapshot};
use super::control::{OperationControl, OperationKind};
use super::intent::{OperationPermit, QueryIntent, QueryIntentMetadata};
use super::operation::{
    OperationClass, OperationDispatchMode, OperationExecution, OperationMetadata, OperationOrigin,
};
use crate::runtime::facade::CodingSessionError;

/// Typed admission owner for runtime-affecting operations.
///
/// The scheduler intentionally delegates guard ownership to `OperationControl`
/// during the first migration slice. This keeps cancellation and prompt-control
/// lifetimes stable while making admission policy explicit and testable.
pub(crate) struct OperationScheduler;

impl OperationScheduler {
    pub(crate) fn admit(
        control: &OperationControl,
        admission: &OperationExecution,
        expected_dispatch: OperationDispatchMode,
    ) -> Result<OperationPermit, AdmissionRejection> {
        if admission.metadata.dispatch_mode != expected_dispatch {
            return Err(AdmissionRejection::DispatchMismatch {
                kind: admission.kind,
                expected: expected_dispatch,
                actual: admission.metadata.dispatch_mode,
            });
        }

        match admission.metadata.class {
            OperationClass::Query | OperationClass::Child => {
                return Err(AdmissionRejection::DedicatedPathRequired {
                    kind: admission.kind,
                    class: admission.metadata.class,
                });
            }
            OperationClass::ReadOnly | OperationClass::Control => {
                return Ok(OperationPermit::unguarded(
                    admission.kind,
                    admission.metadata.class,
                    admission.clone(),
                ));
            }
            OperationClass::SessionWriteRoot
            | OperationClass::NonSessionRoot
            | OperationClass::RuntimeWrite => {}
        }

        control
            .begin_root_with_capability_generation(
                admission.metadata.class,
                admission.kind,
                admission.capability_snapshot.operation_id.clone(),
                admission.capability_snapshot.generation,
            )
            .map(|guard| {
                OperationPermit::guarded(
                    admission.kind,
                    admission.metadata.class,
                    guard,
                    admission.clone(),
                )
            })
            .map_err(AdmissionRejection::Control)
    }

    pub(crate) fn admit_query(
        _control: &OperationControl,
        intent: QueryIntent,
    ) -> QueryIntentMetadata {
        intent.metadata()
    }

    pub(crate) fn admit_child(
        control: &OperationControl,
        kind: OperationKind,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Result<OperationPermit, AdmissionRejection> {
        match &capability_snapshot.actor {
            ActorId::ChildOperation(parent_id) if !parent_id.is_empty() => {
                let parent_id = parent_id.clone();
                control
                    .begin_child_with_capability_generation(
                        kind,
                        capability_snapshot.operation_id.clone(),
                        parent_id,
                        capability_snapshot.generation,
                    )
                    .map(|guard| {
                        let execution = OperationExecution::child(
                            kind,
                            OperationMetadata {
                                static_kind: Some(kind),
                                origin: OperationOrigin::ParentChild,
                                class: OperationClass::Child,
                                dispatch_mode: OperationDispatchMode::Async,
                            },
                            capability_snapshot,
                            guard.parent_operation_id().to_owned(),
                            guard.root_operation_id().to_owned(),
                        );
                        OperationPermit::child(kind, execution, guard)
                    })
                    .map_err(AdmissionRejection::Control)
            }
            _ => Err(AdmissionRejection::ChildLineageMissing { kind }),
        }
    }

    #[cfg(test)]
    pub(crate) fn classify(
        kind: OperationKind,
        class: OperationClass,
        dispatch: OperationDispatchMode,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> OperationExecution {
        OperationExecution::root(
            kind,
            super::operation::OperationMetadata {
                static_kind: Some(kind),
                origin: super::operation::OperationOrigin::ClientRoot,
                class,
                dispatch_mode: dispatch,
            },
            None,
            Some("scheduler-test-session".into()),
            capability_snapshot,
        )
    }
}

#[derive(Debug)]
pub(crate) enum AdmissionRejection {
    DispatchMismatch {
        kind: OperationKind,
        expected: OperationDispatchMode,
        actual: OperationDispatchMode,
    },
    Control(CodingSessionError),
    ChildLineageMissing {
        kind: OperationKind,
    },
    DedicatedPathRequired {
        kind: OperationKind,
        class: OperationClass,
    },
}

impl AdmissionRejection {
    pub(crate) fn into_error(self) -> CodingSessionError {
        match self {
            Self::DispatchMismatch {
                kind,
                expected,
                actual,
            } => CodingSessionError::UnsupportedCapability {
                capability: format!(
                    "{} operation was sent to the wrong dispatcher (requires {}, received {})",
                    kind.as_str(),
                    expected.dispatcher_label(),
                    actual.dispatcher_label(),
                ),
            },
            Self::Control(error) => error,
            Self::ChildLineageMissing { kind } => CodingSessionError::UnsupportedCapability {
                capability: format!(
                    "{} child operation is missing a valid parent lineage",
                    kind.as_str()
                ),
            },
            Self::DedicatedPathRequired { kind, class } => {
                CodingSessionError::UnsupportedCapability {
                    capability: format!(
                        "{} {:?} operation requires its dedicated admission path",
                        kind.as_str(),
                        class,
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::operation::OperationOrigin;

    fn admission(class: OperationClass, dispatch: OperationDispatchMode) -> OperationExecution {
        OperationScheduler::classify(
            OperationKind::Export,
            class,
            dispatch,
            OperationCapabilitySnapshot::permissive("scheduler-test"),
        )
    }

    #[test]
    fn unguarded_classes_bypass_busy_root_guard() {
        let control = OperationControl::new();
        let root = control.begin(OperationKind::Prompt, "root".into()).unwrap();
        for (class, dispatch) in [
            (
                OperationClass::ReadOnly,
                OperationDispatchMode::SyncReadOnly,
            ),
            (OperationClass::Control, OperationDispatchMode::SyncMutable),
        ] {
            let permit = OperationScheduler::admit(&control, &admission(class, dispatch), dispatch)
                .expect("read-only and control operations should bypass the root guard");
            assert!(!permit.is_guarded());
            assert_eq!(permit.class(), class);
            assert_eq!(control.active(), Some(OperationKind::Prompt));
        }
        drop(root);
    }

    #[test]
    fn session_writer_and_non_session_root_use_independent_slots() {
        let control = OperationControl::new();
        let root = control.begin(OperationKind::Prompt, "root".into()).unwrap();

        let non_session = OperationScheduler::admit(
            &control,
            &admission(OperationClass::NonSessionRoot, OperationDispatchMode::Async),
            OperationDispatchMode::Async,
        )
        .expect("a non-session root may coexist with the session writer");
        assert_eq!(control.active(), Some(OperationKind::Prompt));

        let rejection = OperationScheduler::admit(
            &control,
            &admission(
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            OperationDispatchMode::Async,
        )
        .expect_err("a second session writer must be rejected");
        assert!(matches!(
            rejection,
            AdmissionRejection::Control(CodingSessionError::Busy { .. })
        ));

        drop(non_session);
        drop(root);
    }

    #[test]
    fn non_session_roots_obey_the_explicit_runtime_limit() {
        let control = OperationControl::with_non_session_root_limit(2);
        let first = control
            .begin_root(
                OperationClass::NonSessionRoot,
                OperationKind::AgentInvocation,
                "root-1".into(),
            )
            .unwrap();
        let second = control
            .begin_root(
                OperationClass::NonSessionRoot,
                OperationKind::AgentTeam,
                "root-2".into(),
            )
            .unwrap();

        assert_eq!(
            control
                .begin_root(
                    OperationClass::NonSessionRoot,
                    OperationKind::PluginCommand,
                    "root-3".into(),
                )
                .unwrap_err(),
            CodingSessionError::Busy {
                operation: "agent_invocation".into(),
            }
        );

        drop(first);
        assert!(
            control
                .begin_root(
                    OperationClass::NonSessionRoot,
                    OperationKind::PluginCommand,
                    "root-3".into(),
                )
                .is_ok()
        );
        drop(second);
    }

    #[test]
    fn runtime_write_is_exclusive_in_both_directions() {
        let control = OperationControl::new();
        let session_write = control
            .begin(OperationKind::Prompt, "session".into())
            .unwrap();
        let rejection = control
            .begin_root(
                OperationClass::RuntimeWrite,
                OperationKind::PluginLoad,
                "runtime".into(),
            )
            .unwrap_err();
        assert_eq!(
            rejection,
            CodingSessionError::Busy {
                operation: "prompt".into(),
            }
        );
        drop(session_write);

        let runtime_write = control
            .begin_root(
                OperationClass::RuntimeWrite,
                OperationKind::PluginLoad,
                "runtime".into(),
            )
            .unwrap();
        for (class, kind) in [
            (OperationClass::SessionWriteRoot, OperationKind::Prompt),
            (
                OperationClass::NonSessionRoot,
                OperationKind::AgentInvocation,
            ),
            (OperationClass::RuntimeWrite, OperationKind::PluginLoad),
        ] {
            assert!(matches!(
                control.begin_root(class, kind, "blocked".into()),
                Err(CodingSessionError::Busy { operation }) if operation == "plugin_load"
            ));
        }
        drop(runtime_write);
    }

    #[test]
    fn query_and_child_classes_require_dedicated_admission_paths() {
        let control = OperationControl::new();
        for class in [OperationClass::Query, OperationClass::Child] {
            let rejection = OperationScheduler::admit(
                &control,
                &admission(class, OperationDispatchMode::Async),
                OperationDispatchMode::Async,
            )
            .expect_err("query and child classes must not bypass dedicated admission");
            assert!(matches!(
                rejection,
                AdmissionRejection::DedicatedPathRequired {
                    kind: OperationKind::Export,
                    class: rejected_class,
                } if rejected_class == class
            ));
        }
        assert_eq!(control.active(), None);
    }

    #[test]
    fn dispatch_mismatch_is_rejected_before_control_mutation() {
        let control = OperationControl::new();
        let rejection = OperationScheduler::admit(
            &control,
            &admission(
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            OperationDispatchMode::SyncMutable,
        )
        .expect_err("dispatch mismatch must fail closed");
        assert!(matches!(
            rejection,
            AdmissionRejection::DispatchMismatch { .. }
        ));
        assert_eq!(control.active(), None);
    }

    #[test]
    fn classified_admission_keeps_client_root_origin() {
        let admission = admission(OperationClass::RuntimeWrite, OperationDispatchMode::Async);
        assert_eq!(admission.metadata.origin, OperationOrigin::ClientRoot);
        assert_eq!(admission.metadata.class, OperationClass::RuntimeWrite);
    }

    #[test]
    fn child_admission_accepts_only_nonempty_parent_lineage() {
        let control = OperationControl::new();
        let _root = control
            .begin(OperationKind::AgentInvocation, "parent-op".into())
            .unwrap();
        let mut child = OperationCapabilitySnapshot::permissive("child-op");
        child.actor = ActorId::ChildOperation("parent-op".into());
        let permit = OperationScheduler::admit_child(&control, OperationKind::Prompt, child)
            .expect("child actor with active parent lineage should be admitted");
        assert!(permit.is_guarded());
        assert_eq!(permit.class(), OperationClass::Child);
        assert_eq!(permit.execution().operation_id, "child-op");
        assert_eq!(
            permit.execution().parent_operation_id.as_deref(),
            Some("parent-op")
        );
        assert_eq!(
            permit.execution().root_operation_id.as_deref(),
            Some("parent-op")
        );
        assert_eq!(control.child_count(), 1);
        drop(permit);
        assert_eq!(control.child_count(), 0);

        let root = OperationCapabilitySnapshot::permissive("root-op");
        let rejection =
            OperationScheduler::admit_child(&control, OperationKind::AgentInvocation, root)
                .expect_err("root actor must not enter the child admission path");
        assert!(matches!(
            rejection,
            AdmissionRejection::ChildLineageMissing {
                kind: OperationKind::AgentInvocation
            }
        ));
    }
}

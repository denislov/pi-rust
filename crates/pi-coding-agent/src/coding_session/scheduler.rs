use super::CodingSessionError;
use super::capability_snapshot::{ActorId, OperationCapabilitySnapshot};
use super::intent_router::{OperationPermit, QueryIntent, QueryIntentMetadata};
use super::operation::{OperationAdmission, OperationClass, OperationDispatchMode};
use super::operation_control::{OperationControl, OperationKind};

/// Typed admission owner for runtime-affecting operations.
///
/// The scheduler intentionally delegates guard ownership to `OperationControl`
/// during the first migration slice. This keeps cancellation and prompt-control
/// lifetimes stable while making admission policy explicit and testable.
pub(crate) struct OperationScheduler;

impl OperationScheduler {
    pub(crate) fn admit(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected_dispatch: OperationDispatchMode,
    ) -> Result<OperationPermit, AdmissionRejection> {
        if admission.metadata.dispatch_mode != expected_dispatch {
            return Err(AdmissionRejection::DispatchMismatch {
                kind: admission.kind,
                expected: expected_dispatch,
                actual: admission.metadata.dispatch_mode,
            });
        }

        if admission.metadata.class == OperationClass::ReadOnly {
            return Ok(OperationPermit::unguarded(
                admission.kind,
                admission.metadata.class,
                admission.capability_snapshot.clone(),
            ));
        }

        control
            .begin(
                admission.kind,
                admission.capability_snapshot.operation_id.clone(),
            )
            .map(|guard| {
                OperationPermit::guarded(
                    admission.kind,
                    admission.metadata.class,
                    guard,
                    admission.capability_snapshot.clone(),
                )
            })
            .map_err(|error| AdmissionRejection::Control(error))
    }

    pub(crate) fn admit_query(
        _control: &OperationControl,
        intent: QueryIntent,
    ) -> QueryIntentMetadata {
        intent.metadata()
    }

    pub(crate) fn admit_child(
        kind: OperationKind,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> Result<OperationPermit, AdmissionRejection> {
        match &capability_snapshot.actor {
            ActorId::ChildOperation(parent_id) if !parent_id.is_empty() => Ok(
                OperationPermit::unguarded(kind, OperationClass::Child, capability_snapshot),
            ),
            _ => Err(AdmissionRejection::ChildLineageMissing { kind }),
        }
    }

    pub(crate) fn classify(
        kind: OperationKind,
        class: OperationClass,
        dispatch: OperationDispatchMode,
        capability_snapshot: OperationCapabilitySnapshot,
    ) -> OperationAdmission {
        OperationAdmission::new(
            kind,
            super::operation::OperationMetadata {
                static_kind: Some(kind),
                origin: super::operation::OperationOrigin::ClientRoot,
                class,
                dispatch_mode: dispatch,
            },
            None,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::operation::OperationOrigin;

    fn admission(class: OperationClass, dispatch: OperationDispatchMode) -> OperationAdmission {
        OperationScheduler::classify(
            OperationKind::Export,
            class,
            dispatch,
            OperationCapabilitySnapshot::permissive("scheduler-test"),
        )
    }

    #[test]
    fn read_only_admission_bypasses_busy_root_guard() {
        let control = OperationControl::new();
        let root = control.begin(OperationKind::Prompt, "root".into()).unwrap();
        let permit = OperationScheduler::admit(
            &control,
            &admission(
                OperationClass::ReadOnly,
                OperationDispatchMode::SyncReadOnly,
            ),
            OperationDispatchMode::SyncReadOnly,
        )
        .expect("read-only operation should be admitted while root is active");
        assert!(!permit.is_guarded());
        assert_eq!(control.active(), Some(OperationKind::Prompt));
        drop(permit);
        drop(root);
    }

    #[test]
    fn write_admission_returns_typed_busy_rejection() {
        let control = OperationControl::new();
        let root = control.begin(OperationKind::Prompt, "root".into()).unwrap();
        let rejection = OperationScheduler::admit(
            &control,
            &admission(
                OperationClass::SessionWriteRoot,
                OperationDispatchMode::Async,
            ),
            OperationDispatchMode::Async,
        )
        .expect_err("second write must be rejected while root is active");
        assert!(matches!(
            rejection,
            AdmissionRejection::Control(CodingSessionError::Busy { .. })
        ));
        drop(root);
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
        let mut child = OperationCapabilitySnapshot::permissive("child-op");
        child.actor = ActorId::ChildOperation("parent-op".into());
        let permit = OperationScheduler::admit_child(OperationKind::AgentInvocation, child)
            .expect("child actor with parent lineage should be admitted");
        assert!(!permit.is_guarded());
        assert_eq!(permit.class(), OperationClass::Child);

        let root = OperationCapabilitySnapshot::permissive("root-op");
        let rejection = OperationScheduler::admit_child(OperationKind::AgentInvocation, root)
            .expect_err("root actor must not enter the child admission path");
        assert!(matches!(
            rejection,
            AdmissionRejection::ChildLineageMissing {
                kind: OperationKind::AgentInvocation
            }
        ));
    }
}

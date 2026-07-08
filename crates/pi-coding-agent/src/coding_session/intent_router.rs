use super::CodingSessionError;
use super::operation::{Operation, OperationAdmission, OperationDispatchMode};
use super::operation_control::{OperationControl, OperationGuard};

pub(crate) struct IntentRouter;

impl IntentRouter {
    pub(crate) fn static_admission(
        operation: &Operation,
    ) -> Result<OperationAdmission, CodingSessionError> {
        let metadata = operation.metadata();
        if operation.static_kind().is_none() {
            return Err(CodingSessionError::UnsupportedCapability {
                capability: "dynamic operation requires async dispatcher".into(),
            });
        }
        Ok(OperationAdmission::new(operation.kind(), metadata, None))
    }

    pub(crate) fn begin(
        control: &OperationControl,
        admission: &OperationAdmission,
        expected: OperationDispatchMode,
    ) -> Result<OperationGuard, CodingSessionError> {
        if admission.metadata.dispatch_mode != expected {
            return Err(Self::unsupported_dispatch(admission));
        }

        control.begin(admission.kind)
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

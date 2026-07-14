use super::client_projection::ClientConnectionId;
use super::client_projection::UiSnapshot;
use super::snapshot_coordinator::{
    ClientHandle, ClientRegistryError, DraftRecord, SnapshotCoordinator,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct ClientService {
    pub(crate) coordinator: Arc<SnapshotCoordinator>,
}

impl ClientService {
    pub(crate) fn new(coordinator: Arc<SnapshotCoordinator>) -> Self {
        Self { coordinator }
    }
    pub(crate) fn connect_or_takeover(
        &self,
        id: ClientConnectionId,
    ) -> Result<ClientHandle, ClientRegistryError> {
        self.coordinator.connect_or_takeover(id)
    }
    pub(crate) fn client_snapshot(
        &self,
        handle: &ClientHandle,
    ) -> Result<UiSnapshot, ClientRegistryError> {
        self.coordinator.client_snapshot(handle)
    }
    pub(crate) fn set_prompt_draft(
        &self,
        handle: &ClientHandle,
        draft: Option<DraftRecord>,
    ) -> Result<(), ClientRegistryError> {
        self.coordinator.set_prompt_draft(handle, draft)
    }
    pub(crate) fn enqueue_control_draft(
        &self,
        handle: &ClientHandle,
        draft: DraftRecord,
    ) -> Result<(), ClientRegistryError> {
        self.coordinator.enqueue_draft(handle, draft)
    }
    pub(crate) fn commit_submission_running(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        descriptor: super::public_operation::OperationDescriptor,
        expected_prompt_draft: Option<&DraftRecord>,
    ) -> Result<(), ClientRegistryError> {
        self.coordinator.commit_submission_running(
            handle,
            operation_id,
            descriptor,
            expected_prompt_draft,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::client_projection::ClientDraftKind;

    #[test]
    fn takeover_increments_generation_and_rejects_old_handle() {
        let service = ClientService::new(SnapshotCoordinator::new());
        let id = ClientConnectionId::new("client");
        let first = service.connect_or_takeover(id.clone()).unwrap();
        let second = service.connect_or_takeover(id).unwrap();
        assert_eq!(first.generation.0, 1);
        assert_eq!(second.generation.0, 2);
    }

    #[test]
    fn queue_is_bounded_and_duplicate_ids_update_in_place() {
        let service = ClientService::new(SnapshotCoordinator::new());
        let handle = service
            .connect_or_takeover(ClientConnectionId::new("client"))
            .unwrap();
        service
            .enqueue_control_draft(
                &handle,
                DraftRecord {
                    id: "d1".into(),
                    kind: ClientDraftKind::Steer,
                    text: "one".into(),
                },
            )
            .unwrap();
        service
            .enqueue_control_draft(
                &handle,
                DraftRecord {
                    id: "d1".into(),
                    kind: ClientDraftKind::Steer,
                    text: "two".into(),
                },
            )
            .unwrap();
        let state = service.coordinator.state.lock().unwrap();
        assert_eq!(state.clients[&handle.id].steer_drafts.len(), 1);
        assert_eq!(state.clients[&handle.id].steer_drafts[0].text, "two");
    }
}

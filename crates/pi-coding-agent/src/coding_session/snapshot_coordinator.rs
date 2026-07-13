use super::client_projection::ClientConnectionId;
use super::operation_control::OperationKind;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

pub(crate) const MAX_CLIENTS: usize = 64;
pub(crate) const MAX_DRAFTS: usize = 64;
pub(crate) const MAX_RECEIPTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ClientGeneration(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientHandle {
    pub(crate) id: ClientConnectionId,
    pub(crate) generation: ClientGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerminalAcknowledgementAnchor {
    pub(crate) operation_id: String,
    pub(crate) terminal_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmittedOperationStatus {
    Accepted {
        operation_id: String,
        kind: OperationKind,
    },
    Running {
        operation_id: String,
        kind: OperationKind,
    },
    Terminal {
        operation_id: String,
        kind: OperationKind,
        anchor: TerminalAcknowledgementAnchor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftRecord {
    pub(crate) id: String,
    pub(crate) kind: super::client_projection::ClientDraftKind,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientRecord {
    pub(crate) generation: ClientGeneration,
    pub(crate) acknowledged_sequence: u64,
    pub(crate) prompt_draft: Option<DraftRecord>,
    pub(crate) steer_drafts: VecDeque<DraftRecord>,
    pub(crate) follow_up_drafts: VecDeque<DraftRecord>,
    pub(crate) submitted_operation: Option<SubmittedOperationStatus>,
    pub(crate) control_receipts: HashMap<String, String>,
    pub(crate) control_receipt_order: VecDeque<String>,
}

impl ClientRecord {
    fn new(generation: ClientGeneration) -> Self {
        Self {
            generation,
            acknowledged_sequence: 0,
            prompt_draft: None,
            steer_drafts: VecDeque::new(),
            follow_up_drafts: VecDeque::new(),
            submitted_operation: None,
            control_receipts: HashMap::new(),
            control_receipt_order: VecDeque::new(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct SnapshotState {
    pub(crate) clients: HashMap<ClientConnectionId, ClientRecord>,
}

#[derive(Debug, Default)]
pub(crate) struct SnapshotCoordinator {
    pub(crate) state: Mutex<SnapshotState>,
}

impl SnapshotCoordinator {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn connect_or_takeover(
        &self,
        id: ClientConnectionId,
    ) -> Result<ClientHandle, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        if let Some(record) = state.clients.get_mut(&id) {
            record.generation.0 += 1;
            return Ok(ClientHandle {
                id,
                generation: record.generation,
            });
        }
        if state.clients.len() >= MAX_CLIENTS {
            return Err(ClientRegistryError::ClientCapacityExceeded { limit: MAX_CLIENTS });
        }
        let generation = ClientGeneration(1);
        state
            .clients
            .insert(id.clone(), ClientRecord::new(generation));
        Ok(ClientHandle { id, generation })
    }

    fn record<'a>(
        state: &'a mut SnapshotState,
        handle: &ClientHandle,
    ) -> Result<&'a mut ClientRecord, ClientRegistryError> {
        let record = state
            .clients
            .get_mut(&handle.id)
            .ok_or(ClientRegistryError::StaleClient)?;
        if record.generation != handle.generation {
            return Err(ClientRegistryError::StaleClient);
        }
        Ok(record)
    }

    pub(crate) fn acknowledge(
        &self,
        handle: &ClientHandle,
        sequence: u64,
    ) -> Result<u64, ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if sequence < record.acknowledged_sequence {
            return Ok(record.acknowledged_sequence);
        }
        record.acknowledged_sequence = sequence;
        if let Some(SubmittedOperationStatus::Terminal { anchor, .. }) = &record.submitted_operation
        {
            if sequence >= anchor.terminal_sequence {
                record.submitted_operation = None;
            }
        }
        Ok(record.acknowledged_sequence)
    }

    pub(crate) fn set_prompt_draft(
        &self,
        handle: &ClientHandle,
        draft: Option<DraftRecord>,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        Self::record(&mut state, handle)?.prompt_draft = draft;
        Ok(())
    }

    pub(crate) fn enqueue_draft(
        &self,
        handle: &ClientHandle,
        draft: DraftRecord,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        let queue = match draft.kind {
            super::client_projection::ClientDraftKind::Steer => &mut record.steer_drafts,
            super::client_projection::ClientDraftKind::FollowUp => &mut record.follow_up_drafts,
            super::client_projection::ClientDraftKind::Prompt => {
                return Err(ClientRegistryError::InvalidInput);
            }
        };
        if queue.iter().any(|item| item.id == draft.id) {
            if let Some(item) = queue.iter_mut().find(|item| item.id == draft.id) {
                *item = draft;
            }
            return Ok(());
        }
        if queue.len() >= MAX_DRAFTS {
            return Err(ClientRegistryError::QueueCapacityExceeded { limit: MAX_DRAFTS });
        }
        queue.push_back(draft);
        Ok(())
    }

    pub(crate) fn mark_submitted(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if record.submitted_operation.is_some() {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation =
            Some(SubmittedOperationStatus::Accepted { operation_id, kind });
        if kind == OperationKind::Prompt {
            record.prompt_draft = None;
        }
        Ok(())
    }

    pub(crate) fn mark_running(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        match &record.submitted_operation {
            Some(SubmittedOperationStatus::Accepted {
                operation_id: stored_id,
                kind: stored_kind,
            }) if stored_id == &operation_id && *stored_kind == kind => {
                record.submitted_operation = Some(SubmittedOperationStatus::Running {
                    operation_id: operation_id.clone(),
                    kind,
                });
                Ok(())
            }
            _ => {
                let _ = (operation_id, kind);
                Err(ClientRegistryError::SubmittedRegression)
            }
        }
    }

    pub(crate) fn mark_terminal(
        &self,
        handle: &ClientHandle,
        operation_id: String,
        kind: OperationKind,
        terminal_sequence: u64,
    ) -> Result<(), ClientRegistryError> {
        let mut state = self.state.lock().unwrap();
        let record = Self::record(&mut state, handle)?;
        if !matches!(
            record.submitted_operation,
            Some(
                SubmittedOperationStatus::Accepted { .. }
                    | SubmittedOperationStatus::Running { .. }
            )
        ) {
            return Err(ClientRegistryError::SubmittedRegression);
        }
        record.submitted_operation = Some(SubmittedOperationStatus::Terminal {
            operation_id: operation_id.clone(),
            kind,
            anchor: TerminalAcknowledgementAnchor {
                operation_id,
                terminal_sequence,
            },
        });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ClientRegistryError {
    #[error("stale client connection")]
    StaleClient,
    #[error("client capacity exceeded: {limit}")]
    ClientCapacityExceeded { limit: usize },
    #[error("draft queue capacity exceeded: {limit}")]
    QueueCapacityExceeded { limit: usize },
    #[error("accepted receipt capacity exceeded: {limit}")]
    ReceiptCapacityExceeded { limit: usize },
    #[error("invalid client input")]
    InvalidInput,
    #[error("submitted operation transition regressed")]
    SubmittedRegression,
}

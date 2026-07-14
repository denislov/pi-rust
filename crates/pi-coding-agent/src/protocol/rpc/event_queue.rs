use crate::coding_session::ProductEvent;
use tokio::sync::mpsc;

pub(super) const RPC_PRODUCT_EVENT_QUEUE_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RpcQueuedProductEvent {
    Event(ProductEvent),
    Overflow { skipped: u64 },
}

#[derive(Clone)]
pub(super) struct RpcProductEventQueue {
    event_sender: mpsc::Sender<ProductEvent>,
    control_sender: mpsc::Sender<RpcQueuedProductEvent>,
}

pub(super) struct RpcProductEventReceiver {
    event_receiver: mpsc::Receiver<ProductEvent>,
    control_receiver: mpsc::Receiver<RpcQueuedProductEvent>,
}

impl RpcProductEventQueue {
    pub(super) fn new() -> (Self, RpcProductEventReceiver) {
        Self::with_capacity(RPC_PRODUCT_EVENT_QUEUE_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> (Self, RpcProductEventReceiver) {
        let capacity = capacity.max(1);
        let (event_sender, event_receiver) = mpsc::channel(capacity);
        let (control_sender, control_receiver) = mpsc::channel(1);
        (
            Self {
                event_sender,
                control_sender,
            },
            RpcProductEventReceiver {
                event_receiver,
                control_receiver,
            },
        )
    }

    #[cfg(test)]
    pub(super) fn for_tests(capacity: usize) -> (Self, RpcProductEventReceiver) {
        Self::with_capacity(capacity)
    }

    pub(super) async fn send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::SendError<ProductEvent>> {
        self.event_sender.send(event).await
    }

    pub(super) async fn send_overflow(
        &self,
        skipped: u64,
    ) -> Result<(), mpsc::error::SendError<RpcQueuedProductEvent>> {
        self.control_sender
            .send(RpcQueuedProductEvent::Overflow { skipped })
            .await
    }

    #[cfg(test)]
    pub(super) fn try_send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::TrySendError<ProductEvent>> {
        self.event_sender.try_send(event)
    }
}

impl RpcProductEventReceiver {
    pub(super) async fn recv(&mut self) -> Option<RpcQueuedProductEvent> {
        if let Ok(item) = self.control_receiver.try_recv() {
            return Some(item);
        }
        tokio::select! {
            biased;
            control = self.control_receiver.recv() => control,
            event = self.event_receiver.recv() => event.map(RpcQueuedProductEvent::Event),
        }
    }

    pub(super) fn try_recv(&mut self) -> Result<RpcQueuedProductEvent, mpsc::error::TryRecvError> {
        match self.control_receiver.try_recv() {
            Ok(item) => Ok(item),
            Err(mpsc::error::TryRecvError::Empty) => self
                .event_receiver
                .try_recv()
                .map(RpcQueuedProductEvent::Event),
            Err(mpsc::error::TryRecvError::Disconnected) => self
                .event_receiver
                .try_recv()
                .map(RpcQueuedProductEvent::Event),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{CodingAgentEvent, ProductEvent, ProductEventSequence};

    fn event(sequence: u64) -> ProductEvent {
        ProductEvent::from_event_for_tests(
            ProductEventSequence::new(sequence),
            CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: format!("event {sequence}"),
            },
        )
    }

    #[tokio::test]
    async fn rpc_product_event_queue_is_bounded_and_ordered() {
        let (sender, mut receiver) = RpcProductEventQueue::for_tests(2);

        sender.send_event(event(1)).await.unwrap();
        sender.send_event(event(2)).await.unwrap();

        assert!(matches!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Event(product_event) if product_event.sequence() == ProductEventSequence::new(1)
        ));
        assert!(matches!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Event(product_event) if product_event.sequence() == ProductEventSequence::new(2)
        ));
    }

    #[tokio::test]
    async fn rpc_product_event_queue_can_report_overflow_recovery() {
        let (sender, mut receiver) = RpcProductEventQueue::for_tests(1);

        sender.send_overflow(3).await.unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Overflow { skipped: 3 }
        );
    }

    #[tokio::test]
    async fn rpc_product_event_queue_prioritizes_overflow_over_full_data_lane() {
        let (sender, mut receiver) = RpcProductEventQueue::for_tests(1);
        sender.try_send_event(event(1)).unwrap();
        sender.send_overflow(2).await.unwrap();

        assert_eq!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Overflow { skipped: 2 }
        );
        assert!(matches!(
            receiver.recv().await.unwrap(),
            RpcQueuedProductEvent::Event(product_event)
                if product_event.sequence() == ProductEventSequence::new(1)
        ));
    }
}

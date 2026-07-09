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
    sender: mpsc::Sender<RpcQueuedProductEvent>,
}

impl RpcProductEventQueue {
    pub(super) fn new() -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        Self::with_capacity(RPC_PRODUCT_EVENT_QUEUE_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        (Self { sender }, receiver)
    }

    #[cfg(test)]
    pub(super) fn for_tests(capacity: usize) -> (Self, mpsc::Receiver<RpcQueuedProductEvent>) {
        Self::with_capacity(capacity)
    }

    pub(super) async fn send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::SendError<RpcQueuedProductEvent>> {
        self.sender.send(RpcQueuedProductEvent::Event(event)).await
    }

    pub(super) async fn send_overflow(
        &self,
        skipped: u64,
    ) -> Result<(), mpsc::error::SendError<RpcQueuedProductEvent>> {
        self.sender
            .send(RpcQueuedProductEvent::Overflow { skipped })
            .await
    }

    #[cfg(test)]
    pub(super) fn try_send_event(
        &self,
        event: ProductEvent,
    ) -> Result<(), mpsc::error::TrySendError<RpcQueuedProductEvent>> {
        self.sender.try_send(RpcQueuedProductEvent::Event(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding_session::{CodingAgentEvent, ProductEvent, ProductEventSequence};

    fn event(sequence: u64) -> ProductEvent {
        ProductEvent::from_compat_event(
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
}

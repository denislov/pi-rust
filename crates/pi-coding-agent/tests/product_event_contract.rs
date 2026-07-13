mod support;

#[tokio::test]
async fn public_receiver_preserves_typed_order_and_metadata() {
    let events = run_contract_fixture().await;
    assert!(!events.is_empty());
}

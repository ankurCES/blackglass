use blackglass_core::broker::{ConfirmationBroker, Decision};

#[tokio::test]
async fn allow_resolves_pending() {
    let broker = ConfirmationBroker::new();
    let (id, rx) = broker.register().await;
    let broker2 = broker.clone();
    let id2 = id.clone();
    let pending = tokio::spawn(async move {
        broker2.resolve(&id2, Decision::Allow).await
    });
    let decision = rx.await.unwrap();
    assert_eq!(decision, Decision::Allow);
    pending.await.unwrap().unwrap();
    assert!(broker.is_empty().await);
}

#[tokio::test]
async fn resolve_unknown_id_returns_err() {
    // Documents the `deny_late` invariant: if the chokepoint has already
    // timed out and removed the pending entry, the operator socket's
    // late `resolve` call gets Err and emits a second
    // OperatorConfirmationResolved{decision: "deny_late"} event.
    let broker = ConfirmationBroker::new();
    let result = broker.resolve("00000000-0000-0000-0000-000000000000", Decision::Allow).await;
    assert!(result.is_err());
}

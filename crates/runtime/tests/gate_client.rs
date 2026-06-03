use blackglass_audit::Chain;
use blackglass_core::{
    chokepoint::Chokepoint,
    gates::{AllowAll, Gate3, Gate4},
    server::Server,
};
use blackglass_engagement::{Engagement, Target, TargetKind};
use blackglass_profile::Profile;
use blackglass_runtime::GateClient;
use std::{sync::Arc, time::Duration};
use tempfile::tempdir;

#[tokio::test]
async fn gate_client_ping_succeeds_after_auth() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("r.sock");
    let audit = dir.path().join("a.jsonl");

    let chain = Chain::open(&audit).unwrap();
    let mut eng = Engagement::new("e", "t", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let cp = Chokepoint::new(
        chain, Profile::analyst_default(), eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
    );

    let server = Server::bind(&sock, "tok".into(), cp).await.unwrap();
    let rt = tokio::runtime::Handle::current();
    let _h = std::thread::spawn(move || {
        rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(3), server.serve()).await;
        });
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = GateClient::new(sock, "tok".to_string());
    client.ping().await.expect("ping should succeed after auth");
}

#[tokio::test]
async fn gate_client_execute_action_round_trips() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("r2.sock");
    let audit = dir.path().join("a2.jsonl");

    let chain = Chain::open(&audit).unwrap();
    let mut eng = Engagement::new("e", "t", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let cp = Chokepoint::new(
        chain, Profile::analyst_default(), eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
    );

    let server = Server::bind(&sock, "tok".into(), cp).await.unwrap();
    let rt = tokio::runtime::Handle::current();
    let _h = std::thread::spawn(move || {
        rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(3), server.serve()).await;
        });
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = GateClient::new(sock, "tok".to_string());
    let outcome = client
        .execute("osint", "read_only", "10.0.0.5", serde_json::json!({}))
        .await
        .expect("execute should succeed");
    assert!(!outcome.stdout.is_empty(), "stdout should not be empty");
}

//! End-to-end smoke test for the Gate 3 operator confirmation wiring.
//!
//! Sub-plan 3 §13 of the design doc identified a pre-existing wiring gap:
//! production `main.rs` was hardcoding `AllowAll` for Gate 3, so destructive
//! actions were silently approved without operator confirmation. The fix
//! swaps in `BrokerGate3` and shares a `ConfirmChannel` between
//! `BrokerGate3` and `operator_server::run`, so a `confirm.request` pushed
//! by the chokepoint is forwarded verbatim to connected operator clients
//! (e.g. the Tauri UI).
//!
//! This test exercises the full happy path without ever launching a
//! subprocess: it stands up the `Server` (runtime RPC) and the
//! `operator_server::run` (Tauri-facing) in-process, connects two
//! `UnixStream` clients (one to each), and verifies that:
//!
//! 1. The runtime client sends an `execute_action` and *blocks* on it
//!    (because Gate 3 is now waiting for operator confirmation).
//! 2. The operator client receives a `confirm.request` notification
//!    carrying the action's domain/class/target.
//! 3. The operator client sends `confirm.resolve{decision: "allow"}`.
//! 4. The runtime client's execute returns a successful outcome.
//! 5. The audit log contains `ActionRequested` followed by
//!    `ActionExecuted` (proving the action ran, not denied).

use blackglass_audit::Chain;
use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::{BrokerGate3, Gate3, Gate4};
use blackglass_core::operator_server::{run as run_operator, ConfirmChannel};
use blackglass_core::sanitizer::RealSanitizer;
use blackglass_core::server::Server;
use blackglass_engagement::{Engagement, Target, TargetKind};
use blackglass_ipc::encode_frame;
use blackglass_profile::Profile;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn destructive_action_requires_operator_allow() {
    // --- arrange ----------------------------------------------------------
    let dir = tempdir().unwrap();
    let runtime_sock = dir.path().join("runtime.sock");
    let operator_sock = dir.path().join("operator.sock");
    let audit_path = dir.path().join("audit.jsonl");
    let evidence_dir = dir.path().join("evidence");
    std::fs::create_dir_all(&evidence_dir).unwrap();

    // Build a profile that allows `destructive` so the request actually
    // reaches Gate 3 (otherwise Gate 1 denies it before Gate 3 is called).
    // The default `Profile::analyst_default()` only allows `read_only`,
    // which is exactly the gap that §13 was about: we want to *prove*
    // Gate 3 fires for destructive, not that Gate 1 denies it.
    let profile = Profile {
        name: "operator-test".into(),
        tier: blackglass_profile::Tier::Operator,
        allowed_domains: vec!["core".into(), "osint".into()],
        allowed_action_classes: vec!["read_only".into(), "destructive".into()],
    };

    let mut eng = Engagement::new("e", "t", "2020-01-01T00:00:00Z", "2099-12-31T23:59:59Z");
    eng.add_target(Target {
        value: "10.0.0.5".into(),
        kind: TargetKind::Ip,
    });

    let chain = Chain::open(&audit_path).unwrap();
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let gate3: Arc<dyn Gate3> = Arc::new(BrokerGate3::new(
        broker.clone(),
        channel.clone(),
        "osint",
        "smoke",
    ));

    // Sanitizer: not exercised meaningfully here (no real output), but
    // must exist as a Gate 4 to satisfy Chokepoint::new.
    let gate4: Arc<dyn Gate4> = Arc::new(RealSanitizer::new(100 * 1024, evidence_dir.clone()));
    let cp = Chokepoint::new(chain, profile, eng, gate3, gate4).with_evidence_dir(evidence_dir);
    let server = Server::bind(&runtime_sock, "tok".into(), cp).await.unwrap();

    // --- act: start both servers in the background -----------------------
    let server_handle = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), server.serve()).await;
    });

    let op_broker = broker.clone();
    let op_channel = channel.clone();
    let op_sock = operator_sock.clone();
    let operator_handle = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), run_operator(&op_sock, op_broker, op_channel)).await;
    });

    // Wait for both sockets to come up.
    for path in [&runtime_sock, &operator_sock] {
        for _ in 0..50 {
            if path.exists() { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(path.exists(), "socket did not appear: {}", path.display());
    }

    // --- 1. Operator client connects and starts reading -------------------
    let op_client = UnixStream::connect(&operator_sock).await.unwrap();
    let (op_read, mut op_write) = op_client.into_split();
    let mut op_lines = tokio::io::BufReader::new(op_read).lines();

    // --- 2. Runtime client connects, authenticates, sends execute --------
    let mut rt_client = UnixStream::connect(&runtime_sock).await.unwrap();

    // Auth
    let auth = serde_json::json!({"id": 1, "method": "auth", "token": "tok"});
    rt_client.write_all(&encode_frame(auth.to_string().as_bytes())).await.unwrap();

    let mut lenb = [0u8; 4];
    rt_client.read_exact(&mut lenb).await.unwrap();
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    rt_client.read_exact(&mut buf).await.unwrap();
    let auth_resp: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert!(auth_resp["ok"].as_bool().unwrap_or(false), "auth should succeed");

    // Execute
    let exec = serde_json::json!({
        "id": 2,
        "method": "execute_action",
        "domain": "osint",
        "action_class": "destructive",
        "target": "10.0.0.5",
        "args": {},
    });
    rt_client.write_all(&encode_frame(exec.to_string().as_bytes())).await.unwrap();

    // The execute should now block (Gate 3 is awaiting the broker). Give
    // it a moment to register the pending confirmation, then read from
    // the operator socket.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // --- 3. Operator client receives confirm.request ---------------------
    let line = tokio::time::timeout(Duration::from_secs(2), op_lines.next_line())
        .await
        .expect("operator should receive confirm.request within 2s")
        .expect("read should not EOF")
        .expect("read should not error");
    assert!(
        line.contains("\"method\":\"confirm.request\""),
        "expected confirm.request notification, got: {line}"
    );
    assert!(line.contains("\"class\":\"destructive\""), "got: {line}");
    assert!(line.contains("\"target\":\"10.0.0.5\""), "got: {line}");

    // --- 4. Operator client sends confirm.resolve{decision: "allow"} -----
    // Extract the id from the confirm.request so we resolve the right one.
    let req_payload: serde_json::Value = serde_json::from_str(&line).unwrap();
    let confirm_id = req_payload["params"]["id"].as_str().unwrap().to_string();
    let resolve = serde_json::json!({
        "id": 3,
        "method": "confirm.resolve",
        "params": {
            "id": confirm_id,
            "decision": "allow",
        },
    });
    op_write.write_all(resolve.to_string().as_bytes()).await.unwrap();
    op_write.write_all(b"\n").await.unwrap();
    op_write.flush().await.unwrap();

    // --- 5. Runtime client receives the execute response -----------------
    let resp_line = tokio::time::timeout(Duration::from_secs(2), async {
        let mut lenb = [0u8; 4];
        rt_client.read_exact(&mut lenb).await?;
        let len = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; len];
        rt_client.read_exact(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    })
    .await
    .expect("runtime client should receive execute response within 2s")
    .expect("read should not error");
    let resp: serde_json::Value = serde_json::from_slice(&resp_line).unwrap();
    assert!(resp["ok"].as_bool().unwrap_or(false), "execute should succeed after operator allow, got: {resp}");
    assert!(resp["result"]["stdout"].is_string(), "result.stdout should be a string: {resp}");

    // --- 6. Audit log: ActionRequested + ActionExecuted (no ActionDenied) -
    let audit_text = std::fs::read_to_string(&audit_path).unwrap();
    assert!(audit_text.contains("\"kind\":\"action_requested\""), "audit should contain action_requested, got: {audit_text}");
    assert!(audit_text.contains("\"kind\":\"action_executed\""), "audit should contain action_executed, got: {audit_text}");
    assert!(!audit_text.contains("\"kind\":\"action_denied\""), "audit should NOT contain action_denied: {audit_text}");

    // --- cleanup ---------------------------------------------------------
    server_handle.abort();
    operator_handle.abort();
}

/// The `deny` path: operator denies, the runtime client gets a
/// `Gate3Denied` error in the RPC response, and the audit log records
/// `action_denied` with `gate: 3`. This is the second half of the
/// confirmation flow — proves the channel is bidirectional, not just a
/// notification pipe.
#[tokio::test]
async fn destructive_action_can_be_denied_by_operator() {
    let dir = tempdir().unwrap();
    let runtime_sock = dir.path().join("runtime.sock");
    let operator_sock = dir.path().join("operator.sock");
    let audit_path = dir.path().join("audit.jsonl");
    let evidence_dir = dir.path().join("evidence");
    std::fs::create_dir_all(&evidence_dir).unwrap();

    let profile = Profile {
        name: "operator-test".into(),
        tier: blackglass_profile::Tier::Operator,
        allowed_domains: vec!["core".into(), "osint".into()],
        allowed_action_classes: vec!["read_only".into(), "destructive".into()],
    };
    let mut eng = Engagement::new("e", "t", "2020-01-01T00:00:00Z", "2099-12-31T23:59:59Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });

    let chain = Chain::open(&audit_path).unwrap();
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let gate3: Arc<dyn Gate3> = Arc::new(BrokerGate3::new(broker.clone(), channel.clone(), "osint", "smoke"));
    let gate4: Arc<dyn Gate4> = Arc::new(RealSanitizer::new(100 * 1024, evidence_dir.clone()));
    let cp = Chokepoint::new(chain, profile, eng, gate3, gate4).with_evidence_dir(evidence_dir);
    let server = Server::bind(&runtime_sock, "tok".into(), cp).await.unwrap();

    let server_handle = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), server.serve()).await;
    });
    let op_broker = broker.clone();
    let op_channel = channel.clone();
    let op_sock = operator_sock.clone();
    let operator_handle = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), run_operator(&op_sock, op_broker, op_channel)).await;
    });

    for path in [&runtime_sock, &operator_sock] {
        for _ in 0..50 {
            if path.exists() { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let op_client = UnixStream::connect(&operator_sock).await.unwrap();
    let (op_read, mut op_write) = op_client.into_split();
    let mut op_lines = tokio::io::BufReader::new(op_read).lines();

    let mut rt_client = UnixStream::connect(&runtime_sock).await.unwrap();

    // Auth
    let auth = serde_json::json!({"id": 1, "method": "auth", "token": "tok"});
    rt_client.write_all(&encode_frame(auth.to_string().as_bytes())).await.unwrap();
    let mut lenb = [0u8; 4];
    rt_client.read_exact(&mut lenb).await.unwrap();
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    rt_client.read_exact(&mut buf).await.unwrap();
    let _: serde_json::Value = serde_json::from_slice(&buf).unwrap();

    // Execute
    let exec = serde_json::json!({
        "id": 2,
        "method": "execute_action",
        "domain": "osint",
        "action_class": "destructive",
        "target": "10.0.0.5",
        "args": {},
    });
    rt_client.write_all(&encode_frame(exec.to_string().as_bytes())).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let line = tokio::time::timeout(Duration::from_secs(2), op_lines.next_line())
        .await.expect("op client should receive confirm.request")
        .expect("read should not EOF")
        .expect("read should not error");
    let req_payload: serde_json::Value = serde_json::from_str(&line).unwrap();
    let confirm_id = req_payload["params"]["id"].as_str().unwrap().to_string();

    // DENY
    let resolve = serde_json::json!({
        "id": 3,
        "method": "confirm.resolve",
        "params": {
            "id": confirm_id,
            "decision": "deny",
        },
    });
    op_write.write_all(resolve.to_string().as_bytes()).await.unwrap();
    op_write.write_all(b"\n").await.unwrap();
    op_write.flush().await.unwrap();

    let resp_line = tokio::time::timeout(Duration::from_secs(2), async {
        let mut lenb = [0u8; 4];
        rt_client.read_exact(&mut lenb).await?;
        let len = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; len];
        rt_client.read_exact(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    })
    .await.expect("runtime client should receive execute response")
    .expect("read should not error");
    let resp: serde_json::Value = serde_json::from_slice(&resp_line).unwrap();
    assert!(!resp["ok"].as_bool().unwrap_or(true), "execute should be DENIED after operator deny, got: {resp}");
    assert!(resp["error"].as_str().unwrap_or("").contains("gate3") || resp["error"].as_str().unwrap_or("").contains("deny"),
        "error should mention gate3/deny, got: {resp}");

    let audit_text = std::fs::read_to_string(&audit_path).unwrap();
    assert!(audit_text.contains("\"kind\":\"action_denied\""), "audit should contain action_denied, got: {audit_text}");
    assert!(audit_text.contains("\"gate\":3"), "audit should show gate:3 in the denial, got: {audit_text}");
    assert!(!audit_text.contains("\"kind\":\"action_executed\""), "audit should NOT contain action_executed after deny");

    server_handle.abort();
    operator_handle.abort();
}

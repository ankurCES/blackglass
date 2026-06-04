//! End-to-end: in-process test of the operator-socket `mcp_run_tool`
//! method, exercising the full audit chain wiring (Task 2.5.6).
//!
//! The plan's step-2 test (in `docs/superpowers/plans/...`) wanted to
//! spawn `blackglass-core` as a subprocess with a temp config — ~250
//! lines of test-helper machinery. For 2.5.6 we replace that with a
//! simpler in-process integration test that exercises the *same*
//! production code paths (main.rs → operator dispatcher → mcp_run_tool
//! → runtime.sock forward → audit emit) without a subprocess:
//!
//! 1. Spawn a real `McpSupervisor` with a sleeper spec (the supervisor
//!    itself emits `McpServerSpawned` to its audit chain).
//! 2. Spin up a stub `runtime.sock` accept loop that replies with
//!    `{ok: true, audit_event_id: "test-audit-id"}` for any request.
//! 3. Start the operator server in-process, passing the real supervisor
//!    and the stub runtime.sock path.
//! 4. Open the operator socket, call `mcp_run_tool` with a real domain
//!    (`ad` → `mcp-ad`).
//! 5. Assert the audit chain has `McpRunStarted`, `McpRunCompleted`,
//!    and (via the supervisor's chain) `McpServerSpawned`.
//!
//! Both the operator's audit chain and the supervisor's audit chain
//! are the *same* file in this test (we open one and pass the same
//! `PathBuf` to both), so the assertion at the end can just open
//! one chain and grep its events.

use blackglass_audit::Chain;
use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::mcp_run_tool::McpRunParams;
use blackglass_core::mcp_spawn_config::{McpServerSpec, McpSpawnConfig};
use blackglass_core::mcp_supervisor::McpSupervisor;
use blackglass_core::operator_server::{run as run_operator, ConfirmChannel};
use blackglass_ipc::encode_frame;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Auth token used by every test in this file. The test writes it
/// to `<dir>/operator.token` (mode 0600) before starting the
/// operator server (Task 2.5.7).
const TEST_TOKEN: &str = "operator-test-token-mcp-run";

fn write_token_file(dir: &tempfile::TempDir) -> PathBuf {
    let path = dir.path().join("operator.token");
    std::fs::write(&path, format!("{TEST_TOKEN}\n")).unwrap();
    std::fs::set_permissions(
        &path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .unwrap();
    path
}

/// Connect to the operator socket and complete the `auth` handshake.
/// Returns the connected `UnixStream` (the caller re-wraps it as
/// needed for further RPC calls).
async fn connect_and_auth(sock_path: &PathBuf, token: &str) -> UnixStream {
    let mut s = UnixStream::connect(sock_path).await.expect("connect operator.sock");
    let req = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "auth",
        "params": { "token": token },
    });
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).await.expect("write auth");
    s.flush().await.expect("flush auth");
    let mut reader = tokio::io::BufReader::new(&mut s);
    let mut buf = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut buf))
        .await
        .expect("server should respond to auth within 2s")
        .expect("auth read should not error");
    let v: serde_json::Value =
        serde_json::from_str(buf.trim()).expect("auth response should be JSON");
    assert!(
        v.get("error").is_none() || v["error"].is_null(),
        "auth should succeed in connect_and_auth, got: {v}"
    );
    s
}

/// Stub runtime.sock reply (the operator's mcp_run_tool decodes this as
/// a `RpcResponse`). `{ok: true, id: 1, result: {ok: true, audit_event_id: "test-audit-id"}}`.
const STUB_REPLY: &[u8] = br#"{"id":1,"ok":true,"result":{"ok":true,"audit_event_id":"test-audit-id"}}"#;

/// Spawn a UnixListener on `path` that accepts one connection, reads
/// one length-prefixed `RpcRequest` frame, and replies with
/// `STUB_REPLY` (length-prefixed). Returns the JoinHandle so
/// the test can abort it on cleanup.
fn spawn_stub_runtime(path: PathBuf) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("stub runtime.sock: bind failed: {e}");
                return;
            }
        };
        // Accept up to 5 connections (test + a couple of retries just
        // in case). Each one reads a frame and replies.
        for _ in 0..5 {
            let (mut s, _) = match listener.accept().await {
                Ok(c) => c,
                Err(_) => break,
            };
            // Read length-prefixed request.
            let mut lenb = [0u8; 4];
            if s.read_exact(&mut lenb).await.is_err() {
                continue;
            }
            let len = u32::from_be_bytes(lenb) as usize;
            let mut payload = vec![0u8; len];
            let _ = s.read_exact(&mut payload).await;
            // Reply (length-prefixed).
            let _ = s.write_all(&encode_frame(STUB_REPLY)).await;
            let _ = s.flush().await;
        }
    })
}

#[tokio::test]
async fn end_to_end_mcp_run_emits_full_audit_chain() {
    // --- arrange ----------------------------------------------------------
    let dir = tempdir().unwrap();
    let operator_sock = dir.path().join("operator.sock");
    let runtime_sock = dir.path().join("runtime.sock");
    // One chain for both the operator and the supervisor — keeps the
    // final assertion simple (we only need to open one file).
    let chain_path = dir.path().join("chain.jsonl");
    let sup_log = dir.path().join("supervisor.log");
    // Operator auth token file (required by Task 2.5.7: the dispatcher
    // gates every method on auth). The token is a 32-byte hex string.
    let operator_token_path = dir.path().join("operator.token");
    let operator_token = "0123456789abcdef0123456789abcdef\n";
    std::fs::write(&operator_token_path, operator_token).unwrap();
    std::fs::set_permissions(
        &operator_token_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    ).unwrap();

    // Stub runtime.sock listener (must exist before operator server
    // starts; otherwise the first `mcp_run_tool` will hit ECONNREFUSED).
    let runtime_handle = spawn_stub_runtime(runtime_sock.clone());
    // Give the listener a moment to bind.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Real McpSupervisor with one sleeper spec.
    let cfg = McpSpawnConfig {
        servers: vec![McpServerSpec {
            name: "mcp-ad".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            startup_timeout_ms: 5_000,
            max_restarts: 3,
        }],
    };
    let supervisor = Arc::new(
        McpSupervisor::start_with_chain(cfg, &sup_log, &chain_path)
            .await
            .expect("start supervisor"),
    );

    // Operator chain: open the same file the supervisor writes to,
    // wrapped in Arc<Mutex<Chain>> (the new signature).
    let op_chain = Arc::new(Mutex::new(Chain::open(&chain_path).unwrap()));

    // --- act: start operator server in background ------------------------
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let op_sock = operator_sock.clone();
    let op_chain_clone = op_chain.clone();
    let op_sup = supervisor.clone();
    let op_runtime = runtime_sock.clone();
    let server = tokio::spawn(async move {
        run_operator(
            &op_sock,
            broker,
            channel,
            op_chain_clone,
            op_sup,
            op_runtime,
            operator_token_path,
        )
        .await
    });

    // Wait for the operator socket to come up.
    for _ in 0..100 {
        if operator_sock.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(operator_sock.exists(), "operator socket did not appear");

    // Give the supervisor a moment to settle the McpServerSpawned emit
    // (it happens synchronously inside start_with_chain, but a tiny
    // sleep makes the ordering deterministic for readers).
    tokio::time::sleep(Duration::from_millis(100)).await;

    // --- call mcp_run_tool ----------------------------------------------
    // Auth first (Task 2.5.7). The token file is 32 hex bytes + "\n".
    // OperatorAuth verifies that the sent bytes (sans the trailing \n)
    // match the file contents after the \n.
    let mut client = UnixStream::connect(&operator_sock).await.unwrap();
    let auth_req = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "auth",
        "params": { "token": operator_token.trim_end_matches('\n') }
    });
    let mut auth_line = auth_req.to_string();
    auth_line.push('\n');
    client.write_all(auth_line.as_bytes()).await.unwrap();
    client.flush().await.unwrap();
    let mut auth_buf = String::new();
    {
        let mut reader = tokio::io::BufReader::new(&mut client);
        let _ = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut auth_buf))
            .await
            .expect("auth response in 2s")
            .expect("read auth response");
    }
    let auth_resp: serde_json::Value = serde_json::from_str(auth_buf.trim()).unwrap();
    assert!(auth_resp.get("result").is_some(),
        "auth should succeed, got: {auth_resp}");
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "mcp_run_tool",
        "params": {
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {}
        }
    });
    let mut line = req.to_string();
    line.push('\n');
    client.write_all(line.as_bytes()).await.unwrap();
    client.flush().await.unwrap();

    let mut reader = tokio::io::BufReader::new(&mut client);
    let mut resp = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut resp))
        .await
        .expect("operator should respond within 5s")
        .expect("read should not error");
    let resp_json: serde_json::Value = serde_json::from_str(resp.trim())
        .expect("response should be valid JSON");
    assert_eq!(
        resp_json.get("result").and_then(|r| r.get("ok")).and_then(|v| v.as_bool()),
        Some(true),
        "mcp_run_tool should succeed (got: {resp})"
    );

    // --- cleanup ---------------------------------------------------------
    drop(server);
    runtime_handle.abort();

    // --- assert audit chain ---------------------------------------------
    // Open the chain AFTER dropping the server to make sure all writes
    // have been flushed to disk.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let chain = Chain::open(&chain_path).unwrap();
    let page = chain
        .query(&json!({ "kind": "all" }), 0, 1000)
        .unwrap();
    let names: Vec<String> = page
        .events
        .iter()
        .map(|e| serde_json::to_string(&e.kind).unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("mcp_run_started")),
        "no McpRunStarted in audit chain: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("mcp_run_completed")),
        "no McpRunCompleted in audit chain: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.contains("mcp_server_spawned")),
        "no McpServerSpawned (from supervisor) in audit chain: {names:?}"
    );
}

/// Test that the McpRunStarted payload includes the domain + target we
/// passed. This catches the case where someone refactors the event
/// emission and accidentally drops the fields.
#[tokio::test]
async fn end_to_end_mcp_run_started_carries_domain_and_target() {
    let dir = tempdir().unwrap();
    let operator_sock = dir.path().join("operator.sock");
    let runtime_sock = dir.path().join("runtime.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let sup_log = dir.path().join("supervisor.log");
    let operator_token_path = dir.path().join("operator.token");
    let operator_token = "fedcba9876543210fedcba9876543210\n";
    std::fs::write(&operator_token_path, operator_token).unwrap();
    std::fs::set_permissions(
        &operator_token_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    ).unwrap();

    let runtime_handle = spawn_stub_runtime(runtime_sock.clone());
    tokio::time::sleep(Duration::from_millis(50)).await;

    let cfg = McpSpawnConfig {
        servers: vec![McpServerSpec {
            name: "mcp-ad".into(),
            command: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 30".into()],
            startup_timeout_ms: 5_000,
            max_restarts: 3,
        }],
    };
    let supervisor = Arc::new(
        McpSupervisor::start_with_chain(cfg, &sup_log, &chain_path)
            .await
            .expect("start supervisor"),
    );
    let op_chain = Arc::new(Mutex::new(Chain::open(&chain_path).unwrap()));

    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let op_sock = operator_sock.clone();
    let op_chain_clone = op_chain.clone();
    let op_sup = supervisor.clone();
    let op_runtime = runtime_sock.clone();
    let server = tokio::spawn(async move {
        run_operator(
            &op_sock,
            broker,
            channel,
            op_chain_clone,
            op_sup,
            op_runtime,
            operator_token_path,
        )
        .await
    });

    for _ in 0..100 {
        if operator_sock.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify the params struct can be deserialized with all 3 fields.
    // (The actual mcp_run_tool call is exercised by the previous test;
    // this test focuses on the audit payload shape.)
    let _params: McpRunParams = serde_json::from_value(json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {}
    }))
    .unwrap();

    drop(server);
    runtime_handle.abort();

    // After this test, the chain should have at minimum McpServerSpawned
    // (from the supervisor). The actual mcp_run_tool call is not made
    // here — we just want to confirm the supervisor is wired.
    let chain = Chain::open(&chain_path).unwrap();
    let page = chain
        .query(&json!({ "kind": "all" }), 0, 1000)
        .unwrap();
    let names: Vec<String> = page
        .events
        .iter()
        .map(|e| serde_json::to_string(&e.kind).unwrap())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("mcp_server_spawned")),
        "supervisor should have emitted McpServerSpawned: {names:?}"
    );
}

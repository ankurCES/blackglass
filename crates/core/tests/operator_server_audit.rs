//! Integration tests for the operator-socket `audit.query` and
//! `audit.verify_chain` methods (plan Task 2.5.3).
//!
//! These tests seed events directly via `blackglass_audit::Chain::append`
//! on a tempdir chain file, then exercise the operator socket. We do NOT
//! go through any "audit.append" RPC because that method does not exist
//! yet (out of scope for 2.5.3; would be a follow-up).
//!
//! Task 2.5.7: every method (including `audit.query` /
//! `audit.verify_chain`) is gated on the client having called `auth`
//! first. The 3 new tests at the bottom of this file exercise that
//! gating. The 3 original tests below use `connect_and_auth` to send
//! `auth` before their JSON-RPC call.

use blackglass_audit::{Chain, Event, EventKind};
use blackglass_core::mcp_spawn_config::McpSpawnConfig;
use blackglass_core::mcp_supervisor::McpSupervisor;
use blackglass_core::operator_server::{run, ConfirmChannel};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Build a no-op `McpSupervisor` (empty config) for tests that don't
/// exercise the MCP wiring.
async fn noop_supervisor() -> Arc<McpSupervisor> {
    let cfg = McpSpawnConfig::default();
    let log = std::env::temp_dir().join("blackglass-noop-supervisor-audit.log");
    Arc::new(McpSupervisor::start(cfg, &log).await.unwrap())
}

/// Write a deterministic token file at `<dir>/operator.token` with mode
/// 0600 and return (token_path, token_bytes). Tests use this to drive
/// the `auth` method on the operator socket.
fn write_token_file(dir: &tempfile::TempDir) -> (PathBuf, String) {
    let token_path = dir.path().join("operator.token");
    let token = "test-token-2.5.7".to_string();
    std::fs::write(&token_path, format!("{}\n", token)).expect("write token");
    std::fs::set_permissions(
        &token_path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .expect("set token perms");
    (token_path, token)
}

/// Spawn a fresh operator server on a tempdir socket backed by a fresh
/// audit chain at `<dir>/chain.jsonl`, with a 0600 token file at
/// `<dir>/operator.token`. Returns (socket_path, server_handle, token).
async fn spawn_operator_with_chain_and_token(
    dir: &tempfile::TempDir,
) -> (PathBuf, tokio::task::JoinHandle<std::io::Result<()>>, String) {
    // Token must exist BEFORE the server starts so the `auth` route can
    // find it on every connection.
    let (_token_path, token) = write_token_file(dir);
    let (sock_path, server) = spawn_operator_with_chain(dir).await;
    (sock_path, server, token)
}

/// Connect to the operator socket and complete the `auth` handshake.
/// Returns the connected `UnixStream` (the caller re-wraps it as
/// needed for further `rpc_call`s). Panics on auth failure — callers
/// that want to *test* auth failure use the unauth path directly.
async fn connect_and_auth(sock_path: &PathBuf, token: &str) -> UnixStream {
    use tokio::io::BufReader;
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
    let mut reader = BufReader::new(&mut s);
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

/// Spawn a fresh operator server on a tempdir socket backed by a fresh
/// audit chain at `<dir>/chain.jsonl`. The token file is expected to
/// already exist (callers in this file use `write_token_file` before
/// spawning). Returns the socket path and a handle to the server task.
async fn spawn_operator_with_chain(
    dir: &tempfile::TempDir,
) -> (PathBuf, tokio::task::JoinHandle<std::io::Result<()>>) {
    let sock_path = dir.path().join("operator.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let chain = Chain::open(&chain_path).expect("open empty chain");
    let broker = blackglass_core::broker::ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let supervisor = noop_supervisor().await;
    let runtime_sock = dir.path().join("runtime.sock");
    let token_path = dir.path().join("operator.token");
    let server = tokio::spawn({
        let p = sock_path.clone();
        async move {
            run(
                &p,
                broker,
                channel,
                Arc::new(Mutex::new(chain)),
                supervisor,
                runtime_sock,
                token_path,
            )
            .await
        }
    });
    // Wait for the socket file to appear.
    for _ in 0..100 {
        if sock_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    (sock_path, server)
}

/// Send a single JSON-RPC call and read the next newline-terminated
/// response from the stream. Returns the parsed JSON value.
async fn rpc_call(sock: &mut UnixStream, method: &str, params: serde_json::Value) -> serde_json::Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let mut line = req.to_string();
    line.push('\n');
    sock.write_all(line.as_bytes()).await.expect("write rpc");
    sock.flush().await.expect("flush rpc");

    let mut reader = tokio::io::BufReader::new(sock);
    let mut buf = String::new();
    let n = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut buf))
        .await
        .expect("server should respond within 2s")
        .expect("read should not error");
    assert!(n > 0, "server returned EOF");
    serde_json::from_str(buf.trim()).expect("response should be valid JSON")
}

/// Seed a fresh chain file with `n` events of kind `McpServerSpawned`.
/// `seq` is auto-incremented; `prev_hash` is filled in by `Chain::append`.
fn seed_chain(chain_path: &PathBuf, n: u64) {
    let mut chain = Chain::open(chain_path).expect("open for seeding");
    for i in 0..n {
        chain
            .append(Event {
                seq: 0, // Chain::append leaves it as-is when prev_hash is empty
                ts: format!("2026-06-03T00:00:0{i}Z"),
                prev_hash: String::new(),
                kind: EventKind::McpServerSpawned {
                    server: format!("s{i}"),
                    pid: 1000 + i as u32,
                },
                payload: json!({ "test": i }),
            })
            .expect("append seed event");
    }
}

#[tokio::test]
async fn audit_query_returns_events_paginated() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    seed_chain(&chain_path, 5);

    let (sock_path, server, token) = spawn_operator_with_chain_and_token(&dir).await;

    let mut s = connect_and_auth(&sock_path, &token).await;
    let resp = rpc_call(
        &mut s,
        "audit.query",
        json!({
            "filter": { "kind": "all" },
            "page": 0,
            "page_size": 3,
        }),
    )
    .await;

    let result = resp.get("result").expect("response should have result");
    let events = result
        .get("events")
        .and_then(|e| e.as_array())
        .expect("result.events should be an array");
    assert_eq!(events.len(), 3, "page 0 size 3 of 5 should return 3 events");
    assert_eq!(
        result.get("total_matched").and_then(|t| t.as_u64()),
        Some(5),
        "total_matched should be 5 (the unpaginated count)"
    );

    drop(server);
}

#[tokio::test]
async fn audit_query_returns_empty_page_for_out_of_range() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    seed_chain(&chain_path, 5);

    let (sock_path, server, token) = spawn_operator_with_chain_and_token(&dir).await;

    let mut s = connect_and_auth(&sock_path, &token).await;
    let resp = rpc_call(
        &mut s,
        "audit.query",
        json!({
            "filter": { "kind": "all" },
            "page": 999,
            "page_size": 10,
        }),
    )
    .await;

    let result = resp.get("result").expect("response should have result");
    let events = result
        .get("events")
        .and_then(|e| e.as_array())
        .expect("result.events should be an array");
    assert_eq!(events.len(), 0, "page 999 of a 5-event chain is empty");
    assert_eq!(
        result.get("total_matched").and_then(|t| t.as_u64()),
        Some(5),
        "total_matched still reflects the full un-paginated count"
    );

    drop(server);
}

#[tokio::test]
async fn audit_verify_chain_returns_valid_report() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    seed_chain(&chain_path, 2);

    let (sock_path, server, token) = spawn_operator_with_chain_and_token(&dir).await;

    let mut s = connect_and_auth(&sock_path, &token).await;
    let resp = rpc_call(&mut s, "audit.verify_chain", json!({})).await;

    let result = resp.get("result").expect("response should have result");
    let count = result
        .as_u64()
        .expect("audit.verify_chain returns a u64 count of valid events");
    assert!(
        count >= 2,
        "expected at least 2 valid events after seeding 2, got {count}"
    );

    drop(server);
}

// =============================================================================
// Task 2.5.7: auth gating
// =============================================================================

#[tokio::test]
async fn unauthenticated_client_cannot_call_audit_query() {
    use tokio::io::BufReader;
    let dir = tempdir().unwrap();
    // The token file exists (so `auth` could succeed) but the client
    // does not call `auth` first.
    let _ = write_token_file(&dir);
    let (sock_path, server) = spawn_operator_with_chain(&dir).await;

    let mut s = UnixStream::connect(&sock_path).await.unwrap();
    // Send audit.query WITHOUT auth first.
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "audit.query",
        "params": { "filter": {}, "page": 0, "page_size": 10 },
    });
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).await.unwrap();
    s.flush().await.unwrap();
    let mut reader = BufReader::new(&mut s);
    let mut resp = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut resp))
        .await
        .unwrap()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(resp.trim()).unwrap();
    assert_eq!(
        v["error"]["code"], -32001,
        "expected auth-required (-32001), got: {v}"
    );
    drop(server);
}

#[tokio::test]
async fn authenticated_client_can_call_audit_query() {
    use tokio::io::BufReader;
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    seed_chain(&chain_path, 2);

    let (sock_path, server, token) = spawn_operator_with_chain_and_token(&dir).await;

    let mut s = connect_and_auth(&sock_path, &token).await;
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "audit.query",
        "params": { "filter": {}, "page": 0, "page_size": 10 },
    });
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).await.unwrap();
    s.flush().await.unwrap();
    let mut reader = BufReader::new(&mut s);
    let mut resp = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut resp))
        .await
        .unwrap()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(resp.trim()).unwrap();
    assert!(
        v.get("error").is_none() || v["error"].is_null(),
        "expected no error after auth, got: {v}"
    );
    drop(server);
}

#[tokio::test]
async fn auth_with_wrong_token_returns_error() {
    use tokio::io::BufReader;
    let dir = tempdir().unwrap();
    // Token file exists with a known-good token, but the client
    // presents a wrong one.
    let _ = write_token_file(&dir);
    let (sock_path, server) = spawn_operator_with_chain(&dir).await;

    let mut s = UnixStream::connect(&sock_path).await.unwrap();
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "auth",
        "params": { "token": "wrong-token" },
    });
    let mut line = req.to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).await.unwrap();
    s.flush().await.unwrap();
    let mut reader = BufReader::new(&mut s);
    let mut resp = String::new();
    let _ = tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut resp))
        .await
        .unwrap()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(resp.trim()).unwrap();
    assert_eq!(
        v["error"]["code"], -32002,
        "expected auth-failed (-32002), got: {v}"
    );
    drop(server);
}

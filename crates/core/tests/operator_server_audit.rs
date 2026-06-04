//! Integration tests for the operator-socket `audit.query` and
//! `audit.verify_chain` methods (plan Task 2.5.3).
//!
//! These tests seed events directly via `blackglass_audit::Chain::append`
//! on a tempdir chain file, then exercise the operator socket. We do NOT
//! go through any "audit.append" RPC because that method does not exist
//! yet (out of scope for 2.5.3; would be a follow-up).
//!
//! Auth gating (Task 2.5.7) is also out of scope; the dispatcher routes
//! the new methods unconditionally for now and a `// TODO: gate on auth`
//! marker is left in `operator_server.rs`.

use blackglass_audit::{Chain, Event, EventKind};
use blackglass_core::operator_server::{run, ConfirmChannel};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Spawn a fresh operator server on a tempdir socket backed by a fresh
/// audit chain at `<dir>/chain.jsonl`. Returns the socket path and a
/// handle to the server task (caller drops the handle to stop it).
async fn spawn_operator_with_chain(
    dir: &tempfile::TempDir,
) -> (PathBuf, tokio::task::JoinHandle<std::io::Result<()>>) {
    let sock_path = dir.path().join("operator.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let chain = Chain::open(&chain_path).expect("open empty chain");
    let broker = blackglass_core::broker::ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let server = tokio::spawn({
        let p = sock_path.clone();
        async move { run(&p, broker, channel, Arc::new(chain)).await }
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

    let (sock_path, server) = spawn_operator_with_chain(&dir).await;

    let mut s = UnixStream::connect(&sock_path).await.unwrap();
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

    let (sock_path, server) = spawn_operator_with_chain(&dir).await;

    let mut s = UnixStream::connect(&sock_path).await.unwrap();
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

    let (sock_path, server) = spawn_operator_with_chain(&dir).await;

    let mut s = UnixStream::connect(&sock_path).await.unwrap();
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

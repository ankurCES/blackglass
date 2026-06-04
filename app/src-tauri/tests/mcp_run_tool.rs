//! Tauri command tests. We don't use Tauri's mock runtime — we test
//! the `commands` module's pure functions directly, with a fake
//! operator socket.
//!
//! The fake operator's job is to model the real core's protocol:
//! it must respond to the `auth` frame with `{"ok":true}` (or a
//! `{"error":...}` if the test wants to exercise the auth-fail
//! path) and then respond to the *next* frame (the actual method
//! call) with the scripted tool response.

use blackglass_app::commands::{audit_query, mcp_run_tool, McpRunRequest, McpRunResponse};
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

/// Spawn a fake operator server on a tempdir socket. `responses[0]`
/// is sent in reply to the `auth` frame; `responses[1]` (if present)
/// is sent in reply to the next method call.
fn spawn_fake_operator(responses: Vec<String>) -> PathBuf {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("op.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();
    let responses = Arc::new(responses);
    // Keep the dir alive for the duration of the test.
    std::mem::forget(dir);
    std::thread::spawn(move || {
        // One connection, two reads (auth frame + method frame),
        // two writes (auth response + method response).
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut idx = 0usize;
        loop {
            // Read one newline-terminated frame. We don't care what's
            // in it; we just need to advance the client's writer.
            let mut buf = vec![0u8; 4096];
            let n = match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if n == 0 { break; }
            if idx >= responses.len() { break; }
            // The real operator sends each frame terminated with '\n';
            // the client uses BufReader::read_line which blocks until
            // it sees the terminator. Append '\n' so the client's
            // read_line returns instead of hanging forever.
            let response = format!("{}\n", responses[idx]);
            if stream.write_all(response.as_bytes()).is_err() { break; }
            idx += 1;
            // After the auth response (idx 0), the test sends a 2nd
            // frame (the method call). We read it above, then write
            // the method response (idx 1). After that the test
            // disconnects — read returns 0, we break.
        }
    });
    sock_path
}

#[tokio::test]
async fn mcp_run_tool_returns_ok_when_operator_returns_ok() {
    let sock = spawn_fake_operator(vec![
        // auth response
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
        // mcp_run_tool response
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await.unwrap();
    assert!(resp.ok);
}

#[tokio::test]
async fn mcp_run_tool_returns_auth_error_when_socket_rejects() {
    let sock = spawn_fake_operator(vec![
        // auth rejects — no method call needed
        r#"{"jsonrpc":"2.0","id":0,"error":{"code":-32002,"message":"auth failed"}}"#.to_string(),
    ]);
    let token = "wrong\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await;
    assert!(resp.is_err());
    let err = resp.unwrap_err();
    assert!(
        err.contains("auth") || err.contains("32002"),
        "error should mention auth or 32002, got: {err}"
    );
}

#[tokio::test]
async fn mcp_run_tool_returns_mcp_down_when_socket_refuses_connection() {
    let sock = PathBuf::from("/tmp/does-not-exist-blackglass-test-12345.sock");
    let token = "any\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await;
    assert!(resp.is_err());
    let err = resp.unwrap_err();
    assert!(
        err.contains("connect")
            || err.contains("refused")
            || err.contains("disconnected")
            || err.contains("No such file"),
        "error should mention connect/refused, got: {err}"
    );
}

#[tokio::test]
async fn mcp_run_tool_parses_full_response_shape() {
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true,"stdout":"hello\n","stderr":"","audit_event_id":"evt-1"}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let req = McpRunRequest {
        domain: "osint".into(),
        target: "osint-nmap".into(),
        args: serde_json::json!({"target": "10.0.0.1"}),
    };
    let resp: McpRunResponse = mcp_run_tool(req, &sock, &token).await.unwrap();
    assert!(resp.ok);
    assert_eq!(resp.stdout.as_deref(), Some("hello\n"));
    assert_eq!(resp.audit_event_id.as_deref(), Some("evt-1"));
}

#[tokio::test]
async fn audit_query_returns_rows_when_operator_returns_ok() {
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true,"events":[{"id":"evt-1","actor":"alice","action":"ad-impacket_psexec","target":"dc01","ts":1700000000,"verdict":"allow"}],"chain_head":"abc","verified":true,"page":0,"page_size":50,"total":1}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let filter = serde_json::json!({});
    let result = audit_query(filter, 0, 50, &sock, &token).await.unwrap();
    let events = result["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["actor"], "alice");
    assert_eq!(events[0]["verdict"], "allow");
    assert_eq!(result["verified"], true);
    assert_eq!(result["total"], 1);
}

#[tokio::test]
async fn audit_query_returns_empty_when_no_events() {
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true,"events":[],"chain_head":"abc","verified":true,"page":0,"page_size":10,"total":0}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let filter = serde_json::json!({"since": 1700000000});
    let result = audit_query(filter, 0, 10, &sock, &token).await.unwrap();
    let events = result["events"].as_array().expect("events array");
    assert!(events.is_empty());
    assert_eq!(result["total"], 0);
}

#[tokio::test]
async fn audit_query_returns_err_when_operator_returns_error() {
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"audit log corrupted"}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let filter = serde_json::json!({});
    let resp = audit_query(filter, 0, 10, &sock, &token).await;
    assert!(resp.is_err(), "expected error when operator returns JSON-RPC error");
}

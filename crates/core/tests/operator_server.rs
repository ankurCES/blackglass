use blackglass_audit::Chain;
use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::mcp_spawn_config::McpSpawnConfig;
use blackglass_core::mcp_supervisor::McpSupervisor;
use blackglass_core::operator_server::{run, ConfirmChannel, ConfirmRequest};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Build a no-op `McpSupervisor` (empty config) for tests that don't
/// exercise the MCP wiring. Returns the supervisor wrapped in an Arc;
/// the supervisor's monitor task is harmless when the config is empty.
async fn noop_supervisor() -> Arc<McpSupervisor> {
    let cfg = McpSpawnConfig::default();
    let log = std::env::temp_dir().join("blackglass-noop-supervisor.log");
    Arc::new(McpSupervisor::start(cfg, &log).await.unwrap())
}

/// Write a 0600 token file at `<dir>/operator.token` containing
/// `TEST_TOKEN`. Returns the token string. The token file must exist
/// before `run()` starts so the auth route can find it.
const TEST_TOKEN: &str = "operator-test-token";

fn write_token_file(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("operator.token");
    std::fs::write(&path, format!("{TEST_TOKEN}\n")).unwrap();
    std::fs::set_permissions(
        &path,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .unwrap();
    TEST_TOKEN.to_string()
}

/// Connect to the operator socket and complete the `auth` handshake.
/// Returns the connected `UnixStream` (the caller re-wraps it as
/// needed). Used by every test that subsequently calls a JSON-RPC
/// method.
async fn connect_and_auth(sock_path: &std::path::Path, token: &str) -> UnixStream {
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

#[tokio::test]
async fn accepts_connections_and_survives_malformed_input() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("operator.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let token_path = dir.path().join("operator.token");
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let chain = Arc::new(Mutex::new(Chain::open(&chain_path).unwrap()));
    let supervisor = noop_supervisor().await;
    let runtime_sock = dir.path().join("runtime.sock");
    let token = write_token_file(&dir);

    let server = tokio::spawn({
        let p = sock_path.clone();
        async move { run(&p, broker, channel, chain, supervisor, runtime_sock, token_path).await }
    });

    for _ in 0..50 {
        if sock_path.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 1. Malformed line is ignored. (Pre-auth is OK because the parse
    //    error is handled before the auth gate; the server should
    //    still respond with -32700 to non-JSON lines, not crash.)
    {
        let mut s = UnixStream::connect(&sock_path).await.unwrap();
        s.write_all(b"not-json\n").await.unwrap();
        let mut buf = [0u8; 256];
        let _ = tokio::time::timeout(Duration::from_millis(100), s.read(&mut buf)).await;
    }

    // 2. After auth, ping returns pong.
    {
        let mut s = connect_and_auth(&sock_path, &token).await;
        s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n").await.unwrap();
        let mut buf = [0u8; 256];
        let n = tokio::time::timeout(Duration::from_millis(500), s.read(&mut buf))
            .await.expect("server should respond to ping")
            .unwrap();
        let resp = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(resp.contains("\"result\":\"pong\""), "got: {resp}");
    }

    drop(server);
}

/// `ConfirmChannel` is the bridge between the chokepoint's gate3 (which
/// pushes requests when an action needs confirmation) and the operator
/// socket (which forwards them to connected Tauri clients). This test
/// verifies the full push: when a `ConfirmRequest` is pushed to the
/// shared channel *after* a client has connected, the client receives a
/// `confirm.request` notification on its read side. The Tauri shell
/// filters on `method == "confirm.request"`, so the shape of the line
/// written to the client matters.
#[tokio::test]
async fn channel_push_forwards_to_connected_client() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("operator.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let token_path = dir.path().join("operator.token");
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let chain = Arc::new(Mutex::new(Chain::open(&chain_path).unwrap()));
    let supervisor = noop_supervisor().await;
    let runtime_sock = dir.path().join("runtime.sock");
    // The token file must exist (the server's auth route reads it on
    // every `auth` call, and a missing file would surface as an
    // error). The push test never calls `auth`, but a missing file
    // doesn't break the push path either — we still write it for
    // consistency with the other tests in this file.
    let _ = write_token_file(&dir);

    let server = tokio::spawn({
        let p = sock_path.clone();
        let c = channel.clone();
        async move { run(&p, broker, c, chain, supervisor, runtime_sock, token_path).await }
    });

    for _ in 0..50 {
        if sock_path.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Connect a client and start reading before pushing.
    let mut s = UnixStream::connect(&sock_path).await.unwrap();
    let (read, mut _write) = s.split();
    let mut reader = tokio::io::BufReader::new(read).lines();

    // Give the server a moment to register the broadcast subscriber.
    tokio::time::sleep(Duration::from_millis(50)).await;

    channel.push(ConfirmRequest {
        id: "test-id".into(),
        request_id: 42,
        tool: "whois".into(),
        domain: "osint".into(),
        class: "destructive".into(),
        target: "example.com".into(),
        source: "osint".into(),
        deadline_in_ms: 15_000,
    });

    let line = tokio::time::timeout(Duration::from_secs(1), reader.next_line())
        .await
        .expect("server should forward channel push within 1s")
        .expect("read should not EOF")
        .expect("read should not error");

    assert!(
        line.contains("\"method\":\"confirm.request\""),
        "forwarded line should carry method=confirm.request, got: {line}"
    );
    assert!(
        line.contains("\"id\":\"test-id\""),
        "forwarded line should carry the ConfirmRequest id, got: {line}"
    );
    assert!(
        line.contains("\"target\":\"example.com\""),
        "forwarded line should carry the target, got: {line}"
    );

    drop(server);
}

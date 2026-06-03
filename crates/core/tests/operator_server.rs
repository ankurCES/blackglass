use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::operator_server::{run, ConfirmChannel, ConfirmRequest};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn accepts_connections_and_survives_malformed_input() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("operator.sock");
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();

    let server = tokio::spawn({
        let p = sock_path.clone();
        async move { run(&p, broker, channel).await }
    });

    for _ in 0..50 {
        if sock_path.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 1. Malformed line is ignored.
    {
        let mut s = UnixStream::connect(&sock_path).await.unwrap();
        s.write_all(b"not-json\n").await.unwrap();
        let mut buf = [0u8; 256];
        let _ = tokio::time::timeout(Duration::from_millis(100), s.read(&mut buf)).await;
    }

    // 2. Ping returns pong.
    {
        let mut s = UnixStream::connect(&sock_path).await.unwrap();
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
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();

    let server = tokio::spawn({
        let p = sock_path.clone();
        let c = channel.clone();
        async move { run(&p, broker, c).await }
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

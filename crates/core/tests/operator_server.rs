use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::operator_server::run;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn accepts_connections_and_survives_malformed_input() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("operator.sock");
    let broker = ConfirmationBroker::new();

    let server = tokio::spawn({
        let p = sock_path.clone();
        async move { run(&p, broker).await }
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

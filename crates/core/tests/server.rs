use blackglass_audit::Chain;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::AllowAll;
use blackglass_core::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_core::server::Server;
use blackglass_engagement::Engagement;
use blackglass_ipc::encode_frame;
use blackglass_profile::Profile;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

fn send_recv(c: &mut UnixStream, req: &RpcRequest) -> RpcResponse {
    let bytes = serde_json::to_vec(req).unwrap();
    c.write_all(&encode_frame(&bytes)).unwrap();
    let mut lenb = [0u8; 4];
    c.read_exact(&mut lenb).unwrap();
    let n = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; n];
    c.read_exact(&mut buf).unwrap();
    serde_json::from_slice(&buf).unwrap()
}

#[test]
fn ping_succeeds_after_auth_and_fails_before() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("c.sock");
    let audit = dir.path().join("a.jsonl");
    let chain = Chain::open(&audit).unwrap();
    let cp = Chokepoint::new(
        chain,
        Profile::analyst_default(),
        Engagement::new("e", "t", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z"),
        Arc::new(AllowAll),
        Arc::new(AllowAll),
        tokio::sync::broadcast::channel(64).0,
    );

    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async { Server::bind(&sock, "secret".into(), cp).await.unwrap() });
    let sock_path = sock.clone();
    let _handle = std::thread::spawn(move || {
        rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(2), server.serve()).await;
        });
    });
    // Give the accept loop a moment to spin up.
    std::thread::sleep(Duration::from_millis(100));

    // 1) Ping before auth on a fresh connection → must fail.
    let mut c1 = UnixStream::connect(&sock_path).unwrap();
    let r = send_recv(&mut c1, &RpcRequest { id: 1, method: Method::Ping });
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("not authenticated"));

    // 2) Bad token on a fresh connection → must fail.
    let mut c2 = UnixStream::connect(&sock_path).unwrap();
    let r = send_recv(&mut c2, &RpcRequest { id: 2, method: Method::Auth { token: "wrong".into() } });
    assert!(!r.ok);

    // 3+4) Authenticate and then ping on the *same* connection.
    // Auth state is per-connection, so both messages must share one stream.
    let mut c3 = UnixStream::connect(&sock_path).unwrap();
    let r = send_recv(&mut c3, &RpcRequest { id: 3, method: Method::Auth { token: "secret".into() } });
    assert!(r.ok, "auth failed: {r:?}");
    let r = send_recv(&mut c3, &RpcRequest { id: 4, method: Method::Ping });
    assert!(r.ok, "ping after auth failed: {r:?}");
}

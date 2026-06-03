use blackglass_ipc::{decode_frame, encode_frame, FrameError, MAX_FRAME};

#[test]
fn round_trips_short_message() {
    let msg = b"hello";
    let framed = encode_frame(msg);
    assert_eq!(framed.len(), 4 + msg.len());
    let (rest, out) = decode_frame(&framed).unwrap();
    assert!(rest.is_empty());
    assert_eq!(out, msg);
}

#[test]
fn rejects_oversize() {
    let big = vec![0u8; MAX_FRAME + 1];
    let err = decode_frame(&(big.len() as u32).to_be_bytes()).unwrap_err();
    assert!(matches!(err, FrameError::TooLarge { .. }));
}

use blackglass_ipc::rpc::{Request, Response};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

#[test]
fn end_to_end_request_response_over_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.sock");
    let listener = UnixListener::bind(&path).unwrap();

    let server_path = path.clone();
    let t = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut lenb = [0u8; 4];
        s.read_exact(&mut lenb).unwrap();
        let len = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; len];
        s.read_exact(&mut buf).unwrap();
        let req: Request = serde_json::from_slice(&buf).unwrap();
        assert_eq!(req.method, "ping");
        let resp = Response {
            id: req.id,
            ok: true,
            result: Some(serde_json::json!("pong")),
            error: None,
        };
        let bytes = serde_json::to_vec(&resp).unwrap();
        s.write_all(&encode_frame(&bytes)).unwrap();
    });

    let mut c = UnixStream::connect(server_path).unwrap();
    let req = Request { id: 7, method: "ping".into(), params: serde_json::json!({}) };
    c.write_all(&encode_frame(&serde_json::to_vec(&req).unwrap())).unwrap();
    let mut lenb = [0u8; 4];
    c.read_exact(&mut lenb).unwrap();
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    c.read_exact(&mut buf).unwrap();
    let resp: Response = serde_json::from_slice(&buf).unwrap();
    assert!(resp.ok);
    assert_eq!(resp.result.unwrap(), serde_json::json!("pong"));

    t.join().unwrap();
}

#[test]
fn request_must_carry_an_id_and_method() {
    let bad = serde_json::json!({ "method": 7 });
    let r: Result<blackglass_ipc::rpc::Request, _> = serde_json::from_value(bad);
    assert!(r.is_err(), "request without id must fail to deserialize");
}

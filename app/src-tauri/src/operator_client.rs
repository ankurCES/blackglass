//! Opens the operator socket, authenticates, and returns a frame
//! writer + frame reader pair for a Tauri command to use.
//!
//! Used by `commands::mcp_run_tool` and `commands::audit_event` to
//! talk to the core's `operator.sock` (per design §2.4a). Each command
//! opens its own short-lived connection — the Tauri shell does NOT
//! maintain a long-lived authenticated session per the v1 model.

use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

#[derive(Debug)]
pub enum OpError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Op { code: i64, message: String },
    Disconnected(String),
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpError::Io(e) => write!(f, "io: {e}"),
            OpError::Json(e) => write!(f, "json: {e}"),
            OpError::Op { code, message } => {
                write!(f, "operator returned error: code={code}, message={message}")
            }
            OpError::Disconnected(s) => write!(f, "disconnected: {s}"),
        }
    }
}

impl std::error::Error for OpError {}

impl From<std::io::Error> for OpError {
    fn from(e: std::io::Error) -> Self {
        OpError::Io(e)
    }
}
impl From<serde_json::Error> for OpError {
    fn from(e: serde_json::Error) -> Self {
        OpError::Json(e)
    }
}

/// Open the operator socket, send the auth frame, return the
/// (now-authenticated) stream. The Tauri side is expected to call this
/// once per command; the per-connection state is the operator server's
/// `authenticated` flag, which is reset on disconnect.
pub fn connect_and_auth(sock_path: &Path, token: &str) -> Result<UnixStream, OpError> {
    let mut stream = UnixStream::connect(sock_path)?;
    // Send the auth frame. The operator server reads the token, matches
    // it against the 0600 token file, and flips the per-connection
    // `authenticated` flag on success.
    let frame = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "auth",
        "params": { "token": token }
    });
    let s = format!("{}\n", serde_json::to_string(&frame)?);
    stream.write_all(s.as_bytes())?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        return Err(OpError::Op {
            code: err["code"].as_i64().unwrap_or(0),
            message: err["message"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(stream)
}

/// Send a JSON-RPC call on an authenticated stream and read the next
/// newline-terminated response. Returns the `result` field on success,
/// or an `OpError` if the operator returned an error or the socket
/// closed.
pub fn call(
    stream: &mut UnixStream,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, OpError> {
    let frame = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    let s = format!("{}\n", serde_json::to_string(&frame)?);
    stream.write_all(s.as_bytes())?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(OpError::Disconnected(
            "operator closed the socket".into(),
        ));
    }
    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        return Err(OpError::Op {
            code: err["code"].as_i64().unwrap_or(0),
            message: err["message"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

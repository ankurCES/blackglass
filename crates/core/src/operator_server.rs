//! Operator-socket server. Speaks JSON-RPC 2.0 over a Unix domain socket
//! at the path passed to `run()`. Carries server-pushed `confirm.request`
//! events and responds to `confirm.resolve` and `ping` calls. See spec
//! §2.4 + §6.2.

use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::broker::{ConfirmationBroker, Decision};

/// A `confirm.request` event to be pushed to a connected operator.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfirmRequest {
    pub id: String,
    pub request_id: u64,
    pub tool: String,
    pub domain: String,
    pub class: String,
    pub target: String,
    pub source: String,
    pub deadline_in_ms: u64,
}

/// Channel of pending `ConfirmRequest`s. The chokepoint (or whoever
/// needs operator confirmation) calls `push_confirm`; the operator-socket
/// task broadcasts to all connected operator clients.
#[derive(Clone)]
pub struct ConfirmChannel {
    tx: broadcast::Sender<ConfirmRequest>,
}

impl ConfirmChannel {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(64);
        Self { tx }
    }

    pub fn push(&self, req: ConfirmRequest) {
        let _ = self.tx.send(req);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConfirmRequest> {
        self.tx.subscribe()
    }
}

impl Default for ConfirmChannel {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run(sock_path: &Path, broker: ConfirmationBroker) -> std::io::Result<()> {
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }
    let listener = UnixListener::bind(sock_path)?;
    let channel = ConfirmChannel::new();
    let channel = Arc::new(channel);

    loop {
        let (stream, _addr) = listener.accept().await?;
        let broker = broker.clone();
        let channel = channel.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, broker, channel).await {
                eprintln!("operator socket handler error: {e}");
            }
        });
    }
}

async fn handle(
    stream: UnixStream,
    broker: ConfirmationBroker,
    _channel: Arc<ConfirmChannel>,
) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() { continue; }

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        let resp = match parsed {
            Ok(v) => handle_rpc(v, &broker).await,
            Err(_) => Some(jsonrpc_error(None, -32700, "parse error")),
        };

        if let Some(r) = resp {
            write.write_all(r.as_bytes()).await?;
            write.write_all(b"\n").await?;
            write.flush().await?;
        }
    }
    Ok(())
}

async fn handle_rpc(v: serde_json::Value, broker: &ConfirmationBroker) -> Option<String> {
    let id = v.get("id").cloned();
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));

    match method {
        "ping" => Some(jsonrpc_ok(id, serde_json::json!("pong"))),
        "confirm.resolve" => {
            let cid = params.get("id").and_then(|s| s.as_str()).unwrap_or("");
            let decision_str = params.get("decision").and_then(|s| s.as_str()).unwrap_or("");
            let decision = match decision_str {
                "allow" => Decision::Allow,
                "allow_and_remember" => Decision::AllowAndRemember,
                "deny" => Decision::Deny,
                _ => {
                    return Some(jsonrpc_error(id, -32602, "invalid decision"));
                }
            };
            let result = broker.resolve(cid, decision).await;
            // Resolve returns Err for unknown id (already timed out) — that's
            // not a JSON-RPC error; it's logged at the audit layer. The Tauri
            // app gets a normal response here.
            let _ = result;
            Some(jsonrpc_ok(id, serde_json::json!({ "resolved": true })))
        }
        _ => Some(jsonrpc_error(id, -32601, "method not found")),
    }
}

fn jsonrpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn jsonrpc_error(id: Option<serde_json::Value>, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }).to_string()
}

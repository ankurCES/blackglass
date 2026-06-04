//! Operator-socket server. Speaks JSON-RPC 2.0 over a Unix domain socket
//! at the path passed to `run()`. Carries server-pushed `confirm.request`
//! events and responds to `confirm.resolve` and `ping` calls. See spec
//! §2.4 + §6.2.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::audit_query;
use crate::broker::{ConfirmationBroker, Decision};
use crate::mcp_run_tool;
use crate::mcp_supervisor::McpSupervisor;
use crate::operator_auth::OperatorAuth;
use blackglass_audit::{Chain, Event};

/// Type alias for the per-connection write half. The read loop and
/// every push/subscribe task hold a clone of this `Arc<Mutex<...>>`
/// so they can write to the same `OwnedWriteHalf` without races.
type ConnWrite = Arc<tokio::sync::Mutex<OwnedWriteHalf>>;

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

pub async fn run(
    sock_path: &Path,
    broker: ConfirmationBroker,
    channel: ConfirmChannel,
    chain: Arc<Mutex<Chain>>,
    supervisor: Arc<McpSupervisor>,
    runtime_sock_path: PathBuf,
    operator_token_path: PathBuf,
    event_tx: broadcast::Sender<Event>,
) -> std::io::Result<()> {
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }
    let listener = UnixListener::bind(sock_path)?;
    let channel = Arc::new(channel);

    loop {
        let (stream, _addr) = listener.accept().await?;
        let broker = broker.clone();
        let channel = channel.clone();
        let chain = chain.clone();
        let supervisor = supervisor.clone();
        let runtime_sock_path = runtime_sock_path.clone();
        let operator_token_path = operator_token_path.clone();
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(
                stream,
                broker,
                channel,
                chain,
                supervisor,
                runtime_sock_path,
                operator_token_path,
                event_tx,
            )
            .await
            {
                eprintln!("operator socket handler error: {e}");
            }
        });
    }
}

async fn handle(
    stream: UnixStream,
    broker: ConfirmationBroker,
    channel: Arc<ConfirmChannel>,
    chain: Arc<Mutex<Chain>>,
    supervisor: Arc<McpSupervisor>,
    runtime_sock_path: PathBuf,
    operator_token_path: PathBuf,
    event_tx: broadcast::Sender<Event>,
) -> std::io::Result<()> {
    let (read, write) = stream.into_split();
    // The write half is shared between tasks:
    //   - the read loop, which writes JSON-RPC responses to client RPCs
    //   - the push task, which writes server-pushed `confirm.request` events
    //   - the subscribe task(s), which write `audit.event` pushes
    // We wrap it in an `Arc<Mutex<...>>` so all of them can write without races.
    let write: ConnWrite = Arc::new(tokio::sync::Mutex::new(write));
    let mut lines = BufReader::new(read).lines();
    let mut events = channel.subscribe();

    // Per-connection auth state. Every method (other than `auth` itself)
    // is gated on this being `true`. Set by the `auth` method on a
    // successful `OperatorAuth::verify`. Lives for the lifetime of the
    // connection; the connection itself is dropped when the client
    // disconnects (or the operator server stops), which clears the flag
    // automatically.
    let authenticated = Arc::new(AtomicBool::new(false));

    // Forward server-pushed `confirm.request` notifications to the client.
    // The Tauri shell filters on `method == "confirm.request"`.
    let write_for_push = write.clone();
    let push_task = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(req) => {
                    let payload = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "confirm.request",
                        "params": req,
                    });
                    let mut w = write_for_push.lock().await;
                    if w.write_all(payload.to_string().as_bytes()).await.is_err() {
                        break;
                    }
                    if w.write_all(b"\n").await.is_err() {
                        break;
                    }
                    if w.flush().await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() { continue; }

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        let resp = match parsed {
            Ok(v) => {
                handle_rpc(
                    v,
                    &broker,
                    &chain,
                    &supervisor,
                    runtime_sock_path.clone(),
                    operator_token_path.clone(),
                    &authenticated,
                    &event_tx,
                    &write,
                )
                .await
            }
            Err(_) => Some(jsonrpc_error(None, -32700, "parse error")),
        };

        if let Some(r) = resp {
            let mut w = write.lock().await;
            w.write_all(r.as_bytes()).await?;
            w.write_all(b"\n").await?;
            w.flush().await?;
        }
    }
    push_task.abort();
    Ok(())
}

async fn handle_rpc(
    v: serde_json::Value,
    broker: &ConfirmationBroker,
    chain: &Mutex<Chain>,
    supervisor: &Arc<McpSupervisor>,
    runtime_sock_path: PathBuf,
    operator_token_path: PathBuf,
    authenticated: &AtomicBool,
    event_tx: &broadcast::Sender<Event>,
    write: &ConnWrite,
) -> Option<String> {
    let id = v.get("id").cloned();
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));

    // Auth gate (Task 2.5.7): every method *except* `auth` itself
    // requires a successful prior `auth` call on this connection.
    // The 7-arg signature is a deliberate deviation from the plan's
    // "state struct" wrapper — see the plan's amendment §2.5.7.
    //
    // The `auth` method is special: it both reads from and sets the
    // `authenticated` flag, so it's handled inline before the gate.
    if method == "auth" {
        return handle_auth(
            id,
            params,
            &operator_token_path,
            authenticated,
        );
    }
    if !authenticated.load(Ordering::Relaxed) {
        return Some(jsonrpc_error(
            id,
            -32001,
            "auth required: call `auth` first",
        ));
    }

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
        "audit.query" => match serde_json::from_value::<audit_query::QueryParams>(params) {
            Ok(p) => match audit_query::handle_query(chain, p) {
                Ok(resp) => match serde_json::to_value(&resp) {
                    Ok(v) => Some(jsonrpc_ok(id, v)),
                    Err(e) => Some(jsonrpc_error(id, -32603, &format!("serialize: {e}"))),
                },
                Err(e) => Some(jsonrpc_error(id, -32603, &format!("audit: {e}"))),
            },
            Err(e) => Some(jsonrpc_error(id, -32602, &format!("invalid params: {e}"))),
        },
        "audit.verify_chain" => match audit_query::handle_verify(chain) {
            Ok(count) => Some(jsonrpc_ok(id, serde_json::json!(count))),
            Err(e) => Some(jsonrpc_error(id, -32603, &format!("audit: {e}"))),
        },
        "mcp_run_tool" => match serde_json::from_value::<mcp_run_tool::McpRunParams>(params) {
            Ok(p) => match mcp_run_tool::handle_mcp_run_tool(p, supervisor, &runtime_sock_path, chain, event_tx).await {
                Ok(resp) => match serde_json::to_value(&resp) {
                    Ok(v) => Some(jsonrpc_ok(id, v)),
                    Err(e) => Some(jsonrpc_error(id, -32603, &format!("serialize: {e}"))),
                },
                // Per-variant error codes:
                //   -32010 UnknownDomain
                //   -32011 McpDown
                //   -32012 Timeout
                //   -32013 McpError (chokepoint denied, or runtime error)
                // The Tauri UI can switch on `error.code` to render
                // domain-specific UX (e.g. "MCP not running, retry?" vs
                // "denied by policy"). The `error.message` is the
                // `Display` of the variant, which carries a
                // recognizable substring per the test plan.
                Err(mcp_run_tool::McpRunError::UnknownDomain(_)) => {
                    Some(jsonrpc_error(id, -32010, "domain is not routed to any MCP server"))
                }
                Err(mcp_run_tool::McpRunError::McpDown(ref s)) => {
                    Some(jsonrpc_error(id, -32011, &format!("mcp server {s} is not running")))
                }
                Err(e @ mcp_run_tool::McpRunError::Timeout(_, _)) => {
                    // Use the variant's own Display so the test's
                    // `msg.contains("timeout")` assertion succeeds and
                    // the message stays in one place.
                    Some(jsonrpc_error(id, -32012, &format!("{e}")))
                }
                Err(e @ mcp_run_tool::McpRunError::McpError(..)) => {
                    // The inner message from the chokepoint (e.g. "gate
                    // denied: ...") is in `e`'s Display, so we forward
                    // it verbatim. The Tauri UI checks substrings like
                    // "denied" / "gate" to render the right UX.
                    Some(jsonrpc_error(id, -32013, &format!("{e}")))
                }
                Err(e) => Some(jsonrpc_error(id, -32603, &format!("mcp: {e}"))),
            },
            Err(e) => Some(jsonrpc_error(id, -32602, &format!("invalid params: {e}"))),
        },
        "subscribe" => handle_subscribe(id, params, event_tx, write),
        _ => Some(jsonrpc_error(id, -32601, "method not found")),
    }
}

/// Handle the `subscribe` method. The only channel we currently
/// support is `audit.event` — the live tail of all `chain.append`
/// events. On a valid `{"channel":"audit.event"}` request, we
/// subscribe a fresh per-connection task to the broadcast channel
/// and respond with `{"ok":true}`. From that point on, every event
/// appended via `audit_broadcast::append_and_broadcast` is pushed
/// to the client as a `{"jsonrpc":"2.0","method":"audit.event",
/// "params":{"event": <Event>}}` newline-terminated frame.
///
/// The task is detached (`tokio::spawn`) — it lives for the
/// connection's lifetime. When the client disconnects, the next
/// `write_all` fails, the task breaks out of the loop, and the
/// `Receiver` is dropped. The broadcast channel itself is shared
/// with all other connections and with the chokepoint / supervisor
/// emitters; the per-connection receiver is the only thing that
/// dies on disconnect.
///
/// Failure modes:
///   -32602 — invalid `params` shape (missing `channel`, wrong type)
///   -32602 — unknown channel name (e.g. "logs", "events")
fn handle_subscribe(
    id: Option<serde_json::Value>,
    params: serde_json::Value,
    event_tx: &broadcast::Sender<Event>,
    write: &ConnWrite,
) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct SubscribeParams {
        channel: String,
    }
    let p: SubscribeParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Some(jsonrpc_error(id, -32602, &format!("invalid params: {e}")));
        }
    };
    if p.channel != "audit.event" {
        return Some(jsonrpc_error(
            id,
            -32602,
            &format!("unknown channel `{}`", p.channel),
        ));
    }
    let mut rx = event_tx.subscribe();
    let write = write.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let frame = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "audit.event",
                        "params": { "event": event },
                    });
                    let mut w = write.lock().await;
                    if w.write_all(frame.to_string().as_bytes()).await.is_err() {
                        break;
                    }
                    if w.write_all(b"\n").await.is_err() {
                        break;
                    }
                    if w.flush().await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    Some(jsonrpc_ok(id, serde_json::json!({"ok": true})))
}

/// Handle the `auth` method: verify the presented token against the
/// token file on disk, and on success flip the per-connection
/// `authenticated` flag so subsequent calls bypass the -32001 gate.
///
/// Wire format:
/// ```json
/// {"jsonrpc":"2.0","id":N,"method":"auth","params":{"token":"<the-literal-token>"}}
/// ```
/// The token on disk is `\n`-terminated (see `OperatorAuth::verify` and
/// the 2.5.2 tests), so the caller sends the *literal* token bytes (no
/// trailing newline) and we append `\n` here before delegating to
/// `OperatorAuth::verify` — which is what the file actually contains.
///
/// Error codes:
///   -32002 — auth failed (token mismatch, file missing/loose perms, etc.)
fn handle_auth(
    id: Option<serde_json::Value>,
    params: serde_json::Value,
    operator_token_path: &Path,
    authenticated: &AtomicBool,
) -> Option<String> {
    let presented = params
        .get("token")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    // Add the trailing newline that the on-disk file carries.
    let mut presented_with_nl = presented.as_bytes().to_vec();
    presented_with_nl.push(b'\n');
    let auth = OperatorAuth::new(operator_token_path);
    match auth.verify(&presented_with_nl) {
        Ok(()) => {
            authenticated.store(true, Ordering::Relaxed);
            Some(jsonrpc_ok(id, serde_json::json!({ "ok": true })))
        }
        Err(e) => Some(jsonrpc_error(id, -32002, &e.to_string())),
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

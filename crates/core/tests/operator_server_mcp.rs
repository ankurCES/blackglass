//! Integration tests for the operator-socket `mcp_run_tool` method
//! (plan Task 2.5.5).
//!
//! The test harness follows the same pattern as
//! `operator_server_audit.rs` (Task 2.5.3): spin up the operator server
//! in-process, connect a raw `UnixStream` client, and exercise the
//! dispatcher. No `connect_operator_with_token` helper — auth is a
//! 2.5.7 deliverable, so the test writes JSON-RPC lines directly.
//!
//! For `mcp_run_tool` the operator also needs a `McpSupervisor` (to do
//! the liveness pre-flight) and a real `runtime.sock` (to forward the
//! `execute_action` to). Both are stubbed:
//!
//! - The supervisor is a real `McpSupervisor` with a single server
//!   spec. The spec's `command` is either `sleep` (the "sleeper" —
//!   stays alive forever, so `status() == Alive`) or
//!   `/bin/sh -c "exit 1"` (the "crasher" — exits immediately, the
//!   supervisor transitions to `Restarting` after a few hundred ms).
//!   The supervisor is a real supervisor (not a mock) because the
//!   `mcp_run_tool` code path only knows how to ask it for status;
//!   any fake would be a different abstraction.
//!
//! - The runtime.sock is a `tokio::net::UnixListener` on a tempdir
//!   path. A small accept loop reads `RpcRequest` frames and replies
//!   with a controllable `RpcResponse` (allow/deny, with optional
//!   delay). The reply shape is whatever the test's `StubBehavior`
//!   says: `{ok: true, audit_event_id: "test-id"}` for the success
//!   case, `{ok: false, error: "gate denied: ..."}` for the deny
//!   case, etc.
//!
//! This keeps every test hermetic (no real MCP, no real chokepoint)
//! while exercising the production wire path (length-prefixed Rpc
//! frames) and the production supervisor (real child processes with
//! `kill_on_drop`).

use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::mcp_run_tool::{mcp_for_domain, McpRunParams};
use blackglass_core::mcp_spawn_config::McpServerSpec;
use blackglass_core::mcp_supervisor::McpSupervisor;
use blackglass_core::operator_server::{run as run_operator, ConfirmChannel};
use blackglass_ipc::{encode_frame, MAX_FRAME};
use blackglass_audit::Chain;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;

/// What the fake runtime.sock should reply with. The operator's
/// `mcp_run_tool` code path treats `result.ok == false` and
/// `result.ok == true` differently from a "runtime error"
/// (`rpc.ok == false, rpc.error = Some(...)`).
#[derive(Debug, Clone)]
enum StubBehavior {
    /// Reply with `rpc.ok = true, result = {"ok": true, "audit_event_id": "test-id"}`.
    Ok,
    /// Reply with `rpc.ok = true, result = {"ok": false, "error": <message>}`.
    /// Used to simulate the chokepoint returning a successful RPC
    /// whose action outcome is "denied by gate".
    Denied(String),
    /// Hold the connection open for `delay` before sending the `Ok`
    /// reply. Used to exercise the timeout path.
    DelayedOk(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerKind {
    /// `sleep 600` — the supervisor sees a child that stays alive.
    Sleeper,
    /// `sh -c "exit 1"` — the supervisor sees a child that exits
    /// immediately, status becomes `Restarting` after a few hundred ms.
    Crasher,
}

/// Per-test handle. Dropping it cleans up the tempdir, aborts the
/// operator + supervisor tasks, and lets the stub listener exit
/// (its `accept` loop checks a `stop` flag, and a `Drop` on the
/// tempdir will remove the socket file).
struct TestCore {
    operator_sock: PathBuf,
    _runtime_sock: PathBuf,
    _dir: tempfile::TempDir,
    _behavior: Arc<Mutex<StubBehavior>>,
    _supervisor: Arc<McpSupervisor>,
    _operator_task: tokio::task::JoinHandle<()>,
    _stub_task: tokio::task::JoinHandle<()>,
    _stop_stub: Arc<Notify>,
    /// Token string for the `auth` handshake. Tests that make
    /// JSON-RPC calls use it via `connect_and_auth(&self.operator_sock,
    /// &self.token)`.
    token: String,
}

impl Drop for TestCore {
    fn drop(&mut self) {
        // Signal the stub to stop (so its `accept` loop can exit
        // without holding the tempdir). We don't need to wait — the
        // stub task is also aborted below, and dropping `_dir` will
        // unlink the socket file.
        self._stop_stub.notify_waiters();
        self._stub_task.abort();
        self._operator_task.abort();
        // `_supervisor` keeps the monitor loops alive via its
        // internal Arc clones; when the test ends, the supervisor
        // is dropped, the monitor loops' references are released,
        // and the children (with `kill_on_drop`) are killed.
    }
}

async fn spawn_test_core(server_kind: ServerKind) -> TestCore {
    spawn_test_core_with_behavior(server_kind, StubBehavior::Ok).await
}

async fn spawn_test_core_with_behavior(
    server_kind: ServerKind,
    initial_behavior: StubBehavior,
) -> TestCore {
    let dir = tempdir().unwrap();
    let operator_sock = dir.path().join("operator.sock");
    let runtime_sock = dir.path().join("runtime.sock");
    let chain_path = dir.path().join("chain.jsonl");
    let sup_log = dir.path().join("supervisor.log");

    // --- supervisor with one child ------------------------------------
    // The child command is whatever the test wants: a sleeper (always
    // alive) or a crasher (exits immediately, status becomes
    // `Restarting`).
    let (command, args) = match server_kind {
        ServerKind::Sleeper => ("sleep".to_string(), vec!["600".to_string()]),
        ServerKind::Crasher => (
            "/bin/sh".to_string(),
            vec!["-c".to_string(), "exit 1".to_string()],
        ),
    };
    let cfg = blackglass_core::mcp_spawn_config::McpSpawnConfig {
        servers: vec![McpServerSpec {
            name: "mcp-ad".into(),
            command,
            args,
            startup_timeout_ms: 5_000,
            max_restarts: 100, // keep restarting forever so the test sees Restarting, not GivenUp
        }],
    };
    let supervisor = McpSupervisor::start_with_chain(cfg, &sup_log, &chain_path, tokio::sync::broadcast::channel(64).0)
        .await
        .expect("supervisor start");
    // Wrap the supervisor in an Arc so we can hand a clone to the
    // operator task while the test still owns a reference (used to
    // introspect status if needed). The supervisor's monitor tasks
    // hold their own Arc clones of the inner state, so they keep
    // running independently of this Arc.
    let supervisor = Arc::new(supervisor);

    // --- fake runtime.sock (stub listener) ----------------------------
    let behavior = Arc::new(Mutex::new(initial_behavior));
    let stop_stub = Arc::new(Notify::new());
    let stub_task = {
        let runtime_sock = runtime_sock.clone();
        let behavior = behavior.clone();
        let stop = stop_stub.clone();
        tokio::spawn(async move {
            // Remove any stale socket file (UnixListener::bind
            // refuses to bind over an existing file).
            let _ = std::fs::remove_file(&runtime_sock);
            let listener = match UnixListener::bind(&runtime_sock) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("stub bind failed: {e}");
                    return;
                }
            };
            loop {
                tokio::select! {
                    _ = stop.notified() => return,
                    accept = listener.accept() => {
                        let (mut conn, _addr) = match accept {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let behavior = behavior.clone();
                        tokio::spawn(async move {
                            // Read one RpcRequest frame.
                            let mut lenb = [0u8; 4];
                            if conn.read_exact(&mut lenb).await.is_err() {
                                return;
                            }
                            let len = u32::from_be_bytes(lenb) as usize;
                            if len > MAX_FRAME {
                                return;
                            }
                            let mut payload = vec![0u8; len];
                            if conn.read_exact(&mut payload).await.is_err() {
                                return;
                            }
                            // We don't actually inspect the request —
                            // the stub is purely a controllable reply
                            // generator. The test decides what to
                            // reply with.
                            let _ = serde_json::from_slice::<serde_json::Value>(&payload);
                            // Compute the reply per the behavior.
                            let (reply, delay) = {
                                let b = behavior.lock().unwrap();
                                match &*b {
                                    StubBehavior::Ok => (
                                        json!({
                                            "id": 1,
                                            "ok": true,
                                            "result": {
                                                "ok": true,
                                                "audit_event_id": "test-id",
                                            },
                                        }),
                                        Duration::from_millis(0),
                                    ),
                                    StubBehavior::Denied(msg) => (
                                        json!({
                                            "id": 1,
                                            "ok": true,
                                            "result": {
                                                "ok": false,
                                                "error": msg,
                                            },
                                        }),
                                        Duration::from_millis(0),
                                    ),
                                    StubBehavior::DelayedOk(d) => (
                                        json!({
                                            "id": 1,
                                            "ok": true,
                                            "result": {
                                                "ok": true,
                                                "audit_event_id": "test-id",
                                            },
                                        }),
                                        *d,
                                    ),
                                }
                            };
                            if !delay.is_zero() {
                                tokio::time::sleep(delay).await;
                            }
                            let bytes = serde_json::to_vec(&reply).unwrap();
                            if conn.write_all(&encode_frame(&bytes)).await.is_err() {
                                return;
                            }
                            let _ = conn.flush().await;
                        });
                    }
                }
            }
        })
    };
    // Wait for the runtime.sock to come up before returning, so the
    // operator's `UnixStream::connect` doesn't fail with
    // `ConnectionRefused`. The operator's pre-flight check is a
    // liveness check, not a connection check — but tests want a
    // known-good socket regardless.
    for _ in 0..100 {
        if runtime_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // --- operator server ---------------------------------------------
    // The test owns the chain (it doesn't need a populated one for
    // `mcp_run_tool`), so we open an empty one. We have to clone
    // the supervisor Arc — the test uses the supervisor directly to
    // call `handle_mcp_run_tool` (via the operator's dispatcher),
    // and the operator server also takes a `Arc<McpSupervisor>` to
    // look up liveness. Sharing the same `McpSupervisor` across
    // both callers is exactly what production does.
    let chain = Arc::new(Mutex::new(Chain::open(&chain_path).expect("open empty chain")));
    let broker = ConfirmationBroker::new();
    let channel = ConfirmChannel::new();
    let op_sock = operator_sock.clone();
    let rt_sock_for_op = runtime_sock.clone();
    let op_chain = chain.clone();
    let op_sup = supervisor.clone();
    let op_broker = broker.clone();
    let op_channel = channel.clone();
    // Auth (Task 2.5.7): write the 0600 token file before starting
    // the operator server, then pass its path as the 7th arg.
    let token_path = dir.path().join("operator.token");
    let token = write_token_file(&dir);
    let op_token_path = token_path.clone();
    let operator_task = tokio::spawn(async move {
        let _ = run_operator(
            &op_sock,
            op_broker,
            op_channel,
            op_chain,
            op_sup,
            rt_sock_for_op,
            op_token_path,
            tokio::sync::broadcast::channel(64).0,
        )
        .await;
    });
    // Wait for operator.sock to come up.
    for _ in 0..100 {
        if operator_sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Also wait for the operator to have a chance to register its
    // accept loop, so the first connect doesn't race the bind.

    TestCore {
        operator_sock,
        _runtime_sock: runtime_sock,
        _dir: dir,
        _behavior: behavior,
        _supervisor: supervisor,
        _operator_task: operator_task,
        _stub_task: stub_task,
        _stop_stub: stop_stub,
        token,
    }
}

// ===========================================================================
// JSON-RPC client helpers (operator.sock is line-delimited JSON-RPC)
// ===========================================================================

/// Write a 0600 token file at `<dir>/operator.token` with the
/// test token and return the token string. The token must exist
/// before the operator server starts so the `auth` route can find it.
const TEST_TOKEN: &str = "operator-test-token-mcp";
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
/// needed for further `rpc_call`s).
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

async fn rpc_call(
    sock: &mut UnixStream,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let mut line = req.to_string();
    line.push('\n');
    sock.write_all(line.as_bytes()).await.expect("write rpc");
    sock.flush().await.expect("flush rpc");

    use tokio::io::AsyncBufReadExt;
    let mut reader = tokio::io::BufReader::new(sock);
    let mut buf = String::new();
    let n = tokio::time::timeout(Duration::from_secs(5), reader.read_line(&mut buf))
        .await
        .expect("server should respond within 5s")
        .expect("read should not error");
    assert!(n > 0, "server returned EOF");
    serde_json::from_str(buf.trim()).expect("response should be valid JSON")
}

// ===========================================================================
// Tests
// ===========================================================================

#[tokio::test]
async fn mcp_run_tool_returns_ok_when_mcp_allows() {
    let core = spawn_test_core(ServerKind::Sleeper).await;
    let mut s = connect_and_auth(&core.operator_sock, &core.token).await;
    let resp = rpc_call(
        &mut s,
        1,
        "mcp_run_tool",
        json!({
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {"target": "10.0.0.5", "user": "admin", "cmd": "whoami"},
        }),
    )
    .await;
    // Success path: response has `result` (no `error`).
    assert!(
        resp.get("error").is_none(),
        "expected no error, got: {resp}"
    );
    let result = resp.get("result").expect("response should have result");
    assert_eq!(
        result.get("ok").and_then(|v| v.as_bool()),
        Some(true),
        "expected ok=true, got: {resp}"
    );
    assert_eq!(
        result.get("audit_event_id").and_then(|v| v.as_str()),
        Some("test-id"),
        "expected audit_event_id=test-id, got: {resp}"
    );
}

#[tokio::test]
async fn mcp_run_tool_returns_denied_when_chokepoint_denies() {
    let core = spawn_test_core_with_behavior(
        ServerKind::Sleeper,
        StubBehavior::Denied("gate denied: policy violation".to_string()),
    )
    .await;
    let mut s = connect_and_auth(&core.operator_sock, &core.token).await;
    let resp = rpc_call(
        &mut s,
        1,
        "mcp_run_tool",
        json!({
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {},
        }),
    )
    .await;
    // The chokepoint's "denied" outcome flows through as a successful
    // JSON-RPC response whose `result.ok == false` and `result.error`
    // carries the chokepoint's message. This is the same shape the
    // Tauri UI uses to render the "this action was denied" UX.
    let result = resp.get("result").expect("response should have result on denied outcome");
    assert_eq!(
        result.get("ok").and_then(|v| v.as_bool()),
        Some(false),
        "expected ok=false, got: {resp}"
    );
    let err = result
        .get("error")
        .and_then(|v| v.as_str())
        .expect("error field should be a string");
    assert!(
        err.contains("denied") || err.contains("gate"),
        "expected 'denied' or 'gate' in error, got: {err}"
    );
}

#[tokio::test]
async fn mcp_run_tool_returns_error_when_mcp_server_is_down() {
    // Spawn a crasher — exits immediately, the supervisor transitions
    // the child to `Restarting` after a few hundred ms.
    let core = spawn_test_core(ServerKind::Crasher).await;
    // Wait long enough for the child to have exited and the
    // supervisor to have transitioned to `Restarting`. The first
    // backoff is 1s, so we wait 1.5s to be safe.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut s = connect_and_auth(&core.operator_sock, &core.token).await;
    let resp = rpc_call(
        &mut s,
        1,
        "mcp_run_tool",
        json!({
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {},
        }),
    )
    .await;
    // MCP-down returns a JSON-RPC error with code -32011. The
    // Tauri UI can switch on the code to render a "the MCP server
    // is restarting, please retry" UX.
    let error = resp.get("error").expect("response should have error when MCP is down");
    assert_eq!(
        error.get("code").and_then(|c| c.as_i64()),
        Some(-32011),
        "expected error code -32011 (McpDown), got: {resp}"
    );
    let msg = error
        .get("message")
        .and_then(|m| m.as_str())
        .expect("error.message should be a string");
    assert!(
        msg.contains("not running") || msg.contains("mcp server"),
        "expected 'not running'/'mcp server' in message, got: {msg}"
    );
}

#[tokio::test]
async fn mcp_run_tool_times_out_when_mcp_takes_too_long() {
    // Override the timeout to 500ms. The env var is read on every
    // call (see `mcp_run_tool::timeout`), so a set_var here takes
    // effect immediately. We set it before the call; cleanup is
    // per-test (the env is process-global, but the test process
    // exits after this test).
    std::env::set_var("BLACKGLASS_MCP_RUN_TIMEOUT_MS", "500");
    let core = spawn_test_core_with_behavior(
        ServerKind::Sleeper,
        StubBehavior::DelayedOk(Duration::from_secs(5)),
    )
    .await;
    let mut s = connect_and_auth(&core.operator_sock, &core.token).await;
    let start = std::time::Instant::now();
    let resp = rpc_call(
        &mut s,
        1,
        "mcp_run_tool",
        json!({
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {},
        }),
    )
    .await;
    let elapsed = start.elapsed();
    let error = resp
        .get("error")
        .expect("response should have error on timeout");
    assert_eq!(
        error.get("code").and_then(|c| c.as_i64()),
        Some(-32012),
        "expected error code -32012 (Timeout), got: {resp}"
    );
    let msg = error
        .get("message")
        .and_then(|m| m.as_str())
        .expect("error.message should be a string");
    assert!(
        msg.contains("timeout"),
        "expected 'timeout' in message, got: {msg}"
    );
    // The timeout is 500ms, but allow generous slack for the
    // connect + setup. We want to assert the call didn't wait
    // the full 5s delay (which would mean the timeout didn't
    // fire).
    assert!(
        elapsed < Duration::from_secs(3),
        "timeout test took {elapsed:?}, expected < 3s (timeout is 500ms)"
    );
}

// ===========================================================================
// Unit tests for the pure helpers (no supervisor / no socket)
// ===========================================================================

#[test]
fn mcp_for_domain_maps_known_domains() {
    assert_eq!(mcp_for_domain("ad"), Some("mcp-ad"));
    assert_eq!(mcp_for_domain("flipper"), Some("mcp-flipper"));
    assert_eq!(mcp_for_domain("phish"), Some("mcp-phish"));
    assert_eq!(mcp_for_domain("detect"), Some("mcp-detect"));
    assert_eq!(mcp_for_domain("osint"), None);
    assert_eq!(mcp_for_domain("packets"), None);
    assert_eq!(mcp_for_domain("unknown"), None);
}

#[test]
fn mcp_run_params_deserializes_minimum_fields() {
    let p: McpRunParams = serde_json::from_value(json!({
        "domain": "ad",
        "target": "x",
        "args": {"k": "v"},
    }))
    .unwrap();
    assert_eq!(p.domain, "ad");
    assert_eq!(p.target, "x");
    assert_eq!(p.args, json!({"k": "v"}));
}

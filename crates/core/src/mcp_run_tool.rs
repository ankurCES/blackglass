//! Operator-socket `mcp_run_tool` method. The Tauri app calls this
//! to run any of the 16 existing + new tools via the chokepoint.
//!
//! Flow:
//! 1. Look up which MCP server owns (domain, target) — for v1, the
//!    mapping is hardcoded: `ad` → mcp-ad, `flipper` → mcp-flipper,
//!    `phish` → mcp-phish, `detect` → mcp-detect.
//! 2. Check the MCP server is alive (ask the supervisor).
//! 3. Forward the request to the MCP server over runtime.sock as
//!    an `execute_action` JSON-RPC call. Wait up to 30s for a reply
//!    (override via `BLACKGLASS_MCP_RUN_TIMEOUT_MS`).
//! 4. Return `{ok, stdout?, stderr?, audit_event_id?, error?}`.
//!
//! The error variants map to JSON-RPC error codes:
//!   -32010 UnknownDomain
//!   -32011 McpDown
//!   -32012 Timeout
//!   -32013 McpError (chokepoint denied, or runtime error)
//!
//! These ranges are well above the standard -32700..-32600 range
//! (per JSON-RPC 2.0 §6) so a UI can distinguish "transport /
//! parameter / server" errors from "app" errors.

use crate::mcp_supervisor::{ChildStatus, McpSupervisor};
use crate::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_ipc::encode_frame;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

#[derive(Debug, Error)]
pub enum McpRunError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("domain {0} is not routed to any MCP server")]
    UnknownDomain(String),
    #[error("mcp server {0} is not running")]
    McpDown(String),
    #[error("mcp server {0} timeout after {1}ms")]
    Timeout(String, u64),
    #[error("mcp server {0} returned error: {1}")]
    McpError(String, String),
}

#[derive(Debug, Deserialize)]
pub struct McpRunParams {
    pub domain: String,
    pub target: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpRunResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Resolve (domain) → MCP server name. Hardcoded for v1. Sub-plan 1
/// established these four. `osint` / `packets` are *not* in this mapping —
/// they live in the bridge layer (not the MCP layer) and are reachable
/// directly via the runtime socket, so the operator's MCP-down check is
/// a no-op for them (and they're handled by the "unknown domain" path,
/// which the test asserts surfaces as a 404 to the UI).
pub fn mcp_for_domain(domain: &str) -> Option<&'static str> {
    match domain {
        "ad" => Some("mcp-ad"),
        "flipper" => Some("mcp-flipper"),
        "phish" => Some("mcp-phish"),
        "detect" => Some("mcp-detect"),
        _ => None,
    }
}

/// Read the timeout from `BLACKGLASS_MCP_RUN_TIMEOUT_MS`, falling back to
/// 30000ms (30s). The env var override exists so tests can drop the
/// timeout to ~500ms without making the production code know about
/// "test mode". Read on every call so a `std::env::set_var` from a
/// test (or an operator-driven override) takes effect immediately
/// rather than being frozen at process start.
pub fn timeout() -> Duration {
    match std::env::var("BLACKGLASS_MCP_RUN_TIMEOUT_MS") {
        Ok(s) if !s.is_empty() => {
            if let Ok(ms) = s.parse::<u64>() {
                if ms > 0 {
                    return Duration::from_millis(ms);
                }
            }
            Duration::from_millis(30_000)
        }
        _ => Duration::from_millis(30_000),
    }
}

pub async fn handle_mcp_run_tool(
    params: McpRunParams,
    supervisor: &McpSupervisor,
    runtime_sock_path: &Path,
) -> Result<McpRunResult, McpRunError> {
    let mcp_name = mcp_for_domain(&params.domain)
        .ok_or_else(|| McpRunError::UnknownDomain(params.domain.clone()))?;
    let status = supervisor.status(mcp_name).await;
    match status {
        Some(ChildStatus::Alive) => {}
        // None (server not in supervisor at all), Restarting, or GivenUp
        // all count as "not running" from the operator's point of view.
        _ => return Err(McpRunError::McpDown(mcp_name.into())),
    }

    // Forward to runtime.sock as execute_action. The runtime speaks the
    // length-prefixed `rpc::RpcRequest` protocol (NOT JSON-RPC), so we
    // build that wire shape here. The Tauri UI's `mcp_run_tool` JSON-RPC
    // and the runtime's length-prefixed protocol are bridged at this
    // seam.
    //
    // TODO(2.5.6): action_class is hardcoded to "destructive" as a
    // placeholder. The real lookup (which bridge / which class a given
    // (domain, target) implies) belongs to the chokepoint's dispatch
    // table. Either lift that table to be shared, or have the Tauri UI
    // supply action_class explicitly.
    let req = RpcRequest {
        id: 1,
        method: Method::ExecuteAction(crate::gates::ActionRequest {
            domain: params.domain.clone(),
            action_class: "destructive".to_string(),
            target: params.target.clone(),
            args: params.args.clone(),
        }),
    };
    let req_bytes = serde_json::to_vec(&req)?;
    let mut stream = UnixStream::connect(runtime_sock_path).await?;
    stream.write_all(&encode_frame(&req_bytes)).await?;
    stream.flush().await?;

    let dur = timeout();
    let dur_ms = dur.as_millis() as u64;
    // Read the response with a single overall timeout. We can't easily
    // split the read into "length prefix" + "payload" stages with a
    // single timeout, because if the timeout fires mid-payload the
    // buffered bytes from the length prefix are lost. The simple fix is
    // to wrap the entire read in one timeout. If the timeout fires
    // before the read completes, the read future is dropped (and the
    // socket is closed by the runtime if the runtime cares), and we
    // return Timeout. Slight downside: if the MCP is fast on the prefix
    // but slow on the payload, the timeout includes both. That's the
    // intended semantics — "we've been waiting longer than X for the
    // MCP to reply".
    use tokio::io::AsyncReadExt;
    let read_fut = async {
        let mut lenb = [0u8; 4];
        stream.read_exact(&mut lenb).await?;
        let len = u32::from_be_bytes(lenb) as usize;
        if len > blackglass_ipc::MAX_FRAME {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "runtime frame too large",
            ));
        }
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;
        Ok::<_, std::io::Error>(payload)
    };
    let payload = match tokio::time::timeout(dur, read_fut).await {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => return Err(McpRunError::Io(e)),
        Err(_) => return Err(McpRunError::Timeout(mcp_name.into(), dur_ms)),
    };
    let resp: RpcResponse = serde_json::from_slice(&payload)?;

    if let Some(err) = resp.error {
        // The chokepoint's errors include strings like "gate3 denied: ..."
        // and "gate denied: ..." — those flow through verbatim to the
        // operator's `error.message`, and the test asserts the substring
        // "denied" / "gate" is present.
        return Err(McpRunError::McpError(mcp_name.into(), err));
    }
    if !resp.ok {
        // `resp.ok == false` with no error string is unusual but
        // possible if the runtime synthesized an error response that
        // didn't populate the `error` field. Surface a synthetic message
        // so the operator always has something to display.
        return Err(McpRunError::McpError(
            mcp_name.into(),
            "runtime returned ok=false with no error message".to_string(),
        ));
    }
    let result = resp.result.unwrap_or(serde_json::Value::Null);
    Ok(McpRunResult {
        ok: result.get("ok").and_then(|v| v.as_bool()).unwrap_or(true),
        stdout: result.get("stdout").and_then(|v| v.as_str()).map(String::from),
        stderr: result.get("stderr").and_then(|v| v.as_str()).map(String::from),
        audit_event_id: result
            .get("audit_event_id")
            .and_then(|v| v.as_str())
            .map(String::from),
        error: result
            .get("error")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

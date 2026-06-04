//! The 3 new Tauri commands: `mcp_run_tool`, `mcp_list_tools`,
//! `audit_event`. Each is a thin wrapper over the operator socket
//! (see `operator_client.rs` and design §2.4a).
//!
//! The pure async functions (`mcp_run_tool`, `mcp_list_tools`,
//! `audit_event`) take an explicit socket path + token, so they can
//! be unit-tested without Tauri's runtime. The `*_cmd` functions are
//! the Tauri command bindings that pull socket path + token from
//! `AppState` and are what the Svelte side invokes.

use crate::operator_client::{call, connect_and_auth, OpError};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct McpRunRequest {
    pub domain: String,
    pub target: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRunResponse {
    pub ok: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub audit_event_id: Option<String>,
    pub error: Option<String>,
}

fn op_err_to_string(e: OpError) -> String {
    e.to_string()
}

/// Run a tool via the operator socket's `mcp_run_tool` method. The
/// core looks up the right MCP server, runs the tool through Gate 3
/// (with operator confirmation if the action class requires it), and
/// returns the result inline. The audit chain captures
/// `McpRunStarted` / `McpRunCompleted`.
pub async fn mcp_run_tool(
    req: McpRunRequest,
    sock_path: &Path,
    token: &str,
) -> Result<McpRunResponse, String> {
    let mut stream = connect_and_auth(sock_path, token).map_err(op_err_to_string)?;
    let result = call(
        &mut stream,
        "mcp_run_tool",
        serde_json::json!({
            "domain": req.domain,
            "target": req.target,
            "args": req.args,
        }),
    )
    .map_err(op_err_to_string)?;
    serde_json::from_value(result).map_err(|e| e.to_string())
}

/// List the tools available in a given MCP domain. For v1, the
/// catalog is hardcoded in `lib/toolCatalog.ts` on the Svelte side
/// (per the amendment plan). This command exists as a placeholder
/// that returns an empty list, so the Svelte side can fall back to
/// the bundled catalog if the core ever needs to override. When the
/// core's `mcp.list_servers` is extended to also report tools, this
/// command will be wired to that method.
pub async fn mcp_list_tools(
    _domain: String,
    _sock_path: &Path,
    _token: &str,
) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!([]))
}

/// Fetch a single audit event by id. The core doesn't have a
/// dedicated `audit.event` method — we use the existing `audit.query`
/// with an id filter and return the first match. If no event
/// matches, returns `Ok(serde_json::Value::Null)`.
pub async fn audit_event(
    id: String,
    sock_path: &Path,
    token: &str,
) -> Result<serde_json::Value, String> {
    let mut stream = connect_and_auth(sock_path, token).map_err(op_err_to_string)?;
    let result = call(
        &mut stream,
        "audit.query",
        serde_json::json!({
            "filter": { "id": id },
            "page": 0,
            "page_size": 1
        }),
    )
    .map_err(op_err_to_string)?;
    let events = result["events"].as_array().cloned().unwrap_or_default();
    Ok(events.into_iter().next().unwrap_or(serde_json::Value::Null))
}

/// Page through the audit log. Pass-through to the core's
/// `audit.query` method; returns the full QueryResponse (events +
/// chain head + verified flag + page metadata) so the Svelte audit
/// log can show "chain verified at <hash>" without a second
/// round-trip.
pub async fn audit_query(
    filter: serde_json::Value,
    page: u32,
    page_size: u32,
    sock_path: &Path,
    token: &str,
) -> Result<serde_json::Value, String> {
    let mut stream = connect_and_auth(sock_path, token).map_err(op_err_to_string)?;
    let result = call(
        &mut stream,
        "audit.query",
        serde_json::json!({
            "filter": filter,
            "page": page,
            "page_size": page_size,
        }),
    )
    .map_err(op_err_to_string)?;
    Ok(result)
}

// Tauri command bindings (wrap the pure functions with State access).
// The Svelte side calls these via `invoke("mcp_run_tool_cmd", { ... })`.

#[tauri::command]
pub async fn mcp_run_tool_cmd(
    domain: String,
    target: String,
    args: serde_json::Value,
    state: tauri::State<'_, crate::AppState>,
) -> Result<McpRunResponse, String> {
    mcp_run_tool(
        McpRunRequest {
            domain,
            target,
            args,
        },
        &state.operator_sock_path,
        &state.operator_token,
    )
    .await
}

#[tauri::command]
pub async fn mcp_list_tools_cmd(
    domain: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    mcp_list_tools(domain, &state.operator_sock_path, &state.operator_token).await
}

#[tauri::command]
pub async fn audit_event_cmd(
    id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    audit_event(id, &state.operator_sock_path, &state.operator_token).await
}

#[tauri::command]
pub async fn audit_query_cmd(
    filter: serde_json::Value,
    page: u32,
    page_size: u32,
    state: tauri::State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    audit_query(filter, page, page_size, &state.operator_sock_path, &state.operator_token).await
}

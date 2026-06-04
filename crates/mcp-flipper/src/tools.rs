//! Tool registration for mcp-flipper. 4 tools, all routed through
//! the Python bridge's `blackglass_sidecar.hardware_bridge.flipper_*`
//! functions.

use anyhow::Result;
use blackglass_python_bridge::{BridgeRequest, PythonBridge, StubBridge};
use blackglass_runtime::GateClient;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
    transport::io::stdio,
};
use serde_json::{Value, json};
use std::sync::Arc;

/// The 4 Flipper tools the spec lists.
pub const FLIPPER_TOOLS: &[&str] = &[
    "flipper-list",
    "flipper-read",
    "flipper-write",
    "flipper-run",
];

/// Map an MCP tool name to the (module, function) pair the bridge
/// should call. Returns None for unknown tools.
pub fn tool_to_bridge_fn(tool: &str) -> Option<(&'static str, &'static str)> {
    match tool {
        "flipper-list" => Some(("blackglass_sidecar.hardware_bridge", "flipper_list")),
        "flipper-read" => Some(("blackglass_sidecar.hardware_bridge", "flipper_read")),
        "flipper-write" => Some(("blackglass_sidecar.hardware_bridge", "flipper_write")),
        "flipper-run" => Some(("blackglass_sidecar.hardware_bridge", "flipper_run")),
        _ => None,
    }
}

/// Pure dispatch helper (testable).
pub async fn dispatch(
    bridge: &dyn PythonBridge,
    tool: &str,
    args: Value,
) -> std::result::Result<Value, String> {
    let (module, function) = tool_to_bridge_fn(tool)
        .ok_or_else(|| format!("unknown flipper tool: {tool}"))?;
    let req = BridgeRequest {
        module: module.into(),
        function: function.into(),
        args,
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.map_err(|e| format!("bridge: {e}"))?;
    Ok(resp.result)
}

pub struct FlipperServer {
    gate: Arc<GateClient>,
    bridge: Arc<dyn PythonBridge>,
}

impl ServerHandler for FlipperServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tool_schemas: Vec<Tool> = FLIPPER_TOOLS
            .iter()
            .map(|name| {
                Tool::new(
                    *name,
                    format!("Flipper Zero tool: {name}. Routes through the Python sidecar."),
                    Arc::new(
                        serde_json::from_value(json!({
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Path on the Flipper (e.g. /ext/apps)"},
                                "data": {"type": "string", "description": "Data to write (for write/run)"},
                            },
                            "required": []
                        }))
                        .expect("static schema"),
                    ),
                )
            })
            .collect();
        Ok(ListToolsResult {
            tools: tool_schemas,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let args = request.arguments.unwrap_or_default();
        self.gate
            .execute("flipper", "active_scan", &request.name, json!({}))
            .await
            .map_err(|e| McpError::internal_error(format!("gate denied: {e}"), None))?;
        let v = serde_json::to_value(args)
            .map_err(|e| McpError::internal_error(format!("args: {e}"), None))?;
        match dispatch(self.bridge.as_ref(), &request.name, v).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string(),
            )])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e)])),
        }
    }
}

pub async fn serve(gate: Arc<GateClient>, bridge: Arc<dyn PythonBridge>) -> Result<()> {
    let running = rmcp::serve_server(FlipperServer { gate, bridge }, stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_to_bridge_fn_maps_all_4_tools() {
        assert_eq!(
            tool_to_bridge_fn("flipper-list"),
            Some(("blackglass_sidecar.hardware_bridge", "flipper_list"))
        );
        assert_eq!(
            tool_to_bridge_fn("flipper-read"),
            Some(("blackglass_sidecar.hardware_bridge", "flipper_read"))
        );
        assert_eq!(
            tool_to_bridge_fn("flipper-write"),
            Some(("blackglass_sidecar.hardware_bridge", "flipper_write"))
        );
        assert_eq!(
            tool_to_bridge_fn("flipper-run"),
            Some(("blackglass_sidecar.hardware_bridge", "flipper_run"))
        );
    }

    #[test]
    fn tool_to_bridge_fn_rejects_unknown() {
        assert_eq!(tool_to_bridge_fn("flipper-bogus"), None);
    }

    #[tokio::test]
    async fn dispatch_with_stub_bridge_succeeds() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "flipper-list", json!({})).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_tool() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "flipper-bogus", json!({})).await;
        assert!(res.is_err());
    }
}

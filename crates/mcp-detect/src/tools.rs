//! Tool registration for mcp-detect. 3 tools, all routed through
//! the Python bridge's `blackglass_sidecar.detect_bridge.*` functions.

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

/// The 3 detect tools the spec lists.
pub const DETECT_TOOLS: &[&str] = &[
    "detect-image",
    "detect-video",
    "detect-batch",
];

/// Map an MCP tool name to the (module, function) pair the bridge
/// should call. Returns None for unknown tools.
pub fn tool_to_bridge_fn(tool: &str) -> Option<(&'static str, &'static str)> {
    match tool {
        "detect-image" => Some(("blackglass_sidecar.detect_bridge", "image")),
        "detect-video" => Some(("blackglass_sidecar.detect_bridge", "video")),
        "detect-batch" => Some(("blackglass_sidecar.detect_bridge", "batch")),
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
        .ok_or_else(|| format!("unknown detect tool: {tool}"))?;
    let req = BridgeRequest {
        module: module.into(),
        function: function.into(),
        args,
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.map_err(|e| format!("bridge: {e}"))?;
    Ok(resp.result)
}

pub struct DetectServer {
    gate: Arc<GateClient>,
    bridge: Arc<dyn PythonBridge>,
}

impl ServerHandler for DetectServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tool_schemas: Vec<Tool> = DETECT_TOOLS
            .iter()
            .map(|name| {
                Tool::new(
                    *name,
                    format!("Deepfake detection tool: {name}. Routes through the Python sidecar."),
                    Arc::new(
                        serde_json::from_value(json!({
                            "type": "object",
                            "properties": {
                                "path": {"type": "string", "description": "Path to the media file (or directory for batch)"},
                                "model": {"type": "string", "description": "Detector model name (optional)"},
                            },
                            "required": ["path"]
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
            .execute("detect", "read_only", &request.name, json!({}))
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
    let running = rmcp::serve_server(DetectServer { gate, bridge }, stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_to_bridge_fn_maps_all_3_tools() {
        assert_eq!(
            tool_to_bridge_fn("detect-image"),
            Some(("blackglass_sidecar.detect_bridge", "image"))
        );
        assert_eq!(
            tool_to_bridge_fn("detect-video"),
            Some(("blackglass_sidecar.detect_bridge", "video"))
        );
        assert_eq!(
            tool_to_bridge_fn("detect-batch"),
            Some(("blackglass_sidecar.detect_bridge", "batch"))
        );
    }

    #[test]
    fn tool_to_bridge_fn_rejects_unknown() {
        assert_eq!(tool_to_bridge_fn("detect-audio"), None);
    }

    #[tokio::test]
    async fn dispatch_with_stub_bridge_succeeds() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "detect-image", json!({"path": "/tmp/x.png"})).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_tool() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "detect-bogus", json!({})).await;
        assert!(res.is_err());
    }
}

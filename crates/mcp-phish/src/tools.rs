//! Tool registration for mcp-phish. 5 evilginx + 4 gophish tools.

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

/// The 9 phish tools the spec lists.
pub const PHISH_TOOLS: &[&str] = &[
    // evilginx
    "phish-list",
    "phish-enable",
    "phish-disable",
    "phish-get_captures",
    "phish-lure_create",
    // gophish
    "phish-gophish_campaign_list",
    "phish-gophish_campaign_create",
    "phish-gophish_campaign_status",
    "phish-gophish_results",
];

/// Map an MCP tool name to the (module, function) pair the bridge
/// should call. Returns None for unknown tools.
pub fn tool_to_bridge_fn(tool: &str) -> Option<(&'static str, &'static str)> {
    match tool {
        "phish-list" => Some(("blackglass_sidecar.evilginx_bridge", "list")),
        "phish-enable" => Some(("blackglass_sidecar.evilginx_bridge", "enable")),
        "phish-disable" => Some(("blackglass_sidecar.evilginx_bridge", "disable")),
        "phish-get_captures" => {
            Some(("blackglass_sidecar.evilginx_bridge", "get_captures"))
        }
        "phish-lure_create" => {
            Some(("blackglass_sidecar.evilginx_bridge", "lure_create"))
        }
        "phish-gophish_campaign_list" => Some((
            "blackglass_sidecar.gophish_bridge",
            "campaign_list",
        )),
        "phish-gophish_campaign_create" => Some((
            "blackglass_sidecar.gophish_bridge",
            "campaign_create",
        )),
        "phish-gophish_campaign_status" => Some((
            "blackglass_sidecar.gophish_bridge",
            "campaign_status",
        )),
        "phish-gophish_results" => {
            Some(("blackglass_sidecar.gophish_bridge", "results"))
        }
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
        .ok_or_else(|| format!("unknown phish tool: {tool}"))?;
    let req = BridgeRequest {
        module: module.into(),
        function: function.into(),
        args,
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.map_err(|e| format!("bridge: {e}"))?;
    Ok(resp.result)
}

pub struct PhishServer {
    gate: Arc<GateClient>,
    bridge: Arc<dyn PythonBridge>,
}

impl ServerHandler for PhishServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tool_schemas: Vec<Tool> = PHISH_TOOLS
            .iter()
            .map(|name| {
                Tool::new(
                    *name,
                    format!("Phishing platform tool: {name}. Routes through the Python sidecar."),
                    Arc::new(
                        serde_json::from_value(json!({
                            "type": "object",
                            "properties": {
                                "name": {"type": "string"},
                                "template": {"type": "string"},
                                "target": {"type": "string"},
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
            .execute("phish", "active_scan", &request.name, json!({}))
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
    let running = rmcp::serve_server(PhishServer { gate, bridge }, stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_to_bridge_fn_maps_all_5_evilginx_tools() {
        assert!(tool_to_bridge_fn("phish-list").is_some());
        assert!(tool_to_bridge_fn("phish-enable").is_some());
        assert!(tool_to_bridge_fn("phish-disable").is_some());
        assert!(tool_to_bridge_fn("phish-get_captures").is_some());
        assert!(tool_to_bridge_fn("phish-lure_create").is_some());
    }

    #[test]
    fn tool_to_bridge_fn_maps_all_4_gophish_tools() {
        assert!(tool_to_bridge_fn("phish-gophish_campaign_list").is_some());
        assert!(tool_to_bridge_fn("phish-gophish_campaign_create").is_some());
        assert!(tool_to_bridge_fn("phish-gophish_campaign_status").is_some());
        assert!(tool_to_bridge_fn("phish-gophish_results").is_some());
    }

    #[test]
    fn tool_to_bridge_fn_routes_to_distinct_modules() {
        // evilginx tools go to evilginx_bridge
        let (m1, _) = tool_to_bridge_fn("phish-list").unwrap();
        assert!(m1.contains("evilginx"));
        // gophish tools go to gophish_bridge
        let (m2, _) = tool_to_bridge_fn("phish-gophish_results").unwrap();
        assert!(m2.contains("gophish"));
    }

    #[test]
    fn tool_to_bridge_fn_rejects_unknown() {
        assert_eq!(tool_to_bridge_fn("phish-bogus"), None);
    }

    #[tokio::test]
    async fn dispatch_with_stub_bridge_succeeds() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "phish-list", json!({})).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_tool() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "phish-bogus", json!({})).await;
        assert!(res.is_err());
    }
}

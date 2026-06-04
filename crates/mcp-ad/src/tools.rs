//! Tool registration for mcp-ad. Each tool is a thin wrapper that
//! deserializes args, dispatches to the Python bridge, and returns
//! the result through the chokepoint (gate).

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
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// The 5 AD tools the spec lists. The match key is the (tool, function)
/// pair dispatched into the bridge module `blackglass_sidecar.impacket_bridge`.
pub const AD_TOOLS: &[&str] = &[
    "ad-impacket_psexec",
    "ad-impacket_wmiexec",
    "ad-impacket_secretsdump",
    "ad-impacket_kerberoast",
    "ad-impacket_asreproast",
];

/// Map an MCP tool name to the (module, function) pair the bridge
/// should call. Returns None for unknown tools.
pub fn tool_to_bridge_fn(tool: &str) -> Option<(&'static str, &'static str)> {
    match tool {
        "ad-impacket_psexec" => Some(("blackglass_sidecar.impacket_bridge", "psexec")),
        "ad-impacket_wmiexec" => Some(("blackglass_sidecar.impacket_bridge", "wmiexec")),
        "ad-impacket_secretsdump" => {
            Some(("blackglass_sidecar.impacket_bridge", "secretsdump"))
        }
        "ad-impacket_kerberoast" => {
            Some(("blackglass_sidecar.impacket_bridge", "kerberoast"))
        }
        "ad-impacket_asreproast" => {
            Some(("blackglass_sidecar.impacket_bridge", "asreproast"))
        }
        _ => None,
    }
}

/// Pure dispatch helper: build a BridgeRequest and call the bridge.
/// Returns the raw `result` JSON from the response. Returns Err if
/// the tool is unknown or the bridge fails.
pub async fn dispatch(
    bridge: &dyn PythonBridge,
    tool: &str,
    args: Value,
) -> std::result::Result<Value, String> {
    let (module, function) = tool_to_bridge_fn(tool)
        .ok_or_else(|| format!("unknown ad tool: {tool}"))?;
    let req = BridgeRequest {
        module: module.into(),
        function: function.into(),
        args,
        evidence_dir: None,
    };
    let resp = bridge.invoke(req).await.map_err(|e| format!("bridge: {e}"))?;
    Ok(resp.result)
}

#[derive(Debug, Deserialize)]
pub struct ImpacketArgs {
    pub target: String,
    pub user: String,
    pub hash: String,
    #[serde(default)]
    pub remote_cmd: Option<String>,
}

pub struct AdServer {
    gate: Arc<GateClient>,
    bridge: Arc<dyn PythonBridge>,
}

impl ServerHandler for AdServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        let tool_schemas: Vec<Tool> = AD_TOOLS
            .iter()
            .map(|name| {
                Tool::new(
                    *name,
                    format!("Active Directory tool: {name}. Routes through the Python sidecar."),
                    Arc::new(
                        serde_json::from_value(json!({
                            "type": "object",
                            "properties": {
                                "target": {"type": "string"},
                                "user": {"type": "string"},
                                "hash": {"type": "string"},
                                "remote_cmd": {"type": "string"}
                            },
                            "required": ["target", "user", "hash"]
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
        // The gate authorizes the action before the bridge runs.
        // We use 'active_scan' class as the worst case — the chokepoint's
        // Gate 2 will narrow this to the engagement's allowlist.
        self.gate
            .execute("ad", "active_scan", &request.name, json!({}))
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
    let running = rmcp::serve_server(AdServer { gate, bridge }, stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[allow(dead_code)]
fn _get_str<'a>(
    args: &'a Map<String, Value>,
    key: &str,
) -> std::result::Result<&'a str, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params(format!("missing required argument: {key}"), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_to_bridge_fn_maps_all_5_tools() {
        assert_eq!(
            tool_to_bridge_fn("ad-impacket_psexec"),
            Some(("blackglass_sidecar.impacket_bridge", "psexec"))
        );
        assert_eq!(
            tool_to_bridge_fn("ad-impacket_wmiexec"),
            Some(("blackglass_sidecar.impacket_bridge", "wmiexec"))
        );
        assert_eq!(
            tool_to_bridge_fn("ad-impacket_secretsdump"),
            Some(("blackglass_sidecar.impacket_bridge", "secretsdump"))
        );
        assert_eq!(
            tool_to_bridge_fn("ad-impacket_kerberoast"),
            Some(("blackglass_sidecar.impacket_bridge", "kerberoast"))
        );
        assert_eq!(
            tool_to_bridge_fn("ad-impacket_asreproast"),
            Some(("blackglass_sidecar.impacket_bridge", "asreproast"))
        );
    }

    #[test]
    fn tool_to_bridge_fn_rejects_unknown() {
        assert_eq!(tool_to_bridge_fn("not-a-real-tool"), None);
        assert_eq!(tool_to_bridge_fn(""), None);
    }

    #[tokio::test]
    async fn dispatch_with_stub_bridge_succeeds() {
        let bridge = StubBridge::new();
        let res = dispatch(
            &bridge,
            "ad-impacket_psexec",
            json!({"target": "10.0.0.1", "user": "admin", "hash": "aad3b435b51404ee"}),
        )
        .await;
        // Stub returns a result regardless of args.
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_tool() {
        let bridge = StubBridge::new();
        let res = dispatch(&bridge, "ad-evil-tool", json!({})).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("unknown ad tool"));
    }
}

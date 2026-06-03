use blackglass_runtime::GateClient;
use rmcp::{
    ErrorData as McpError,
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::{MaybeSendFuture, RequestContext, RoleServer},
};
use serde_json::{Value, json};
use std::sync::Arc;
use anyhow::Result;

fn validate_target(t: &str) -> std::result::Result<(), String> {
    if t.is_empty() || t.len() > 253 {
        return Err("target length out of range".into());
    }
    if t.chars().any(|c| matches!(c, ';' | '&' | '|' | '`' | '$' | '\n' | '\r')) {
        return Err("target contains shell-unsafe characters".into());
    }
    Ok(())
}

fn run_cmd(prog: &str, args: &[&str]) -> String {
    std::process::Command::new(prog)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_else(|e| format!("error running {prog}: {e}"))
}

fn tool_schema(param_description: &str) -> Arc<serde_json::Map<String, Value>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "target": {
                "type": "string",
                "description": param_description
            }
        },
        "required": ["target"]
    });
    Arc::new(schema.as_object().expect("json! object is always a Map").clone())
}

pub struct OsintServer {
    gate: Arc<GateClient>,
}

impl OsintServer {
    pub fn new(gate: Arc<GateClient>) -> Self {
        Self { gate }
    }
}

impl ServerHandler for OsintServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = std::result::Result<ListToolsResult, McpError>>
           + MaybeSendFuture
           + '_ {
        let whois = Tool::new(
            "osint-whois",
            "Perform a WHOIS lookup on a domain or IP address",
            tool_schema("Domain name or IP address to look up"),
        );
        let dig = Tool::new(
            "osint-dig",
            "Perform a DNS lookup using dig",
            tool_schema("Domain name or IP address to query"),
        );
        std::future::ready(Ok(ListToolsResult::with_all_items(vec![whois, dig])))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = std::result::Result<CallToolResult, McpError>>
           + MaybeSendFuture
           + '_ {
        let gate = Arc::clone(&self.gate);
        async move {
            let target = request
                .arguments
                .as_ref()
                .and_then(|a| a.get("target"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if let Err(e) = validate_target(target) {
                return Ok(CallToolResult::error(vec![Content::text(e)]));
            }

            match request.name.as_ref() {
                "osint-whois" => {
                    let output = run_cmd("whois", &[target]);
                    gate.execute("osint", "read_only", target, json!({}))
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![Content::text(output)]))
                }
                "osint-dig" => {
                    let output = run_cmd("dig", &[target]);
                    gate.execute("osint", "read_only", target, json!({}))
                        .await
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    Ok(CallToolResult::success(vec![Content::text(output)]))
                }
                name => Ok(CallToolResult::error(vec![Content::text(format!(
                    "unknown tool: {name}"
                ))])),
            }
        }
    }
}

pub async fn serve(gate: Arc<GateClient>) -> Result<()> {
    let server = OsintServer::new(gate);
    let running = rmcp::serve_server(server, rmcp::transport::io::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

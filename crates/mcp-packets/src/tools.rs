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
use serde_json::{Map, Value, json};
use std::sync::Arc;
use anyhow::Result;

fn validate_path(p: &str) -> std::result::Result<(), String> {
    if p.is_empty() {
        return Err("path is empty".into());
    }
    if p.contains("..") {
        return Err("path traversal rejected".into());
    }
    if p.chars().any(|c| matches!(c, ';' | '&' | '|' | '`' | '$' | '\n' | '\r')) {
        return Err("path contains shell-unsafe characters".into());
    }
    Ok(())
}

fn validate_iface(iface: &str) -> std::result::Result<(), String> {
    if iface.is_empty() {
        return Err("interface name is empty".into());
    }
    if !iface.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("interface name contains invalid characters".into());
    }
    Ok(())
}

fn schema_path_only() -> Arc<Map<String, Value>> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to the pcap file"}
            },
            "required": ["path"]
        }))
        .expect("static schema"),
    )
}

fn schema_export() -> Arc<Map<String, Value>> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Source pcap file path"},
                "dest": {"type": "string", "description": "Destination path"}
            },
            "required": ["path", "dest"]
        }))
        .expect("static schema"),
    )
}

fn schema_spec_only() -> Arc<Map<String, Value>> {
    Arc::new(
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "spec": {"type": "string", "description": "Scapy packet specification"}
            },
            "required": ["spec"]
        }))
        .expect("static schema"),
    )
}

fn get_str<'a>(args: &'a Map<String, Value>, key: &str) -> std::result::Result<&'a str, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params(format!("missing required argument: {key}"), None))
}

pub struct PacketsServer {
    gate: Arc<GateClient>,
}

impl ServerHandler for PacketsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: vec![
                Tool::new(
                    "packets-tshark_read",
                    "Read a pcap file and return its decoded contents using tshark",
                    schema_path_only(),
                ),
                Tool::new(
                    "packets-pcap_export",
                    "Copy a pcap file to a destination path",
                    schema_export(),
                ),
                Tool::new(
                    "packets-tshark_capture",
                    "Capture live packets using tshark (ActiveScan)",
                    Arc::new(
                        serde_json::from_value(json!({
                            "type": "object",
                            "properties": {
                                "interface": {"type": "string", "description": "Network interface to capture on (e.g. eth0, lo)"},
                                "count": {"type": "integer", "description": "Number of packets to capture (default 100)"},
                                "output_path": {"type": "string", "description": "Path to write the output pcap file"}
                            },
                            "required": ["interface", "output_path"]
                        }))
                        .expect("static schema"),
                    ),
                ),
                Tool::new(
                    "packets-scapy_craft",
                    "Craft custom packets offline using Scapy. Requires Python sidecar (available in Sub-plan 4).",
                    schema_spec_only(),
                ),
            ],
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        let args = request.arguments.unwrap_or_default();
        match request.name.as_ref() {
            "packets-tshark_read" => {
                let path = get_str(&args, "path")?.to_owned();
                self.tshark_read(&path).await
            }
            "packets-pcap_export" => {
                let path = get_str(&args, "path")?.to_owned();
                let dest = get_str(&args, "dest")?.to_owned();
                self.pcap_export(&path, &dest).await
            }
            "packets-tshark_capture" => {
                let interface = get_str(&args, "interface")?.to_owned();
                let count = args
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100);
                let output_path = get_str(&args, "output_path")?.to_owned();
                self.tshark_capture(&interface, count, &output_path).await
            }
            // Python sidecar is Sub-plan 4; return a clear stub error now
            "packets-scapy_craft" => Err(McpError::invalid_params(
                "scapy_craft requires the Python sidecar which is not available in this build (Sub-plan 4).",
                None,
            )),
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

impl PacketsServer {
    async fn tshark_read(&self, path: &str) -> std::result::Result<CallToolResult, McpError> {
        if let Err(e) = validate_path(path) {
            return Ok(CallToolResult::error(vec![Content::text(e)]));
        }
        if !std::path::Path::new(path).exists() {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "file not found: {path}"
            ))]));
        }
        let out = std::process::Command::new("tshark")
            .args(["-r", path, "-T", "text"])
            .output()
            .map_err(|e| {
                McpError::internal_error(format!("failed to run tshark: {e}"), None)
            })?;
        self.gate
            .execute("packets", "read_only", path, json!({}))
            .await
            .map_err(|e| McpError::internal_error(format!("gate denied: {e}"), None))?;
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            Ok(CallToolResult::success(vec![Content::text(text)]))
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            Ok(CallToolResult::error(vec![Content::text(format!(
                "tshark error: {stderr}"
            ))]))
        }
    }

    async fn tshark_capture(
        &self,
        interface: &str,
        count: u64,
        output_path: &str,
    ) -> std::result::Result<CallToolResult, McpError> {
        if let Err(e) = validate_iface(interface) {
            return Ok(CallToolResult::error(vec![Content::text(e)]));
        }
        if let Err(e) = validate_path(output_path) {
            return Ok(CallToolResult::error(vec![Content::text(e)]));
        }
        self.gate
            .execute("packets", "active_scan", interface, json!({}))
            .await
            .map_err(|e| McpError::internal_error(format!("gate denied: {e}"), None))?;
        let out = std::process::Command::new("tshark")
            .args([
                "-i",
                interface,
                "-c",
                &count.to_string(),
                "-w",
                output_path,
            ])
            .output()
            .map_err(|e| {
                McpError::internal_error(format!("failed to run tshark: {e}"), None)
            })?;
        if out.status.success() {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "captured {count} packets on {interface} → {output_path}"
            ))]))
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            Ok(CallToolResult::error(vec![Content::text(format!(
                "tshark error: {stderr}"
            ))]))
        }
    }

    async fn pcap_export(
        &self,
        path: &str,
        dest: &str,
    ) -> std::result::Result<CallToolResult, McpError> {
        if let Err(e) = validate_path(path) {
            return Ok(CallToolResult::error(vec![Content::text(e)]));
        }
        if let Err(e) = validate_path(dest) {
            return Ok(CallToolResult::error(vec![Content::text(e)]));
        }
        std::fs::copy(path, dest)
            .map_err(|e| McpError::internal_error(format!("copy failed: {e}"), None))?;
        self.gate
            .execute("packets", "read_only", path, json!({}))
            .await
            .map_err(|e| McpError::internal_error(format!("gate denied: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "exported {path} → {dest}"
        ))]))
    }
}

pub async fn serve(gate: Arc<GateClient>) -> Result<()> {
    let running = rmcp::serve_server(PacketsServer { gate }, stdio()).await?;
    running.waiting().await?;
    Ok(())
}

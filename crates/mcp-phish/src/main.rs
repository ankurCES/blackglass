//! blackglass-mcp-phish — Phishing platform MCP server.
//!
//! Exposes 5 evilginx + 4 gophish tools over JSON-RPC-over-stdio. All
//! actual work is routed through the Python bridge.

use anyhow::Result;
use blackglass_python_bridge::{PythonBridge, StubBridge};
use blackglass_runtime::GateClient;
use clap::Parser;
use std::{path::PathBuf, sync::Arc};

mod tools;

#[derive(Parser)]
#[command(name = "blackglass-mcp-phish", version)]
struct Cli {
    #[arg(long, default_value = "~/.local/share/blackglass/runtime.sock")]
    socket: String,
    #[arg(long, default_value = "~/.local/share/blackglass/operator.token")]
    token_file: String,
    #[arg(long)]
    python: Option<PathBuf>,
}

fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();
    let socket = expand(&cli.socket);
    let token = std::fs::read_to_string(expand(&cli.token_file))?
        .trim()
        .to_string();
    let gate = Arc::new(GateClient::new(socket, token));
    let bridge: Arc<dyn PythonBridge> = Arc::new(StubBridge::new());
    tools::serve(gate, bridge).await
}

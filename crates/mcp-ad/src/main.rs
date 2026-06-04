//! blackglass-mcp-ad — Active Directory MCP server.
//!
//! Exposes 5 impacket-wrapping tools over JSON-RPC-over-stdio. All
//! actual work is routed through the Python bridge via the core's
//! chokepoint.

use anyhow::Result;
use blackglass_python_bridge::{PythonBridge, StubBridge};
use blackglass_runtime::GateClient;
use clap::Parser;
use std::{path::PathBuf, sync::Arc};

mod tools;

#[derive(Parser)]
#[command(name = "blackglass-mcp-ad", version)]
struct Cli {
    #[arg(long, default_value = "~/.local/share/blackglass/runtime.sock")]
    socket: String,
    #[arg(long, default_value = "~/.local/share/blackglass/operator.token")]
    token_file: String,
    /// Path to the venv's `python` binary. If absent, uses the stub bridge.
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

    // Build the bridge. Use stub if no --python flag (dev mode).
    let bridge: Arc<dyn PythonBridge> = match &cli.python {
        Some(_p) => {
            // Production builds would link the `real` feature; for
            // the dev binary we use the stub. This is a wire-up
            // test only — the pyo3-gated RealPythonBridge is
            // implemented in a follow-up.
            Arc::new(StubBridge::new())
        }
        None => Arc::new(StubBridge::new()),
    };

    tools::serve(gate, bridge).await
}

use anyhow::Result;
use blackglass_runtime::GateClient;
use clap::Parser;
use std::{path::PathBuf, sync::Arc};

mod tools;

#[derive(Parser)]
#[command(name = "blackglass-mcp-osint", version)]
struct Cli {
    #[arg(long, default_value = "~/.local/share/blackglass/runtime.sock")]
    socket: String,
    #[arg(long, default_value = "~/.local/share/blackglass/operator.token")]
    token_file: String,
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
    let socket = expand(&cli.socket);
    let token = std::fs::read_to_string(expand(&cli.token_file))?.trim().to_string();
    let gate = Arc::new(GateClient::new(socket, token));
    tools::serve(gate).await
}

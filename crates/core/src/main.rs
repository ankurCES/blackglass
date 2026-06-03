use anyhow::Result;
use blackglass_audit::Chain;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::AllowAll;
use blackglass_core::sanitizer::RealSanitizer;
use blackglass_core::server::Server;
use blackglass_engagement::Engagement;
use blackglass_profile::Profile;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "blackglass-core", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Start {
        #[arg(long, default_value = "~/.local/share/blackglass/runtime.sock")]
        socket: String,
        #[arg(long, default_value = "~/.local/share/blackglass/audit/audit.jsonl")]
        audit: String,
        #[arg(long, default_value = "spine-token")]
        token: String,
    },
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
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Start { socket, audit, token } => {
            let socket = expand(&socket);
            let audit = expand(&audit);
            if let Some(parent) = audit.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if let Some(parent) = socket.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let evidence_dir = expand("~/.local/share/blackglass/evidence");
            std::fs::create_dir_all(&evidence_dir)?;
            let chain = Chain::open(&audit)?;
            let profile = Profile::analyst_default();
            let eng = Engagement::new(
                "default",
                "default engagement",
                "1970-01-01T00:00:00Z",
                "9999-12-31T00:00:00Z",
            );
            let cp = Chokepoint::new(
                chain,
                profile,
                eng,
                Arc::new(AllowAll),
                Arc::new(RealSanitizer::new(100 * 1024, evidence_dir.clone())),
            )
            .with_evidence_dir(evidence_dir);
            let server = Server::bind(&socket, token, cp).await?;
            server.serve().await?;
        }
    }
    Ok(())
}

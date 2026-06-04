//! Secondary sidecar launcher.
//!
//! Spawns the FastAPI deepfake-detection server as a child process,
//! forwards stdout/stderr to our own streams, waits for it to exit,
//! and tears it down cleanly on Ctrl-C. The server binds to
//! `127.0.0.1:8511` (loopback only — the AppArmor profile enforces
//! this at the kernel level too).
//!
//! The `python` argument is the path to the venv's Python binary
//! (typically `/var/lib/blackglass/venv-secondary/bin/python` in
//! production). The sidecar package itself must be installed in that
//! venv; see `python/secondary-sidecar/pyproject.toml` and the
//! packaging postinst.
//!
//! v1 has no real model; the FastAPI handlers in
//! `python/secondary-sidecar/src/blackglass_secondary/server.py`
//! return placeholder `unknown` verdicts. v1.1 will add a real
//! model (MesoNet, FaceForensics++, or similar) and the wire format
//! stays the same.

use anyhow::Result;
use clap::Parser;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Parser)]
#[command(name = "blackglass-secondary-sidecar", version)]
struct Cli {
    /// Path to the venv's `python` binary.
    #[arg(long)]
    python: std::path::PathBuf,
    /// Bind address. Loopback only by default; the AppArmor profile
    /// also rejects non-loopback binds.
    #[arg(long, default_value = "127.0.0.1:8511")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    // try_init avoids panicking if the parent process already wired a
    // subscriber (matters in tests; harmless in production).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    // Split "host:port" so uvicorn gets the right flags.
    let (host, port) = cli
        .bind
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("--bind must be host:port, got {:?}", cli.bind))?;
    let module_target = "blackglass_secondary.server:app";

    tracing::info!(?cli.python, %host, %port, "starting secondary sidecar");
    let mut child = Command::new(&cli.python)
        .args([
            "-m", "uvicorn",
            module_target,
            "--host", host,
            "--port", port,
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;

    // Wait for the child, or for Ctrl-C. On Ctrl-C, kill the child
    // and exit with 130 (standard "killed by SIGINT" code).
    tokio::select! {
        status = child.wait() => {
            let status = status?;
            tracing::info!(?status, "uvicorn exited");
            std::process::exit(status.code().unwrap_or(1));
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::warn!("received Ctrl-C, killing uvicorn");
            child.kill().await.ok();
            std::process::exit(130);
        }
    }
}

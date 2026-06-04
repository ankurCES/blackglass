use anyhow::Result;
use blackglass_audit::Chain;
use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::BrokerGate3;
use blackglass_core::operator_server::ConfirmChannel;
use blackglass_core::sanitizer::RealSanitizer;
use blackglass_core::server::Server;
use blackglass_engagement::Engagement;
use blackglass_profile::Profile;
use blackglass_python_bridge::{BridgeKind, PythonBridge};
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
        /// Python sidecar bridge kind: `stub` (default, in-process) or
        /// `real` (pyo3-backed; requires the `real` feature and a working
        /// venv). Unknown values fall back to `stub`.
        #[arg(long, default_value = "stub", value_parser = parse_bridge_kind)]
        python_bridge: BridgeKind,
        /// Path to a Python binary (used when `--python-bridge=real`).
        /// Optional; currently informational — `RealBridge` is selected
        /// at compile time and the sidecar venv is found via
        /// `BLACKGLASS_EVIDENCE_DIR` / default locations.
        #[arg(long)]
        python_bin: Option<PathBuf>,
    },
}

fn parse_bridge_kind(s: &str) -> Result<BridgeKind, String> {
    Ok(BridgeKind::from_str_loose(s))
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
        Cmd::Start { socket, audit, token, python_bridge, python_bin } => {
            // Currently informational only — RealBridge does not consume
            // a path. Kept as a CLI flag so the postinst and packaging
            // can plumb a path through later without breaking callers.
            if let Some(ref p) = python_bin {
                tracing::debug!(?p, "python_bin provided (currently unused)");
            }
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
            // Gate 3 (operator confirmation) needs a broker AND a channel.
            // The channel is shared: BrokerGate3 pushes requests to it, and
            // operator_server::run reads from it to forward to Tauri clients.
            let broker = ConfirmationBroker::new();
            let channel = ConfirmChannel::new();
            let gate3: Arc<dyn blackglass_core::gates::Gate3> =
                Arc::new(BrokerGate3::new_anonymous(broker.clone(), channel.clone()));
            // Construct the Python sidecar bridge. Default is the in-process
            // stub; `--python-bridge=real` requires the `real` feature on
            // blackglass-python-bridge. `--python-bin` is accepted but the
            // current RealBridge does not consume a path — it loads the
            // sidecar from the active Python interpreter's sys.path.
            let python_bridge_impl: Option<Arc<dyn PythonBridge>> = match python_bridge {
                BridgeKind::Stub => {
                    tracing::info!("python bridge: stub (no sidecar)");
                    Some(blackglass_python_bridge::build(BridgeKind::Stub))
                }
                #[cfg(feature = "real")]
                BridgeKind::Real => {
                    tracing::info!(?python_bin, "python bridge: real (pyo3)");
                    Some(blackglass_python_bridge::build(BridgeKind::Real))
                }
                #[cfg(not(feature = "real"))]
                BridgeKind::Real => {
                    tracing::warn!(
                        "python bridge: real requested but `real` feature is disabled; using stub"
                    );
                    Some(blackglass_python_bridge::build(BridgeKind::Stub))
                }
            };
            let cp = Chokepoint::new(
                chain,
                profile,
                eng,
                gate3,
                Arc::new(RealSanitizer::new(100 * 1024, evidence_dir.clone())),
            )
            .with_evidence_dir(evidence_dir)
            .with_python_bridge(python_bridge_impl);
            // Sub-plan 3: operator socket (Tauri UI). Runs concurrently with
            // the runtime socket accept loop below. We pass it the same
            // broker (to resolve confirmations), the same channel (to
            // subscribe to pending confirmations), and a read-only handle
            // to the audit chain (for `audit.query` / `audit.verify_chain`).
            // We re-open the chain on the same path because `Chokepoint`
            // already owns its `Chain` by value, and the operator socket
            // only needs `&Chain` (read-only query/verify).
            let data_dir = socket
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            let operator_sock = data_dir.join("operator.sock");
            let op_broker = broker.clone();
            let op_channel = channel.clone();
            let op_chain = Arc::new(Chain::open(&audit)?);
            // TODO(2.5.6): wire a real `McpSupervisor` (built from
            // `mcp-servers.toml`) and the real `runtime.sock` path
            // (== `socket` above) into the operator server. For 2.5.5
            // we pass placeholders: an empty-config supervisor (whose
            // `status(name)` returns `None` for everything, so
            // `mcp_run_tool` will always return `McpDown` until 2.5.6
            // fills this in) and a placeholder socket path that won't
            // exist (so even if the supervisor is alive, the runtime
            // forward will fail with a clear `NotFound`). Neither path
            // is exercised in production before 2.5.6 ships.
            let placeholder_supervisor = {
                let cfg = blackglass_core::mcp_spawn_config::McpSpawnConfig::default();
                let sup_path = data_dir.join("supervisor-placeholder.log");
                if let Some(parent) = sup_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match blackglass_core::mcp_supervisor::McpSupervisor::start_with_chain(
                    cfg,
                    &sup_path,
                    &data_dir.join("chain-placeholder.jsonl"),
                )
                .await
                {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        eprintln!("failed to start placeholder supervisor: {e}");
                        // Build a no-op supervisor by going through
                        // start_with_chain on a fresh tempdir. If
                        // even that fails, the operator server
                        // can't be wired and we should exit.
                        let tmp = std::env::temp_dir().join(format!(
                            "blackglass-supervisor-fallback-{}.log",
                            std::process::id()
                        ));
                        Arc::new(
                            blackglass_core::mcp_supervisor::McpSupervisor::start_with_chain(
                                blackglass_core::mcp_spawn_config::McpSpawnConfig::default(),
                                &tmp,
                                &tmp.with_extension("jsonl"),
                            )
                            .await
                            .expect("start fallback placeholder supervisor"),
                        )
                    }
                }
            };
            let placeholder_runtime_sock = PathBuf::from("/tmp/blackglass-not-yet-set.sock");
            tokio::spawn(async move {
                if let Err(e) = blackglass_core::operator_server::run(
                    &operator_sock,
                    op_broker,
                    op_channel,
                    op_chain,
                    placeholder_supervisor,
                    placeholder_runtime_sock,
                )
                .await
                {
                    eprintln!("operator socket error: {e}");
                }
            });
            let server = Server::bind(&socket, token, cp).await?;
            server.serve().await?;
        }
    }
    Ok(())
}

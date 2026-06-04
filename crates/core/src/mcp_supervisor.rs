//! Spawns the 4 new MCP servers (`mcp-ad`, `mcp-flipper`, `mcp-phish`,
//! `mcp-detect`) as child processes and supervises them.
//!
//! On child exit, the supervisor restarts with exponential backoff
//! (1s, 2s, 4s, 8s, 16s, then give up). The restart_count and
//! `McpServerExited` audit events are emitted through the audit chain.
//! When the supervisor gives up, the child status transitions to
//! `GivenUp` and subsequent `status(name)` calls keep returning it.
//!
//! The supervisor exposes a `status(name) -> Option<ChildStatus>`
//! method for the Tauri app to query liveness.

use crate::mcp_spawn_config::{McpServerSpec, McpSpawnConfig};
use blackglass_audit::{Chain, Event, EventKind};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, RwLock};
use tracing::{error, warn};

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit: {0}")]
    Audit(#[from] blackglass_audit::AuditError),
    #[error("server {0} not found in supervisor")]
    UnknownServer(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildStatus {
    Alive,
    Restarting { restart_count: u32 },
    GivenUp { restart_count: u32 },
}

#[derive(Debug)]
struct ChildHandle {
    // `spec` is retained for future "supervisor-side validation of the
    // child" and for diagnostic dumps; not read in the v1 monitor loop.
    #[allow(dead_code)]
    spec: McpServerSpec,
    child: Option<Child>,
    /// True iff the supervisor is in a clean-running state. Distinct
    /// from `restart_count` (which tracks how many restarts have happened
    /// up to and including the current spawn) so that a single
    /// `restart_count` field can be the single source of truth across
    /// Alive/Restarting/GivenUp transitions.
    phase: Phase,
    restart_count: u32,
    /// Path the monitor uses to re-spawn. Captured at first spawn so the
    /// re-spawn path doesn't need to re-derive it (and doesn't need the
    /// placeholder `/tmp/blackglass-supervisor.log` the plan had).
    log_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Alive,
    Restarting,
    GivenUp,
}

pub struct McpSupervisor {
    inner: Arc<RwLock<HashMap<String, ChildHandle>>>,
    // `chain` is held for future "operator-socket /audit" methods to
    // query the chain (the supervisor appends to it via the monitor
    // tasks' own Arc<Mutex<Chain>> clones). Not read in v1.
    #[allow(dead_code)]
    chain: Arc<Mutex<Chain>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
}

impl McpSupervisor {
    /// Start the supervisor. Spawns all child processes; returns once
    /// they're all spawned (or failed to spawn). The audit chain is
    /// opened at `<log_path parent>/chain.jsonl`.
    pub async fn start(config: McpSpawnConfig, log_path: &Path) -> Result<Self, SupervisorError> {
        // The plan's 2-arg constructor derives the chain from
        // `log_path.parent()`. That's fragile in the abstract but
        // workable for the test (and for the production startup path,
        // where `log_path` and `chain_path` live in the same state dir).
        // Keep it for parity with the plan.
        let chain_path = log_path.parent().unwrap().join("chain.jsonl");
        Self::start_with_chain(config, log_path, &chain_path).await
    }

    pub async fn start_with_chain(
        config: McpSpawnConfig,
        log_path: &Path,
        chain_path: &Path,
    ) -> Result<Self, SupervisorError> {
        let chain = Arc::new(Mutex::new(Chain::open(chain_path)?));
        let inner: Arc<RwLock<HashMap<String, ChildHandle>>> =
            Arc::new(RwLock::new(HashMap::new()));
        // broadcast::Receiver is cloneable, so each monitor task gets its
        // own subscription. mpsc::Receiver isn't cloneable in tokio, so
        // broadcast is the right primitive for "fan out a shutdown
        // signal to N tasks".
        let (shutdown_tx, _) = broadcast::channel::<()>(1);

        // Spawn each child + its monitor task.
        for spec in config.servers.iter() {
            let child = Self::spawn_child(spec, log_path).await?;
            let pid = child.id().unwrap_or(0);
            chain.lock().unwrap().append(Event {
                seq: 0, // Chain::append overwrites this from its internal counter.
                ts: chrono::Utc::now().to_rfc3339(),
                prev_hash: String::new(), // empty → Chain::append fills it from `self.last`
                kind: EventKind::McpServerSpawned {
                    server: spec.name.clone(),
                    pid,
                },
                payload: json!({}),
            })?;
            inner.write().await.insert(
                spec.name.clone(),
                ChildHandle {
                    spec: spec.clone(),
                    child: Some(child),
                    phase: Phase::Alive,
                    restart_count: 0,
                    log_path: log_path.to_path_buf(),
                },
            );

            // Spawn the monitor task for this child.
            let inner_for_task = inner.clone();
            let chain_for_task = chain.clone();
            let spec_for_task = spec.clone();
            let mut shutdown_rx_for_task = shutdown_tx.subscribe();
            tokio::spawn(async move {
                Self::monitor_child(
                    spec_for_task,
                    inner_for_task,
                    chain_for_task,
                    &mut shutdown_rx_for_task,
                )
                .await;
            });
        }

        Ok(Self {
            inner,
            chain,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    async fn spawn_child(spec: &McpServerSpec, log_path: &Path) -> Result<Child, SupervisorError> {
        let mut cmd = Command::new(&spec.command);
        cmd.args(&spec.args);
        // stdout/stderr go to a per-server log under log_path/<name>.log
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path.with_file_name(format!("{}.log", spec.name)))?;
        cmd.stdout(log_file.try_clone()?);
        cmd.stderr(log_file);
        cmd.kill_on_drop(true);
        Ok(cmd.spawn()?)
    }

    async fn monitor_child(
        spec: McpServerSpec,
        inner: Arc<RwLock<HashMap<String, ChildHandle>>>,
        chain: Arc<Mutex<Chain>>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        let backoffs = [1u64, 2, 4, 8, 16];
        let max_restarts = spec.max_restarts.min(backoffs.len() as u32);
        // Per-fix #4: the spec field is unused in v1.
        let _startup_timeout_ms = spec.startup_timeout_ms;
        // TODO(#mcp-v1.1): enforce startup_timeout_ms by racing spawn() against a tokio::time::sleep.
        loop {
            // Take the child out of the map, await its exit, put it back (or restart).
            let mut child = {
                let mut guard = inner.write().await;
                let handle = guard
                    .get_mut(&spec.name)
                    .expect("child disappeared");
                handle
                    .child
                    .take()
                    .expect("child was None")
            };
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    // Clean shutdown: kill the child, exit the monitor.
                    let _ = child.kill().await;
                    return;
                }
                status = child.wait() => {
                    let code = match status {
                        Ok(s) => s.code().unwrap_or(-1) as i32,
                        Err(_) => -1,
                    };
                    // Look up current restart_count (set at last spawn).
                    let current_count = {
                        let guard = inner.read().await;
                        guard.get(&spec.name).map(|h| h.restart_count).unwrap_or(0)
                    };
                    chain.lock().unwrap().append(Event {
                        seq: 0, // Chain::append overwrites this from its internal counter.
                        ts: chrono::Utc::now().to_rfc3339(),
                        prev_hash: String::new(), // empty → Chain::append fills it
                        kind: EventKind::McpServerExited {
                            server: spec.name.clone(),
                            code,
                            restart_count: current_count,
                        },
                        payload: json!({}),
                    }).ok();
                    if current_count >= max_restarts {
                        // Give up.
                        let mut guard = inner.write().await;
                        if let Some(h) = guard.get_mut(&spec.name) {
                            h.phase = Phase::GivenUp;
                        }
                        error!(server = %spec.name, "supervisor giving up after {} restarts", current_count);
                        return;
                    }
                    // Decide backoff: 1s, 2s, 4s, 8s, 16s. The restart
                    // we're about to do is the (current_count + 1)-th.
                    let backoff_idx = current_count as usize;
                    let backoff = backoffs[backoff_idx.min(backoffs.len() - 1)];
                    let new_count = current_count + 1;
                    warn!(server = %spec.name, "child exited (code {}), restarting in {}s (attempt {}/{})", code, backoff, new_count, max_restarts);
                    {
                        let mut guard = inner.write().await;
                        if let Some(h) = guard.get_mut(&spec.name) {
                            h.phase = Phase::Restarting;
                            h.restart_count = new_count;
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    // Re-spawn using the log_path captured at first spawn.
                    let log_path = {
                        let guard = inner.read().await;
                        guard.get(&spec.name).map(|h| h.log_path.clone()).unwrap_or_else(|| PathBuf::from("/tmp/blackglass-supervisor.log"))
                    };
                    match Self::spawn_child(&spec, &log_path).await {
                        Ok(new_child) => {
                            let pid = new_child.id().unwrap_or(0);
                            chain.lock().unwrap().append(Event {
                                seq: 0, // Chain::append overwrites this from its internal counter.
                                ts: chrono::Utc::now().to_rfc3339(),
                                prev_hash: String::new(),
                                kind: EventKind::McpServerSpawned {
                                    server: spec.name.clone(),
                                    pid,
                                },
                                payload: json!({}),
                            }).ok();
                            let mut guard = inner.write().await;
                            if let Some(h) = guard.get_mut(&spec.name) {
                                h.child = Some(new_child);
                                h.phase = Phase::Alive;
                                // restart_count is already set to new_count.
                            }
                        }
                        Err(e) => {
                            error!(server = %spec.name, "re-spawn failed: {}", e);
                            return;
                        }
                    }
                }
            }
        }
    }

    pub async fn status(&self, name: &str) -> Option<ChildStatus> {
        let guard = self.inner.read().await;
        guard.get(name).map(|h| match h.phase {
            Phase::Alive => ChildStatus::Alive,
            Phase::Restarting => ChildStatus::Restarting {
                restart_count: h.restart_count,
            },
            Phase::GivenUp => ChildStatus::GivenUp {
                restart_count: h.restart_count,
            },
        })
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            // broadcast::Sender::send is synchronous and returns the
            // number of receivers it reached. We don't care about
            // either the count or any error here — the test path is
            // a one-shot signal.
            let _ = tx.send(());
        }
    }
}

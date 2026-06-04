//! Python sidecar bridge.
//!
//! The chokepoint calls into this crate to dispatch tool calls that
//! need Python (scapy, impacket, hardware-bridge). Two implementations:
//!
//! - [`StubBridge`] (default): answers from in-process state. No Python
//!   interpreter is loaded. The dev and CI builds use this.
//! - [`RealBridge`]: pyo3-backed bridge that imports the
//!   `blackglass_sidecar` package and calls into it. Requires the
//!   `real` feature and a working Python venv with the sidecar
//!   installed. See `packaging/debian/postinst`.
//!
//! The chokepoint never sees the concrete type — it uses the
//! [`PythonBridge`] trait via an `Arc<dyn PythonBridge>` stored in the
//! chokepoint state. Construction is decided at startup based on the
//! `--python-bridge=stub|real` flag.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("python bridge disabled (stub build)")]
    Disabled,
    #[error("python bridge not implemented for tool {0}")]
    Unimplemented(String),
    #[error("python runtime error: {0}")]
    Runtime(String),
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    /// The Python module to call into, e.g. `blackglass_sidecar.scapy_bridge`.
    pub module: String,
    /// The function name on that module, e.g. `craft`.
    pub function: String,
    /// Arguments to the function. Must be JSON-serializable.
    pub args: Value,
    /// Optional evidence-dir override; if None, the bridge uses the
    /// env var `BLACKGLASS_EVIDENCE_DIR` or `/var/lib/blackglass/evidence`.
    pub evidence_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    /// Function return value (JSON-serializable).
    pub result: Value,
    /// Sidecar-stdout, captured for the audit log.
    pub stdout: String,
    /// Sidecar-stderr, captured for the audit log.
    pub stderr: String,
    /// Path to any evidence file the sidecar wrote, if any.
    pub evidence_path: Option<String>,
}

#[async_trait]
pub trait PythonBridge: Send + Sync {
    /// Whether the bridge can handle the given tool. Used to filter
    /// requests before they hit the runtime.
    fn handles(&self, tool: &str) -> bool;
    /// Dispatch a request.
    async fn invoke(&self, req: BridgeRequest) -> Result<BridgeResponse, BridgeError>;
}

// ---------------------------------------------------------------------------
// Stub implementation
// ---------------------------------------------------------------------------

/// In-process stub. The default. Returns a fixed-shape response for any
/// tool, so the chokepoint wiring can be exercised end-to-end without
/// a Python interpreter. The audit chain records the call as
/// `PythonBridgeInvoked` so the absence of a real sidecar is visible.
pub struct StubBridge;

impl StubBridge {
    pub fn new() -> Self { Self }
}

impl Default for StubBridge {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl PythonBridge for StubBridge {
    fn handles(&self, tool: &str) -> bool {
        // The stub claims to handle all Python-routed tools so the
        // chokepoint always gets a structured response. The audit log
        // distinguishes stub from real via the `bridge` field.
        matches!(tool,
            "scapy_craft" | "scapy_read" | "scapy_send" |
            "impacket_psexec" | "impacket_secretsdump" | "impacket_ntlmrelayx" |
            "flipper_run" | "flipper_read" | "flipper_write" |
            "detect_via_rest" | "detect_via_dns"
        )
    }

    async fn invoke(&self, req: BridgeRequest) -> Result<BridgeResponse, BridgeError> {
        info!(module = %req.module, function = %req.function, "stub bridge invoke");
        if !is_safe_module(&req.module) {
            return Err(BridgeError::InvalidArg(format!(
                "module {} is not in the allow-list", req.module
            )));
        }
        Ok(BridgeResponse {
            result: serde_json::json!({
                "stub": true,
                "module": req.module,
                "function": req.function,
                "echo_args": req.args,
            }),
            stdout: String::new(),
            stderr: format!("[stub] {}::{} called", req.module, req.function),
            evidence_path: None,
        })
    }
}

fn is_safe_module(m: &str) -> bool {
    // Only modules under the `blackglass_sidecar` namespace are allowed.
    // This is enforced at the bridge boundary so the chokepoint can't
    // be tricked into calling arbitrary Python.
    m == "blackglass_sidecar.scapy_bridge"
        || m == "blackglass_sidecar.impacket_bridge"
        || m == "blackglass_sidecar.hardware_bridge"
        || m == "blackglass_sidecar.detect_bridge"
        || m == "blackglass_sidecar.audit_types"
}

/// What kind of bridge the chokepoint should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BridgeKind {
    /// In-process stub. Default. No Python interpreter loaded.
    #[default]
    Stub,
    /// pyo3-backed bridge. Requires the `real` feature.
    Real,
}

impl BridgeKind {
    /// Parse from a string flag value. Unknown → Stub (safer default).
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "real" | "pyo3" | "sidecar" => BridgeKind::Real,
            _ => BridgeKind::Stub,
        }
    }
}

#[cfg(feature = "real")]
mod real {
    use super::*;
    use pyo3::prelude::*;

    /// pyo3-backed bridge. Loads the sidecar module once at startup
    /// and calls into it via `Python::with_gil`.
    pub struct RealBridge {
        /// Cached module references; populated lazily on first use.
        initialized: std::sync::atomic::AtomicBool,
    }

    impl RealBridge {
        pub fn new() -> Self {
            Self {
                initialized: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl PythonBridge for RealBridge {
        fn handles(&self, tool: &str) -> bool {
            // Same allow-list as the stub.
            super::StubBridge.handles(tool)
        }

        async fn invoke(&self, req: BridgeRequest) -> Result<BridgeResponse, BridgeError> {
            if !super::is_safe_module(&req.module) {
                return Err(BridgeError::InvalidArg(format!(
                    "module {} is not in the allow-list", req.module
                )));
            }
            // pyo3 calls are sync, so we run on a blocking task to
            // avoid stalling the async runtime.
            let req2 = req.clone();
            tokio::task::spawn_blocking(move || -> Result<BridgeResponse, BridgeError> {
                Python::with_gil(|py| {
                    let module = py.import(&req2.module)
                        .map_err(|e| BridgeError::Runtime(format!("import: {e}")))?;
                    let args_str = serde_json::to_string(&req2.args)
                        .map_err(|e| BridgeError::Runtime(format!("args: {e}")))?;
                    let kwargs = pyo3::types::IntoPyDict::into_py_dict([("args_json", args_str)], py);
                    let result = module.call_method(&req2.function, (kwargs,), None)
                        .map_err(|e| BridgeError::Runtime(format!("call: {e}")))?;
                    let result_json: String = result.extract()
                        .map_err(|e| BridgeError::Runtime(format!("extract: {e}")))?;
                    let parsed: Value = serde_json::from_str(&result_json)
                        .map_err(|e| BridgeError::Runtime(format!("parse: {e}")))?;
                    Ok(BridgeResponse {
                        result: parsed,
                        stdout: String::new(),
                        stderr: String::new(),
                        evidence_path: None,
                    })
                })
            })
            .await
            .map_err(|e| BridgeError::Runtime(format!("join: {e}")))?
        }
    }
}

#[cfg(feature = "real")]
pub use real::RealBridge;

/// Construct a bridge of the given kind. Returns `StubBridge` for
/// `Stub` regardless of feature; returns `RealBridge` for `Real` only
/// if the `real` feature is enabled.
pub fn build(kind: BridgeKind) -> Arc<dyn PythonBridge> {
    match kind {
        BridgeKind::Stub => Arc::new(StubBridge::new()),
        BridgeKind::Real => {
            #[cfg(feature = "real")]
            {
                Arc::new(RealBridge::new())
            }
            #[cfg(not(feature = "real"))]
            {
                warn!("`real` bridge requested but the `real` feature is not enabled; falling back to stub");
                Arc::new(StubBridge::new())
            }
        }
    }
}

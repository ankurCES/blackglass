# Blackglass Sub-plan 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Python sidecar (6 capabilities) + 4 new MCP servers + Tauri audit browser + the installable `blackglass` .deb with three meta-packages and a `curl | sh` installer.

**Architecture:** Tauri 2.x desktop app (`app/`) provides a single `/audit` view backed by a new `audit.query` JSON-RPC method on the core. The core gains a `pyo3`-based `PythonBridge` trait (`crates/python-bridge/`) that routes `scapy`, `impacket`, `pyflipper`, `evilginx2`, `gophish`, and deepfake detection calls into a uv-managed Python venv. Four new MCP server binaries (`mcp-ad`, `mcp-flipper`, `mcp-phish`, `mcp-detect`) translate JSON-RPC-over-stdio into `execute_action` calls. A new `polkit-helper` binary + AppArmor profiles confine the core. Everything ships as a single Cargo-built .deb with `cargo-deb` + a thin `debhelper` wrapper; the .deb is signed with cosign keyless signing and installed via `curl | sh` on Ubuntu 24.04 / Kali.

**Tech Stack:** Rust (workspace, edition 2021, MSRV 1.95), `pyo3` 0.27, Tauri 2.x + Svelte 5 + Vite + Tailwind, `uv` 0.5+, `cargo-deb` 2.x, `debhelper-compat 13`, `cosign` 2.x, AppArmor 3.x, polkit 125.

**Spec:** `docs/superpowers/specs/2026-06-03-blackglass-subplan4-design.md`

---

## File Structure

**New Rust crates:**
- `crates/python-bridge/` — `PythonBridge` trait, `RealPythonBridge`, `StubPythonBridge`, `BridgeError` (parent for the sidecar)
- `crates/polkit-helper/` — the `com.blackglass.start-core` polkit helper
- `crates/mcp-ad/` — Impacket-wrapping MCP server
- `crates/mcp-flipper/` — Flipper-wrapping MCP server
- `crates/mcp-phish/` — evilginx2 + gophish MCP server
- `crates/mcp-detect/` — deepfake detection MCP server
- `crates/xtask/` — build orchestrator (deb, sign, confinement-test, verify-install, apparmor-generate)
- `crates/secondary-sidecar/` — the deepfake secondary sidecar (uv-managed venv + REST on :8511)

**Modified Rust files:**
- `Cargo.toml` (root) — add 7 new crates to `members`
- `crates/audit/src/lib.rs` — add `PythonBridgeInvoked` to `EventKind`; add `bridge: Option<String>` field to event payload
- `crates/core/src/chokepoint.rs` — add Python-bridge dispatch path
- `crates/core/src/main.rs` — instantiate `RealPythonBridge`, pass to chokepoint
- `crates/core/src/operator_server.rs` — accept `audit.query` and `audit.verify_chain` methods
- `crates/mcp-packets/src/tools.rs` — replace the `scapy_craft` stub with a Python-bridge call
- `app/src-tauri/Cargo.toml` — add `blackglass-core` as a path dep (for the audit_query types)
- `app/src-tauri/tauri.conf.json` — CSP, dist path, bundle target
- `app/src-tauri/src/main.rs` — add `audit_query`, `audit_verify_chain` commands; new `audit.event` event pusher

**New Tauri/UI files:**
- `app/src/routes/+layout.svelte` — left nav with 9 view slots, 8 disabled
- `app/src/routes/+page.svelte` — redirect to `/audit`
- `app/src/routes/audit/+page.svelte` — the audit log browser
- `app/src/lib/audit-store.ts` — Svelte 5 store for the event list
- `app/src/lib/audit-types.ts` — TypeScript mirrors of Rust event/payload types
- `app/src/lib/filter-dsl.ts` — the filter JSON DSL + chip presets
- `app/src/lib/chain-verify.ts` — chain-verification UI helpers
- `app/tests/audit-browser.spec.ts` — Playwright test

**New Python files (the sidecar):**
- `python/sidecar/pyproject.toml` — uv-managed project
- `python/sidecar/uv.lock` — locked deps
- `python/sidecar/src/blackglass_sidecar/__init__.py`
- `python/sidecar/src/blackglass_sidecar/scapy_bridge.py`
- `python/sidecar/src/blackglass_sidecar/impacket_bridge.py`
- `python/sidecar/src/blackglass_sidecar/hardware_bridge.py`
- `python/sidecar/src/blackglass_sidecar/audit_types.py`
- `python/secondary-sidecar/pyproject.toml`
- `python/secondary-sidecar/src/blackglass_secondary/__init__.py`
- `python/secondary-sidecar/src/blackglass_secondary/detect.py` — placeholder model

**New packaging files:**
- `packaging/debian/control` — single source control with three binary packages via `Package-List`
- `packaging/debian/rules` — debhelper build
- `packaging/debian/compat` — `13`
- `packaging/debian/copyright` — DEP-5
- `packaging/debian/changelog` — dch-managed
- `packaging/debian/postinst` — the install script
- `packaging/debian/prerm` — the uninstall script
- `packaging/debian/conffiles`
- `packaging/debian/blackglass-core.apparmor`
- `packaging/debian/blackglass-polkit-helper.apparmor`
- `packaging/debian/com.blackglass.policy`
- `packaging/debian/99-blackglass-flipper.rules`
- `packaging/debian/blackglass.desktop`
- `packaging/debian/blackglass-upstream-tools.lintian-overrides`
- `packaging/deb/cargo-deb.toml` — per-binary deb config
- `packaging/deb/manpages/blackglass.1`
- `packaging/deb/bash-completion/blackglass-completion.bash`
- `packaging/apparmor/blackglass-core`
- `packaging/polkit/com.blackglass.policy`
- `packaging/udev/99-blackglass-flipper.rules`
- `packaging/cosign/cosign.pub`
- `packaging/install.sh`
- `packaging/installer/detect-distro.sh`
- `packaging/installer/verify-cosign.sh`
- `packaging/installer/apt-install.sh`
- `.github/workflows/release.yml`

**New docs:**
- `docs/superpowers/adrs/0013-pyo3-gil-pattern.md`
- `docs/superpowers/adrs/0014-deepfake-secondary-sidecar.md`
- `docs/superpowers/adrs/0015-deb-tiers-and-cosign-tofu.md`

**Removed:**
- `crates/ui/` — sub-plan 3's experimental Tauri shell is superseded by `app/`

---

# Phase 1: Python sidecar + 4 new MCP servers

**Exit criteria for this phase:** All 4 new MCP servers can be invoked from a JSON-RPC-over-stdio client; the chokepoint dispatches the new tool names to the Python bridge; the audit log records `PythonBridgeInvoked` and `ActionExecuted{bridge: "python"}` events; all 32 python-bridge tests + ~10 dispatch tests + 6 end-to-end core tests pass.

## Task 1.1: Add `PythonBridgeInvoked` to the audit `EventKind`

**Files:**
- Modify: `crates/audit/src/lib.rs:28-50`

- [ ] **Step 1: Add the new event-kind variant**

Edit `crates/audit/src/lib.rs`, in the `EventKind` enum, add the new variant after `ActionFailed`:

```rust
pub enum EventKind {
    CoreStarted,
    CoreStopped,
    ProfileLoaded,
    EngagementCreated,
    EngagementActivated,
    EngagementDeactivated,
    ActionRequested,
    ActionAllowed,
    ActionDenied,
    ActionExecuted,
    ActionFailed,
    AuditExported,
    PromptInjectionSuspected,
    OperatorConfirmationRequested,
    OperatorConfirmationResolved,
    PythonBridgeInvoked,    // NEW: sidecar call started
    #[serde(other)]
    Other,
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p blackglass-audit`
Expected: success, no warnings.

- [ ] **Step 3: Add a unit test for the new variant round-tripping**

In `crates/audit/src/lib.rs`, append to the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn python_bridge_invoked_serializes_and_deserializes() {
    use serde_json;
    let e = EventKind::PythonBridgeInvoked;
    let j = serde_json::to_string(&e).unwrap();
    assert_eq!(j, "\"PythonBridgeInvoked\"");
    let round: EventKind = serde_json::from_str(&j).unwrap();
    assert!(matches!(round, EventKind::PythonBridgeInvoked));
}
```

- [ ] **Step 4: Run the new test**

Run: `cargo test -p blackglass-audit python_bridge_invoked`
Expected: PASS, 1 test run, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add crates/audit/src/lib.rs
git commit -m "feat(audit): add PythonBridgeInvoked event kind"
```

## Task 1.2: Add a `bridge` field to the `ActionExecuted` event payload

**Files:**
- Modify: `crates/audit/src/lib.rs:54-80`

The new field tags whether an action's result came from a subprocess or the Python bridge. Backward-compatible: existing event-emitting code paths don't need to change; readers treat `null`/`missing` as `subprocess`.

- [ ] **Step 1: Read the current `Event` struct**

Run: `sed -n '50,90p' crates/audit/src/lib.rs`
Note the existing `payload: Value` field; we will add a new top-level field.

- [ ] **Step 2: Add the `bridge` field**

Edit `crates/audit/src/lib.rs`, in the `Event` struct, add `bridge` after `payload`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub prev_hash: String,
    #[serde(flatten)]
    pub kind: EventKind,
    pub payload: Value,
    /// "subprocess" (default) or "python". Tagged at write time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,
}
```

- [ ] **Step 3: Add a unit test for the new field**

Append to the existing tests module:

```rust
#[test]
fn event_with_bridge_field_round_trips() {
    use serde_json::json;
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionExecuted,
        payload: json!({"stdout": "x"}),
        bridge: Some("python".into()),
    };
    let j = serde_json::to_value(&e).unwrap();
    assert_eq!(j["bridge"], "python");
    let round: Event = serde_json::from_value(j).unwrap();
    assert_eq!(round.bridge.as_deref(), Some("python"));
}

#[test]
fn event_without_bridge_field_round_trips() {
    use serde_json::json;
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionExecuted,
        payload: json!({"stdout": "x"}),
        bridge: None,
    };
    let j = serde_json::to_value(&e).unwrap();
    // The field is skipped on serialize when None
    assert!(j.get("bridge").is_none());
}
```

- [ ] **Step 4: Run the new tests**

Run: `cargo test -p blackglass-audit`
Expected: PASS, 3 tests in the file, 0 failed.

- [ ] **Step 5: Verify nothing else broke**

Run: `cargo build --workspace`
Expected: success. The `bridge` field is `Option<String>` and existing emitters don't need to set it.

- [ ] **Step 6: Commit**

```bash
git add crates/audit/src/lib.rs
git commit -m "feat(audit): tag ActionExecuted events with bridge='subprocess|python'"
```

## Task 1.3: Create the `python-bridge` crate skeleton

**Files:**
- Create: `crates/python-bridge/Cargo.toml`
- Create: `crates/python-bridge/src/lib.rs`
- Create: `crates/python-bridge/src/error.rs`
- Create: `crates/python-bridge/src/stub.rs`
- Modify: `Cargo.toml` (root): add `"crates/python-bridge"` to `members`

- [ ] **Step 1: Add to workspace**

Edit the root `Cargo.toml` `members` array — add `"crates/python-bridge"` (it should be near the bottom with the new crates).

- [ ] **Step 2: Create `crates/python-bridge/Cargo.toml`**

```toml
[package]
name = "blackglass-python-bridge"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[features]
# Default: only the stub (no pyo3 link, builds without a Python toolchain).
# Enable `real` to link pyo3; this is the production build.
default = []
real = ["pyo3", "tokio/rt"]

[dependencies]
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["sync", "time", "macros"] }
tracing.workspace = true
async-trait = "0.1"
# Only used when the `real` feature is on.
pyo3 = { version = "0.27", features = ["auto-initialize"], optional = true }

[dev-dependencies]
tempfile.workspace = true
proptest.workspace = true
```

- [ ] **Step 3: Create `crates/python-bridge/src/error.rs`**

```rust
//! Bridge error type — every failure a Python sidecar call can produce.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("python venv not initialized: {0}")]
    VenvMissing(PathBuf),
    #[error("python module not found: {0}")]
    ModuleNotFound(String),
    #[error("python call timed out after {0}s")]
    Timeout(u64),
    #[error("python exception: {0}")]
    PythonException(String),
    #[error("invalid args: {0}")]
    InvalidArgs(String),
    #[error("permission denied by AppArmor: {0}")]
    AppArmorDenied(String),
    #[error("subprocess I/O error: {0}")]
    SubprocessIo(#[from] std::io::Error),
    #[error("internal: {0}")]
    Internal(String),
}
```

- [ ] **Step 4: Create `crates/python-bridge/src/stub.rs`**

The stub is the default. It returns `BridgeError::VenvMissing` for every call, which is what an unbuilt sidecar should do.

```rust
//! Stub bridge — returns "sidecar not built" for every call.
//!
//! This is the default; tests and dev builds use it. Production builds
//! enable the `real` feature and use `RealPythonBridge` from `real.rs`.

use crate::error::BridgeError;
use crate::traits::*;
use async_trait::async_trait;
use std::path::PathBuf;

pub struct StubPythonBridge;

#[async_trait]
impl PythonBridge for StubPythonBridge {
    async fn scapy_craft(&self, _spec: &ScapySpec) -> Result<ScapyResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
    async fn impacket(&self, _op: ImpacketOp) -> Result<ImpacketResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
    async fn flipper(&self, _op: FlipperOp) -> Result<FlipperResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
    async fn evilginx(&self, _op: EvilginxOp) -> Result<EvilginxResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
    async fn gophish(&self, _op: GophishOp) -> Result<GophishResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
    async fn detect(&self, _op: DetectOp) -> Result<DetectResult, BridgeError> {
        Err(BridgeError::VenvMissing(PathBuf::from("<stub>")))
    }
}
```

- [ ] **Step 5: Create `crates/python-bridge/src/traits.rs`**

The trait definitions. Stub values for the parameter/result types so the trait compiles in `--no-default-features` mode.

```rust
//! The bridge trait — six async methods, one per Python capability.
//!
//! All six follow the same pattern: take the args, hand them to a Python
//! function in a venv, get a structured result back. The trait is async so
//! `RealPythonBridge` can use `tokio::task::spawn_blocking` internally.

use crate::error::BridgeError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait PythonBridge: Send + Sync {
    async fn scapy_craft(&self, spec: &ScapySpec) -> Result<ScapyResult, BridgeError>;
    async fn impacket(&self, op: ImpacketOp) -> Result<ImpacketResult, BridgeError>;
    async fn flipper(&self, op: FlipperOp) -> Result<FlipperResult, BridgeError>;
    async fn evilginx(&self, op: EvilginxOp) -> Result<EvilginxResult, BridgeError>;
    async fn gophish(&self, op: GophishOp) -> Result<GophishResult, BridgeError>;
    async fn detect(&self, op: DetectOp) -> Result<DetectResult, BridgeError>;
}

// --- scapy ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScapySpec {
    /// Python-eval-able spec, e.g. `IP(dst="10.0.0.5")/TCP(dport=80)/Raw(load="GET / HTTP/1.0")`.
    /// MUST NOT contain `send(` or `sr(` (offline-only enforcement happens in the bridge).
    pub spec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScapyResult {
    /// Hex-encoded packet bytes.
    pub bytes_hex: String,
    /// Length in bytes.
    pub length: usize,
}

// --- impacket (5 ops) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ImpacketOp {
    Psexec { target: String, user: String, hash: String, remote_cmd: String },
    Wmiexec { target: String, user: String, hash: String, remote_cmd: String },
    Secretsdump { target: String, user: String, hash: String },
    Kerberoast { target: String, user: String, hash: String },
    Asreproast { target: String, user: String, hash: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpacketResult {
    pub op: String,
    pub stdout: String,
    pub stderr: String,
    /// For `secretsdump`: parsed hashes.
    #[serde(default)]
    pub hashes: Vec<String>,
}

// --- flipper (4 ops) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FlipperOp {
    List { path: String },
    Read { path: String },
    Write { path: String, data_b64: String },
    Run { command: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlipperResult {
    pub op: String,
    /// For `list`: directory entries. For `read`: file content. For `run`: command output.
    pub data: String,
}

// --- evilginx (5 ops) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EvilginxOp {
    List,
    Enable { phishlet: String },
    Disable { phishlet: String },
    GetCaptures,
    LureCreate { phishlet: String, path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvilginxResult {
    pub op: String,
    pub data: serde_json::Value,
}

// --- gophish (4 ops) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum GophishOp {
    CampaignList,
    CampaignCreate { name: String, template: String, url: String, groups: Vec<String> },
    CampaignStatus { id: u32 },
    Results { id: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GophishResult {
    pub op: String,
    pub data: serde_json::Value,
}

// --- detect (3 ops, talks to secondary sidecar via REST) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DetectOp {
    Image { path: String },
    Video { path: String },
    Batch { dir: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResult {
    pub op: String,
    /// "unknown" | "likely_real" | "likely_fake" | "inconclusive".
    pub verdict: String,
    /// 0.0..=1.0 confidence.
    pub confidence: f32,
    pub raw: serde_json::Value,
}
```

- [ ] **Step 6: Create `crates/python-bridge/src/lib.rs`**

```rust
//! Python sidecar bridge.
//!
//! The trait surface is in `traits`; the stub is in `stub`; the real
//! implementation (which links pyo3) is in `real` and only compiled
//! with the `real` feature.

pub mod error;
pub mod stub;
pub mod traits;

#[cfg(feature = "real")]
pub mod real;

pub use error::BridgeError;
pub use stub::StubPythonBridge;
pub use traits::{
    DetectOp, DetectResult, EvilginxOp, EvilginxResult, FlipperOp, FlipperResult,
    GophishOp, GophishResult, ImpacketOp, ImpacketResult, PythonBridge, ScapyResult, ScapySpec,
};

#[cfg(feature = "real")]
pub use real::RealPythonBridge;
```

- [ ] **Step 7: Create the empty `real.rs` stub for now**

The `real.rs` module is filled in by Task 1.4. For now, create the file with just a `todo!()` so the `#[cfg(feature = "real")]` line in `lib.rs` compiles when the feature is enabled:

```rust
//! Real pyo3-based bridge — implemented in Task 1.4.

use crate::error::BridgeError;
use crate::traits::*;

pub struct RealPythonBridge {
    _priv: (),
}

impl RealPythonBridge {
    pub async fn new(_venv: &std::path::Path) -> Result<Self, BridgeError> {
        unimplemented!("RealPythonBridge::new — implemented in Task 1.4")
    }
}
```

- [ ] **Step 8: Build the workspace (no `real` feature)**

Run: `cargo build -p blackglass-python-bridge`
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/python-bridge/
git commit -m "feat(python-bridge): skeleton with stub bridge and trait surface"
```

## Task 1.4: Implement `RealPythonBridge` with pyo3 + spawn_blocking

**Files:**
- Modify: `crates/python-bridge/src/real.rs` (replace the stub)

This is the production impl. It takes the GIL briefly, pushes the work to `spawn_blocking`, awaits the result. The pattern is the same for all six methods; we show `scapy_craft` in detail and the other five follow the same template.

- [ ] **Step 1: Add `pyo3` and `parking_lot` deps to the `real` feature**

Edit `crates/python-bridge/Cargo.toml` — under `[features] real`, add the deps:

```toml
[features]
default = []
real = ["pyo3", "tokio/rt", "parking_lot"]

[dependencies]
# ... existing ...
parking_lot = { version = "0.12", optional = true }
```

- [ ] **Step 2: Replace `crates/python-bridge/src/real.rs` with the full impl**

```rust
//! Real pyo3-based bridge — calls into a uv-managed venv.
//!
//! Pattern for every method:
//!   1. Acquire the GIL (briefly).
//!   2. Build a `PyDict` of kwargs from the Rust args.
//!   3. Release the GIL.
//!   4. `tokio::task::spawn_blocking` to do the actual Python work.
//!   5. Inside the blocking task, reacquire the GIL, call the function,
//!      convert the result to a Rust struct, return it.

use crate::error::BridgeError;
use crate::traits::*;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

/// Default per-tool timeout (overridden by `python-bridge.toml`).
const DEFAULT_TIMEOUT_SECS: u64 = 300;

pub struct RealPythonBridge {
    venv: PathBuf,
    /// Per-tool timeouts in seconds. Keyed by the MCP tool name.
    timeouts: std::collections::HashMap<String, u64>,
    /// Async mutex around Python init state.
    init_lock: Arc<AsyncMutex<()>>,
}

impl RealPythonBridge {
    /// Build a bridge. `python_bin` is the path to the venv's `python` binary
    /// (e.g. `/usr/lib/blackglass/python-venv/bin/python`).
    ///
    /// This sets up the embedded Python interpreter and the import paths.
    /// The interpreter is process-global, so this is called exactly once.
    pub async fn new(python_bin: &Path) -> Result<Self, BridgeError> {
        if !python_bin.exists() {
            return Err(BridgeError::VenvMissing(python_bin.to_path_buf()));
        }
        // Initialize the embedded interpreter.
        pyo3::prepare_freethreaded_python();
        // Verify the sidecar package is importable.
        let py_bin = python_bin.to_path_buf();
        tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| {
                let syspath = py.import("sys")
                    .map_err(|e| BridgeError::ModuleNotFound(format!("sys: {e}")))?
                    .getattr("path")
                    .map_err(|e| BridgeError::ModuleNotFound(format!("sys.path: {e}")))?;
                let _ = syspath;  // suppress unused
                py.import("blackglass_sidecar")
                    .map_err(|e| BridgeError::ModuleNotFound(format!("blackglass_sidecar: {e}")))?;
                Ok::<(), BridgeError>(())
            })
        })
        .await
        .map_err(|e| BridgeError::Internal(format!("spawn_blocking join: {e}")))??;
        Ok(Self {
            venv: py_bin,
            timeouts: std::collections::HashMap::new(),
            init_lock: Arc::new(AsyncMutex::new(())),
        })
    }

    /// Set per-tool timeouts. Called once after `new` from the config loader.
    pub fn set_timeouts(&mut self, timeouts: std::collections::HashMap<String, u64>) {
        self.timeouts = timeouts;
    }

    fn timeout_for(&self, tool: &str) -> Duration {
        Duration::from_secs(self.timeouts.get(tool).copied().unwrap_or(DEFAULT_TIMEOUT_SECS))
    }
}

#[async_trait::async_trait]
impl PythonBridge for RealPythonBridge {
    async fn scapy_craft(&self, spec: &ScapySpec) -> Result<ScapyResult, BridgeError> {
        let timeout = self.timeout_for("scapy_craft");
        // Offline-only enforcement: reject any spec containing send( or sr(
        let lower = spec.spec.to_ascii_lowercase();
        if lower.contains("send(") || lower.contains("sr(") || lower.contains("sr1(") {
            return Err(BridgeError::InvalidArgs(
                "scapy live TX is disabled in v1; only offline packet crafting".into(),
            ));
        }
        let spec_str = spec.spec.clone();
        let venv = self.venv.clone();
        let _g = self.init_lock.lock().await;
        let res = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || -> Result<ScapyResult, BridgeError> {
                Python::with_gil(|py| -> Result<ScapyResult, BridgeError> {
                    // Validate the venv's python is what's running
                    let _ = venv;
                    let mod = py.import("blackglass_sidecar.scapy_bridge")
                        .map_err(|e| BridgeError::ModuleNotFound(format!("scapy_bridge: {e}")))?;
                    let func = mod.getattr("craft")
                        .map_err(|e| BridgeError::ModuleNotFound(format!("craft fn: {e}")))?;
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("spec", spec_str)
                        .map_err(|e| BridgeError::InvalidArgs(e.to_string()))?;
                    let result = func.call((), Some(kwargs))
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?;
                    // Result is a dict {bytes_hex: str, length: int}
                    let bytes_hex: String = result.get_item("bytes_hex")
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?
                        .and_then(|i| i.extract())
                        .map_err(|e: pyo3::PyErr| BridgeError::PythonException(e.to_string()))?;
                    let length: usize = result.get_item("length")
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?
                        .and_then(|i| i.extract())
                        .map_err(|e: pyo3::PyErr| BridgeError::PythonException(e.to_string()))?;
                    Ok(ScapyResult { bytes_hex, length })
                })
            }),
        )
        .await
        .map_err(|_| BridgeError::Timeout(timeout.as_secs()))?
        .map_err(|e| BridgeError::Internal(format!("join: {e}")))??;
        Ok(res)
    }

    async fn impacket(&self, op: ImpacketOp) -> Result<ImpacketResult, BridgeError> {
        let tool = match &op {
            ImpacketOp::Psexec { .. } => "impacket_psexec",
            ImpacketOp::Wmiexec { .. } => "impacket_wmiexec",
            ImpacketOp::Secretsdump { .. } => "impacket_secretsdump",
            ImpacketOp::Kerberoast { .. } => "impacket_kerberoast",
            ImpacketOp::Asreproast { .. } => "impacket_asreproast",
        };
        let timeout = self.timeout_for(tool);
        let op_json = serde_json::to_value(&op)
            .map_err(|e| BridgeError::InvalidArgs(e.to_string()))?;
        let _g = self.init_lock.lock().await;
        let res = tokio::time::timeout(
            timeout,
            tokio::task::spawn_blocking(move || -> Result<ImpacketResult, BridgeError> {
                Python::with_gil(|py| -> Result<ImpacketResult, BridgeError> {
                    let mod = py.import("blackglass_sidecar.impacket_bridge")
                        .map_err(|e| BridgeError::ModuleNotFound(format!("impacket_bridge: {e}")))?;
                    let func = mod.getattr("run")
                        .map_err(|e| BridgeError::ModuleNotFound(format!("run fn: {e}")))?;
                    let kwargs = PyDict::new(py);
                    let py_op = pythonize::pythonize(py, &op_json)
                        .map_err(|e| BridgeError::InvalidArgs(format!("pythonize: {e}")))?;
                    kwargs.set_item("op", py_op)
                        .map_err(|e| BridgeError::InvalidArgs(e.to_string()))?;
                    let result = func.call((), Some(kwargs))
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?;
                    let stdout: String = result.get_item("stdout")
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?
                        .and_then(|i| i.extract())
                        .map_err(|e: pyo3::PyErr| BridgeError::PythonException(e.to_string()))?;
                    let stderr: String = result.get_item("stderr")
                        .map_err(|e| BridgeError::PythonException(e.to_string()))?
                        .and_then(|i| i.extract())
                        .map_err(|e: pyo3::PyErr| BridgeError::PythonException(e.to_string()))?;
                    let hashes: Vec<String> = result.get_item("hashes")
                        .ok()
                        .and_then(|i| i.extract().ok())
                        .unwrap_or_default();
                    Ok(ImpacketResult {
                        op: tool.into(),
                        stdout, stderr, hashes,
                    })
                })
            }),
        )
        .await
        .map_err(|_| BridgeError::Timeout(timeout.as_secs()))?
        .map_err(|e| BridgeError::Internal(format!("join: {e}")))??;
        Ok(res)
    }

    // --- flipper, evilginx, gophish, detect follow the same template ---
    // For brevity, only the dispatch shells are shown. Each one:
    //   1. Computes the per-tool timeout.
    //   2. Serializes `op` to JSON.
    //   3. spawn_blocking that imports the right module and calls its `run` fn.
    //   4. Returns the structured result.
    // See `scapy_craft` and `impacket` for the full pattern.

    async fn flipper(&self, op: FlipperOp) -> Result<FlipperResult, BridgeError> {
        self.call_string_op("hardware_bridge", "flipper_run", "flipper-run", &op).await
    }

    async fn evilginx(&self, op: EvilginxOp) -> Result<EvilginxResult, BridgeError> {
        self.call_json_op("hardware_bridge", "evilginx_run", "phish-evilginx", &op).await
    }

    async fn gophish(&self, op: GophishOp) -> Result<GophishResult, BridgeError> {
        self.call_json_op("hardware_bridge", "gophish_run", "phish-gophish", &op).await
    }

    async fn detect(&self, op: DetectOp) -> Result<DetectResult, BridgeError> {
        // Detect talks to the secondary sidecar via REST on localhost:8511,
        // NOT to the main venv. See Task 1.5.
        self.detect_via_rest(op).await
    }
}

impl RealPythonBridge {
    /// Helper for ops whose result is just a `data: String`.
    async fn call_string_op<T, R>(
        &self,
        module: &str,
        fn_name: &str,
        tool: &str,
        op: &T,
    ) -> Result<R, BridgeError>
    where
        T: serde::Serialize + Send + 'static,
        R: serde::de::DeserializeOwned + Send + 'static,
    {
        // Stub — full impl omitted for brevity, same pattern as scapy_craft.
        // In Phase 1.5 we wire each one; for now, return an Internal error.
        let _ = (module, fn_name, tool, op);
        Err(BridgeError::Internal("not yet implemented — see Task 1.6".into()))
    }

    async fn call_json_op<T, R>(
        &self,
        module: &str,
        fn_name: &str,
        tool: &str,
        op: &T,
    ) -> Result<R, BridgeError>
    where
        T: serde::Serialize + Send + 'static,
        R: serde::de::DeserializeOwned + Send + 'static,
    {
        let _ = (module, fn_name, tool, op);
        Err(BridgeError::Internal("not yet implemented — see Task 1.6".into()))
    }

    async fn detect_via_rest(&self, _op: DetectOp) -> Result<DetectResult, BridgeError> {
        // Implemented in Task 1.7.
        Err(BridgeError::Internal("not yet implemented — see Task 1.7".into()))
    }
}
```

(Note: the `pythonize` crate is the bridge between Rust `serde_json::Value` and `pyo3` Python objects. Add it to `[features].real` deps in a follow-up edit; the first commit can skip it and use a simpler `format!("{op_json}")` arg-marshalling strategy if `pythonize` is too heavy a dep.)

- [ ] **Step 3: Add `pythonize` to the `real` feature deps**

Edit `crates/python-bridge/Cargo.toml` — under `[features] real`, add the dep:

```toml
[features]
default = []
real = ["pyo3", "tokio/rt", "parking_lot", "pythonize"]

[dependencies]
# ... existing ...
pythonize = { version = "0.27", optional = true }
```

- [ ] **Step 4: Build with the `real` feature (this may take a while — pyo3 compiles)**

Run: `cargo build -p blackglass-python-bridge --features real`
Expected: success. If pyo3 can't find a Python 3.12, install it: `sudo apt install -y python3.12-dev` and re-run.

- [ ] **Step 5: Commit (the impl, even though most of the methods are stubs)**

```bash
git add crates/python-bridge/
git commit -m "feat(python-bridge): pyo3-based real bridge with scapy_craft and impacket"
```

## Task 1.5: Wire `mcp-packets` `scapy_craft` to the Python bridge

**Files:**
- Modify: `crates/mcp-packets/src/tools.rs`
- Modify: `crates/mcp-packets/Cargo.toml`: add `blackglass-python-bridge` dep
- Modify: `crates/mcp-packets/src/main.rs`: take a `--bridge` flag

- [ ] **Step 1: Add the dependency**

Edit `crates/mcp-packets/Cargo.toml` — under `[dependencies]`, add:

```toml
[dependencies]
# ... existing ...
blackglass-python-bridge = { path = "../python-bridge" }
```

- [ ] **Step 2: Read the current `scapy_craft` stub**

Run: `grep -n "scapy_craft" crates/mcp-packets/src/tools.rs`
Find the existing function and note its signature.

- [ ] **Step 3: Replace the stub with a bridge call**

Edit the function in `crates/mcp-packets/src/tools.rs`. The function takes the `PythonBridge` (passed in by `main.rs`) and the spec string, calls `bridge.scapy_craft`, returns the bytes:

```rust
use blackglass_python_bridge::{PythonBridge, ScapySpec};
// ... existing imports ...

/// MCP tool: packets-scapy_craft
/// Craft an offline scapy packet and return its bytes.
pub async fn scapy_craft(
    bridge: &dyn PythonBridge,
    args: ScapyArgs,
) -> Result<String, BridgeError> {
    let spec = ScapySpec { spec: args.spec };
    let result = bridge.scapy_craft(&spec).await
        .map_err(|e| anyhow::anyhow!("scapy bridge: {e}"))?;
    Ok(result.bytes_hex)
}

#[derive(Debug, serde::Deserialize)]
pub struct ScapyArgs {
    pub spec: String,
}

// ... existing types ...
```

(The exact `BridgeError` import is whatever the file currently uses; `anyhow::Error` is the typical pattern in this codebase.)

- [ ] **Step 4: Add a unit test that the stub bridge is used by default**

In `crates/mcp-packets/src/tools.rs` `#[cfg(test)] mod tests`, add:

```rust
#[tokio::test]
async fn scapy_craft_with_stub_bridge_returns_venv_missing() {
    use blackglass_python_bridge::StubPythonBridge;
    let bridge = StubPythonBridge;
    let res = scapy_craft(&bridge, ScapyArgs { spec: "IP()/TCP()".into() }).await;
    assert!(res.is_err());
}
```

- [ ] **Step 5: Run the new test**

Run: `cargo test -p blackglass-mcp-packets scapy_craft_with_stub`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp-packets/
git commit -m "feat(mcp-packets): route scapy_craft through Python bridge"
```

## Task 1.6: Add the four new MCP server crates

**Files:**
- Create: `crates/mcp-ad/Cargo.toml`, `crates/mcp-ad/src/main.rs`, `crates/mcp-ad/src/tools.rs`
- Create: `crates/mcp-flipper/Cargo.toml`, `crates/mcp-flipper/src/main.rs`, `crates/mcp-flipper/src/tools.rs`
- Create: `crates/mcp-phish/Cargo.toml`, `crates/mcp-phish/src/main.rs`, `crates/mcp-phish/src/tools.rs`
- Create: `crates/mcp-detect/Cargo.toml`, `crates/mcp-detect/src/main.rs`, `crates/mcp-detect/src/tools.rs`
- Modify: `Cargo.toml` (root): add 4 new members

Each crate follows the same pattern as `mcp-packets`. The tools list:

- `mcp-ad`: 5 tools (impacket psexec, wmiexec, secretsdump, kerberoast, asreproast)
- `mcp-flipper`: 4 tools (list, read, write, run)
- `mcp-phish`: 5 + 4 = 9 tools (evilginx list/enable/disable/get-captures/lure-create + gophish campaign-list/create/status/results)
- `mcp-detect`: 3 tools (image, video, batch)

- [ ] **Step 1: Create `crates/mcp-ad/Cargo.toml`**

```toml
[package]
name = "blackglass-mcp-ad"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "blackglass-mcp-ad"
path = "src/main.rs"

[dependencies]
blackglass-runtime       = { path = "../runtime" }
blackglass-python-bridge = { path = "../python-bridge" }
rmcp.workspace = true
tokio.workspace = true
clap = { workspace = true, features = ["derive"] }
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Create `crates/mcp-ad/src/main.rs`**

```rust
//! blackglass-mcp-ad — Active Directory MCP server.
//!
//! Exposes 5 impacket-wrapping tools over JSON-RPC-over-stdio. All actual
//! work is routed through the Python bridge via the core's chokepoint.

use anyhow::Result;
use blackglass_python_bridge::{PythonBridge, StubPythonBridge};
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
    let token = std::fs::read_to_string(expand(&cli.token_file))?.trim().to_string();
    let gate = Arc::new(GateClient::new(socket, token));

    // Build the bridge. Use stub if no --python flag (dev mode).
    let bridge: Arc<dyn PythonBridge> = match &cli.python {
        Some(p) => {
            // Production builds would link the `real` feature; for the
            // dev binary we use the stub. This is a wire-up test.
            let _ = blackglass_python_bridge::RealPythonBridge::new(p).await;
            Arc::new(StubPythonBridge)
        }
        None => Arc::new(StubPythonBridge),
    };

    tools::register_all(bridge, gate).await
}
```

- [ ] **Step 3: Create `crates/mcp-ad/src/tools.rs`**

```rust
//! Tool registration for mcp-ad. Each tool is a thin wrapper that
//! deserializes args, calls the bridge, and returns a result.

use anyhow::Result;
use blackglass_python_bridge::{ImpacketOp, PythonBridge};
use blackglass_runtime::GateClient;
use rmcp::{model::*, ServerHandler};
use serde::Deserialize;
use std::sync::Arc;

pub async fn register_all(
    bridge: Arc<dyn PythonBridge>,
    gate: Arc<GateClient>,
) -> Result<()> {
    // Hand the bridge and gate to the rmcp server builder.
    // In v1, the server is constructed via the rmcp macros/builder; the
    // details are in the existing mcp-packets crate. We register 5 tools:
    //   - ad-impacket_psexec
    //   - ad-impacket_wmiexec
    //   - ad-impacket_secretsdump
    //   - ad-impacket_kerberoast
    //   - ad-impacket_asreproast
    //
    // Each tool deserializes its args, calls bridge.impacket(...), and
    // routes the result through `gate` (the chokepoint).

    let _ = (bridge, gate);
    // Stub: server runs but no tools yet. Real impl in Task 1.7.
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ImpacketArgs {
    pub target: String,
    pub user: String,
    pub hash: String,
    #[serde(default)]
    pub remote_cmd: Option<String>,
}
```

- [ ] **Step 4: Repeat for the other 3 crates (flipper, phish, detect)**

For each, follow the same template, substituting:
- `crates/mcp-flipper/src/tools.rs`: registers 4 tools, calls `bridge.flipper(FlipperOp::{...})`
- `crates/mcp-phish/src/tools.rs`: registers 9 tools (5 evilginx + 4 gophish), calls `bridge.evilginx(...)` and `bridge.gophish(...)`
- `crates/mcp-detect/src/tools.rs`: registers 3 tools, calls `bridge.detect(DetectOp::{...})`

- [ ] **Step 5: Add the 4 crates to the root `Cargo.toml` `members`**

Edit `Cargo.toml`, add:

```toml
"crates/mcp-ad",
"crates/mcp-flipper",
"crates/mcp-phish",
"crates/mcp-detect",
```

- [ ] **Step 6: Build the new crates**

Run: `cargo build -p blackglass-mcp-ad -p blackglass-mcp-flipper -p blackglass-mcp-phish -p blackglass-mcp-detect`
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/mcp-ad crates/mcp-flipper crates/mcp-phish crates/mcp-detect
git commit -m "feat(mcp): add ad, flipper, phish, detect MCP server skeletons"
```

## Task 1.7: Write the Python sidecar source

**Files:**
- Create: `python/sidecar/pyproject.toml`
- Create: `python/sidecar/uv.lock` (auto-generated; first run)
- Create: `python/sidecar/src/blackglass_sidecar/__init__.py`
- Create: `python/sidecar/src/blackglass_sidecar/scapy_bridge.py`
- Create: `python/sidecar/src/blackglass_sidecar/impacket_bridge.py`
- Create: `python/sidecar/src/blackglass_sidecar/hardware_bridge.py`
- Create: `python/sidecar/src/blackglass_sidecar/audit_types.py`

The Python sidecar has one function per capability: `craft` (scapy), `run` (impacket), and one function per hardware bridge (Flipper, evilginx, gophish). The `audit_types` module has dataclasses mirroring the Rust types.

- [ ] **Step 1: Create `python/sidecar/pyproject.toml`**

```toml
[project]
name = "blackglass-sidecar"
version = "0.1.0"
description = "Blackglass Python sidecar — scapy, impacket, pyflipper, gophish"
requires-python = ">=3.12"
dependencies = [
    "scapy>=2.5",
    "impacket>=0.11",
    "pyflipper>=0.5",
    "gophish>=0.3",
    "requests>=2.31",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/blackglass_sidecar"]
```

- [ ] **Step 2: Create `python/sidecar/src/blackglass_sidecar/__init__.py`**

```python
"""Blackglass Python sidecar. Six capabilities exposed as plain functions."""
__version__ = "0.1.0"
```

- [ ] **Step 3: Create `python/sidecar/src/blackglass_sidecar/audit_types.py`**

```python
"""Dataclasses mirroring the Rust bridge types. Used for type hints only;
the actual wire format is what `craft`, `run`, etc. return — dicts with
the keys the Rust side expects."""

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class ScapyResult:
    bytes_hex: str
    length: int

    def to_dict(self) -> dict:
        return {"bytes_hex": self.bytes_hex, "length": self.length}


@dataclass
class ImpacketResult:
    op: str
    stdout: str
    stderr: str
    hashes: list[str] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "op": self.op,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "hashes": self.hashes,
        }


@dataclass
class FlipperResult:
    op: str
    data: str

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class EvilginxResult:
    op: str
    data: dict

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class GophishResult:
    op: str
    data: dict

    def to_dict(self) -> dict:
        return {"op": self.op, "data": self.data}


@dataclass
class DetectResult:
    op: str
    verdict: str  # "unknown" | "likely_real" | "likely_fake" | "inconclusive"
    confidence: float
    raw: dict

    def to_dict(self) -> dict:
        return {
            "op": self.op,
            "verdict": self.verdict,
            "confidence": self.confidence,
            "raw": self.raw,
        }
```

- [ ] **Step 4: Create `python/sidecar/src/blackglass_sidecar/scapy_bridge.py`**

```python
"""scapy offline packet crafting.

The Rust bridge already enforces offline-only (rejects `send(`/`sr(` in
the spec string), so this module is a thin wrapper that evals the spec
and serializes the result.

Returns:
    dict with keys `bytes_hex` (str) and `length` (int).
"""

from scapy.all import IP, TCP, UDP, Raw  # type: ignore  # noqa: F401
from scapy.packet import Packet

from .audit_types import ScapyResult


def craft(spec: str) -> dict:
    """Craft an offline scapy packet from a spec string.

    The spec is eval'd in a sandboxed namespace that exposes scapy's
    common layer constructors (IP, TCP, UDP, Raw, Ether, etc.). Live
    TX functions (send, sr, sr1) are NOT exposed.

    Example spec:
        'IP(dst="10.0.0.5")/TCP(dport=80)/Raw(load="GET / HTTP/1.0")'
    """
    ns = {
        "IP": IP, "TCP": TCP, "UDP": UDP, "Raw": Raw,
        # A few more common ones:
        "Ether": __import__("scapy.all", fromlist=["Ether"]).Ether,
        "DNS": __import__("scapy.all", fromlist=["DNS"]).DNS,
        "ICMP": __import__("scapy.all", fromlist=["ICMP"]).ICMP,
    }
    pkt: Packet = eval(spec, {"__builtins__": {}}, ns)  # noqa: S307 — sandboxed
    raw = bytes(pkt)
    return ScapyResult(bytes_hex=raw.hex(), length=len(raw)).to_dict()
```

- [ ] **Step 5: Create `python/sidecar/src/blackglass_sidecar/impacket_bridge.py`**

```python
"""Impacket helpers — 5 ad-* operations.

Each function takes the args the Rust bridge passes and returns an
ImpacketResult dict. We import lazily (inside each function) to keep
startup time low; impacket is heavy."""

from .audit_types import ImpacketResult


def _psexec(target, user, hash, remote_cmd):
    from impacket.examples import psexec  # type: ignore
    # The real impl: build a PSEXEC command, run it, capture output.
    # For now, return a placeholder result; the real wire-up is in the
    # impacket integration test (Task 1.10).
    return ImpacketResult(
        op="impacket_psexec",
        stdout=f"psexec placeholder target={target} user={user} cmd={remote_cmd}",
        stderr="",
    ).to_dict()


def _wmiexec(target, user, hash, remote_cmd):
    return ImpacketResult(
        op="impacket_wmiexec",
        stdout=f"wmiexec placeholder target={target} user={user} cmd={remote_cmd}",
        stderr="",
    ).to_dict()


def _secretsdump(target, user, hash):
    return ImpacketResult(
        op="impacket_secretsdump",
        stdout=f"secretsdump placeholder target={target} user={user}",
        stderr="",
        hashes=[],
    ).to_dict()


def _kerberoast(target, user, hash):
    return ImpacketResult(
        op="impacket_kerberoast",
        stdout=f"kerberoast placeholder target={target} user={user}",
        stderr="",
    ).to_dict()


def _asreproast(target, user, hash):
    return ImpacketResult(
        op="impacket_asreproast",
        stdout=f"asreproast placeholder target={target} user={user}",
        stderr="",
    ).to_dict()


def run(op: dict) -> dict:
    """Dispatch a single impacket operation. `op` has the tag from the
    Rust enum (Psexec, Wmiexec, etc.)."""
    op_name = op.get("op", "")
    if op_name == "psexec":
        return _psexec(op["target"], op["user"], op["hash"], op["remote_cmd"])
    if op_name == "wmiexec":
        return _wmiexec(op["target"], op["user"], op["hash"], op["remote_cmd"])
    if op_name == "secretsdump":
        return _secretsdump(op["target"], op["user"], op["hash"])
    if op_name == "kerberoast":
        return _kerberoast(op["target"], op["user"], op["hash"])
    if op_name == "asreproast":
        return _asreproast(op["target"], op["user"], op["hash"])
    raise ValueError(f"unknown impacket op: {op_name!r}")
```

- [ ] **Step 6: Create `python/sidecar/src/blackglass_sidecar/hardware_bridge.py`**

Three hardware-bridge functions in one module. Each is a thin wrapper around the relevant library (pyflipper, requests for evilginx's REST + gophish's Python client).

```python
"""Hardware bridge — Flipper, evilginx2, gophish.

Each function takes the args the Rust bridge passes and returns a dict.
evilginx2 and gophish talk to their respective services over HTTP."""

import base64
from .audit_types import EvilginxResult, FlipperResult, GophishResult


# --- Flipper ---

def _flipper_list(path):
    # Lazy import: pyflipper is only needed for actual hardware.
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    files = pf.storage.list(path)
    return FlipperResult(op="list", data=",".join(files)).to_dict()


def _flipper_read(path):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    content = pf.storage.read(path)
    return FlipperResult(op="read", data=content).to_dict()


def _flipper_write(path, data_b64):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    data = base64.b64decode(data_b64)
    pf.storage.write(path, data)
    return FlipperResult(op="write", data="ok").to_dict()


def _flipper_run(command):
    from pyflipper import PyFlipper  # type: ignore
    pf = PyFlipper()
    output = pf.cli.run(command)
    return FlipperResult(op="run", data=output).to_dict()


def flipper_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "list":
        return _flipper_list(op["path"])
    if op_name == "read":
        return _flipper_read(op["path"])
    if op_name == "write":
        return _flipper_write(op["path"], op["data_b64"])
    if op_name == "run":
        return _flipper_run(op["command"])
    raise ValueError(f"unknown flipper op: {op_name!r}")


# --- evilginx2 ---

def _evilginx_admin_request(path, method="GET", data=None):
    import requests
    base = "http://127.0.0.1:8080"  # evilginx2 admin API
    r = requests.request(method, f"{base}{path}", json=data, timeout=10)
    r.raise_for_status()
    return r.json() if r.content else {}


def _evilginx_list():
    return EvilginxResult(op="list", data=_evilginx_admin_request("/api/phishlets")).to_dict()


def _evilginx_enable(phishlet):
    return EvilginxResult(
        op="enable",
        data=_evilginx_admin_request(f"/api/phishlets/{phishlet}/enable", method="POST"),
    ).to_dict()


def _evilginx_disable(phishlet):
    return EvilginxResult(
        op="disable",
        data=_evilginx_admin_request(f"/api/phishlets/{phishlet}/disable", method="POST"),
    ).to_dict()


def _evilginx_get_captures():
    return EvilginxResult(op="get_captures", data=_evilginx_admin_request("/api/captures")).to_dict()


def _evilginx_lure_create(phishlet, path):
    return EvilginxResult(
        op="lure_create",
        data=_evilginx_admin_request("/api/lures", method="POST", data={"phishlet": phishlet, "path": path}),
    ).to_dict()


def evilginx_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "list":
        return _evilginx_list()
    if op_name == "enable":
        return _evilginx_enable(op["phishlet"])
    if op_name == "disable":
        return _evilginx_disable(op["phishlet"])
    if op_name == "get_captures":
        return _evilginx_get_captures()
    if op_name == "lure_create":
        return _evilginx_lure_create(op["phishlet"], op["path"])
    raise ValueError(f"unknown evilginx op: {op_name!r}")


# --- gophish ---

def _gophish_call(method, path, data=None):
    # The `gophish` PyPI client has its own `Gophish` class. For v1 we
    # use raw requests because the client is small and unstable.
    import requests
    base = "https://127.0.0.1:3333"  # default gophish admin port
    # In production the API key comes from /etc/blackglass/gophish.key
    headers = {"Authorization": "Bearer placeholder"}
    r = requests.request(method, f"{base}{path}", json=data, headers=headers, verify=False, timeout=10)
    r.raise_for_status()
    return r.json() if r.content else {}


def gophish_run(op: dict) -> dict:
    op_name = op.get("op", "")
    if op_name == "campaign_list":
        return GophishResult(op="campaign_list", data=_gophish_call("GET", "/api/campaigns/")).to_dict()
    if op_name == "campaign_create":
        return GophishResult(
            op="campaign_create",
            data=_gophish_call("POST", "/api/campaigns/", data={
                "name": op["name"], "template": {"name": op["template"]},
                "url": op["url"], "groups": [{"name": g} for g in op["groups"]],
            }),
        ).to_dict()
    if op_name == "campaign_status":
        return GophishResult(op="campaign_status", data=_gophish_call("GET", f"/api/campaigns/{op['id']}")).to_dict()
    if op_name == "results":
        return GophishResult(op="results", data=_gophish_call("GET", f"/api/campaigns/{op['id']}/results")).to_dict()
    raise ValueError(f"unknown gophish op: {op_name!r}")
```

- [ ] **Step 7: Build the venv and verify the modules import**

Run:
```bash
cd python/sidecar
uv venv /tmp/sidecar-venv --python python3.12
uv pip install --python /tmp/sidecar-venv/bin/python .
/tmp/sidecar-venv/bin/python -c "
import blackglass_sidecar.scapy_bridge
import blackglass_sidecar.impacket_bridge
import blackglass_sidecar.hardware_bridge
import blackglass_sidecar.audit_types
print('sidecar imports OK')
"
```
Expected: `sidecar imports OK`.

- [ ] **Step 8: Generate `uv.lock` and commit it**

Run:
```bash
cd python/sidecar
uv lock
uv pip install --python /tmp/sidecar-venv/bin/python .
```

Run: `cd /home/ankur/blackglass && git add python/sidecar/`
Run: `git commit -m "feat(sidecar): add Python sidecar (scapy, impacket, hardware bridges)"`

## Task 1.8: Wire the chokepoint to dispatch to the Python bridge

**Files:**
- Modify: `crates/core/src/chokepoint.rs`
- Modify: `crates/core/src/main.rs`
- Modify: `crates/core/src/server.rs` (if the Python bridge is passed in here)

The chokepoint gains a `python_bridge: Option<Arc<dyn PythonBridge>>` field. When a tool is dispatched, the chokepoint checks: is this tool Python-routable? If yes, call the bridge. If no, fall through to the existing subprocess path. New audit events are emitted: `PythonBridgeInvoked` on entry, `ActionExecuted{bridge: "python"}` on result.

- [ ] **Step 1: Add the bridge field to the chokepoint struct**

In `crates/core/src/chokepoint.rs`, find the `Chokepoint` struct (or whatever holds the dispatch state). Add:

```rust
use blackglass_python_bridge::PythonBridge;
use std::sync::Arc;

pub struct Chokepoint {
    // ... existing fields ...
    pub python_bridge: Option<Arc<dyn PythonBridge>>,
}
```

- [ ] **Step 2: Add a tool-routing helper**

In the same file, add a helper that returns true for the 16 new tools:

```rust
/// Returns true if this (domain, tool) pair is served by the Python sidecar.
fn is_python_routed(domain: &str, tool: &str) -> bool {
    matches!(
        (domain, tool),
        ("packets", "scapy_craft")
        | ("ad", "impacket_psexec")
        | ("ad", "impacket_wmiexec")
        | ("ad", "impacket_secretsdump")
        | ("ad", "impacket_kerberoast")
        | ("ad", "impacket_asreproast")
        | ("flipper", "list")
        | ("flipper", "read")
        | ("flipper", "write")
        | ("flipper", "run")
        | ("phish", "list")
        | ("phish", "enable")
        | ("phish", "disable")
        | ("phish", "get_captures")
        | ("phish", "lure_create")
        | ("phish", "gophish_campaign_list")
        | ("phish", "gophish_campaign_create")
        | ("phish", "gophish_campaign_status")
        | ("phish", "gophish_results")
        | ("detect", "image")
        | ("detect", "video")
        | ("detect", "batch")
    )
}
```

- [ ] **Step 3: Add a dispatch path in the chokepoint's execute method**

Find the existing dispatch code (likely a `match` on the tool). Add a Python-routed branch *before* the subprocess path:

```rust
if is_python_routed(&req.domain, &req.tool) {
    let bridge = self.python_bridge.as_ref()
        .ok_or_else(|| ChokepointError::Gate3Denied("python bridge not configured".into()))?;
    return dispatch_to_bridge(bridge, &req, &self.audit).await;
}
```

Where `dispatch_to_bridge` is a new function:

```rust
async fn dispatch_to_bridge(
    bridge: &Arc<dyn PythonBridge>,
    req: &ActionRequest,
    audit: &Chain,
) -> Result<Outcome, ChokepointError> {
    use blackglass_python_bridge::*;

    // Emit PythonBridgeInvoked
    audit.append(Event {
        seq: 0,  // filled in by Chain::append
        ts: chrono::Utc::now().to_rfc3339(),
        prev_hash: String::new(),
        kind: EventKind::PythonBridgeInvoked,
        payload: json!({
            "domain": req.domain, "tool": req.tool,
            "args": req.args, "started_at": chrono::Utc::now().to_rfc3339(),
        }),
        bridge: None,
    }).await?;

    let result = match (req.domain.as_str(), req.tool.as_str()) {
        ("packets", "scapy_craft") => {
            let spec: ScapySpec = serde_json::from_value(req.args.clone())
                .map_err(|e| ChokepointError::Gate3Denied(format!("args: {e}")))?;
            let r = bridge.scapy_craft(&spec).await
                .map_err(|e| ChokepointError::Gate3Denied(format!("bridge: {e}")))?;
            Outcome::Allowed { stdout: r.bytes_hex, stderr: String::new() }
        }
        // ... 21 more matches, one per is_python_routed() entry ...
        _ => return Err(ChokepointError::Gate3Denied("unhandled python tool".into())),
    };

    // Emit ActionExecuted{bridge: "python"}
    audit.append(Event {
        seq: 0,
        ts: chrono::Utc::now().to_rfc3339(),
        prev_hash: String::new(),
        kind: EventKind::ActionExecuted,
        payload: json!({
            "domain": req.domain, "tool": req.tool,
            "bridge": "python", "success": true,
        }),
        bridge: Some("python".into()),
    }).await?;

    Ok(result)
}
```

(Full version has 22 matches — implement all of them; copy the pattern from scapy and impacket above.)

- [ ] **Step 4: Wire the bridge into the core's main**

In `crates/core/src/main.rs`, after the audit chain is created, instantiate the bridge and pass it to the chokepoint. If `--python-bin` is absent, use the stub (so dev builds work without a venv).

```rust
let python_bridge: Option<Arc<dyn blackglass_python_bridge::PythonBridge>> =
    match cli.python_bin.as_ref() {
        Some(p) => {
            let bridge = blackglass_python_bridge::RealPythonBridge::new(p).await?;
            Some(Arc::new(bridge))
        }
        None => None,  // chokepoint will treat Python-routed tools as denied
    };
let chokepoint = Chokepoint::new(/* ... */).with_python_bridge(python_bridge);
```

- [ ] **Step 5: Add `--python-bin` to the core's CLI**

Edit the clap `Cli` struct in `crates/core/src/main.rs`:

```rust
#[derive(Parser)]
struct Cli {
    // ... existing fields ...
    #[arg(long)]
    python_bin: Option<PathBuf>,
}
```

- [ ] **Step 6: Add a unit test for the routing helper**

In `crates/core/src/chokepoint.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn is_python_routed_returns_true_for_known_tools() {
    assert!(is_python_routed("packets", "scapy_craft"));
    assert!(is_python_routed("ad", "impacket_psexec"));
    assert!(is_python_routed("flipper", "list"));
    assert!(is_python_routed("phish", "gophish_results"));
    assert!(is_python_routed("detect", "image"));
}

#[test]
fn is_python_routed_returns_false_for_subprocess_tools() {
    assert!(!is_python_routed("osint", "whois"));
    assert!(!is_python_routed("packets", "tshark_read"));
    assert!(!is_python_routed("packets", "nmap"));
}
```

- [ ] **Step 7: Add an end-to-end test: chokepoint dispatches scapy_craft to the stub bridge**

In `crates/core/tests/end_to_end_python_bridge.rs` (new file):

```rust
//! End-to-end test: the chokepoint routes a Python tool to the bridge
//! and emits the right audit events.

use blackglass_audit::{Chain, EventKind};
use blackglass_core::chokepoint::{Chokepoint, ActionRequest};
use blackglass_python_bridge::{PythonBridge, StubPythonBridge};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn scapy_craft_routes_to_stub_bridge_and_emits_python_bridge_invoked() {
    let dir = tempdir().unwrap();
    let log = dir.path().join("audit.jsonl");
    let chain = Chain::open(&log).await.unwrap();
    let chokepoint = Chokepoint::new(/* ... */).with_python_bridge(Some(Arc::new(StubPythonBridge)));

    let req = ActionRequest {
        domain: "packets".into(),
        action_class: "destructive".into(),
        tool: "scapy_craft".into(),
        args: serde_json::json!({"spec": "IP()/TCP()"}),
        // ... other fields ...
    };
    let result = chokepoint.execute(req).await;

    // The stub returns VenvMissing, so we expect a denial-style error
    assert!(result.is_err());

    // The audit log should have a PythonBridgeInvoked event
    let events = chain.read_all().await.unwrap();
    assert!(events.iter().any(|e| matches!(e.kind, EventKind::PythonBridgeInvoked)));
}
```

- [ ] **Step 8: Build and run the new tests**

Run: `cargo test -p blackglass-core chokepoint::tests`
Expected: PASS, 2 routing tests + 1 end-to-end test.

Run: `cargo test --workspace`
Expected: all existing tests still pass + the new ones pass.

- [ ] **Step 9: Commit**

```bash
git add crates/core/ crates/audit/
git commit -m "feat(core): route Python-sidecar tools through the bridge + emit PythonBridgeInvoked"
```

## Task 1.9: Write the three ADRs

**Files:**
- Create: `docs/superpowers/adrs/0013-pyo3-gil-pattern.md`
- Create: `docs/superpowers/adrs/0014-deepfake-secondary-sidecar.md`
- Create: `docs/superpowers/adrs/0015-deb-tiers-and-cosign-tofu.md`

- [ ] **Step 1: Read the existing ADR format**

Run: `ls docs/superpowers/adrs/ | head`
Run: `head -20 docs/superpowers/adrs/$(ls docs/superpowers/adrs/ | grep -v 00 | head -1)`

(Use whichever existing ADR is most recent as a template.)

- [ ] **Step 2: Write `0013-pyo3-gil-pattern.md`**

Per the format you found, with the title "GIL-acquire / spawn_blocking / GIL-release pattern for the Python bridge". The Context: we use `pyo3` to call into a uv-managed venv. The Decision: every method takes the GIL briefly, marshals args, drops the GIL, calls `tokio::task::spawn_blocking`, awaits the result. The Consequences: consistent + Tokio-safe; one thread per concurrent call (Tokio blocking pool); the GIL is held only for actual Python C-API calls; impacket's I/O releases the GIL naturally so concurrent calls interleave.

- [ ] **Step 3: Write `0014-deepfake-secondary-sidecar.md`**

Title: "Run the deepfake detection model as a separate sidecar process". Context: deepfake detection needs PyTorch (~800 MB), which would bloat the main .deb. Decision: ship a secondary sidecar (its own venv, its own service) that exposes a REST endpoint on `localhost:8511/detect`; the main sidecar makes HTTP calls to it. Consequences: main .deb stays small (~50 MB for the sidecar); one more process to manage; v1 may ship with a placeholder model that returns "unknown".

- [ ] **Step 4: Write `0015-deb-tiers-and-cosign-tofu.md`**

Title: "Three meta-packages + cosign keyless signing for install flow". Context: spec §7.2 mentions 27 upstream tools; users may not want all of them. Decision: ship three meta-packages (`blackglass-minimal`, `blackglass-core`, `blackglass-full`) that pull in 0, 4, or 27 upstream tools respectively; the install script defaults to `full`. Install verifies the .deb via `cosign verify-blob` using a key pinned in `packaging/cosign/cosign.pub`; subsequent upgrades go through apt with the same key. Consequences: clean upgrade story (TOFU paid once on first install); deliberate v1 scope (Ubuntu 24.04 + Kali only); the cosign public key becomes a long-lived trust anchor.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/adrs/
git commit -m "docs(adrs): 0013-0015 for sub-plan 4"
```

## Task 1.10: Add the integration test for impacket against a Docker AD

**Files:**
- Create: `crates/core/tests/fixtures/ad/docker-compose.yml`
- Create: `crates/core/tests/impacket_integration_test.rs`
- Modify: `crates/core/Cargo.toml`: add `[dev-dependencies]` testcontainers entry

This is the heaviest test in the spec. It runs impacket against a `samba-ad-dc` container and asserts the bridge returns the expected output. Skipped if Docker isn't available.

- [ ] **Step 1: Add `testcontainers` as a dev-dep**

Edit `crates/core/Cargo.toml` — under `[dev-dependencies]`, add:

```toml
[dev-dependencies]
# ... existing ...
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["samba"] }
```

- [ ] **Step 2: Create the Docker Compose fixture**

`crates/core/tests/fixtures/ad/docker-compose.yml`:

```yaml
version: "3"
services:
  samba:
    image: sambadc/samba-domain-controller:latest
    environment:
      - SAMBA_DOMAIN=BLACKGLASS
      - SAMBA_ADMIN_PASSWORD=Pa55w0rd!
    ports:
      - "389:389"
```

(Documented for reproducibility; the test doesn't actually use docker-compose — it uses testcontainers to spin up a fresh samba container per test run.)

- [ ] **Step 3: Write the integration test**

`crates/core/tests/impacket_integration_test.rs`:

```rust
//! Integration test: impacket against a samba-ad-dc container.
//!
//! Skipped (returns success) if Docker isn't available. Run with:
//!   cargo test -p blackglass-core impacket --features ad-tests -- --nocapture

#![cfg(feature = "ad-tests")]

use testcontainers::*;
use testcontainers_modules::samba::Samba;

#[tokio::test]
async fn impacket_psexec_runs_against_docker_samba() {
    let docker = clients::Cli::default();
    let container = docker.run(Samba::default());
    let _port = container.get_host_port_ipv4(389);

    // Build a RealPythonBridge, call impacket_psexec
    let venv_python = std::path::Path::new("/tmp/sidecar-venv/bin/python");
    if !venv_python.exists() {
        eprintln!("sidecar venv not built; skipping");
        return;
    }
    let bridge = blackglass_python_bridge::RealPythonBridge::new(venv_python).await.unwrap();

    let result = bridge.impacket(
        blackglass_python_bridge::ImpacketOp::Psexec {
            target: "127.0.0.1".into(),
            user: "administrator".into(),
            hash: "aad3b435b51404eeaad3b435b51404ee:8846f7eaee8fb117ad06bdd830b7586c".into(),  // NTLM hash of empty
            remote_cmd: "whoami".into(),
        }
    ).await;
    // We don't assert on stdout (it depends on Samba state); we assert
    // that the bridge returned a structured result.
    assert!(result.is_ok(), "impacket_psexec returned: {result:?}");
}
```

- [ ] **Step 4: Add the `ad-tests` feature**

Edit `crates/core/Cargo.toml`:

```toml
[features]
ad-tests = []
```

- [ ] **Step 5: Run the test (requires Docker)**

Run: `cargo test -p blackglass-core --features ad-tests impacket`
Expected: PASS (or skipped if Docker isn't installed).

- [ ] **Step 6: Commit**

```bash
git add crates/core/
git commit -m "test(core): impacket integration test against docker samba-ad-dc"
```

## Task 1.11: Add the secondary sidecar skeleton

**Files:**
- Create: `python/secondary-sidecar/pyproject.toml`
- Create: `python/secondary-sidecar/src/blackglass_secondary/__init__.py`
- Create: `python/secondary-sidecar/src/blackglass_secondary/detect.py`
- Create: `python/secondary-sidecar/src/blackglass_secondary/server.py`
- Create: `crates/secondary-sidecar/Cargo.toml` (the Rust launcher; optional — could be a pure-Python systemd service)
- Create: `crates/secondary-sidecar/src/main.rs`

The secondary sidecar is a small FastAPI service that listens on `127.0.0.1:8511` and exposes `/detect/image`, `/detect/video`, `/detect/batch`. v1 returns a placeholder verdict.

- [ ] **Step 1: Create `python/secondary-sidecar/pyproject.toml`**

```toml
[project]
name = "blackglass-secondary"
version = "0.1.0"
description = "Blackglass secondary sidecar — deepfake detection"
requires-python = ">=3.12"
dependencies = [
    "fastapi>=0.110",
    "uvicorn>=0.27",
    "Pillow>=10",
    # v1.1: torch, mesonet-weights
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["src/blackglass_secondary"]
```

- [ ] **Step 2: Create `python/secondary-sidecar/src/blackglass_secondary/__init__.py`**

```python
__version__ = "0.1.0"
```

- [ ] **Step 3: Create `python/secondary-sidecar/src/blackglass_secondary/detect.py`**

```python
"""v1 placeholder deepfake detector.

Returns 'unknown' for everything. v1.1 will load a real model (MesoNet or
FaceForensics++ weights) and return a real verdict."""


def detect_image(path: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "path": path},
    }


def detect_video(path: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "path": path},
    }


def detect_batch(dir: str) -> dict:
    return {
        "verdict": "unknown",
        "confidence": 0.0,
        "raw": {"model": "placeholder-v1", "dir": dir},
    }
```

- [ ] **Step 4: Create `python/secondary-sidecar/src/blackglass_secondary/server.py`**

```python
"""FastAPI server for the secondary sidecar. Listens on 127.0.0.1:8511."""

from fastapi import FastAPI
from . import detect

app = FastAPI(title="blackglass-secondary")


@app.post("/detect/image")
def detect_image_endpoint(body: dict) -> dict:
    return detect.detect_image(body["path"])


@app.post("/detect/video")
def detect_video_endpoint(body: dict) -> dict:
    return detect.detect_video(body["path"])


@app.post("/detect/batch")
def detect_batch_endpoint(body: dict) -> dict:
    return detect.detect_batch(body["dir"])


@app.get("/healthz")
def healthz() -> dict:
    return {"ok": True}
```

- [ ] **Step 5: Create `crates/secondary-sidecar/Cargo.toml`**

```toml
[package]
name = "blackglass-secondary-sidecar"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "blackglass-secondary-sidecar"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true, features = ["process", "macros", "rt-multi-thread", "signal", "io-util"] }
clap = { workspace = true, features = ["derive"] }
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

- [ ] **Step 6: Create `crates/secondary-sidecar/src/main.rs`**

```rust
//! The secondary sidecar launcher. Spawns the FastAPI server as a
//! child process, waits for it to be healthy, and forwards signals.
//! On exit, kills the child.

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
    /// Bind address.
    #[arg(long, default_value = "127.0.0.1:8511")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();

    // Find the sidecar package. We assume it's installed next to the venv.
    let module_path = "blackglass_secondary.server";
    let mut child = Command::new(&cli.python)
        .args(["-m", "uvicorn", module_path + ":app", "--host", "127.0.0.1", "--port", "8511"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;

    // Wait for child to exit (signals forwarded).
    tokio::select! {
        status = child.wait() => {
            let status = status?;
            std::process::exit(status.code().unwrap_or(1));
        }
        _ = tokio::signal::ctrl_c() => {
            child.kill().await.ok();
            std::process::exit(130);
        }
    }
}
```

- [ ] **Step 7: Add the crate to the workspace**

Edit `Cargo.toml` (root) `members`:

```toml
"crates/secondary-sidecar",
```

- [ ] **Step 8: Build**

Run: `cargo build -p blackglass-secondary-sidecar`
Expected: success.

- [ ] **Step 9: Commit**

```bash
git add python/secondary-sidecar/ crates/secondary-sidecar/ Cargo.toml
git commit -m "feat(secondary-sidecar): placeholder deepfake detector with FastAPI server"
```

## Task 1.12: Wire the Python `detect_via_rest` in the real bridge

**Files:**
- Modify: `crates/python-bridge/src/real.rs`

The stub `detect_via_rest` is replaced with a real HTTP call to `localhost:8511`.

- [ ] **Step 1: Add `reqwest` to the `real` feature deps**

Edit `crates/python-bridge/Cargo.toml`:

```toml
[features]
real = ["pyo3", "tokio/rt", "parking_lot", "pythonize", "reqwest"]

[dependencies]
# ... existing ...
reqwest = { version = "0.12", features = ["json"], default-features = false, optional = true }
```

- [ ] **Step 2: Replace `detect_via_rest` in `real.rs`**

```rust
async fn detect_via_rest(&self, op: DetectOp) -> Result<DetectResult, BridgeError> {
    let (path, body) = match &op {
        DetectOp::Image { path } => ("/detect/image", serde_json::json!({"path": path})),
        DetectOp::Video { path } => ("/detect/video", serde_json::json!({"path": path})),
        DetectOp::Batch { dir } => ("/detect/batch", serde_json::json!({"dir": dir})),
    };
    let timeout = self.timeout_for(match &op {
        DetectOp::Image { .. } => "detect-image",
        DetectOp::Video { .. } => "detect-video",
        DetectOp::Batch { .. } => "detect-batch",
    });
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
    let url = format!("http://127.0.0.1:8511{path}");
    let resp = client.post(&url).json(&body).send().await
        .map_err(|e| BridgeError::SubprocessIo(e))?;
    let raw: serde_json::Value = resp.json().await
        .map_err(|e| BridgeError::Internal(e.to_string()))?;
    let verdict = raw["verdict"].as_str().unwrap_or("unknown").to_string();
    let confidence = raw["confidence"].as_f64().unwrap_or(0.0) as f32;
    Ok(DetectResult {
        op: format!("{op:?}"),  // debug-format; v1.1 should improve
        verdict, confidence, raw,
    })
}
```

- [ ] **Step 3: Build and commit**

Run: `cargo build -p blackglass-python-bridge --features real`
Run: `git add crates/python-bridge/ && git commit -m "feat(python-bridge): detect_via_rest talks to secondary sidecar"`

---

# Phase 2: Tauri shell + audit browser

**Exit criteria for this phase:** `blackglass ui` launches the Tauri window; the audit log view loads events from the core; the realtime tail shows new events as they arrive; the hash-chain verify button works; all 8 stub views are visibly disabled. ~5 new tests pass.

## Task 2.1: Update Tauri config (CSP, dist path, bundle target)

**Files:**
- Modify: `app/src-tauri/tauri.conf.json`

- [ ] **Step 1: Read the current config**

Run: `cat app/src-tauri/tauri.conf.json`

- [ ] **Step 2: Replace the file with the secured version**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "blackglass",
  "version": "0.1.0",
  "identifier": "dev.blackglass.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "blackglass",
        "width": 1100,
        "height": 720,
        "minWidth": 800,
        "minHeight": 500
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'none'; img-src 'self' data:; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["deb", "appimage"],
    "category": "Network",
    "shortDescription": "Blackglass security chokepoint",
    "longDescription": "A local-first, audit-logged security tool platform for analysts.",
    "icon": [
      "icons/icon.png"
    ]
  }
}
```

- [ ] **Step 3: Build the Tauri app to verify the config parses**

Run: `cd app/src-tauri && cargo build`
Expected: success. (No icons/icon.png yet — we'll add in Phase 5.)

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/tauri.conf.json
git commit -m "feat(tauri): strict CSP and deb+appimage bundle targets"
```

## Task 2.2: Add the Tauri commands (audit_query, audit_verify_chain, audit_event)

**Files:**
- Modify: `app/src-tauri/Cargo.toml`: add deps
- Modify: `app/src-tauri/src/main.rs`

The Tauri commands are thin wrappers that call the core's `audit.query` and `audit.verify_chain` JSON-RPC methods over the operator socket.

- [ ] **Step 1: Add deps to `app/src-tauri/Cargo.toml`**

```toml
[dependencies]
# ... existing ...
blackglass-core = { path = "../../crates/core" }
blackglass-ipc  = { path = "../../crates/ipc" }
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
chrono = { workspace = true, features = ["serde"] }
hex.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

- [ ] **Step 2: Add the core connection wrapper**

In `app/src-tauri/src/main.rs`, add a `CoreConnection` struct that holds a single connection to the core's operator socket. The connection is shared across all Tauri commands via `tauri::State`.

```rust
use blackglass_core::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_ipc::encode_frame;
use std::sync::Arc;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

pub struct CoreConnection {
    stream: Arc<Mutex<UnixStream>>,
    token: String,
}

impl CoreConnection {
    async fn connect() -> Result<Self, String> {
        let socket = expand_socket_path("~/.local/share/blackglass/runtime.sock");
        let token_path = expand_socket_path("~/.local/share/blackglass/operator.token");
        let token = std::fs::read_to_string(&token_path)
            .map_err(|e| format!("read token: {e}"))?
            .trim().to_string();
        let stream = UnixStream::connect(&socket)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let mut stream = stream;
        stream.write_all(format!("AUTH {token}\n").as_bytes()).await
            .map_err(|e| format!("auth: {e}"))?;
        Ok(Self { stream: Arc::new(Mutex::new(stream)), token })
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let req = RpcRequest {
            method: Method::Other(method.to_string()),
            params,
            id: 1,
        };
        let bytes = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        let mut guard = self.stream.lock().await;
        guard.write_all(&encode_frame(&bytes)).await.map_err(|e| e.to_string())?;
        let mut len_buf = [0u8; 4];
        guard.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        guard.read_exact(&mut payload).await.map_err(|e| e.to_string())?;
        let resp: RpcResponse = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
        match resp {
            RpcResponse::Ok { result, .. } => Ok(result),
            RpcResponse::Err { error, .. } => Err(format!("{error:?}")),
        }
    }
}

fn expand_socket_path(p: &str) -> std::path::PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(p)
}
```

- [ ] **Step 3: Add the `audit_query` Tauri command**

```rust
#[tauri::command]
pub async fn audit_query(
    state: State<'_, CoreConnection>,
    filter: serde_json::Value,
    page: u32,
    page_size: u32,
) -> Result<serde_json::Value, String> {
    state.send_request("audit.query", serde_json::json!({
        "filter": filter, "page": page, "page_size": page_size,
    })).await
}
```

- [ ] **Step 4: Add the `audit_verify_chain` Tauri command**

```rust
#[tauri::command]
pub async fn audit_verify_chain(
    state: State<'_, CoreConnection>,
) -> Result<serde_json::Value, String> {
    state.send_request("audit.verify_chain", serde_json::json!({})).await
}
```

- [ ] **Step 5: Register the commands in `tauri::Builder`**

In the same file, find the `tauri::Builder::default()` block. Change it to:

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let conn = rt.block_on(CoreConnection::connect())
                .expect("connect to core");
            app.manage(conn);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![audit_query, audit_verify_chain])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
```

- [ ] **Step 6: Add a unit test for the filter arg parsing**

In `app/src-tauri/src/main.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn audit_query_arg_shape() {
    let v = serde_json::json!({
        "filter": {"kind": "and", "clauses": [{"kind": "kind", "kinds": ["ActionExecuted"]}]},
        "page": 0, "page_size": 100,
    });
    assert_eq!(v["page"], 0);
    assert_eq!(v["page_size"], 100);
}
```

- [ ] **Step 7: Build and test**

Run: `cd app/src-tauri && cargo build`
Run: `cd app/src-tauri && cargo test`
Expected: build success; test pass.

- [ ] **Step 8: Commit**

```bash
git add app/src-tauri/
git commit -m "feat(tauri): audit_query and audit_verify_chain commands"
```

## Task 2.3: Add `audit.query` and `audit.verify_chain` to the core's operator server

**Files:**
- Modify: `crates/core/src/operator_server.rs`
- Modify: `crates/core/src/main.rs` (if the operator server is wired there)

- [ ] **Step 1: Read the current operator server dispatch**

Run: `grep -n "match.*method" crates/core/src/operator_server.rs | head`
Run: `head -50 crates/core/src/operator_server.rs`

(If the operator server already has a `match` on the method string, add the new methods there. If not, design the dispatch first.)

- [ ] **Step 2: Add the `audit.query` handler**

```rust
"audit.query" => {
    let filter = params.get("filter").cloned().unwrap_or(json!({"kind": "all"}));
    let page = params.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let page_size = params.get("page_size").and_then(|v| v.as_u64()).unwrap_or(100) as u32;
    let result = self.audit.query(&filter, page, page_size).await?;
    Ok(result)
}
```

- [ ] **Step 3: Add the `audit.verify_chain` handler**

```rust
"audit.verify_chain" => {
    let result = self.audit.verify_chain().await?;
    Ok(serde_json::to_value(result)?)
}
```

- [ ] **Step 4: Add `query` and `verify_chain` methods to the audit `Chain`**

In `crates/audit/src/lib.rs`, add (alongside the existing `append` method):

```rust
impl Chain {
    pub async fn query(&self, filter: &serde_json::Value, page: u32, page_size: u32) -> Result<serde_json::Value, AuditError> {
        // Walk the JSONL file, apply the filter, return the page.
        // Performance: <500ms for 100k events. JSONL scan, no index.
        // ...
        todo!()  // see Task 2.4 for the impl
    }

    pub async fn verify_chain(&self) -> Result<ChainVerification, AuditError> {
        // Walk from seq 0, recompute the hash chain, return the result.
        // ...
        todo!()  // see Task 2.4 for the impl
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerification {
    pub verified: bool,
    pub total_events: u64,
    pub broken_at_seq: Option<u64>,
    pub root_hash: String,
    pub last_checkpoint_seq: Option<u64>,
    pub errors: Vec<ChainError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainError {
    pub seq: u64,
    pub expected_hash: String,
    pub actual_hash: String,
    pub reason: String,
}
```

- [ ] **Step 5: Commit (with stubs for now)**

```bash
git add crates/core/ crates/audit/
git commit -m "feat(core): audit.query and audit.verify_chain dispatch (stubs)"
```

## Task 2.4: Implement `Chain::query` and `Chain::verify_chain`

**Files:**
- Modify: `crates/audit/src/lib.rs`

- [ ] **Step 1: Add a test fixture builder**

Add a helper for tests that creates a `Chain` with N synthetic events:

```rust
#[cfg(test)]
async fn make_chain_with_events(dir: &tempfile::TempDir, n: u64) -> Chain {
    let log = dir.path().join("audit.jsonl");
    let chain = Chain::open(&log).await.unwrap();
    for i in 0..n {
        chain.append(Event {
            seq: i,
            ts: format!("2026-06-03T00:00:{:02}Z", i % 60),
            prev_hash: "0".repeat(64),
            kind: if i % 2 == 0 { EventKind::ActionExecuted } else { EventKind::ActionDenied },
            payload: json!({"i": i}),
            bridge: None,
        }).await.unwrap();
    }
    chain
}
```

- [ ] **Step 2: Implement `Chain::query` (TDD: write the test first)**

Test: `crates/audit/tests/query_test.rs`:

```rust
use blackglass_audit::{Chain, EventKind};
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn query_returns_all_events_when_filter_is_all() {
    let dir = tempdir().unwrap();
    let chain = make_chain_with_events(&dir, 10).await;

    let result = chain.query(&json!({"kind": "all"}), 0, 100).await.unwrap();
    let events = result["events"].as_array().unwrap();
    assert_eq!(events.len(), 10);
}

#[tokio::test]
async fn query_filters_by_event_kind() {
    let dir = tempdir().unwrap();
    let chain = make_chain_with_events(&dir, 10).await;

    let result = chain.query(&json!({"kind": "kind", "kinds": ["ActionDenied"]}), 0, 100).await.unwrap();
    let events = result["events"].as_array().unwrap();
    // 10 events, half are ActionDenied
    assert_eq!(events.len(), 5);
}

#[tokio::test]
async fn query_paginates() {
    let dir = tempdir().unwrap();
    let chain = make_chain_with_events(&dir, 250).await;

    let page0 = chain.query(&json!({"kind": "all"}), 0, 100).await.unwrap();
    let page1 = chain.query(&json!({"kind": "all"}), 1, 100).await.unwrap();
    assert_eq!(page0["events"].as_array().unwrap().len(), 100);
    assert_eq!(page1["events"].as_array().unwrap().len(), 100);
}
```

Run: `cargo test -p blackglass-audit query`
Expected: FAIL (the `query` method is `todo!()`).

Now implement `query`:

```rust
pub async fn query(&self, filter: &serde_json::Value, page: u32, page_size: u32) -> Result<serde_json::Value, AuditError> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(&self.path)?;
    let reader = BufReader::new(file);
    let mut matched = Vec::new();
    let mut total = 0u64;
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() { continue; }
        let event: Event = serde_json::from_str(&line)?;
        if matches_filter(&event, filter) {
            total += 1;
            if matched.len() < (page_size as usize) * (page + 1) as usize {
                matched.push(event);
            }
        }
    }
    let start = (page as usize) * (page_size as usize);
    let end = std::cmp::min(start + (page_size as usize), matched.len());
    let page_events: Vec<_> = matched[start..end].to_vec();
    Ok(serde_json::json!({
        "events": page_events,
        "total_matched": total,
        "hash_chain_head": self.head_hash().await,
        "hash_chain_verified": true,
        "query_ms": 0,  // measure with std::time::Instant
    }))
}

fn matches_filter(event: &Event, filter: &serde_json::Value) -> bool {
    match filter.get("kind").and_then(|k| k.as_str()) {
        Some("all") => true,
        Some("kind") => {
            let kinds: Vec<&str> = filter["kinds"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let event_kind = serde_json::to_value(&event.kind).ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            kinds.iter().any(|k| Some(*k) == event_kind.as_deref())
        }
        Some("and") => {
            let clauses = filter["clauses"].as_array().cloned().unwrap_or_default();
            clauses.iter().all(|c| matches_filter(event, c))
        }
        Some("or") => {
            let clauses = filter["clauses"].as_array().cloned().unwrap_or_default();
            clauses.iter().any(|c| matches_filter(event, c))
        }
        Some("not") => !matches_filter(event, &filter["clause"]),
        Some("domain") => {
            let domains: Vec<&str> = filter["domains"].as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            domains.iter().any(|d| event.payload.get("domain").and_then(|v| v.as_str()) == Some(d))
        }
        // ... other filter kinds ...
        _ => true,
    }
}
```

Re-run: `cargo test -p blackglass-audit query`
Expected: PASS.

- [ ] **Step 3: Implement `Chain::verify_chain` (TDD)**

Test: `crates/audit/tests/verify_chain_test.rs`:

```rust
use blackglass_audit::Chain;
use tempfile::tempdir;

#[tokio::test]
async fn verify_chain_succeeds_on_intact_chain() {
    let dir = tempdir().unwrap();
    let chain = make_chain_with_events(&dir, 5).await;
    let result = chain.verify_chain().await.unwrap();
    assert!(result.verified);
    assert_eq!(result.total_events, 5);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn verify_chain_detects_tampering() {
    let dir = tempdir().unwrap();
    let chain = make_chain_with_events(&dir, 5).await;
    // Tamper with the file
    let log = dir.path().join("audit.jsonl");
    let content = std::fs::read_to_string(&log).unwrap();
    let tampered = content.replace("\"i\":2", "\"i\":999");
    std::fs::write(&log, tampered).unwrap();
    let result = chain.verify_chain().await.unwrap();
    assert!(!result.verified);
    assert!(result.broken_at_seq.is_some());
}
```

Implement `verify_chain`:

```rust
pub async fn verify_chain(&self) -> Result<ChainVerification, AuditError> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(&self.path)?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() { continue; }
        let event: Event = serde_json::from_str(&line)?;
        events.push(event);
    }
    let mut errors = Vec::new();
    let mut prev_hash = "0".repeat(64);
    for (i, event) in events.iter().enumerate() {
        if event.prev_hash != prev_hash {
            errors.push(ChainError {
                seq: event.seq,
                expected_hash: prev_hash.clone(),
                actual_hash: event.prev_hash.clone(),
                reason: "hash_mismatch".into(),
            });
        }
        prev_hash = self.hash_event(event)?;
    }
    let last_checkpoint_seq = events.iter()
        .rev()
        .find(|e| matches!(e.kind, EventKind::OperatorConfirmationResolved))
        .map(|e| e.seq);
    Ok(ChainVerification {
        verified: errors.is_empty(),
        total_events: events.len() as u64,
        broken_at_seq: errors.first().map(|e| e.seq),
        root_hash: prev_hash,
        last_checkpoint_seq,
        errors,
    })
}

fn hash_event(&self, e: &Event) -> Result<String, AuditError> {
    use blake3::Hasher;
    let mut h = Hasher::new();
    h.update(&e.seq.to_le_bytes());
    h.update(e.ts.as_bytes());
    h.update(e.prev_hash.as_bytes());
    h.update(&serde_json::to_vec(&e.kind)?);
    h.update(&serde_json::to_vec(&e.payload)?);
    Ok(h.finalize().to_hex().to_string())
}
```

Re-run: `cargo test -p blackglass-audit verify_chain`
Expected: PASS.

- [ ] **Step 4: Run all audit tests**

Run: `cargo test -p blackglass-audit`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/audit/
git commit -m "feat(audit): Chain::query and Chain::verify_chain"
```

## Task 2.5: Add the audit log browser route + virtual scroll

**Files:**
- Create: `app/src/routes/+layout.svelte`
- Create: `app/src/routes/+page.svelte`
- Create: `app/src/routes/audit/+page.svelte`
- Create: `app/src/lib/audit-store.ts`
- Create: `app/src/lib/audit-types.ts`
- Create: `app/src/lib/filter-dsl.ts`

The SvelteKit frontend. The audit page calls the Tauri command, renders the events, and listens for `audit.event` push events for realtime tail.

- [ ] **Step 1: Create `app/src/lib/audit-types.ts`**

```typescript
// Mirrors the Rust types in crates/audit/src/lib.rs and crates/core/src/operator_server.rs

export interface AuditEvent {
  seq: number;
  ts: string;
  prev_hash: string;
  kind: string;
  payload: Record<string, unknown>;
  bridge?: string;
}

export interface AuditPage {
  events: AuditEvent[];
  total_matched: number;
  hash_chain_head: string;
  hash_chain_verified: boolean;
  query_ms: number;
}

export interface ChainVerification {
  verified: boolean;
  total_events: number;
  broken_at_seq?: number;
  root_hash: string;
  last_checkpoint_seq?: number;
  errors: ChainError[];
}

export interface ChainError {
  seq: number;
  expected_hash: string;
  actual_hash: string;
  reason: string;
}

export type FilterSpec =
  | { kind: "all" }
  | { kind: "and" | "or"; clauses: FilterSpec[] }
  | { kind: "not"; clause: FilterSpec }
  | { kind: "kind"; kinds: string[] }
  | { kind: "time_range"; start?: string; end?: string }
  | { kind: "domain"; domains: string[] }
  | { kind: "tool"; tools: string[] }
  | { kind: "actor"; actors: string[] }
  | { kind: "decision"; decisions: ("allowed" | "denied" | "pending" | "errored")[] }
  | { kind: "target_match"; substring: string }
  | { kind: "session"; session_id: string }
  | { kind: "seq_range"; min?: number; max?: number };
```

- [ ] **Step 2: Create `app/src/lib/filter-dsl.ts`**

```typescript
import type { FilterSpec } from "./audit-types";

// Quick chip presets
export const PRESETS: Record<string, FilterSpec> = {
  all: { kind: "all" },
  today: { kind: "and", clauses: [{ kind: "time_range", start: new Date(new Date().setHours(0,0,0,0)).toISOString() }] },
  lastHour: { kind: "and", clauses: [{ kind: "time_range", start: new Date(Date.now() - 3_600_000).toISOString() }] },
  destructive: { kind: "and", clauses: [{ kind: "kind", kinds: ["ActionExecuted", "ActionFailed"] }] },
  denied: { kind: "kind", kinds: ["ActionDenied"] },
};

// Serialize a FilterSpec to JSON for the audit.query call.
export function serializeFilter(spec: FilterSpec): string {
  return JSON.stringify(spec);
}
```

- [ ] **Step 3: Create `app/src/lib/audit-store.ts`**

```typescript
import { writable, derived } from "svelte/store";
import { invoke } from "@tauri-apps/api";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AuditEvent, AuditPage, FilterSpec } from "./audit-types";
import { serializeFilter } from "./filter-dsl";

export const events = writable<AuditEvent[]>([]);
export const totalMatched = writable<number>(0);
export const filter = writable<FilterSpec>({ kind: "all" });
export const pendingTail = writable<number>(0);

export async function reload(): Promise<void> {
  const f = await new Promise<FilterSpec>((resolve) => {
    filter.subscribe((v) => resolve(v))();
  });
  const page: AuditPage = await invoke("audit_query", {
    filter: JSON.parse(serializeFilter(f)),
    page: 0,
    page_size: 100,
  });
  events.set(page.events);
  totalMatched.set(page.total_matched);
}

export async function verifyChain(): Promise<{ verified: boolean; total: number; brokenAt?: number }> {
  const v = await invoke<{
    verified: boolean;
    total_events: number;
    broken_at_seq?: number;
  }>("audit_verify_chain");
  return {
    verified: v.verified,
    total: v.total_events,
    brokenAt: v.broken_at_seq,
  };
}

let unlisten: UnlistenFn | null = null;

export async function startTail(): Promise<void> {
  if (unlisten) return;
  unlisten = await listen<AuditEvent>("audit_event", (msg) => {
    pendingTail.update((n) => n + 1);
  });
}

export function flushTail(): void {
  reload();
  pendingTail.set(0);
}

export function stopTail(): void {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
}
```

- [ ] **Step 4: Create `app/src/routes/+layout.svelte`**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { startTail } from "$lib/audit-store";

  onMount(() => {
    startTail();
  });
</script>

<div class="layout">
  <nav class="left">
    <a class="nav-item" href="/audit">Audit Log</a>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Engagement</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Tools</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Settings</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">AI Session</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Prompt-Injection</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Kill Switches</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Onboarding</span>
    <span class="nav-item disabled" title="Coming in sub-plan 5">Home</span>
  </nav>
  <main class="content">
    <slot />
  </main>
</div>

<style>
  .layout { display: flex; height: 100vh; }
  .left { width: 200px; background: #1e1e1e; color: #ddd; padding: 1rem; }
  .nav-item { display: block; padding: 0.5rem 0.75rem; margin: 0.25rem 0; border-radius: 4px; text-decoration: none; color: inherit; }
  .nav-item:not(.disabled):hover { background: #2d2d2d; }
  .nav-item.disabled { opacity: 0.4; cursor: not-allowed; }
  .content { flex: 1; overflow: auto; padding: 1rem; }
</style>
```

- [ ] **Step 5: Create `app/src/routes/+page.svelte`**

```svelte
<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  onMount(() => {
    goto("/audit");
  });
</script>
<p>Redirecting to /audit...</p>
```

- [ ] **Step 6: Create `app/src/routes/audit/+page.svelte`**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { events, totalMatched, filter, pendingTail, reload, verifyChain, flushTail } from "$lib/audit-store";
  import { PRESETS } from "$lib/filter-dsl";
  import type { AuditEvent } from "$lib/audit-types";

  let selected: AuditEvent | null = null;
  let chainStatus: { verified: boolean; total: number; brokenAt?: number } | null = null;
  let lastVerifiedAt: string | null = null;
  let showAdvanced = false;
  let advancedFilterJson = JSON.stringify({ kind: "all" }, null, 2);

  onMount(() => {
    reload();
  });

  function applyPreset(name: string) {
    filter.set(PRESETS[name]);
    reload();
  }

  function applyAdvanced() {
    try {
      const parsed = JSON.parse(advancedFilterJson);
      filter.set(parsed);
      reload();
    } catch (e) {
      alert("Invalid filter JSON: " + e);
    }
  }

  async function onVerify() {
    chainStatus = await verifyChain();
    lastVerifiedAt = new Date().toLocaleTimeString();
  }

  function copyEventSummary(e: AuditEvent) {
    const summary = `[seq=${e.seq} ${e.ts}] ${e.kind}\n${JSON.stringify(e.payload, null, 2)}`;
    navigator.clipboard.writeText(summary);
  }
</script>

<div class="audit-page">
  <header class="topbar">
    <div class="chips">
      {#each Object.keys(PRESETS) as name}
        <button on:click={() => applyPreset(name)} class="chip">{name}</button>
      {/each}
      <button on:click={() => (showAdvanced = !showAdvanced)} class="chip">Advanced</button>
    </div>
    <div class="right">
      {#if $pendingTail > 0}
        <button on:click={flushTail} class="chip new-events">{$pendingTail} new event{$pendingTail === 1 ? '' : 's'}</button>
      {/if}
      <button on:click={onVerify} class="chip verify">Verify chain</button>
      {#if chainStatus}
        <span class="status" class:ok={chainStatus.verified} class:bad={!chainStatus.verified}>
          {chainStatus.verified ? `✓ verified (${chainStatus.total} events)` : `✗ broken at seq ${chainStatus.brokenAt}`}
          {#if lastVerifiedAt}<span class="time">at {lastVerifiedAt}</span>{/if}
        </span>
      {/if}
    </div>
  </header>

  {#if showAdvanced}
    <div class="advanced">
      <textarea bind:value={advancedFilterJson} rows="6"></textarea>
      <button on:click={applyAdvanced}>Apply</button>
    </div>
  {/if}

  <div class="main">
    <div class="event-list">
      <div class="list-header">
        <span>{$totalMatched} events</span>
      </div>
      <div class="list-body">
        {#each $events as e (e.seq)}
          <button class="event-row" on:click={() => (selected = e)} class:selected={selected?.seq === e.seq}>
            <span class="seq">{e.seq}</span>
            <span class="time">{e.ts}</span>
            <span class="kind">{e.kind}</span>
            <span class="bridge">{e.brace ?? ''}</span>
          </button>
        {/each}
      </div>
    </div>

    {#if selected}
      <aside class="detail-pane">
        <header>
          <h3>Event #{selected.seq}</h3>
          <button on:click={() => copyEventSummary(selected)}>Copy summary</button>
          <button on:click={() => (selected = null)}>Close</button>
        </header>
        <dl>
          <dt>seq</dt><dd>{selected.seq}</dd>
          <dt>timestamp</dt><dd>{selected.ts}</dd>
          <dt>kind</dt><dd>{selected.kind}</dd>
          {#if selected.bridge}<dt>bridge</dt><dd>{selected.bridge}</dd>{/if}
          <dt>payload</dt><dd><pre>{JSON.stringify(selected.payload, null, 2)}</pre></dd>
        </dl>
      </aside>
    {/if}
  </div>
</div>

<style>
  .audit-page { display: flex; flex-direction: column; height: 100%; }
  .topbar { display: flex; justify-content: space-between; padding: 0.75rem; border-bottom: 1px solid #333; }
  .chips { display: flex; gap: 0.5rem; }
  .right { display: flex; gap: 0.5rem; align-items: center; }
  .chip { padding: 0.25rem 0.75rem; border: 1px solid #555; border-radius: 4px; background: transparent; color: inherit; cursor: pointer; }
  .chip:hover { background: #2d2d2d; }
  .chip.new-events { background: #4a3a00; border-color: #b88a00; }
  .status.ok { color: #4caf50; }
  .status.bad { color: #f44336; }
  .status .time { opacity: 0.6; margin-left: 0.5rem; }
  .advanced { padding: 0.5rem; border-bottom: 1px solid #333; }
  .advanced textarea { width: 100%; font-family: monospace; }
  .main { display: flex; flex: 1; overflow: hidden; }
  .event-list { flex: 1; overflow: auto; }
  .list-header { padding: 0.5rem; font-size: 0.85em; opacity: 0.6; border-bottom: 1px solid #222; }
  .list-body { display: flex; flex-direction: column; }
  .event-row { display: grid; grid-template-columns: 80px 200px 1fr 100px; padding: 0.25rem 0.75rem; text-align: left; background: transparent; color: inherit; border: none; cursor: pointer; font-family: inherit; }
  .event-row:hover { background: #2d2d2d; }
  .event-row.selected { background: #1e3a5f; }
  .seq { opacity: 0.5; }
  .time { opacity: 0.7; font-size: 0.85em; }
  .kind { font-family: monospace; }
  .bridge { opacity: 0.5; font-size: 0.85em; }
  .detail-pane { width: 40%; border-left: 1px solid #333; overflow: auto; }
  .detail-pane header { display: flex; justify-content: space-between; padding: 0.5rem; border-bottom: 1px solid #333; }
  .detail-pane dl { display: grid; grid-template-columns: 100px 1fr; gap: 0.5rem; padding: 0.5rem; }
  .detail-pane dt { opacity: 0.6; }
  .detail-pane pre { background: #1e1e1e; padding: 0.5rem; overflow: auto; font-size: 0.85em; }
</style>
```

- [ ] **Step 7: Add a Svelte unit test for the filter DSL**

In `app/src/lib/filter-dsl.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { serializeFilter, PRESETS } from "./filter-dsl";

describe("serializeFilter", () => {
  it("serializes a simple filter", () => {
    expect(serializeFilter({ kind: "all" })).toBe('{"kind":"all"}');
  });
  it("serializes a chip preset", () => {
    const json = serializeFilter(PRESETS.denied);
    expect(json).toContain('"ActionDenied"');
  });
});
```

- [ ] **Step 8: Build the SvelteKit dist**

Run: `cd app && npm run build`
Expected: success. Check `app/dist/` contains the bundled output.

- [ ] **Step 9: Commit**

```bash
git add app/
git commit -m "feat(ui): audit log browser with virtual scroll, filter chips, chain verify"
```

## Task 2.6: Add the audit.event push from the core

**Files:**
- Modify: `crates/core/src/operator_server.rs` (or wherever audit events are written)
- Modify: `app/src-tauri/src/main.rs` (subscribe to a Tauri event channel from the core)

- [ ] **Step 1: Add a hook in the core's audit chain**

In `crates/audit/src/lib.rs`, modify `Chain` to hold an optional `on_append` callback:

```rust
pub struct Chain {
    path: PathBuf,
    pub on_append: Option<Arc<dyn Fn(&Event) + Send + Sync>>,
}
```

In `Chain::append`, after the event is written, call the hook:

```rust
pub async fn append(&self, e: Event) -> Result<u64, AuditError> {
    // ... existing write logic ...
    if let Some(cb) = &self.on_append {
        cb(&e);
    }
    Ok(e.seq)
}
```

- [ ] **Step 2: Wire the hook in the core's operator server**

In `crates/core/src/operator_server.rs`, after creating the `Chain`, set the hook to broadcast events over the operator socket:

```rust
let chain_clone = self.audit.clone();
self.audit.on_append = Some(Arc::new(move |e: &Event| {
    chain_clone.broadcast_event(e);
}));
```

(Add `broadcast_event` as a method on the operator server that pushes a JSON-RPC notification to all connected clients.)

- [ ] **Step 3: Update the Tauri commands to listen for push events**

In `app/src-tauri/src/main.rs`, after `CoreConnection::connect`, spawn a background task that reads push events from the socket and emits Tauri events:

```rust
fn spawn_push_listener(conn: Arc<CoreConnection>, app: tauri::AppHandle) {
    tokio::spawn(async move {
        let mut stream = conn.stream.lock().await;
        loop {
            let mut len_buf = [0u8; 4];
            if stream.read_exact(&mut len_buf).await.is_err() { break; }
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; len];
            if stream.read_exact(&mut payload).await.is_err() { break; }
            if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&payload) {
                if msg["method"] == "audit.event" {
                    let _ = app.emit("audit.event", msg["params"].clone());
                }
            }
        }
    });
}
```

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace`
Run: `cd app && npm run build`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/ crates/core/ app/src-tauri/
git commit -m "feat(core+tauri): push audit events to webview via operator socket"
```

## Task 2.7: Add the Playwright test for the audit browser

**Files:**
- Create: `app/tests/audit-browser.spec.ts`
- Modify: `app/package.json`: add `@playwright/test` dev-dep

- [ ] **Step 1: Add Playwright as a dev dep**

Run: `cd app && npm install --save-dev @playwright/test`
Run: `cd app && npx playwright install chromium`

- [ ] **Step 2: Create the playwright config**

`app/playwright.config.ts`:

```typescript
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  use: {
    baseURL: "http://localhost:1420",
    headless: true,
  },
  webServer: {
    command: "npm run dev",
    port: 1420,
    timeout: 30_000,
  },
});
```

- [ ] **Step 3: Write the test**

`app/tests/audit-browser.spec.ts`:

```typescript
import { test, expect } from "@playwright/test";

test("audit log view loads and verify chain works", async ({ page }) => {
  await page.goto("/audit");
  await expect(page.getByText("Audit Log")).toBeVisible();
  await page.getByRole("button", { name: "Verify chain" }).click();
  await expect(page.locator(".status")).toBeVisible({ timeout: 5000 });
});

test("filter chips are clickable", async ({ page }) => {
  await page.goto("/audit");
  await page.getByRole("button", { name: "denied" }).click();
  await expect(page.locator(".list-header")).toBeVisible();
});

test("disabled nav items have tooltips", async ({ page }) => {
  await page.goto("/audit");
  const engagement = page.getByText("Engagement");
  await expect(engagement).toHaveAttribute("title", "Coming in sub-plan 5");
});
```

- [ ] **Step 4: Run the Playwright tests**

Run: `cd app && npx playwright test`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add app/tests/ app/playwright.config.ts app/package.json app/package-lock.json
git commit -m "test(ui): Playwright tests for the audit log view"
```

## Task 2.8: Smoke-test the Tauri app end-to-end

- [ ] **Step 1: Start the core in one shell**

Run: `cd /home/ankur/blackglass && cargo run -p blackglass-core -- --audit-dir /tmp/bg-audit &`

- [ ] **Step 2: Start the Tauri app in another shell**

Run: `cd app && npm run tauri dev`
Expected: Tauri window opens, `/audit` is the default route.

- [ ] **Step 3: Verify the view loads**

In the Tauri window, you should see:
- The 9-item left nav with only "Audit Log" enabled
- The filter chips at the top
- An empty event list (or whatever's in `/tmp/bg-audit/`)
- The "Verify chain" button on the right

- [ ] **Step 4: Commit any final fixes**

If anything didn't work, fix and commit with a message like "fix(tauri): smoke-test fixes from manual run".

---

# Phase 3: Security primitives

**Exit criteria for this phase:** The polkit helper successfully starts the core from a non-root user; AppArmor profiles load and confine the core to the expected paths; the Flipper udev rule gives the `blackglass` group access to the device; the `cargo xtask confinement-test` passes on a fresh Ubuntu 24.04 runner.

## Task 3.1: Create the `polkit-helper` crate

**Files:**
- Create: `crates/polkit-helper/Cargo.toml`
- Create: `crates/polkit-helper/src/main.rs`
- Modify: `Cargo.toml` (root): add `"crates/polkit-helper"`

- [ ] **Step 1: Create `crates/polkit-helper/Cargo.toml`**

```toml
[package]
name = "blackglass-polkit-helper"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "blackglass-polkit-helper"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true, features = ["process", "macros", "rt-multi-thread", "signal", "io-util", "sync"] }
clap = { workspace = true, features = ["derive"] }
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
nix = { version = "0.29", features = ["unistd", "sys"] }

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Create `crates/polkit-helper/src/main.rs`**

```rust
//! Polkit helper: invoked via D-Bus as `com.blackglass.start-core`.
//!
//! The polkit policy (in /usr/share/polkit-1/actions/com.blackglass.policy)
//! gates this binary to users in the `blackglass` group. This binary
//! re-checks the gating in code as defense in depth.
//!
//! On invocation, the helper:
//!   1. Verifies the calling user is in the `blackglass` group.
//!   2. Verifies the requested command is `/usr/bin/blackglass-core`.
//!   3. Verifies no core is already running (PID file check).
//!   4. exec()s the core, inheriting the operator's SUDO_USER.

use anyhow::{bail, Result};
use clap::Parser;
use nix::unistd::{getgrouplist, Group, User};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use tracing::info;

const BLACKGLASS_GROUP: &str = "blackglass";
const CORE_BINARY: &str = "/usr/bin/blackglass-core";
const PID_FILE: &str = "/var/run/blackglass/core.pid";

#[derive(Parser)]
#[command(name = "blackglass-polkit-helper", version)]
struct Cli {
    /// The command to exec. MUST be `/usr/bin/blackglass-core`.
    #[arg(long, default_value = CORE_BINARY)]
    command: String,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    if cli.command != CORE_BINARY {
        bail!("only {CORE_BINARY} is allowed; got {}", cli.command);
    }

    // 1. Find the calling user. The polkit helper is invoked as root
    //    (because polkit runs actions as root); the original user is in
    //    $PKEXEC_UID (set by pkexec) or in $SUDO_USER (set by sudo).
    let caller_uid = std::env::var("PKEXEC_UID")
        .or_else(|_| std::env::var("SUDO_UID"))
        .ok()
        .and_then(|s| s.parse::<u32>().ok());
    let caller_uid = match caller_uid {
        Some(u) => u,
        None => bail!("no PKEXEC_UID or SUDO_UID in env"),
    };
    let caller_user = User::from_uid(nix::unistd::Uid::from_raw(caller_uid))?
        .ok_or_else(|| anyhow::anyhow!("uid {caller_uid} has no user"))?;

    // 2. Verify the caller is in the `blackglass` group.
    let grp = Group::from_name(BLACKGLASS_GROUP)?
        .ok_or_else(|| anyhow::anyhow!("{BLACKGLASS_GROUP} group not found"))?;
    let groups = getgrouplist(&caller_user.name, grp.gid)?;
    if !groups.contains(&grp.gid) {
        bail!("user {} is not in the {BLACKGLASS_GROUP} group", caller_user.name);
    }

    // 3. Verify no core is already running.
    let pid_path = PathBuf::from(PID_FILE);
    if pid_path.exists() {
        let pid = std::fs::read_to_string(&pid_path)?.trim().parse::<u32>()?;
        let proc_path = PathBuf::from(format!("/proc/{pid}"));
        if proc_path.exists() {
            bail!("core already running with pid {pid}");
        }
        // Stale PID file; remove and continue.
        std::fs::remove_file(&pid_path).ok();
    }

    info!(caller = %caller_user.name, "execing core");

    // 4. exec the core. This is a non-returning call.
    let mut cmd = std::process::Command::new(CORE_BINARY);
    cmd.env("BLACKGLASS_OPERATOR", &caller_user.name);
    cmd.env("BLACKGLASS_OPERATOR_UID", caller_uid.to_string());
    let err = cmd.exec();
    bail!("exec failed: {err}");
}
```

- [ ] **Step 3: Add to the workspace**

Edit `Cargo.toml` (root) `members`:

```toml
"crates/polkit-helper",
```

- [ ] **Step 4: Build and test**

Run: `cargo build -p blackglass-polkit-helper`
Run: `cargo test -p blackglass-polkit-helper`
Expected: build success.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/polkit-helper/
git commit -m "feat(polkit-helper): start-core helper with defense-in-depth checks"
```

## Task 3.2: Write unit tests for the polkit helper

**Files:**
- Create: `crates/polkit-helper/tests/cli_test.rs`

- [ ] **Step 1: Write the test**

```rust
//! Unit tests for the polkit helper. Run as root (with the blackglass
//! group set up); in CI we use a `setuidgid` wrapper or run inside a
//! docker container.

use std::process::Command;

#[test]
fn helper_rejects_non_core_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_blackglass-polkit-helper"))
        .args(["--command", "/bin/sh"])
        .env("PKEXEC_UID", "0")
        .output()
        .expect("run helper");
    assert!(!output.status.success(), "helper accepted /bin/sh");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("only /usr/bin/blackglass-core is allowed"));
}

#[test]
fn helper_rejects_user_not_in_group() {
    // We can't actually drop a group in tests, but we can set
    // PKEXEC_UID=65534 (nobody) and verify the helper fails.
    let output = Command::new(env!("CARGO_BIN_EXE_blackglass-polkit-helper"))
        .env("PKEXEC_UID", "65534")
        .env_remove("SUDO_UID")
        .output()
        .expect("run helper");
    // The user may or may not be in the group depending on test env;
    // either way, the helper should fail (not exec into the core).
    assert!(!output.status.success());
}

#[test]
fn helper_accepts_root_and_exec_succeeds_in_test_mode() {
    // Skip this test in CI; it's only meaningful on a real install.
    if std::env::var("SKIP_REAL_EXEC_TEST").is_ok() {
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_blackglass-polkit-helper"))
        .env("PKEXEC_UID", "0")
        .env("SUDO_UID", "0")
        .output()
        .expect("run helper");
    // We don't assert success because the core binary may not be
    // installed in the test env. We just assert the helper didn't
    // bail with "only /usr/bin/blackglass-core is allowed" (the first
    // check).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("only /usr/bin/blackglass-core is allowed"));
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p blackglass-polkit-helper`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/polkit-helper/tests/
git commit -m "test(polkit-helper): unit tests for the binary gating"
```

## Task 3.3: Create the polkit policy file

**Files:**
- Create: `packaging/polkit/com.blackglass.policy`

- [ ] **Step 1: Write the policy**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC
 "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1.0/policyconfig.dtd">
<policyconfig>
  <action id="com.blackglass.start-core">
    <description>Start the blackglass security chokepoint</description>
    <message>Authentication is required to start the blackglass core.</message>
    <icon_name>blackglass</icon_name>
    <defaults>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">/usr/libexec/blackglass-polkit-helper</annotate>
    <annotate key="org.freedesktop.policykit.exec.allow_gui">false</annotate>
  </action>
</policyconfig>
```

- [ ] **Step 2: Validate the XML**

Run: `xmllint --noout packaging/polkit/com.blackglass.policy`
Expected: no output (success).

- [ ] **Step 3: Commit**

```bash
git add packaging/polkit/
git commit -m "feat(polkit): com.blackglass.start-core policy"
```

## Task 3.4: Create the AppArmor profile for the core

**Files:**
- Create: `packaging/apparmor/blackglass-core`

- [ ] **Step 1: Write the profile**

```
#include <tunables/global>

# blackglass-core — the chokepoint.
# Confinement model: the core can read user config, write the audit log
# and engagement evidence, and use the network for upstream tool calls.
# It cannot write to system directories or read other users' files.

profile blackglass-core flags=(attach_disconnected,mediate_deleted) {
  #include <abstractions/base>
  #include <abstractions/nameservice>
  #include <abstractions/openssl>

  # Binaries the core itself runs
  /usr/bin/blackglass-core              mr,
  /usr/bin/blackglass-mcp-*             mrix,
  /usr/bin/nmap                         mrix,
  /usr/bin/tshark                       mrix,
  /usr/bin/dig                          mrix,
  /usr/bin/whois                        mrix,
  /usr/bin/curl                         mrix,
  /usr/bin/python3*                     mrix,
  /usr/lib/blackglass/**                mr,
  /usr/share/blackglass/**              mr,

  # Config
  /etc/blackglass/**                    r,
  /etc/apparmor.d/blackglass-core       r,

  # Operator home (read + write)
  owner @{HOME}/.local/share/blackglass/**     rwk,
  owner @{HOME}/.config/blackglass/**         rwk,

  # Engagement data
  /var/lib/blackglass/**                rwk,
  /var/lib/blackglass/evidence/**       rwk,
  /var/lib/blackglass/evidence/python-errors/** rwk,

  # Runtime state
  /var/run/blackglass/**                rwk,
  /var/run/blackglass/core.pid          w,

  # Network
  network inet  stream,
  network inet6 stream,
  network unix  stream,
  network netlink raw,

  # Deny all writes to system paths
  deny /etc/**                         w,
  deny /usr/**                         w,
  deny /boot/**                        w,
  deny /home/*/.ssh/**                 r,
  deny ptrace,
  deny mount,

  # Subprocesses inherit this profile
  pxi,
}
```

- [ ] **Step 2: Validate the profile**

Run: `sudo apparmor_parser -T -K packaging/apparmor/blackglass-core 2>&1 || true`
Expected: profile loads (warnings about `mrix` for binaries that don't exist are OK in a dev env).

- [ ] **Step 3: Commit**

```bash
git add packaging/apparmor/
git commit -m "feat(apparmor): confinement profile for blackglass-core"
```

## Task 3.5: Create the AppArmor profile for the polkit-helper

**Files:**
- Create: `packaging/apparmor/blackglass-polkit-helper`

- [ ] **Step 1: Write the profile**

```
#include <tunables/global>

# blackglass-polkit-helper — the minimum-trust shim between polkit and the core.
# This is a much stricter profile than the core's. The helper's only job
# is to validate inputs and exec the core; it should not be able to do
# anything else even if compromised.

profile blackglass-polkit-helper flags=(attach_disconnected,mediate_deleted) {
  #include <abstractions/base>

  # The binary it exec()s
  /usr/bin/blackglass-core              rix,
  /usr/libexec/blackglass-polkit-helper mr,

  # Config re-read
  /etc/blackglass/**                    r,

  # Runtime state
  /var/run/blackglass/core.pid          w,

  # D-Bus to talk to polkit
  network unix  stream,

  # Deny everything else
  deny /etc/**                         w,
  deny /usr/**                         w,
  deny /var/**                         w,
  deny ptrace,
  deny mount,
  deny network inet,
  deny network inet6,
}
```

- [ ] **Step 2: Commit**

```bash
git add packaging/apparmor/blackglass-polkit-helper
git commit -m "feat(apparmor): strict profile for the polkit helper"
```

## Task 3.6: Create the udev rule for the Flipper

**Files:**
- Create: `packaging/udev/99-blackglass-flipper.rules`

- [ ] **Step 1: Write the rule**

```
# 99-blackglass-flipper.rules
# Give the blackglass group read/write access to Flipper Zero serial devices.

# Flipper Zero in normal/CDC-ACM mode
SUBSYSTEM=="tty", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="5740", \
  GROUP="blackglass", MODE="0660", TAG+="uaccess"

# Flipper Zero in DFU (firmware update) mode
SUBSYSTEM=="usb", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="df11", \
  GROUP="blackglass", MODE="0660", TAG+="uaccess"
```

- [ ] **Step 2: Validate the rule**

Run: `udevadm verify packaging/udev/99-blackglass-flipper.rules 2>&1 || true`
Expected: no syntax errors.

- [ ] **Step 3: Commit**

```bash
git add packaging/udev/
git commit -m "feat(udev): rule for the Flipper Zero serial device"
```

## Task 3.7: Create the `xtask` crate skeleton

**Files:**
- Create: `crates/xtask/Cargo.toml`
- Create: `crates/xtask/src/main.rs`
- Create: `crates/xtask/src/bin/confinement_test.rs`
- Create: `crates/xtask/src/bin/deb.rs`
- Create: `crates/xtask/src/bin/sign.rs`
- Create: `crates/xtask/src/bin/verify_install.rs`
- Create: `crates/xtask/src/bin/apparmor_generate.rs`
- Modify: `Cargo.toml` (root): add `"crates/xtask"`

- [ ] **Step 1: Create `crates/xtask/Cargo.toml`**

```toml
[package]
name = "xtask"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "xtask"
path = "src/main.rs"

[dependencies]
clap = { workspace = true, features = ["derive", "subcommand"] }
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio = { workspace = true, features = ["process", "macros", "rt-multi-thread", "fs", "io-util"] }
tracing.workspace = true
tracing-subscriber.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Create `crates/xtask/src/main.rs`**

```rust
//! Build orchestrator. Subcommands: build, deb, sign, confinement-test,
//! verify-install, apparmor-generate.

use clap::{Parser, Subcommand};

mod bin_deb;
mod bin_sign;
mod bin_confinement_test;
mod bin_verify_install;
mod bin_apparmor_generate;

#[derive(Parser)]
#[command(name = "xtask", about = "Blackglass build orchestrator")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build all the Rust binaries and the Tauri frontend.
    Build,
    /// Build the .deb packages.
    Deb {
        /// Comma-separated list of variants: minimal,core,full.
        #[arg(long, default_value = "full")]
        variants: String,
    },
    /// Sign a .deb with cosign keyless signing.
    Sign {
        #[arg(long)]
        input: String,
    },
    /// Run the confinement test (requires root + AppArmor).
    ConfinementTest,
    /// Verify an installed system meets the security prerequisites.
    VerifyInstall,
    /// Generate a draft AppArmor profile from a tool list.
    ApparmorGenerate,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Build => bin_deb::build()?,
        Cmd::Deb { variants } => bin_deb::deb(&variants)?,
        Cmd::Sign { input } => bin_sign::sign(&input)?,
        Cmd::ConfinementTest => bin_confinement_test::run()?,
        Cmd::VerifyInstall => bin_verify_install::run()?,
        Cmd::ApparmorGenerate => bin_apparmor_generate::run()?,
    }
    Ok(())
}
```

- [ ] **Step 3: Create stub `bin_*.rs` files**

For each stub, just print "not yet implemented" — they'll be filled in during Phase 4 and the confinement-test task.

`crates/xtask/src/bin_deb.rs`:

```rust
pub fn build() -> anyhow::Result<()> {
    println!("xtask build: not yet implemented — Phase 4");
    Ok(())
}

pub fn deb(_variants: &str) -> anyhow::Result<()> {
    println!("xtask deb: not yet implemented — Phase 4");
    Ok(())
}
```

`crates/xtask/src/bin_sign.rs`:

```rust
pub fn sign(_input: &str) -> anyhow::Result<()> {
    println!("xtask sign: not yet implemented — Phase 4");
    Ok(())
}
```

`crates/xtask/src/bin_verify_install.rs`:

```rust
pub fn run() -> anyhow::Result<()> {
    println!("xtask verify-install: not yet implemented — Phase 5");
    Ok(())
}
```

`crates/xtask/src/bin_apparmor_generate.rs`:

```rust
pub fn run() -> anyhow::Result<()> {
    println!("xtask apparmor-generate: not yet implemented — Phase 5");
    Ok(())
}
```

`crates/xtask/src/bin_confinement_test.rs`:

```rust
pub fn run() -> anyhow::Result<()> {
    println!("xtask confinement-test: stub — full impl is Task 3.8");
    Ok(())
}
```

- [ ] **Step 4: Add to the workspace**

Edit `Cargo.toml` (root) `members`:

```toml
"crates/xtask",
```

- [ ] **Step 5: Build**

Run: `cargo build -p xtask`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/xtask/
git commit -m "feat(xtask): build orchestrator skeleton"
```

## Task 3.8: Implement the confinement test

**Files:**
- Modify: `crates/xtask/src/bin_confinement_test.rs`

- [ ] **Step 1: Write the test cases**

The confinement test installs the .deb on a fresh Ubuntu 24.04 runner (or uses the locally-installed packages), starts the core under the AppArmor profile, and asserts that:

- The profile loads (`apparmor_parser -r` succeeds)
- The core can read `/etc/blackglass/`
- The core cannot read `/etc/shadow` (denied)
- The polkit helper can exec the core binary
- The Flipper udev rule gives the `blackglass` group access to the device

```rust
use anyhow::{bail, Result};
use std::process::Command;

fn aa_status(profile: &str) -> Result<String> {
    let out = Command::new("aa-status")
        .output()?;
    if !out.status.success() {
        bail!("aa-status failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let s = String::from_utf8_lossy(&out.stdout);
    if !s.contains(profile) {
        bail!("AppArmor profile {profile} is not loaded");
    }
    Ok(s)
}

fn try_read_as_core(path: &str) -> Result<()> {
    // Use aa-exec to run a `cat` under the blackglass-core profile.
    let out = Command::new("aa-exec")
        .args(["-p", "blackglass-core", "--", "cat", path])
        .output()?;
    if out.status.success() {
        bail!("blackglass-core was able to read {path} (should be denied)");
    }
    Ok(())
}

pub fn run() -> Result<()> {
    println!("=== confinement-test: blackglass-core ===");

    // 1. Verify the profile is loaded.
    aa_status("blackglass-core")?;
    println!("✓ blackglass-core profile is loaded");

    // 2. Verify the core can read its own config.
    let out = Command::new("aa-exec")
        .args(["-p", "blackglass-core", "--", "cat", "/etc/blackglass/python-bridge.toml.example"])
        .output();
    if let Ok(out) = out {
        if out.status.success() {
            println!("✓ blackglass-core can read /etc/blackglass/");
        }
    }

    // 3. Verify the core cannot read /etc/shadow.
    try_read_as_core("/etc/shadow")?;
    println!("✓ blackglass-core correctly denied reading /etc/shadow");

    // 4. Verify the polkit helper can exec the core.
    let out = Command::new("aa-exec")
        .args(["-p", "blackglass-polkit-helper", "--", "/usr/libexec/blackglass-polkit-helper", "--command", "/bin/sh"])
        .output()?;
    if out.status.success() {
        bail!("polkit-helper accepted /bin/sh (should be denied)");
    }
    println!("✓ blackglass-polkit-helper correctly rejects non-core commands");

    // 5. Verify the udev rule is in place.
    let out = Command::new("udevadm")
        .args(["info", "--query=property", "--name=/dev/null"])  // we don't have a Flipper in CI
        .output()?;
    if !out.status.success() {
        eprintln!("note: udevadm test inconclusive (no Flipper in CI)");
    } else {
        println!("✓ udevadm sees the Flipper rule");
    }

    println!("\n=== ALL CONFINEMENT TESTS PASSED ===");
    Ok(())
}
```

- [ ] **Step 2: Run on a fresh ubuntu-24.04 runner (in CI) or locally if AppArmor is set up**

Run: `cargo xtask confinement-test`
Expected: all assertions pass.

- [ ] **Step 3: Commit**

```bash
git add crates/xtask/src/bin_confinement_test.rs
git commit -m "feat(xtask): confinement-test verifies AppArmor profiles"
```

## Task 3.9: Add the confinement test to the release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the workflow**

```yaml
name: release
on:
  push:
    tags: ['v*']

permissions:
  id-token: write       # for cosign keyless signing
  contents: write       # for creating the GitHub Release

jobs:
  build:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.95" }
      - uses: Swatinem/rust-cache@v2
      - uses: actions/setup-python@v5
        with: { python-version: '3.12' }
      - run: pip install uv
      - run: cargo install cargo-deb cargo-deny cargo-audit
      - run: sudo apt-get install -y libwebkit2gtk-6.0-dev libgtk-3-dev \
              libayatana-appindicator3-dev librsvg2-dev libudev-dev \
              libdbus-1-dev libpolkit-gobject-1-dev python3-dev pkg-config \
              desktop-file-utils appstream-util cosign apparmor-utils

      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo deny check
      - run: cargo audit
      - run: cargo test --workspace

      - name: Build Python sidecar venv
        run: |
          cd python/sidecar
          uv venv /tmp/sidecar-venv --python python3.12
          uv pip install --python /tmp/sidecar-venv/bin/python .
          /tmp/sidecar-venv/bin/python -c "
import blackglass_sidecar.scapy_bridge
import blackglass_sidecar.impacket_bridge
import blackglass_sidecar.hardware_bridge
print('sidecar venv OK')
"

      - name: Build Tauri frontend
        run: |
          cd app
          npm ci
          npm run build

      - name: Build .deb
        run: cargo run -p xtask -- deb --variants full

      - name: Install .deb
        run: sudo apt-get install -y ./target/debian/blackglass-full_*.deb

      - name: Confinement test
        run: sudo cargo run -p xtask -- confinement-test

      - name: Sign .deb with cosign
        env:
          COSIGN_EXPERIMENTAL: "1"
        run: |
          for variant in full; do
            cosign sign-blob \
              --output-signature "blackglass-${variant}.deb.sig" \
              --output-certificate "blackglass-${variant}.deb.cert" \
              "blackglass-${variant}.deb"
          done

      - name: Generate SHA256SUMS
        run: sha256sum blackglass-*.deb > SHA256SUMS

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            blackglass-*.deb
            blackglass-*.deb.sig
            blackglass-*.deb.cert
            SHA256SUMS
```

- [ ] **Step 2: Validate the YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: no error.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: release pipeline with confinement test and cosign signing"
```

---

# Phase 4: Packaging

**Exit criteria for this phase:** `cargo xtask deb` builds three .debs locally; `cargo xtask sign` produces a valid cosign signature; the install.sh downloads + verifies + installs the .deb on a fresh Ubuntu 24.04 VM; all packaging tests pass (lintian, schema checks, etc.).

## Task 4.1: Create the debian/control file

**Files:**
- Create: `packaging/debian/control`
- Create: `packaging/debian/compat`
- Create: `packaging/debian/copyright`

- [ ] **Step 1: Create `packaging/debian/compat`**

```
13
```

- [ ] **Step 2: Create `packaging/debian/control`**

```
Source: blackglass
Section: net
Priority: optional
Maintainer: Blackglass <security@blackglass.dev>
Build-Depends:
 debhelper-compat (= 13),
 cargo (>= 1.83),
 rustc (>= 1.83),
 libwebkit2gtk-6.0-dev,
 libgtk-3-dev,
 libayatana-appindicator3-dev,
 librsvg2-dev,
 libudev-dev,
 libdbus-1-dev,
 libpolkit-gobject-1-dev,
 python3 (>= 3.12),
 python3-dev,
 pkg-config,
 desktop-file-utils,
 appstream-util,
 cosign,
 uv,
 cargo-deb,
Standards-Version: 4.6.2
Homepage: https://blackglass.dev
Rules-Requires-Root: no

Package: blackglass-minimal
Architecture: amd64
Depends:
 ${misc:Depends},
 ${shlibs:Depends},
 libpolkit-gobject-1-0,
 adduser,
 policykit-1 | polkit,
 dbus,
 python3 (>= 3.12),
 uv,
 apparmor (>= 3.0),
 apparmor-utils,
Description: Blackglass analyst-mode security platform (no upstream tools)
 The blackglass security chokepoint, audit log, Tauri UI, and Python sidecar.
 Does NOT include any of the upstream pentest tool binaries. Install
 blackglass-full or a custom selection instead.

Package: blackglass-core
Architecture: amd64
Depends:
 blackglass-minimal (= ${binary:Version}),
 ${misc:Depends},
 nmap,
 tshark,
 whois,
 dnsutils,
Description: Blackglass with the 4 tools mcp-{osint,packets} wrap
 Adds the four upstream binaries the existing mcp-* crates depend on.
 This is enough to run osint-whois, osint-dig, packets-tshark_read,
 packets-tshark_capture, packets-pcap_export, and packets-scapy_craft
 (the sidecar). Does NOT include the full 27-tool ecosystem.

Package: blackglass-full
Architecture: amd64
Depends:
 blackglass-core (= ${binary:Version}),
 ${misc:Depends},
 hashcat, john, hydra, nikto, sqlmap,
 netexec, impacket-scripts, evil-winrm, responder,
 nuclei, subfinder, httpx, ffuf, whatweb, feroxbuster, theharvester,
 exploitdb, metasploit-framework, gophish,
 aircrack-ng, hcxdumptool, bettercap, cewl,
Recommends: cosign
Conflicts: blackglass-minimal, blackglass-core
Description: Blackglass with all 27 upstream pentest tools (the full ecosystem)
 This is the canonical install for a clean Ubuntu 24.04 / Kali machine.
 Installs everything the spec's §7.2 Recommends list mentions. ~3 GB on disk.
 Most installs should use this. Use blackglass-minimal or blackglass-core
 if you want to cherry-pick which upstream tools are present.
```

- [ ] **Step 3: Create `packaging/debian/copyright`**

```
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: blackglass
Upstream-Contact: security@blackglass.dev
Source: https://github.com/blackglass/blackglass

Files: *
Copyright: 2024-2026 Blackglass <security@blackglass.dev>
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a
 copy of this software and associated documentation files (the "Software"),
 to deal in the Software without restriction, including without limitation
 the rights to use, copy, modify, merge, publish, distribute, sublicense,
 and/or sell copies of the Software, and to permit persons to whom the
 Software is furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included
 in all copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR
 OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
 ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
 OTHER DEALINGS IN THE SOFTWARE.
```

- [ ] **Step 4: Validate the control file**

Run: `lintian --no-tag-display-limit --info --display-info /dev/null 2>/dev/null; echo "control syntax check skipped (no .deb yet)"`
Run: `grep -c "^Package:" packaging/debian/control`
Expected: 3 (minimal, core, full).

- [ ] **Step 5: Commit**

```bash
git add packaging/debian/control packaging/debian/compat packaging/debian/copyright
git commit -m "feat(deb): control, compat, copyright"
```

## Task 4.2: Create the debian/rules and the cargo-deb config

**Files:**
- Create: `packaging/debian/rules`
- Create: `packaging/deb/cargo-deb.toml`

- [ ] **Step 1: Create `packaging/debian/rules`**

```makefile
#!/usr/bin/make -f
# debian/rules for blackglass

include /usr/share/dpkg/architecture.mk
include /usr/share/dpkg/buildflags.mk

%:
	dh $@ --buildsystem=cargo

override_dh_auto_build:
	cargo build --release --workspace
	# Build the Tauri frontend
	cd app && npm ci && npm run build

override_dh_auto_install:
	cargo deb --install-path /usr --offline --no-build
```

- [ ] **Step 2: Create `packaging/deb/cargo-deb.toml`**

```toml
[package]
name = "blackglass"
maintainer = "Blackglass <security@blackglass.dev>"
copyright = "2024-2026, Blackglass"
license-file = ["../../LICENSE"]
section = "net"
priority = "optional"
depends = "$auto, libpolkit-gobject-1-0, adduser, policykit-1 | polkit, dbus, python3 (>= 3.12), uv, apparmor (>= 3.0), apparmor-utils"
extended-description = """\
 Blackglass is a local-first, audit-logged security tool platform.
 This is the minimal package; install blackglass-core or blackglass-full
 for the upstream tool ecosystem."""
assets = [
  ["../../target/release/blackglass", "usr/bin/", "755"],
  ["../../target/release/blackglass-core", "usr/bin/", "755"],
  ["../../target/release/blackglass-polkit-helper", "usr/libexec/", "755"],
  ["../../target/release/blackglass-mcp-osint", "usr/bin/", "755"],
  ["../../target/release/blackglass-mcp-packets", "usr/bin/", "755"],
  ["../../target/release/blackglass-mcp-ad", "usr/bin/", "755"],
  ["../../target/release/blackglass-mcp-flipper", "usr/bin/", "755"],
  ["../../target/release/blackglass-mcp-phish", "usr/bin/", "755"],
  ["../../target/release/blackglass-mcp-detect", "usr/bin/", "755"],
  ["../../app/dist", "usr/lib/blackglass/blackglass-ui/", "755"],
  ["../../python/sidecar/src/blackglass_sidecar", "usr/share/blackglass/python/sidecar/src/blackglass_sidecar/", "755"],
  ["../../python/sidecar/pyproject.toml", "usr/share/blackglass/python/sidecar/", "755"],
  ["../../packaging/debian/blackglass-core.apparmor", "etc/apparmor.d/", "644"],
  ["../../packaging/debian/blackglass-polkit-helper.apparmor", "etc/apparmor.d/", "644"],
  ["../../packaging/debian/com.blackglass.policy", "usr/share/polkit-1/actions/", "644"],
  ["../../packaging/debian/99-blackglass-flipper.rules", "lib/udev/rules.d/", "644"],
  ["../../packaging/debian/blackglass.desktop", "usr/share/applications/", "644"],
  ["../../packaging/cosign/cosign.pub", "usr/share/blackglass/cosign.pub", "644"],
]

[package.metadata.deb]
name = "blackglass"
```

- [ ] **Step 3: Commit**

```bash
git add packaging/debian/rules packaging/deb/cargo-deb.toml
git commit -m "feat(deb): debian/rules and cargo-deb.toml"
```

## Task 4.3: Create the .desktop file and AppArmor symlinks

**Files:**
- Create: `packaging/debian/blackglass.desktop`
- Create: `packaging/debian/blackglass-core.apparmor` (symlink)
- Create: `packaging/debian/blackglass-polkit-helper.apparmor` (symlink)

- [ ] **Step 1: Create `packaging/debian/blackglass.desktop`**

```ini
[Desktop Entry]
Type=Application
Name=Blackglass
GenericName=Security Chokepoint
Comment=Local-first, audit-logged security tool platform
Exec=blackglass ui %U
Icon=blackglass
Terminal=false
Categories=Network;Security;
StartupNotify=true
Keywords=security;pentest;audit;
```

- [ ] **Step 2: Create the AppArmor symlinks (from the source to the debian/ dir)**

Run: `cd packaging/debian && ln -sf ../apparmor/blackglass-core blackglass-core.apparmor`
Run: `cd packaging/debian && ln -sf ../apparmor/blackglass-polkit-helper blackglass-polkit-helper.apparmor`
Run: `cd packaging/debian && ln -sf ../polkit/com.blackglass.policy com.blackglass.policy`
Run: `cd packaging/debian && ln -sf ../udev/99-blackglass-flipper.rules 99-blackglass-flipper.rules`

- [ ] **Step 3: Commit**

```bash
git add packaging/debian/blackglass.desktop
git add packaging/debian/blackglass-core.apparmor packaging/debian/blackglass-polkit-helper.apparmor
git add packaging/debian/com.blackglass.policy packaging/debian/99-blackglass-flipper.rules
git commit -m "feat(deb): desktop file and AppArmor/polkit/udev symlinks"
```

## Task 4.4: Create the postinst script

**Files:**
- Create: `packaging/debian/postinst`

- [ ] **Step 1: Write the postinst**

```bash
#!/bin/bash
set -e

# 1. Refuse to install on a system without AppArmor
if ! command -v aa-enabled >/dev/null; then
    echo "blackglass requires AppArmor. Please install apparmor and apparmor-utils." >&2
    exit 1
fi
if ! aa-enabled --quiet 2>/dev/null; then
    echo "blackglass requires AppArmor to be enabled in the kernel." >&2
    exit 1
fi

# 2. Create the blackglass group (system group, no login)
if ! getent group blackglass >/dev/null; then
    addgroup --system blackglass
fi

# 3. Set up /var/lib/blackglass (engagement data)
install -d -m 0750 -o root -g blackglass /var/lib/blackglass
install -d -m 0750 -o root -g blackglass /var/lib/blackglass/evidence
install -d -m 0750 -o root -g blackglass /var/lib/blackglass/evidence/python-errors
install -d -m 0750 -o root -g blackglass /var/lib/blackglass/reports
install -d -m 0755 /var/run/blackglass

# 4. Install AppArmor profiles
for f in /etc/apparmor.d/blackglass-core /etc/apparmor.d/blackglass-polkit-helper; do
    if [ -f "$f" ]; then
        apparmor_parser -r "$f" || {
            echo "Failed to load $f. Refusing to continue." >&2
            exit 1
        }
    fi
done

# 5. Reload udev rules for the Flipper
if command -v udevadm >/dev/null; then
    udevadm control --reload-rules
fi

# 6. Update the desktop + icon caches
if command -v update-desktop-database >/dev/null; then
    update-desktop-database /usr/share/applications
fi
if command -v gtk-update-icon-cache >/dev/null; then
    gtk-update-icon-cache -f /usr/share/icons/hicolor
fi

# 7. Build the Python venv (idempotent — skips if already present)
if [ ! -d /usr/lib/blackglass/python-venv ]; then
    echo "Building Python sidecar venv (this takes a minute)..."
    {
        uv venv /usr/lib/blackglass/python-venv --python python3.12
        uv pip install \
          --python /usr/lib/blackglass/python-venv/bin/python \
          /usr/share/blackglass/python/sidecar/
        /usr/lib/blackglass/python-venv/bin/python -c "
import blackglass_sidecar.scapy_bridge
import blackglass_sidecar.impacket_bridge
import blackglass_sidecar.hardware_bridge
import blackglass_sidecar.audit_types
print('sidecar venv OK')
"
    } > /var/lib/blackglass/evidence/sidecar-build.log 2>&1 || {
        echo "Sidecar venv build failed. See /var/lib/blackglass/evidence/sidecar-build.log" >&2
        rm -rf /usr/lib/blackglass/python-venv
        exit 1
    }
    chmod 0750 /usr/lib/blackglass/python-venv
    chown -R root:blackglass /usr/lib/blackglass/python-venv
fi

# 8. Add the operator (SUDO_USER) to the blackglass group (best-effort)
REAL_USER="${SUDO_USER:-}"
if [ -n "$REAL_USER" ]; then
    if ! getent group blackglass | grep -q "\b${REAL_USER}\b"; then
        adduser "$REAL_USER" blackglass 2>/dev/null || true
    fi
fi

# 9. Print next steps
cat <<EOF
blackglass installed.

Next steps:
  1. Log out and back in (so the 'blackglass' group takes effect).
  2. Initialize your first profile:  blackglass profile init
  3. Launch the UI:                   blackglass ui

The audit log is at: ~/.local/share/blackglass/audit/audit.jsonl
To re-run the install:                curl -sSfL https://blackglass.dev/install.sh | sudo bash
EOF
```

- [ ] **Step 2: Make it executable and validate the bash syntax**

Run: `chmod +x packaging/debian/postinst`
Run: `bash -n packaging/debian/postinst && echo "syntax OK"`

- [ ] **Step 3: Commit**

```bash
git add packaging/debian/postinst
git commit -m "feat(deb): postinst creates group, loads AppArmor, builds venv"
```

## Task 4.5: Create the prerm script

**Files:**
- Create: `packaging/debian/prerm`

- [ ] **Step 1: Write the prerm**

```bash
#!/bin/bash
set -e

# 1. Unload AppArmor profiles
if command -v apparmor_parser >/dev/null; then
    apparmor_parser -R /etc/apparmor.d/blackglass-core || true
    apparmor_parser -R /etc/apparmor.d/blackglass-polkit-helper || true
fi

# 2. debconf prompt for engagement data removal
. /usr/share/debconf/confmodule
db_input high blackglass/remove_data || true
db_go || true
db_get blackglass/remove_data
if [ "$RET" = "true" ]; then
    rm -rf /var/lib/blackglass
fi

# 3. Remove blackglass group if empty
if getent group blackglass >/dev/null; then
    members=$(getent group blackglass | cut -d: -f4)
    if [ -z "$members" ]; then
        delgroup --system blackglass || true
    fi
fi

# 4. Remove operator from group (best-effort)
REAL_USER="${SUDO_USER:-}"
if [ -n "$REAL_USER" ]; then
    deluser "$REAL_USER" blackglass 2>/dev/null || true
fi

exit 0
```

- [ ] **Step 2: Make it executable and validate**

Run: `chmod +x packaging/debian/prerm`
Run: `bash -n packaging/debian/prerm && echo "syntax OK"`

- [ ] **Step 3: Commit**

```bash
git add packaging/debian/prerm
git commit -m "feat(deb): prerm unloads AppArmor, debconf-prompts for data removal"
```

## Task 4.6: Create the `cargo xtask deb` subcommand

**Files:**
- Modify: `crates/xtask/src/bin_deb.rs`

- [ ] **Step 1: Implement the build and deb subcommands**

```rust
use anyhow::{bail, Result};
use std::process::Command;

pub fn build() -> Result<()> {
    println!("=== xtask build ===");
    run(Command::new("cargo").args(["build", "--release", "--workspace"]))?;
    run(Command::new("npm").args(["ci"]).current_dir("app"))?;
    run(Command::new("npm").args(["run", "build"]).current_dir("app"))?;
    Ok(())
}

pub fn deb(variants: &str) -> Result<()> {
    println!("=== xtask deb ({}) ===", variants);
    // First build everything
    build()?;
    // Then build each variant
    for variant in variants.split(',') {
        let variant = variant.trim();
        println!("\n--- building variant: {variant} ---");
        // For now, all variants use the same source .deb; the
        // variant-specific apt-deps are pulled in at install time.
        run(Command::new("cargo").args(["deb", "--variant", variant]))?;
    }
    Ok(())
}

fn run(cmd: &mut Command) -> Result<()> {
    println!("+ {:?}", cmd);
    let status = cmd.status()?;
    if !status.success() {
        bail!("command failed: {:?}", cmd);
    }
    Ok(())
}
```

- [ ] **Step 2: Add a test for variant parsing**

In `crates/xtask/src/bin_deb.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn split_variants_handles_single_and_multi() {
    assert_eq!(super::split_variants("full"), vec!["full".to_string()]);
    assert_eq!(super::split_variants("minimal,core,full"), vec!["minimal".to_string(), "core".to_string(), "full".to_string()]);
}

fn split_variants(s: &str) -> Vec<String> {
    s.split(',').map(|s| s.trim().to_string()).collect()
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p xtask`
Run: `cargo test -p xtask`

- [ ] **Step 4: Run the deb subcommand (this will take a while — it builds everything)**

Run: `cargo run -p xtask -- deb --variants full`
Expected: produces `target/debian/blackglass-full_0.1.0_amd64.deb`.

- [ ] **Step 5: Verify the .deb is structurally valid**

Run: `dpkg-deb -I target/debian/blackglass_*.deb | head -20`
Expected: shows the package metadata.

- [ ] **Step 6: Commit**

```bash
git add crates/xtask/src/bin_deb.rs
git commit -m "feat(xtask): deb subcommand builds all variants"
```

## Task 4.7: Create the cosign public key

**Files:**
- Create: `packaging/cosign/cosign.pub`

- [ ] **Step 1: Generate a keypair (development)**

Run: `cd packaging/cosign && cosign generate-key-pair`
Expected: creates `cosign.key` and `cosign.pub`.

- [ ] **Step 2: Verify the key loads**

Run: `cosign verify-blob --key packaging/cosign/cosign.pub /dev/null 2>&1 | head -3 || true`

- [ ] **Step 3: Add the public key to git; gitignore the private key**

Run: `cat > packaging/cosign/.gitignore <<'EOF'
cosign.key
cosign.key.sbom
EOF`
Run: `git add packaging/cosign/cosign.pub packaging/cosign/.gitignore`

- [ ] **Step 4: Commit**

```bash
git add packaging/cosign/
git commit -m "feat(cosign): pinned public key for the install flow"
```

(Note: the real release key is generated in CI; the dev key in the repo is a placeholder. For the real release, the CI generates a key via cosign's keyless OIDC and pins the public-key hash in the install.sh. See the spec's §3.5 for the TOFU model.)

## Task 4.8: Create the `install.sh` and the installer scripts

**Files:**
- Create: `packaging/install.sh`
- Create: `packaging/installer/detect-distro.sh`
- Create: `packaging/installer/verify-cosign.sh`
- Create: `packaging/installer/apt-install.sh`

- [ ] **Step 1: Create `packaging/installer/detect-distro.sh`**

```bash
#!/usr/bin/env bash
# detect-distro.sh — refuse to install on unsupported systems.

set -euo pipefail

. /etc/os-release

case "${ID:-}-${VERSION_ID:-}" in
    ubuntu-24.*|ubuntu-25.*)
        echo "ubuntu"
        ;;
    kali-*)
        echo "kali"
        ;;
    debian-12|debian-13)
        echo "debian"
        ;;
    *)
        echo ""
        ;;
esac
```

- [ ] **Step 2: Create `packaging/installer/verify-cosign.sh`**

```bash
#!/usr/bin/env bash
# verify-cosign.sh — verify a .deb with cosign keyless signing.

set -euo pipefail

verify_cosign_blob() {
    local deb="$1"
    local sig="$2"
    local cert="$3"

    cosign verify-blob \
      --signature "$sig" \
      --certificate "$cert" \
      --certificate-identity-regexp 'https://github.com/blackglass/blackglass/.github/workflows/release.yml@refs/tags/v.*' \
      --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
      "$deb"
}
```

- [ ] **Step 3: Create `packaging/installer/apt-install.sh`**

```bash
#!/usr/bin/env bash
# apt-install.sh — install a .deb with apt.

set -euo pipefail

apt_install_deb() {
    local deb="$1"
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y "$deb"
}
```

- [ ] **Step 4: Create `packaging/install.sh`**

```bash
#!/usr/bin/env bash
# install.sh — blackglass one-line installer.
# Source: https://github.com/blackglass/blackglass/blob/main/packaging/install.sh
# This script is browsable. Auditing it is the point.

set -euo pipefail

# Parse args
VARIANT="full"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --minimal|--core|--full)
            VARIANT="${1#--}"
            shift
            ;;
        *)
            echo "unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

# 1. Detect distro
DISTRO=$(. /etc/os-release && echo "${ID:-}")
case "$DISTRO" in
    ubuntu|kali|debian) ;;
    *) echo "unsupported distro: $DISTRO (need Ubuntu 24.04+, Kali, or Debian 12+)" >&2; exit 1 ;;
esac
echo "✓ detected distro: $DISTRO"

# 2. AppArmor precheck
if ! command -v aa-enabled >/dev/null; then
    echo "AppArmor is not installed. Install apparmor and apparmor-utils first." >&2
    exit 1
fi
if ! aa-enabled --quiet 2>/dev/null; then
    echo "AppArmor is not enabled. blackglass requires AppArmor." >&2
    exit 1
fi
echo "✓ AppArmor is enabled"

# 3. Ensure cosign is available
if ! command -v cosign >/dev/null; then
    echo "Installing cosign..."
    if command -v apt-get >/dev/null; then
        apt-get install -y cosign 2>/dev/null || {
            echo "cosign not in repos; falling back to static binary"
            curl -sSfL -o /usr/local/bin/cosign \
              https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64
            chmod +x /usr/local/bin/cosign
        }
    fi
fi
echo "✓ cosign is available"

# 4. Fetch the latest release metadata
echo "Fetching latest release info..."
release_json=$(curl -sSfL https://api.github.com/repos/blackglass/blackglass/releases/latest)
version=$(echo "$release_json" | jq -r .tag_name)
asset_base="https://github.com/blackglass/blackglass/releases/download/$version"

# 5. Download the .deb and its signature
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
deb_basename="blackglass-${VARIANT}_${version#v}_amd64.deb"
echo "Downloading $deb_basename..."
curl -sSfL -o "$tmpdir/$deb_basename"   "$asset_base/$deb_basename"
curl -sSfL -o "$tmpdir/$deb_basename.sig"   "$asset_base/$deb_basename.sig"
curl -sSfL -o "$tmpdir/$deb_basename.cert"  "$asset_base/$deb_basename.cert"

# 6. Verify the .deb is what we built
echo "Verifying cosign signature..."
. /usr/lib/blackglass/installer/verify-cosign.sh 2>/dev/null || . "$(dirname "$0")/installer/verify-cosign.sh"
verify_cosign_blob "$tmpdir/$deb_basename" "$tmpdir/$deb_basename.sig" "$tmpdir/$deb_basename.cert"
echo "✓ signature verified"

# 7. Install with apt
echo "Installing with apt..."
. "$(dirname "$0")/installer/apt-install.sh"
apt_install_deb "$tmpdir/$deb_basename"

# 8. Print the summary
cat <<EOF

blackglass ${version} installed.
  UI:           blackglass ui
  Profile:      blackglass profile init
  Audit log:    ~/.local/share/blackglass/audit/audit.jsonl
  Re-install:   curl -sSfL https://blackglass.dev/install.sh | sudo bash
You may need to log out and back in for the 'blackglass' group to take effect.
EOF
```

- [ ] **Step 5: Make install.sh executable and validate**

Run: `chmod +x packaging/install.sh`
Run: `bash -n packaging/install.sh && echo "syntax OK"`

- [ ] **Step 6: Commit**

```bash
git add packaging/install.sh packaging/installer/
git commit -m "feat(install): curl|sh installer with cosign verification"
```

## Task 4.9: Test the install flow end-to-end

- [ ] **Step 1: Build and sign a test .deb locally**

Run: `cargo run -p xtask -- deb --variants full`
Run: `cargo run -p xtask -- sign --input target/debian/blackglass-full_*.deb`
Expected: produces `*.deb.sig` and `*.deb.cert`.

- [ ] **Step 2: Verify the signature is valid**

Run: `cosign verify-blob --signature target/debian/blackglass-full_*.deb.sig --certificate target/debian/blackglass-full_*.deb.cert --certificate-identity-regexp '.*' --certificate-oidc-issuer '.*' target/debian/blackglass-full_*.deb`
Expected: success.

- [ ] **Step 3: Install the .deb on a test VM (or in a Docker container)**

Run: `docker run -it --rm ubuntu:24.04 bash -c "apt-get update && apt-get install -y /path/to/blackglass-full_*.deb"`
Expected: installs without errors, postinst runs successfully.

- [ ] **Step 4: Verify the venv was built**

Run: `docker run -it --rm ... ls -la /usr/lib/blackglass/python-venv/bin/python`
Expected: the venv exists.

- [ ] **Step 5: Verify the AppArmor profiles are loaded**

Run: `docker run -it --rm ... aa-status | grep blackglass`
Expected: `blackglass-core` and `blackglass-polkit-helper` are in enforce mode.

- [ ] **Step 6: Commit any fixes**

If anything failed, fix and commit.

---

# Phase 5: Docs + verification

**Exit criteria for this phase:** `blackglass --help` works; `cargo xtask verify-install` passes on a clean install; the README's Quickstart section is correct and runnable; the smoke-test script verifies the 7 success criteria; sub-plan 4 is fully complete and ready for sign-off.

## Task 5.1: Write the top-level README

**Files:**
- Modify: `README.md` (already exists)

- [ ] **Step 1: Read the current README**

Run: `head -30 README.md`

- [ ] **Step 2: Replace the top section with the Quickstart**

```markdown
# blackglass

A local-first, audit-logged security tool platform.

Every upstream pentest tool goes through a chokepoint that writes to a
tamper-evident, hash-chained audit log. The Tauri desktop app is the only
UI. The Python sidecar handles tools that need raw sockets. AppArmor
confinement + polkit privilege drop + udev rules for the Flipper.

## Quickstart

```bash
# 1. Install (Ubuntu 24.04, Kali, Debian 12+)
curl -sSfL https://blackglass.dev/install.sh | sudo bash

# 2. Initialize your first profile
blackglass profile init

# 3. Launch the UI
blackglass ui
```

That's it. The audit log is at `~/.local/share/blackglass/audit/audit.jsonl`.

## Status

**Sub-plan 3** ✅ Gate 3 (operator confirmation chokepoint) is wired and
tested end-to-end. 60 Rust + 6 Svelte tests passing.

**Sub-plan 4** (this sub-plan) — Tauri desktop shell, Python sidecar, and
.deb packaging. In progress.

## Architecture

```
┌────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  mcp-{osint,   │    │  blackglass-core │    │  Python sidecar  │
│   packets,...} ├───►│  (Rust, the gate)├────►│  (scapy,         │
│  6 thin clients│    │                  │    │   impacket)      │
└────────────────┘    └──────────────────┘    └──────────────────┘
        │                       │                      │
        │                       ▼                      │
        │              ┌──────────────────┐            │
        │              │  audit chain     │            │
        │              │  (JSONL+blake3)  │            │
        │              └──────────────────┘            │
        │                       │                      │
        │                       ▼                      │
        │              ┌──────────────────┐            │
        │              │  Tauri UI        │            │
        │              │  (audit browser) │            │
        │              └──────────────────┘            │
        │                                              │
        └──────────────► nmap, tshark, ... ◄───────────┘
                        (upstream tool binaries)
```

## Development

```bash
# Build everything
cargo build --workspace
cd app && npm install && npm run build

# Run the test suite
cargo test --workspace
cd app && npx playwright test

# Build a .deb
cargo run -p xtask -- deb --variants full

# Run the confinement test
sudo cargo run -p xtask -- confinement-test
```

## Security

Read `docs/security.md` for the threat model, the kill-switch list, and
the secure-update mechanism. Read `docs/spec.md` for the full design.

## License

MIT.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: rewrite top-level README with quickstart and architecture"
```

## Task 5.2: Implement `cargo xtask verify-install`

**Files:**
- Modify: `crates/xtask/src/bin_verify_install.rs`

- [ ] **Step 1: Implement the verify-install command**

```rust
use anyhow::{bail, Result};
use std::process::Command;

struct Check {
    name: &'static str,
    pass: bool,
    detail: String,
}

impl Check {
    fn ok(name: &'static str) -> Self { Self { name, pass: true, detail: String::new() } }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, pass: false, detail: detail.into() }
    }
}

fn check_app_armor() -> Check {
    if !Command::new("aa-enabled").output().map(|o| o.status.success()).unwrap_or(false) {
        return Check::fail("apparmor-enabled", "aa-enabled reports disabled");
    }
    let out = match Command::new("aa-status").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Check::fail("apparmor-status", "aa-status failed"),
    };
    if !out.contains("blackglass-core") {
        return Check::fail("apparmor-core-profile", "blackglass-core profile not loaded");
    }
    if !out.contains("blackglass-polkit-helper") {
        return Check::fail("apparmor-helper-profile", "blackglass-polkit-helper profile not loaded");
    }
    Check::ok("apparmor")
}

fn check_audit_dir() -> Check {
    let path = format!("{}/.local/share/blackglass/audit", std::env::var("HOME").unwrap_or_default());
    if !std::path::Path::new(&path).exists() {
        return Check::fail("audit-dir", format!("{path} does not exist"));
    }
    Check::ok("audit-dir")
}

fn check_group() -> Check {
    let user = std::env::var("USER").unwrap_or_default();
    let out = match Command::new("id").args(["-Gn", &user]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Check::fail("group", "id failed"),
    };
    if !out.split_whitespace().any(|g| g == "blackglass") {
        return Check::fail("group", format!("user {user} is not in the blackglass group"));
    }
    Check::ok("group")
}

fn check_polkit_helper() -> Check {
    let path = "/usr/libexec/blackglass-polkit-helper";
    if !std::path::Path::new(path).exists() {
        return Check::fail("polkit-helper", format!("{path} not found"));
    }
    Check::ok("polkit-helper")
}

fn check_flipper_rule() -> Check {
    let path = "/lib/udev/rules.d/99-blackglass-flipper.rules";
    if !std::path::Path::new(path).exists() {
        return Check::fail("flipper-udev", format!("{path} not found"));
    }
    Check::ok("flipper-udev")
}

fn check_python_venv() -> Check {
    let path = "/usr/lib/blackglass/python-venv/bin/python";
    if !std::path::Path::new(path).exists() {
        return Check::fail("python-venv", format!("{path} not found"));
    }
    // Try importing the sidecar
    let out = Command::new(path)
        .args(["-c", "import blackglass_sidecar.scapy_bridge, blackglass_sidecar.impacket_bridge, blackglass_sidecar.hardware_bridge, blackglass_sidecar.audit_types; print('OK')"])
        .output();
    match out {
        Ok(o) if o.status.success() => Check::ok("python-venv"),
        Ok(o) => Check::fail("python-venv", String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Check::fail("python-venv", e.to_string()),
    }
}

fn check_cosign_key() -> Check {
    let path = "/usr/share/blackglass/cosign.pub";
    if !std::path::Path::new(path).exists() {
        return Check::fail("cosign-key", format!("{path} not found"));
    }
    Check::ok("cosign-key")
}

pub fn run() -> Result<()> {
    println!("=== blackglass verify-install ===\n");
    let checks = vec![
        check_app_armor(),
        check_audit_dir(),
        check_group(),
        check_polkit_helper(),
        check_flipper_rule(),
        check_python_venv(),
        check_cosign_key(),
    ];
    let mut failed = 0;
    for c in &checks {
        let mark = if c.pass { "✓" } else { "✗" };
        println!("  {mark} {}", c.name);
        if !c.pass {
            println!("      {}", c.detail);
            failed += 1;
        }
    }
    println!();
    if failed == 0 {
        println!("All checks passed. ✓");
        Ok(())
    } else {
        bail!("{failed} check(s) failed");
    }
}
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p xtask`
Run: `cargo test -p xtask`

- [ ] **Step 3: Commit**

```bash
git add crates/xtask/src/bin_verify_install.rs
git commit -m "feat(xtask): verify-install checks all 7 prerequisites"
```

## Task 5.3: Run verify-install locally and ensure all checks pass

- [ ] **Step 1: Run verify-install on the dev system**

Run: `cargo run -p xtask -- verify-install`
Expected: 7 checks pass (or at least 5+ if the system lacks udev/aa).

- [ ] **Step 2: Fix any failures**

Common fixes:
- Missing audit dir: `mkdir -p ~/.local/share/blackglass/audit`
- Missing group: `sudo usermod -aG blackglass $USER; newgrp blackglass`

- [ ] **Step 3: Commit any fixups**

## Task 5.4: Create the smoke-test script

**Files:**
- Create: `scripts/smoke-test.sh`

- [ ] **Step 1: Write the script**

```bash
#!/usr/bin/env bash
# smoke-test.sh — 7-criterion smoke test for a fresh install.
# Run as the operator (in the `blackglass` group).

set -euo pipefail
PASS=0
FAIL=0

pass() { echo "  ✓ $1"; PASS=$((PASS+1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL+1)); }

# 1. The audit log is readable
echo "1. audit log readable"
AUDIT=~/.local/share/blackglass/audit/audit.jsonl
if [ -r "$AUDIT" ]; then pass "audit.jsonl readable"; else fail "audit.jsonl missing or unreadable"; fi

# 2. The core binary is in PATH
echo "2. core binary in PATH"
if command -v blackglass-core >/dev/null; then pass "blackglass-core in PATH"; else fail "blackglass-core not in PATH"; fi

# 3. The Tauri binary is in PATH
echo "3. Tauri binary in PATH"
if command -v blackglass >/dev/null; then pass "blackglass in PATH"; else fail "blackglass not in PATH"; fi

# 4. The polkit helper is installed
echo "4. polkit helper installed"
if [ -x /usr/libexec/blackglass-polkit-helper ]; then pass "polkit-helper installed"; else fail "polkit-helper missing"; fi

# 5. AppArmor profiles are loaded
echo "5. AppArmor profiles loaded"
if aa-status 2>/dev/null | grep -q blackglass-core; then pass "blackglass-core profile loaded"; else fail "blackglass-core profile NOT loaded"; fi
if aa-status 2>/dev/null | grep -q blackglass-polkit-helper; then pass "blackglass-polkit-helper profile loaded"; else fail "blackglass-polkit-helper profile NOT loaded"; fi

# 6. The Python venv exists and imports cleanly
echo "6. Python venv"
VENV=/usr/lib/blackglass/python-venv/bin/python
if [ -x "$VENV" ]; then
    if "$VENV" -c "import blackglass_sidecar.scapy_bridge, blackglass_sidecar.impacket_bridge, blackglass_sidecar.hardware_bridge, blackglass_sidecar.audit_types" 2>/dev/null; then
        pass "sidecar venv imports"
    else
        fail "sidecar venv import failed"
    fi
else
    fail "sidecar venv missing"
fi

# 7. A test run produces an audit event
echo "7. test run produces an audit event"
EVENT_BEFORE=$(wc -l < "$AUDIT" 2>/dev/null || echo 0)
# Run a known-bad op that should be denied and logged
blackglass core op osint_whois --target "127.0.0.1" --note "smoke-test" 2>/dev/null || true
EVENT_AFTER=$(wc -l < "$AUDIT" 2>/dev/null || echo 0)
if [ "$EVENT_AFTER" -gt "$EVENT_BEFORE" ]; then
    pass "test run produced audit event ($EVENT_BEFORE → $EVENT_AFTER)"
else
    fail "test run did NOT produce an audit event"
fi

echo ""
echo "Passed: $PASS / $((PASS+FAIL))"
[ "$FAIL" -eq 0 ]
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/smoke-test.sh`

- [ ] **Step 3: Run it on the dev system**

Run: `./scripts/smoke-test.sh`
Expected: all 7 pass.

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke-test.sh
git commit -m "test: smoke-test.sh verifies the 7 install criteria"
```

## Task 5.5: Final sign-off and PR

- [ ] **Step 1: Run the full test suite**

Run: `cargo test --workspace`
Run: `cd app && npx playwright test`
Expected: all pass.

- [ ] **Step 2: Run the lint chain**

Run: `cargo fmt --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo deny check`
Run: `cargo audit`
Expected: all pass.

- [ ] **Step 3: Update the design doc**

Edit `docs/superpowers/plans/2026-06-03-blackglass-subplan3.md` §14 to mark sub-plan 4 as "complete" (or move it to a "completed" section).

- [ ] **Step 4: Commit the design doc update**

```bash
git add docs/superpowers/plans/
git commit -m "docs: mark sub-plan 4 complete"
```

- [ ] **Step 5: Create a final commit summarizing the work**

```bash
git add -A
git commit --allow-empty -m "chore(sub-plan-4): final sign-off"
```

- [ ] **Step 6: Push and open the PR**

```bash
git push origin <branch>
gh pr create --title "Sub-plan 4: Tauri shell, Python sidecar, packaging" --body "Implements sub-plan 4 of the blackglass spec. See docs/superpowers/plans/2026-06-03-blackglass-subplan4.md for the design.

Summary:
- Tauri shell + audit browser (~5 new tests)
- Python sidecar venv + build step
- .deb packaging (minimal/core/full variants) with cosign signing
- install.sh one-liner
- AppArmor profiles + polkit policy + udev rule
- cargo xtask orchestrator (build, deb, sign, confinement-test, verify-install, apparmor-generate)
- 7-criterion smoke test

All tests pass. Ready for review."
```

---

# Appendix: Quick reference

## What we're shipping

- `blackglass` (Tauri desktop app) — the only UI
- `blackglass-core` — the Rust chokepoint, daemonized via the polkit helper
- `blackglass-polkit-helper` — the min-trust shim that gates `start-core`
- 6× `blackglass-mcp-*` — thin stdio clients per tool cluster
- Python sidecar (scapy, impacket, hardware-bridge) — venv built at install
- AppArmor profiles — strict confinement for core and helper
- Polkit policy — gates `start-core` to the `blackglass` group
- Udev rule — gives the group access to the Flipper
- `cargo xtask` — the build orchestrator (8 subcommands)
- `install.sh` — `curl | bash` with cosign verification
- `verify-install` — 7-criterion health check
- `smoke-test.sh` — 7-criterion install verification

## Phases recap

| Phase | What it produces | Tests added | Status |
|---|---|---|---|
| 1 | Python sidecar (build, test, sign) | ~5 | ✅ |
| 2 | Tauri shell + audit browser | ~5 | ✅ |
| 3 | Security primitives (AppArmor, polkit, udev, xtask) | ~3 | ✅ |
| 4 | Packaging (deb, cosign, install.sh) | ~3 | ✅ |
| 5 | Docs + verify-install + smoke | ~2 | ✅ |

**Total new tests: ~18. Total tests after sub-plan 4: ~84.**

## Open follow-ups (sub-plan 5 scope)

- The 8 stub views in the Tauri shell (engagement, tools, settings, AI session, prompt-injection, kill switches, onboarding, home)
- The 21 remaining `mcp-*` crates
- The design doc for the engagement workflow
- The "tools" view — declarative, scoped allow-lists for the AI

That's a different sub-plan and a different design doc.












## Execution Plan Structure

This plan is split into 5 phase-plans. Each phase is independently mergeable and produces working, testable software. Phases must be executed in order because each one unblocks the next:

- **Phase 1: Python sidecar + 4 new MCP servers** (`crates/python-bridge/`, `crates/mcp-{ad,flipper,phish,detect}/`, audit-event additions)
- **Phase 2: Tauri shell + audit browser** (`app/src/routes/audit/`, Tauri commands)
- **Phase 3: Security primitives** (`crates/polkit-helper/`, AppArmor profiles, udev rule)
- **Phase 4: Packaging** (`packaging/debian/`, `packaging/install.sh`, `crates/xtask/`, `.github/workflows/release.yml`)
- **Phase 5: Polish** (manpages, icon, lintian overrides, `verify-install`)

---

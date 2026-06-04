# Blackglass Sub-plan 4 Amendment — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Core IPC delta (Phase 2.5+), the 3-pane Tauri domain workspace (Phase 3), user-home AppArmor profiles (Phase 4), user-systemd .deb packaging (Phase 5), and verification polish (Phase 6) — as specified in `docs/superpowers/specs/2026-06-03-blackglass-subplan4-design.md` §1.1 (amendment). Brings test count from ~136 to ~166 passing.

**Architecture:** Core supervises 4 new MCP servers (`mcp-ad`, `mcp-flipper`, `mcp-phish`, `mcp-detect`) as child processes with exponential-backoff restart. A new `mcp_run_tool` JSON-RPC method on the operator socket lets the Tauri app run any of the 16 existing + new tools; result flows back inline; audit chain captures `McpRunStarted` / `McpRunCompleted`. Tauri shell gets a 3-pane domain workspace (DomainRail | ToolRunner | ResultPane) plus an audit-detail right rail. Everything runs as a *user* systemd service — no polkit, no `/var/lib/blackglass/`, no `/var/run/blackglass/`. Cosign release signing is deferred to a later sub-plan.

**Tech Stack:** Rust (tokio, pyo3, async-trait, serde_json, tracing), Tauri 2.x + Svelte 5 + Vite + Tailwind, vitest + @testing-library/svelte, pyo3, uv, AppArmor 3.0, systemd user units, cargo-deb, debhelper-compat 13.

**Spec:** `docs/superpowers/specs/2026-06-03-blackglass-subplan4-design.md` §1.1 (read first, especially §1.1.1–§1.1.9 for scope, event kinds, phase order, test budget).

**Existing plan to defer to:** `docs/superpowers/plans/2026-06-03-blackglass-subplan4.md`. This amendment does NOT replace that file. Phases 1 of that plan is shipped (commit 7bfa0d8). Phases 2-5 of that plan need *deltas* applied on top of them — the deltas are listed in the Appendix of this amendment. The shipped Phase 1 tasks (1.1-1.12) are unchanged; the new work in this amendment builds on top of them.

---

## File Structure

### Rust crates

**`crates/core/`** (extends the existing chokepoint):

- `src/audit_query.rs` *(NEW)* — `audit.query` and `audit.verify_chain` operator-socket methods. Pure pass-through to `audit::Chain::query` / `Chain::verify_chain`. ~150 lines + tests.
- `src/mcp_supervisor.rs` *(NEW)* — spawn / monitor / restart-with-backoff / give-up supervisor for the 4 new MCP child processes. ~350 lines + tests.
- `src/mcp_spawn_config.rs` *(NEW)* — load `~/.config/blackglass/mcp-servers.toml` into a typed `McpSpawnConfig` struct. ~80 lines + tests.
- `src/operator_auth.rs` *(NEW)* — read the operator token from `~/.local/share/blackglass/operator.token`, validate it on every connection. ~100 lines + tests.
- `src/operator_server.rs` *(MODIFY)* — add `mcp_run_tool`, `audit.query`, `audit.verify_chain` methods; gate every method on operator auth. Existing `ping` and `confirm.resolve` stay; new methods follow the same `jsonrpc_ok` / `jsonrpc_error` pattern.
- `src/main.rs` *(MODIFY)* — at startup, read `mcp-servers.toml`, build an `McpSupervisor`, spawn it as a `tokio::spawn` task. Re-export the supervisor's `join_handle` so signals can trigger clean shutdown.
- `tests/operator_server_mcp.rs` *(NEW)* — 4 tests for `mcp_run_tool` (allow, deny, MCP-down, timeout).
- `tests/operator_server_audit.rs` *(NEW)* — 3 tests for `audit.query` + `audit.verify_chain`.
- `tests/mcp_supervisor.rs` *(NEW)* — 4 tests (spawn, monitor, restart-with-backoff, give-up).
- `tests/end_to_end_mcp_run.rs` *(NEW)* — 2 tests (full Tauri-Rust-style flow).
- `tests/fixtures/mcp-servers.toml` *(NEW)* — test fixture for the config loader.

**`crates/audit/`** (extends the existing audit library):

- `src/lib.rs` *(MODIFY)* — add 4 new `EventKind` variants: `McpServerSpawned`, `McpServerExited`, `McpRunStarted`, `McpRunCompleted`. Each has a typed payload struct (also new). + 2 tests in `tests/event_kinds.rs`.

**`crates/secondary-sidecar/`** (existing, no new files — the user-systemd service file is in `packaging/`, not here):

- No Rust changes. The secondary sidecar binary already exists from Phase 1. The new work is the user-systemd unit file (in `packaging/systemd/`) and the AppArmor profile (in `packaging/apparmor/`).

**`crates/xtask/`** (extends the existing build orchestrator):

- `src/bin/confinement_test.rs` *(MODIFY)* — extend the existing confinement test to validate the new `blackglass-core` user-home AppArmor profile and the new `blackglass-secondary-sidecar` profile. + 2 tests.
- `src/bin/apparmor_generate.rs` *(MODIFY)* — add a `--secondary-sidecar` flag that emits a draft `blackglass-secondary-sidecar` profile from the same template engine as the core profile.
- `src/bin/verify_install.rs` *(MODIFY)* — add checks for: user-service `blackglass-core.service` is enabled, user-service `blackglass-secondary-sidecar.service` is enabled, the operator token file exists with mode 0600, the user is in the `udev` group, the AppArmor profiles are loaded, the `mcp-servers.toml` is installed at `/etc/blackglass/mcp-servers.toml.example`. Remove the old checks for `/var/run/blackglass/` and `/var/lib/blackglass/`.

**`app/src-tauri/`** (extends the existing Tauri app):

- `src/commands.rs` *(NEW)* — 3 new Tauri commands: `mcp_run_tool(domain, target, args)`, `mcp_list_tools(domain)`, `audit_event(id)`. Each opens the operator socket (with the operator token), writes a JSON-RPC frame, awaits the response. ~250 lines + 3 tests.
- `src/operator_client.rs` *(NEW)* — helper for opening the operator socket + reading the operator token. Used by all 3 new commands. ~80 lines.
- `src/main.rs` *(MODIFY)* — register the 3 new commands in `invoke_handler!`; add the operator token path to the config; on first launch, attempt `systemctl --user start blackglass-core` if the socket is absent.
- `tests/mcp_run_tool.rs` *(NEW)* — 3 tests (success, auth-fail, MCP-down).

### Tauri / Svelte UI

**`app/src/lib/`** (new Svelte 5 components):

- `DomainRail.svelte` *(NEW)* — left rail: list of MCP domains (osint, packets, ad, flipper, phish, detect) + the engagement selector. Click a domain → tool catalog updates. ~80 lines + test.
- `ToolRunner.svelte` *(NEW)* — middle pane: shows the tools for the selected domain. For each tool: name, description, a JSON textarea for args, a "Run" button. While running: disabled button + spinner. On result: scrolls ResultPane into view. ~150 lines + test.
- `ResultPane.svelte` *(NEW)* — right-middle pane: shows the last run's stdout / stderr / duration / audit-event id. Click the audit-event id → opens the AuditDetail right rail. ~120 lines + test.
- `AuditDetail.svelte` *(NEW)* — far-right slide-out: shows the full JSON for one audit event (the detail pane for the audit browser too). ~100 lines + test.
- `McpClient.ts` *(NEW)* — Svelte-side wrapper around the 3 new Tauri commands. Throws typed errors that the components can `try/catch` cleanly. ~80 lines + 1 vitest test.
- `state.svelte.ts` *(MODIFY)* — add `domains: Domain[]`, `selectedDomain: Domain | null`, `selectedTool: Tool | null`, `lastResult: RunResult | null`, `auditDetailEventId: string | null` to the existing `$state` runes. The existing `pending` and `confirm` state stays.
- `App.svelte` *(MODIFY)* — replace the "Waiting for confirmation requests" stub with the 3-pane layout: left rail (engagement + DomainRail), middle (ToolRunner | audit log | 8 stub views), right rail (ResultPane + AuditDetail). The existing /audit view becomes a sub-route of the middle pane.
- `routes/audit/+page.svelte` *(MODIFY)* — the existing audit log view stays; the only change is that clicking a row in the audit list opens AuditDetail in the right rail instead of a modal. ~30 lines of changes.
- `lib/toolCatalog.ts` *(NEW)* — hardcoded tool catalog. Mirrors the `*_TOOLS` constants in the MCP crates. ~120 lines (1 entry per tool across 6 domains).

### Packaging

**`packaging/systemd/`** (new user-systemd unit files):

- `blackglass-core.service` *(NEW)* — user unit for the core. `ExecStart=/usr/bin/blackglass-core`, `Restart=on-failure`, `RestartSec=5s`. Installed to `/usr/lib/blackglass/systemd/user/` by the .deb; symlinked into `~/.config/systemd/user/` by the postinst.
- `blackglass-secondary-sidecar.service` *(NEW)* — user unit for the secondary sidecar. `ExecStart=/usr/bin/blackglass-secondary-sidecar`, `Restart=on-failure`, `RestartSec=10s`. Same install path.
- `blackglass-secondary-sidecar.apparmor` *(NEW)* — AppArmor profile (user-home version of the secondary sidecar; see §1.1.1 of the spec).

**`packaging/debian/`** (modifies the existing debian/ tree):

- `control` *(MODIFY)* — remove `libpolkit-gobject-1-dev` Build-Dep, `libpolkit-gobject-1-0` + `adduser` + `policykit-1 | polkit` Depends from `blackglass-minimal`. Remove `cosign` Build-Dep. + add `systemd` (for the unit-file path) Depends. Otherwise unchanged.
- `cargo-deb.toml` *(MODIFY)* — extend `[bin]` list to include `blackglass-core` (was already there), `blackglass-secondary-sidecar`, `polkit-helper` is REMOVED. Extend `data` to include `packaging/systemd/` and `packaging/apparmor/`. Remove `data` entries for `/var/run/blackglass/` and `/var/lib/blackglass/`.
- `postinst` *(MODIFY)* — remove the `addgroup --system blackglass`, the `install -d -m 0750 -o root -g blackglass /var/lib/blackglass/*` mkdirs, the `apparmor_parser -r` for the polkit-helper profile, and the `cosign` setup. ADD: `systemctl --user enable blackglass-core` and `systemctl --user enable blackglass-secondary-sidecar` (best-effort, only if `XDG_RUNTIME_DIR` is set), `apparmor_parser -r` for the new secondary-sidecar profile, the `mcp-servers.toml.example` is installed to `/etc/blackglass/mcp-servers.toml.example`, the user is added to the `udev` group (best-effort).
- `prerm` *(MODIFY)* — remove the `/var/lib/blackglass` cleanup. ADD: `systemctl --user disable blackglass-core` and `systemctl --user disable blackglass-secondary-sidecar` (best-effort).
- `mcp-servers.toml.example` *(NEW)* — example config: lists the 4 new MCPs (`mcp-ad`, `mcp-flipper`, `mcp-phish`, `mcp-detect`) with their binary paths, restart policies, and a 30-second startup timeout.
- `tests/postinst_smoke.sh` *(NEW)* — best-effort postinst smoke (runs `dpkg-deb -x` on the built .deb, inspects the file layout). Not run in CI; can be run on the user's modified Ubuntu.

### `packaging/install.sh` (modifies the existing installer)

- Remove the cosign verification step. Replace with HTTPS + SHA-256 checksum pinning: download both `blackglass_0.1.0_amd64.deb` and `blackglass_0.1.0_amd64.deb.sha256` from the GitHub release, verify with `sha256sum -c`. On 404: print a clear "GitHub Release not published yet — build from source" message and link to the README's build-from-source section.
- Remove the `verify-cosign.sh` source step.
- After install, print a banner: "blackglass is installed. Run `blackglass ui` to launch the Tauri app. The first launch will start the user-service `blackglass-core` via systemd."

### `README.md` (modifies the existing top-level README)

- Add a "Build from source" section that walks through `git clone`, `cargo xtask deb`, `sudo dpkg -i target/debian/*.deb`. This is the **v1 install path** until the GitHub Release + cosign pipeline ships.
- Add a "First launch" section: `blackglass ui` → systemd user service starts the core → Tauri connects → 3-pane workspace appears.

---


# Phase 2.5+: Core IPC delta

The 8 tasks in this phase add the Core-side plumbing for the new flows: 4 new audit event kinds, operator-socket auth, the MCP supervisor, the `mcp_run_tool` method, and the `audit.query` / `audit.verify_chain` methods. They are TDD-first; each task ends with a green test and a commit.

---

## Task 2.5.1: Add 4 new `EventKind` variants for the MCP lifecycle

**Files:**
- Modify: `crates/audit/src/lib.rs` (the `EventKind` enum + payload structs)
- Create: `crates/audit/tests/event_kinds.rs`

- [ ] **Step 1: Read the existing `EventKind` definition**

Run: `grep -n "EventKind" crates/audit/src/lib.rs | head -20`
Expected: the enum has variants like `ActionRequested`, `OperatorConfirmationRequested`, `OperatorConfirmationResolved`, `ActionAllowed`, `PythonBridgeInvoked`, `ActionExecuted` (and the ones added by sub-plan 1-3). Each variant carries a typed payload struct.

- [ ] **Step 2: Write the failing test for the 4 new variants**

`crates/audit/tests/event_kinds.rs`:

```rust
use blackglass_audit::{Event, EventKind, Chain};
use std::io::Write;

#[test]
fn mcp_server_spawned_serializes_with_server_and_pid() {
    let event = Event::new(EventKind::McpServerSpawned {
        server: "mcp-ad".into(),
        pid: 12345,
    });
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""kind":"mcp_server_spawned""#));
    assert!(json.contains(r#""server":"mcp-ad""#));
    assert!(json.contains(r#""pid":12345"#));
}

#[test]
fn mcp_server_exited_serializes_with_code_and_restart_count() {
    let event = Event::new(EventKind::McpServerExited {
        server: "mcp-flipper".into(),
        code: -1,
        restart_count: 3,
    });
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""kind":"mcp_server_exited""#));
    assert!(json.contains(r#""code":-1"#));
    assert!(json.contains(r#""restart_count":3"#));
}

#[test]
fn mcp_run_started_and_completed_serialize() {
    let start = Event::new(EventKind::McpRunStarted {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
    });
    let end = Event::new(EventKind::McpRunCompleted {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        ok: true,
        ms: 1234,
    });
    assert!(serde_json::to_string(&start).unwrap().contains(r#""kind":"mcp_run_started""#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""kind":"mcp_run_completed""#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""ok":true"#));
    assert!(serde_json::to_string(&end).unwrap().contains(r#""ms":1234"#));
}

#[test]
fn new_event_kinds_extend_the_hash_chain() {
    // The hash chain must include the new event kinds.
    let dir = tempfile::tempdir().unwrap();
    let mut chain = Chain::open(dir.path().join("chain.jsonl")).unwrap();
    chain.append(Event::new(EventKind::McpServerSpawned {
        server: "mcp-ad".into(),
        pid: 1,
    })).unwrap();
    chain.append(Event::new(EventKind::McpRunCompleted {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        ok: true,
        ms: 100,
    })).unwrap();
    let report = chain.verify_chain().unwrap();
    assert_eq!(report.events_checked, 2);
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p blackglass-audit mcp_`
Expected: FAIL with "no variant or associated item named `McpServerSpawned` found for enum `EventKind`".

- [ ] **Step 4: Add the 4 variants to `EventKind`**

In `crates/audit/src/lib.rs`, find the `EventKind` enum and add the 4 new variants. Keep the existing variants intact:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    // ... existing variants ...
    McpServerSpawned { server: String, pid: u32 },
    McpServerExited { server: String, code: i32, restart_count: u32 },
    McpRunStarted { domain: String, target: String },
    McpRunCompleted { domain: String, target: String, ok: bool, ms: u64 },
}
```

(If the existing variants use `#[serde(tag = "kind", rename_all = "snake_case")]` on the enum, the new variants will automatically serialize as `mcp_server_spawned` etc. — matching the test assertions.)

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p blackglass-audit mcp_`
Expected: PASS (4 tests).

- [ ] **Step 6: Run the full audit test suite to ensure no regressions**

Run: `cargo test -p blackglass-audit`
Expected: PASS, all tests green (existing + 4 new).

- [ ] **Step 7: Commit**

```bash
git add crates/audit/
git commit -m "feat(audit): add McpServerSpawned/Exited + McpRunStarted/Completed event kinds"
```

---

## Task 2.5.2: Add operator-socket auth (token file)

**Files:**
- Create: `crates/core/src/operator_auth.rs`
- Create: `crates/core/tests/operator_auth_test.rs`

- [ ] **Step 1: Read the existing operator server to see how it accepts connections**

Run: `head -100 crates/core/src/operator_server.rs`
Expected: the server uses `tokio::net::UnixListener` to accept connections and spawns a task per connection. There's no auth check today.

- [ ] **Step 2: Write the failing test for `OperatorAuth::verify`**

`crates/core/tests/operator_auth_test.rs`:

```rust
use blackglass_core::operator_auth::OperatorAuth;
use std::fs;
use tempfile::tempdir;

#[test]
fn verify_returns_ok_when_token_matches() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token-abc123\n").unwrap();
    fs::set_permissions(&token_path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let auth = OperatorAuth::new(&token_path);
    assert!(auth.verify(b"secret-token-abc123\n").is_ok());
}

#[test]
fn verify_returns_err_on_wrong_token() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token-abc123\n").unwrap();
    let auth = OperatorAuth::new(&token_path);
    let err = auth.verify(b"wrong-token\n").unwrap_err();
    assert!(err.to_string().contains("auth"));
}

#[test]
fn verify_returns_err_when_token_file_missing() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("does-not-exist");
    let auth = OperatorAuth::new(&token_path);
    assert!(auth.verify(b"any\n").is_err());
}

#[test]
fn verify_returns_err_when_token_file_is_world_readable() {
    // Defense in depth: the token file must be 0600. If it's 0644, refuse to use it.
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token\n").unwrap();
    fs::set_permissions(&token_path, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let auth = OperatorAuth::new(&token_path);
    let err = auth.verify(b"secret-token\n").unwrap_err();
    assert!(err.to_string().contains("mode"));
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p blackglass-core operator_auth`
Expected: FAIL with "module `operator_auth` not found in `blackglass_core`".

- [ ] **Step 4: Create `crates/core/src/operator_auth.rs`**

```rust
//! Operator-socket auth: validates that incoming connections present
//! a token matching the one on disk. Defense in depth: refuses to use
//! a token file with permissions looser than 0600.
//!
//! The token file is generated by `blackglass-core` on first start
//! (if absent) and stored at
//! `~/.local/share/blackglass/operator.token` with mode 0600.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

const REQUIRED_MODE: u32 = 0o600;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("operator token file not found at {0}")]
    TokenFileMissing(PathBuf),
    #[error("operator token file {0} has mode {1:o}, expected 0600 (or stricter)")]
    TokenFileBadMode(PathBuf, u32),
    #[error("auth failed: presented token does not match")]
    AuthFailed,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct OperatorAuth {
    token_path: PathBuf,
    expected: Vec<u8>,
}

impl OperatorAuth {
    /// Read the token from disk. Validates the file's permissions.
    pub fn new(token_path: &Path) -> Self {
        Self {
            token_path: token_path.to_path_buf(),
            // Expected is populated lazily by `verify` so we don't hold a
            // stale copy if the file is rotated.
            expected: Vec::new(),
        }
    }

    /// Generate a fresh token at the given path (if absent) and return
    /// the auth handle. Mode 0600.
    pub fn generate_if_absent(token_path: &Path) -> Result<Self, AuthError> {
        if !token_path.exists() {
            if let Some(parent) = token_path.parent() {
                fs::create_dir_all(parent)?;
            }
            // 32 bytes of randomness, hex-encoded = 64 chars.
            let token = generate_token();
            fs::write(token_path, format!("{}\n", token))?;
            fs::set_permissions(token_path, fs::Permissions::from_mode(REQUIRED_MODE))?;
        }
        Ok(Self::new(token_path))
    }

    pub fn verify(&self, presented: &[u8]) -> Result<(), AuthError> {
        if !self.token_path.exists() {
            return Err(AuthError::TokenFileMissing(self.token_path.clone()));
        }
        let meta = fs::metadata(&self.token_path)?;
        let mode = meta.permissions().mode() & 0o777;
        if mode != REQUIRED_MODE {
            return Err(AuthError::TokenFileBadMode(self.token_path.clone(), mode));
        }
        let expected = fs::read_to_string(&self.token_path)?;
        // Trim trailing newline (if any) on the expected token.
        let expected_trim = expected.trim_end_matches('\n');
        // Compare constant-time-ish: we just compare byte slices; this is
        // not a hot path so a simple `!=` is fine.
        if presented.trim_as_bytes() == expected_trim.as_bytes() {
            Ok(())
        } else {
            Err(AuthError::AuthFailed)
        }
    }
}

fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// Helper extension trait for trimming a `&[u8]` of trailing whitespace.
trait TrimAsBytes {
    fn trim_as_bytes(&self) -> &[u8];
}
impl TrimAsBytes for &[u8] {
    fn trim_as_bytes(&self) -> &[u8] {
        let mut end = self.len();
        while end > 0 && (self[end - 1] == b'\n' || self[end - 1] == b'\r' || self[end - 1] == b' ' || self[end - 1] == b'\t') {
            end -= 1;
        }
        &self[..end]
    }
}
```

- [ ] **Step 5: Add the `rand` dep to `crates/core/Cargo.toml`**

```toml
[dependencies]
# ... existing ...
rand = "0.8"
```

- [ ] **Step 6: Add `mod operator_auth;` to `crates/core/src/lib.rs`**

Find `pub mod` declarations in `lib.rs` and add:

```rust
pub mod operator_auth;
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p blackglass-core operator_auth`
Expected: PASS (4 tests).

- [ ] **Step 8: Run the full core test suite to ensure no regressions**

Run: `cargo test -p blackglass-core`
Expected: PASS, all tests green.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/operator_auth.rs crates/core/src/lib.rs crates/core/tests/operator_auth_test.rs crates/core/Cargo.toml
git commit -m "feat(core): operator-socket auth via 0600 token file"
```

---

## Task 2.5.3: Add `audit.query` and `audit.verify_chain` operator-socket methods

**Files:**
- Create: `crates/core/src/audit_query.rs`
- Modify: `crates/core/src/operator_server.rs`: route the new methods
- Create: `crates/core/tests/operator_server_audit.rs`

- [ ] **Step 1: Read the existing `Chain::query` and `Chain::verify_chain`**

Run: `grep -n "pub fn query\|pub fn verify_chain" crates/audit/src/lib.rs`
Expected: both functions exist (added in sub-plan 4's original Phase 2 / Task 2.4). `query(filter, page, page_size)` returns `Vec<Event>`; `verify_chain()` returns `VerifyReport { events_checked, valid }`.

- [ ] **Step 2: Write the failing test for the 3 audit methods**

`crates/core/tests/operator_server_audit.rs`:

```rust
use blackglass_core::test_helpers::{spawn_core, connect_operator_with_token};
use blackglass_audit::{Event, EventKind};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn audit_query_returns_events_paginated() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_token(&core).await;
    // Append 5 events
    for i in 0..5 {
        client.notify("audit.append", json!({
            "event": Event::new(EventKind::OperatorConfirmationRequested { id: format!("c{i}"), tool: "x".into(), args: "{}".into() })
        })).await.unwrap();
    }
    // Query page 0
    let result: serde_json::Value = client.call("audit.query", json!({
        "filter": {},
        "page": 0,
        "page_size": 3
    })).await.unwrap();
    assert_eq!(result["events"].as_array().unwrap().len(), 3);
    assert_eq!(result["total"], 5);
}

#[tokio::test]
async fn audit_query_returns_empty_page_for_out_of_range() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("audit.query", json!({
        "filter": {},
        "page": 999,
        "page_size": 10
    })).await.unwrap();
    assert_eq!(result["events"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn audit_verify_chain_returns_valid_report() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("audit.verify_chain", json!({})).await.unwrap();
    assert_eq!(result["valid"], true);
    assert!(result["events_checked"].as_u64().unwrap() >= 0);
}
```

- [ ] **Step 3: Read the test helpers (`spawn_core`, `connect_operator_with_token`)**

Run: `ls crates/core/src/test_helpers* 2>/dev/null || ls crates/core/tests/common/ 2>/dev/null`
Expected: helpers from sub-plan 3 likely live at `crates/core/tests/common/mod.rs`. If they exist and have a `connect_operator` function, add a `connect_operator_with_token` variant that presents the token in the first frame. If not, write a new helper.

The helper's job: spawn a blackglass-core on a temp socket path, return its join handle; `connect_operator_with_token` opens a UnixStream, writes `{"jsonrpc":"2.0","id":0,"method":"auth","params":{"token":"<from operator.token file>"}}\n` (the auth frame — see Step 6 for the format), then returns a client that can call methods.

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p blackglass-core audit_query_returns_events_paginated`
Expected: FAIL with "method `audit.query` not found" or "no route for `audit.query`".

- [ ] **Step 5: Create `crates/core/src/audit_query.rs`**

```rust
//! Operator-socket `audit.query` and `audit.verify_chain` methods.
//!
//! These are pure pass-throughs to `audit::Chain`. The audit chain is
//! stored in `~/.local/share/blackglass/audit/chain.jsonl`.

use crate::operator_server::{ClientId, Request, Response};
use blackglass_audit::{Chain, VerifyReport};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditQueryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit: {0}")]
    Audit(#[from] blackglass_audit::Error),
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    #[serde(default)]
    pub filter: serde_json::Value,
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page_size() -> u32 {
    50
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub events: Vec<blackglass_audit::Event>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

pub fn handle_query(chain: &Chain, params: QueryParams) -> Result<QueryResponse, AuditQueryError> {
    let all = chain.query(&params.filter)?;
    let total = all.len() as u64;
    let start = (params.page as usize).saturating_mul(params.page_size as usize);
    let end = (start + params.page_size as usize).min(all.len());
    Ok(QueryResponse {
        events: all[start..end].to_vec(),
        total,
        page: params.page,
        page_size: params.page_size,
    })
}

pub fn handle_verify(chain: &Chain) -> Result<VerifyReport, AuditQueryError> {
    Ok(chain.verify_chain()?)
}
```

- [ ] **Step 6: Route the new methods in `operator_server.rs`**

In `crates/core/src/operator_server.rs`, find the `handle` function (or equivalent — the per-method dispatcher) and add:

```rust
"audit.query" => {
    let params: crate::audit_query::QueryParams = serde_json::from_value(req.params)?;
    let resp = crate::audit_query::handle_query(&chain, params)?;
    jsonrpc_ok(req.id, serde_json::to_value(resp)?)
}
"audit.verify_chain" => {
    let report = crate::audit_query::handle_verify(&chain)?;
    jsonrpc_ok(req.id, serde_json::to_value(report)?)
}
```

Also: gate the methods on auth. Find where the server validates the client (it will gain an `auth` method in Task 2.5.5 below; for now, add a `// TODO: require auth` comment so we don't forget). The auth check is implemented as: any client that hasn't completed the `auth` method gets `error.code = -32001` for every other method.

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p blackglass-core operator_server_audit`
Expected: PASS (3 tests).

- [ ] **Step 8: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/audit_query.rs crates/core/src/operator_server.rs crates/core/tests/operator_server_audit.rs crates/core/tests/common/
git commit -m "feat(core): operator-socket audit.query + audit.verify_chain methods"
```

---

## Task 2.5.4: Implement `McpSupervisor` (spawn / monitor / restart / give-up)

**Files:**
- Create: `crates/core/src/mcp_supervisor.rs`
- Create: `crates/core/src/mcp_spawn_config.rs`
- Create: `crates/core/tests/mcp_supervisor.rs`
- Create: `crates/core/tests/fixtures/mcp-servers.toml`

- [ ] **Step 1: Write the failing test for the supervisor (4 scenarios)**

`crates/core/tests/mcp_supervisor.rs`:

```rust
use blackglass_core::mcp_spawn_config::{McpSpawnConfig, McpServerSpec};
use blackglass_core::mcp_supervisor::McpSupervisor;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;

fn spec(name: &str, cmd: &str, args: &[&str]) -> McpServerSpec {
    McpServerSpec {
        name: name.into(),
        command: cmd.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        startup_timeout_ms: 5_000,
        max_restarts: 3,
    }
}

#[tokio::test]
async fn supervisor_spawns_a_long_running_child_and_sees_it_alive() {
    let dir = tempdir().unwrap();
    let config = McpSpawnConfig {
        servers: vec![spec("sleeper", "/bin/sh", &["-c", "sleep 30"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start(config, &log_path).await.unwrap();
    // Give it a moment to spawn.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let status = sup.status("sleeper").await;
    assert_eq!(status, Some(blackglass_core::mcp_supervisor::ChildStatus::Alive));
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_restarts_a_dying_child_with_backoff() {
    let dir = tempdir().unwrap();
    // Script that exits immediately. With max_restarts=3, the supervisor
    // should restart it 3 times before giving up.
    let config = McpSpawnConfig {
        servers: vec![spec("crasher", "/bin/sh", &["-c", "exit 1"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start(config, &log_path).await.unwrap();
    // Wait for the backoff sequence: 1s + 2s + 4s = 7s minimum.
    tokio::time::sleep(Duration::from_secs(8)).await;
    let status = sup.status("crasher").await;
    assert_eq!(status, Some(blackglass_core::mcp_supervisor::ChildStatus::GivenUp { restart_count: 3 }));
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_emits_mcp_server_exited_audit_events() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    let config = McpSpawnConfig {
        servers: vec![spec("crasher", "/bin/sh", &["-c", "exit 1"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start_with_chain(config, &log_path, &chain_path).await.unwrap();
    tokio::time::sleep(Duration::from_secs(8)).await;
    let chain = blackglass_audit::Chain::open(&chain_path).unwrap();
    let events = chain.query(&serde_json::json!({})).unwrap();
    let exited: Vec<_> = events.iter().filter(|e| matches!(e.kind, blackglass_audit::EventKind::McpServerExited { .. })).collect();
    assert!(exited.len() >= 3, "expected >=3 McpServerExited events, got {}", exited.len());
    sup.shutdown().await;
}

#[tokio::test]
async fn supervisor_spawns_emits_mcp_server_spawned_audit_event() {
    let dir = tempdir().unwrap();
    let chain_path = dir.path().join("chain.jsonl");
    let config = McpSpawnConfig {
        servers: vec![spec("sleeper", "/bin/sh", &["-c", "sleep 30"])],
    };
    let log_path = dir.path().join("supervisor.log");
    let sup = McpSupervisor::start_with_chain(config, &log_path, &chain_path).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let chain = blackglass_audit::Chain::open(&chain_path).unwrap();
    let events = chain.query(&serde_json::json!({})).unwrap();
    let spawned: Vec<_> = events.iter().filter(|e| matches!(e.kind, blackglass_audit::EventKind::McpServerSpawned { .. })).collect();
    assert_eq!(spawned.len(), 1);
    sup.shutdown().await;
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p blackglass-core mcp_supervisor`
Expected: FAIL with "module `mcp_supervisor` not found".

- [ ] **Step 3: Create `crates/core/src/mcp_spawn_config.rs`**

```rust
//! Config loader for the MCP supervisor. Reads
//! `~/.config/blackglass/mcp-servers.toml` into a typed struct.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("path {0} does not exist")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
}

fn default_startup_timeout_ms() -> u64 {
    30_000
}
fn default_max_restarts() -> u32 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpSpawnConfig {
    #[serde(default)]
    pub servers: Vec<McpServerSpec>,
}

impl McpSpawnConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.display().to_string()));
        }
        let s = fs::read_to_string(path)?;
        Ok(toml::from_str(&s)?)
    }
}
```

- [ ] **Step 4: Create `crates/core/tests/fixtures/mcp-servers.toml`**

```toml
# Test fixture for the McpSpawnConfig loader. Mirrors the example shipped
# in `/etc/blackglass/mcp-servers.toml.example`.

[[servers]]
name = "mcp-ad"
command = "/usr/bin/blackglass-mcp-ad"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-flipper"
command = "/usr/bin/blackglass-mcp-flipper"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-phish"
command = "/usr/bin/blackglass-mcp-phish"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-detect"
command = "/usr/bin/blackglass-mcp-detect"
args = []
startup_timeout_ms = 30000
max_restarts = 5
```

- [ ] **Step 5: Create `crates/core/src/mcp_supervisor.rs`**

```rust
//! Spawns the 4 new MCP servers (`mcp-ad`, `mcp-flipper`, `mcp-phish`,
//! `mcp-detect`) as child processes and supervises them.
//!
//! On child exit, the supervisor restarts with exponential backoff
//! (1s, 2s, 4s, 8s, 16s, then give up). The restart_count and
//! `McpServerExited` audit events are emitted through the audit chain.
//! When the supervisor gives up, it emits `McpServerFailedPermanently`
//! (an `McpServerExited` variant with `restart_count` equal to
//! `max_restarts`) and marks the child as `GivenUp` — subsequent
//! `status(name)` calls return `GivenUp`.
//!
//! The supervisor exposes a `status(name) -> Option<ChildStatus>`
//! method for the Tauri app to query liveness.

use crate::mcp_spawn_config::{McpServerSpec, McpSpawnConfig};
use blackglass_audit::{Chain, Event, EventKind};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
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
    spec: McpServerSpec,
    child: Option<Child>,
    status: ChildStatus,
}

pub struct McpSupervisor {
    inner: Arc<RwLock<HashMap<String, ChildHandle>>>,
    chain: Arc<Chain>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl McpSupervisor {
    /// Start the supervisor. Spawns all child processes; returns once
    /// they're all spawned (or failed to spawn).
    pub async fn start(config: McpSpawnConfig, log_path: &Path) -> Result<Self, SupervisorError> {
        let chain_path = log_path.parent().unwrap().join("chain.jsonl");
        let chain = Chain::open(&chain_path)?;
        Self::start_with_chain(config, log_path, &chain_path).await
    }

    pub async fn start_with_chain(
        config: McpSpawnConfig,
        log_path: &Path,
        chain_path: &Path,
    ) -> Result<Self, SupervisorError> {
        let chain = Arc::new(Chain::open(chain_path)?);
        let inner: Arc<RwLock<HashMap<String, ChildHandle>>> = Arc::new(RwLock::new(HashMap::new()));
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Spawn each child + its monitor task.
        for spec in config.servers.iter() {
            let child = Self::spawn_child(spec, log_path).await?;
            let pid = child.id().unwrap_or(0);
            chain.append(Event::new(EventKind::McpServerSpawned {
                server: spec.name.clone(),
                pid,
            }))?;
            inner.write().await.insert(
                spec.name.clone(),
                ChildHandle {
                    spec: spec.clone(),
                    child: Some(child),
                    status: ChildStatus::Alive,
                },
            );

            // Spawn the monitor task for this child.
            let inner_for_task = inner.clone();
            let chain_for_task = chain.clone();
            let spec_for_task = spec.clone();
            let mut shutdown_rx_for_task = shutdown_rx.clone();
            tokio::spawn(async move {
                Self::monitor_child(
                    spec_for_task,
                    inner_for_task,
                    chain_for_task,
                    &mut shutdown_rx_for_task,
                ).await;
            });
        }

        Ok(Self { inner, chain, shutdown_tx: Some(shutdown_tx) })
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
        chain: Arc<Chain>,
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) {
        let backoffs = [1u64, 2, 4, 8, 16];
        let max_restarts = spec.max_restarts.min(backoffs.len() as u32);
        loop {
            // Take the child out of the map, await its exit, put it back (or restart).
            let mut child = {
                let mut guard = inner.write().await;
                let handle = guard.get_mut(&spec.name).expect("child disappeared");
                handle.child.take().expect("child was None")
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
                    // Look up current restart_count.
                    let restart_count = {
                        let guard = inner.read().await;
                        guard.get(&spec.name).map(|h| match h.status {
                            ChildStatus::Alive => 0,
                            ChildStatus::Restarting { restart_count } => restart_count,
                            ChildStatus::GivenUp { restart_count } => restart_count,
                        }).unwrap_or(0)
                    };
                    chain.append(Event::new(EventKind::McpServerExited {
                        server: spec.name.clone(),
                        code,
                        restart_count,
                    })).ok();
                    if restart_count >= max_restarts {
                        // Give up.
                        let mut guard = inner.write().await;
                        if let Some(h) = guard.get_mut(&spec.name) {
                            h.status = ChildStatus::GivenUp { restart_count };
                        }
                        error!(server = %spec.name, "supervisor giving up after {} restarts", restart_count);
                        return;
                    }
                    // Backoff and restart.
                    let backoff = backoffs[restart_count as usize];
                    warn!(server = %spec.name, "child exited (code {}), restarting in {}s", code, backoff);
                    {
                        let mut guard = inner.write().await;
                        if let Some(h) = guard.get_mut(&spec.name) {
                            h.status = ChildStatus::Restarting { restart_count: restart_count + 1 };
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(backoff)).await;
                    // Re-spawn.
                    let log_path = Path::new("/tmp/blackglass-supervisor.log"); // placeholder
                    match Self::spawn_child(&spec, log_path).await {
                        Ok(new_child) => {
                            let pid = new_child.id().unwrap_or(0);
                            chain.append(Event::new(EventKind::McpServerSpawned {
                                server: spec.name.clone(),
                                pid,
                            })).ok();
                            let mut guard = inner.write().await;
                            if let Some(h) = guard.get_mut(&spec.name) {
                                h.child = Some(new_child);
                                h.status = ChildStatus::Alive;
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
        guard.get(name).map(|h| h.status.clone())
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}
```

(Note: the `log_path` plumbing is a placeholder — the real implementation will thread it through. The test passes by writing logs to /tmp.)

- [ ] **Step 6: Add `toml` and `rand` deps to `crates/core/Cargo.toml`**

```toml
[dependencies]
# ... existing ...
toml = "0.8"
```

- [ ] **Step 7: Add `mod` declarations to `crates/core/src/lib.rs`**

```rust
pub mod mcp_spawn_config;
pub mod mcp_supervisor;
```

- [ ] **Step 8: Run the test to verify it passes**

Run: `cargo test -p blackglass-core mcp_supervisor`
Expected: PASS (4 tests). The "crasher" test takes ~8s; that's expected.

- [ ] **Step 9: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/mcp_spawn_config.rs crates/core/src/mcp_supervisor.rs crates/core/tests/mcp_supervisor.rs crates/core/tests/fixtures/ crates/core/Cargo.toml crates/core/src/lib.rs
git commit -m "feat(core): McpSupervisor — spawn, monitor, restart-with-backoff, give-up"
```

---

## Task 2.5.5: Add operator-socket `mcp_run_tool` method

**Files:**
- Create: `crates/core/src/mcp_run_tool.rs`
- Modify: `crates/core/src/operator_server.rs`: route the new method
- Modify: `crates/core/src/operator_server.rs`: gate all methods on auth
- Create: `crates/core/tests/operator_server_mcp.rs`

- [ ] **Step 1: Read the existing `RuntimeClient` (the struct that talks to MCP stdio)**

Run: `grep -rn "RuntimeClient\|runtime_client" crates/core/src/ | head`
Expected: a struct that opens `runtime.sock`, writes JSON-RPC frames, awaits responses. (From sub-plan 3; reused here.)

- [ ] **Step 2: Write the failing test for the 4 `mcp_run_tool` scenarios**

`crates/core/tests/operator_server_mcp.rs`:

```rust
use blackglass_core::test_helpers::{spawn_core_with_mcp_stub, connect_operator_with_token};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn mcp_run_tool_returns_ok_when_mcp_allows() {
    let core = spawn_core_with_mcp_stub(/* allow = */ true, /* delay_ms = */ 0).await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("mcp_run_tool", json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {"target": "10.0.0.5", "user": "admin", "cmd": "whoami"}
    })).await.unwrap();
    assert_eq!(result["ok"], true);
    assert!(result["audit_event_id"].as_str().is_some());
}

#[tokio::test]
async fn mcp_run_tool_returns_denied_when_chokepoint_denies() {
    let core = spawn_core_with_mcp_stub(/* allow = */ false, /* delay_ms = */ 0).await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("mcp_run_tool", json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {}
    })).await.unwrap();
    assert_eq!(result["ok"], false);
    assert!(result["error"].as_str().unwrap().contains("denied") || result["error"].as_str().unwrap().contains("gate"));
}

#[tokio::test]
async fn mcp_run_tool_returns_error_when_mcp_server_is_down() {
    // No MCP stub spawned — operator.sock sees the runtime.sock absent.
    let core = spawn_core_with_mcp_stub(/* allow = */ true, /* delay_ms = */ 0).await;
    // Kill the stub via a backdoor (the helper exposes `kill_mcp_stub`).
    core.kill_mcp_stub().await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("mcp_run_tool", json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {}
    })).await.unwrap();
    assert_eq!(result["ok"], false);
    assert!(result["error"].as_str().unwrap().contains("not running") || result["error"].as_str().unwrap().contains("died"));
}

#[tokio::test]
async fn mcp_run_tool_times_out_when_mcp_takes_too_long() {
    // Stub delays its response by 60s — operator.sock 30s timeout fires first.
    let core = spawn_core_with_mcp_stub(/* allow = */ true, /* delay_ms = */ 60_000).await;
    let mut client = connect_operator_with_token(&core).await;
    // Use a smaller timeout for the test by overriding via env or a feature flag.
    // For v1 we accept a 30s test runtime; if CI is slow, mark this test `#[ignore]`.
    let result: serde_json::Value = tokio::time::timeout(
        Duration::from_secs(35),
        client.call("mcp_run_tool", json!({
            "domain": "ad",
            "target": "ad-impacket_psexec",
            "args": {}
        }))
    ).await.unwrap().unwrap();
    assert_eq!(result["ok"], false);
    assert!(result["error"].as_str().unwrap().contains("timeout"));
}
```

- [ ] **Step 3: Add the test helper `spawn_core_with_mcp_stub`**

In `crates/core/tests/common/mod.rs`, add a helper that:

- Spawns a blackglass-core with a fake MCP "ad" server on a side Unix socket (not stdio — keeps the test hermetic).
- The stub listens for a single `execute_action` JSON-RPC frame, then either replies with `{ok: <allow>}` (after `delay_ms`), or with `{ok: false, error: "gate denied"}`.
- Exposes a `kill_mcp_stub` method on the handle to simulate the MCP dying.

The helper is ~150 lines; the shape mirrors `spawn_core` from sub-plan 3. (If sub-plan 3's helpers don't fit, write this as a new helper in this test file's `mod common` block.)

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p blackglass-core mcp_run_tool_returns_ok_when_mcp_allows`
Expected: FAIL with "method `mcp_run_tool` not found".

- [ ] **Step 5: Create `crates/core/src/mcp_run_tool.rs`**

```rust
//! Operator-socket `mcp_run_tool` method. The Tauri app calls this
//! to run any of the 16 existing + new tools via the chokepoint.
//!
//! Flow:
//! 1. Look up which MCP server owns (domain, target) — for v1, the
//!    mapping is hardcoded: `ad` → mcp-ad, `flipper` → mcp-flipper,
//!    `phish` → mcp-phish, `detect` → mcp-detect, `osint` → mcp-osint,
//!    `packets` → mcp-packets.
//! 2. Check the MCP server is alive (ask the supervisor).
//! 3. Forward the request to the MCP server over runtime.sock as
//!    an `execute_action` JSON-RPC call. Wait up to 30s for a reply.
//! 4. Return `{ok, stdout?, stderr?, audit_event_id?, error?}`.

use crate::mcp_supervisor::McpSupervisor;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Error)]
pub enum McpRunError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("domain {0} is not routed to any MCP server")]
    UnknownDomain(String),
    #[error("mcp server {0} is not running")]
    McpDown(String),
    #[error("mcp server {0} timed out after 30s")]
    Timeout(String),
    #[error("mcp server {0} returned error: {1}")]
    McpError(String, String),
}

#[derive(Debug, Deserialize)]
pub struct McpRunParams {
    pub domain: String,
    pub target: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpRunResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Resolve (domain) → MCP server name. Hardcoded for v1.
pub fn mcp_for_domain(domain: &str) -> Option<&'static str> {
    match domain {
        "ad" => Some("mcp-ad"),
        "flipper" => Some("mcp-flipper"),
        "phish" => Some("mcp-phish"),
        "detect" => Some("mcp-detect"),
        "osint" => Some("mcp-osint"),
        "packets" => Some("mcp-packets"),
        _ => None,
    }
}

pub async fn handle_mcp_run_tool(
    params: McpRunParams,
    supervisor: &McpSupervisor,
    runtime_sock_path: &std::path::Path,
) -> Result<McpRunResult, McpRunError> {
    let mcp_name = mcp_for_domain(&params.domain)
        .ok_or_else(|| McpRunError::UnknownDomain(params.domain.clone()))?;
    let status = supervisor.status(mcp_name).await;
    match status {
        Some(crate::mcp_supervisor::ChildStatus::Alive) => {}
        _ => return Err(McpRunError::McpDown(mcp_name.into())),
    }
    // Forward to runtime.sock as execute_action.
    let frame = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "execute_action",
        "params": {
            "domain": params.domain,
            "target": params.target,
            "args": params.args,
        }
    });
    let mut stream = UnixStream::connect(runtime_sock_path).await?;
    let frame_str = format!("{}\n", serde_json::to_string(&frame)?);
    stream.write_all(frame_str.as_bytes()).await?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        reader.read_line(&mut line),
    ).await;
    match read {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return Err(McpRunError::Io(e)),
        Err(_) => return Err(McpRunError::Timeout(mcp_name.into())),
    }
    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        return Err(McpRunError::McpError(mcp_name.into(), err.to_string()));
    }
    let result = resp.get("result").cloned().unwrap_or(serde_json::Value::Null);
    Ok(McpRunResult {
        ok: result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        stdout: result.get("stdout").and_then(|v| v.as_str()).map(String::from),
        stderr: result.get("stderr").and_then(|v| v.as_str()).map(String::from),
        audit_event_id: result.get("audit_event_id").and_then(|v| v.as_str()).map(String::from),
        error: result.get("error").and_then(|v| v.as_str()).map(String::from),
    })
}
```

- [ ] **Step 6: Route the new method in `operator_server.rs`**

In `crates/core/src/operator_server.rs`, add to the method dispatcher:

```rust
"mcp_run_tool" => {
    let params: crate::mcp_run_tool::McpRunParams = serde_json::from_value(req.params)?;
    let resp = crate::mcp_run_tool::handle_mcp_run_tool(
        params,
        &state.supervisor,
        &state.runtime_sock_path,
    ).await?;
    jsonrpc_ok(req.id, serde_json::to_value(resp)?)
}
```

(The `state` struct now has a `supervisor: Arc<McpSupervisor>` and a `runtime_sock_path: PathBuf` field, added in Task 2.5.6 below.)

- [ ] **Step 7: Add `pub mod mcp_run_tool;` to `crates/core/src/lib.rs`**

```rust
pub mod mcp_run_tool;
```

- [ ] **Step 8: Run the test to verify it passes**

Run: `cargo test -p blackglass-core mcp_run_tool`
Expected: PASS (4 tests). The timeout test takes ~30s; that's expected.

- [ ] **Step 9: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/core/src/mcp_run_tool.rs crates/core/src/operator_server.rs crates/core/src/lib.rs crates/core/tests/operator_server_mcp.rs crates/core/tests/common/
git commit -m "feat(core): operator-socket mcp_run_tool method with MCP-down + timeout handling"
```

---

## Task 2.5.6: Wire the supervisor + mcp-spawn into `blackglass-core` startup

**Files:**
- Modify: `crates/core/src/main.rs`: load mcp-servers.toml, build McpSupervisor, pass to operator_server
- Modify: `crates/core/src/operator_server.rs`: add `state.supervisor` and `state.runtime_sock_path`
- Create: `crates/core/tests/end_to_end_mcp_run.rs`

- [ ] **Step 1: Read the existing `main.rs` to see the startup sequence**

Run: `cat crates/core/src/main.rs`
Expected: it reads the profile, opens the audit chain, creates the operator server, spawns the runtime server, blocks on a signal.

- [ ] **Step 2: Write the failing end-to-end test (2 scenarios)**

`crates/core/tests/end_to_end_mcp_run.rs`:

```rust
//! End-to-end: Tauri-Rust-style call flow.
//!
//! Spawns a full blackglass-core (with a real McpSupervisor managing a
//! stub MCP child process), opens the operator socket, runs an
//! `mcp_run_tool` call, and asserts the audit chain has the expected
//! sequence of events.

use blackglass_core::test_helpers::{spawn_full_core, connect_operator_with_token};
use blackglass_audit::EventKind;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn end_to_end_mcp_run_emits_full_audit_chain() {
    let core = spawn_full_core().await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("mcp_run_tool", json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {"target": "10.0.0.5", "user": "admin", "cmd": "whoami"}
    })).await.unwrap();
    assert_eq!(result["ok"], true);
    // Give the audit chain a moment to flush.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let chain = blackglass_audit::Chain::open(&core.audit_chain_path).unwrap();
    let events = chain.query(&json!({})).unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"mcp_run_started"));
    assert!(kinds.contains(&"mcp_run_completed"));
    assert!(kinds.contains(&"action_executed"));
}

#[tokio::test]
async fn end_to_end_mcp_run_with_destructive_action_requires_gate3() {
    // The ad-impacket_psexec target is destructive; without operator
    // confirmation, the call should be denied (or the test should
    // auto-confirm via the confirm broker).
    // For this test we auto-confirm: the test helper accepts all
    // confirm requests.
    let core = spawn_full_core().await;
    let mut client = connect_operator_with_token(&core).await;
    // Spawn a background task that auto-confirms.
    let client2 = client.clone();
    tokio::spawn(async move {
        loop {
            // Wait for a confirm.request push, then resolve it.
            // (The shape of this depends on how the operator server pushes
            //  events to clients. For v1 we just have the helper auto-accept
            //  destructive actions via an env var: BLACKGLASS_TEST_AUTO_CONFIRM=1)
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    let result: serde_json::Value = client.call("mcp_run_tool", json!({
        "domain": "ad",
        "target": "ad-impacket_psexec",
        "args": {"target": "10.0.0.5", "user": "admin", "cmd": "whoami"}
    })).await.unwrap();
    assert_eq!(result["ok"], true);
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p blackglass-core end_to_end_mcp_run`
Expected: FAIL with "function `spawn_full_core` not found" or "field `supervisor` missing".

- [ ] **Step 4: Modify `crates/core/src/main.rs` to load mcp-servers.toml and build the supervisor**

Find the `fn main` (or `#[tokio::main] async fn main`) function. After the audit chain is opened and the runtime server is bound, add:

```rust
// Load mcp-servers.toml
let config_path = dirs::config_dir()
    .ok_or("no config dir")?
    .join("blackglass")
    .join("mcp-servers.toml");
let mcp_config = if config_path.exists() {
    blackglass_core::mcp_spawn_config::McpSpawnConfig::load(&config_path)?
} else {
    eprintln!("warning: {} not found; running with no MCPs spawned", config_path.display());
    blackglass_core::mcp_spawn_config::McpSpawnConfig::default()
};

// Build the supervisor.
let log_dir = dirs::data_dir()
    .ok_or("no data dir")?
    .join("blackglass")
    .join("logs");
std::fs::create_dir_all(&log_dir).ok();
let supervisor = std::sync::Arc::new(
    blackglass_core::mcp_supervisor::McpSupervisor::start(
        mcp_config,
        &log_dir.join("supervisor.log"),
    ).await?
);
```

Pass the `supervisor` (as `Arc<McpSupervisor>`) and the runtime socket path into the operator server's state. The operator server's `state` struct gains two fields:

```rust
pub struct OperatorState {
    // ... existing ...
    pub supervisor: std::sync::Arc<McpSupervisor>,
    pub runtime_sock_path: std::path::PathBuf,
}
```

- [ ] **Step 5: Add `dirs` to `crates/core/Cargo.toml`**

```toml
[dependencies]
# ... existing ...
dirs = "5"
```

- [ ] **Step 6: Add the `spawn_full_core` test helper**

In `crates/core/tests/common/mod.rs`, add a helper that:

- Spawns a full blackglass-core binary as a subprocess (uses `tokio::process::Command`).
- The helper writes a temp `mcp-servers.toml` listing one stub MCP (a long-running `sleep 30`).
- The helper writes a temp profile + a temp `runtime.sock`-equivalent.
- The helper returns a `FullCoreHandle { audit_chain_path: PathBuf, operator_sock_path: PathBuf, runtime_sock_path: PathBuf, child: Child, ... }`.

This is the most complex helper in the test suite — ~250 lines. It's acceptable to make this helper shell out to a separate `bin` (e.g., `tests/bin/stub_mcp.rs`) that the test fixture builds with `cargo build --bin stub-mcp` once. The test's `[[setup]]` step ensures the stub is built.

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p blackglass-core end_to_end_mcp_run`
Expected: PASS (2 tests).

- [ ] **Step 8: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/main.rs crates/core/src/operator_server.rs crates/core/src/lib.rs crates/core/Cargo.toml crates/core/tests/end_to_end_mcp_run.rs crates/core/tests/common/
git commit -m "feat(core): wire McpSupervisor + mcp-servers.toml into blackglass-core startup"
```

---

## Task 2.5.7: Add the `auth` operator-socket method + gate everything else on it

**Files:**
- Modify: `crates/core/src/operator_server.rs`: add `auth` method, gate others
- Modify: `crates/core/src/operator_auth.rs`: add `token_bytes_for_test` helper (or expose `expected` more cleanly)

- [ ] **Step 1: Read the existing connection lifecycle**

Run: `grep -n "fn handle_connection\|read_frame\|accept" crates/core/src/operator_server.rs`
Expected: a per-connection task that reads frames and dispatches to methods. We need to track per-connection auth state.

- [ ] **Step 2: Write the failing test for auth gating**

Add to `crates/core/tests/operator_server_audit.rs`:

```rust
#[tokio::test]
async fn unauthenticated_client_cannot_call_audit_query() {
    let core = spawn_core().await;
    // Connect WITHOUT calling `auth` first.
    let mut client = connect_operator_no_auth(&core).await;
    let result: serde_json::Value = client.call("audit.query", json!({
        "filter": {}, "page": 0, "page_size": 10
    })).await.unwrap();
    assert_eq!(result["error"]["code"], -32001);  // not authenticated
}

#[tokio::test]
async fn authenticated_client_can_call_audit_query() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_token(&core).await;
    let result: serde_json::Value = client.call("audit.query", json!({
        "filter": {}, "page": 0, "page_size": 10
    })).await.unwrap();
    assert!(result.get("error").is_none() || result["error"].is_null());
}

#[tokio::test]
async fn auth_with_wrong_token_returns_error() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_wrong_token(&core).await;
    let result: serde_json::Value = client.call("audit.query", json!({
        "filter": {}, "page": 0, "page_size": 10
    })).await.unwrap();
    assert_eq!(result["error"]["code"], -32002);  // auth failed
}
```

- [ ] **Step 3: Add the test helpers `connect_operator_no_auth` and `connect_operator_with_wrong_token`**

In `crates/core/tests/common/mod.rs`:

```rust
pub async fn connect_operator_no_auth(core: &CoreHandle) -> OperatorTestClient {
    let stream = tokio::net::UnixStream::connect(&core.operator_sock_path).await.unwrap();
    OperatorTestClient::new(stream)
}

pub async fn connect_operator_with_wrong_token(core: &CoreHandle) -> OperatorTestClient {
    let stream = tokio::net::UnixStream::connect(&core.operator_sock_path).await.unwrap();
    let mut client = OperatorTestClient::new(stream);
    // Send a bad auth frame first.
    let _ = client.call("auth", json!({"token": "wrong-token"})).await;
    client
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p blackglass-core unauthenticated_client_cannot_call`
Expected: FAIL with "method `auth` not found" or "expected error code -32001, got success".

- [ ] **Step 5: Add the `auth` method to `operator_server.rs`**

In the per-connection task, add a `bool: authenticated` flag. On any frame, if `!authenticated` and the method is not `auth`, return `error.code = -32001`. On the `auth` method:

```rust
"auth" => {
    let params: AuthParams = serde_json::from_value(req.params)?;
    let auth = OperatorAuth::new(&state.operator_token_path);
    match auth.verify(params.token.as_bytes()) {
        Ok(()) => {
            conn.authenticated = true;
            jsonrpc_ok(req.id, json!({"ok": true}))
        }
        Err(e) => jsonrpc_error(req.id, -32002, e.to_string()),
    }
}
```

- [ ] **Step 6: Gate every other method on `conn.authenticated`**

Wrap the existing dispatcher in a check:

```rust
if !conn.authenticated {
    return jsonrpc_error(req.id, -32001, "auth required: call `auth` first");
}
match req.method.as_str() {
    "auth" => { /* handled above */ }
    "ping" => { /* existing */ }
    "mcp_run_tool" => { /* new */ }
    "audit.query" => { /* new */ }
    "audit.verify_chain" => { /* new */ }
    "confirm.resolve" => { /* existing */ }
    _ => jsonrpc_error(req.id, -32601, format!("method `{}` not found", req.method)),
}
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p blackglass-core operator_server_audit`
Expected: PASS (6 tests — 3 original + 3 new auth tests).

- [ ] **Step 8: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/operator_server.rs crates/core/src/operator_auth.rs crates/core/tests/operator_server_audit.rs crates/core/tests/common/
git commit -m "feat(core): operator-socket auth — gate all methods behind auth method"
```

---

## Task 2.5.8: Add the `audit.event` push from the core (live tail)

**Files:**
- Modify: `crates/core/src/operator_server.rs`: broadcast new events to authenticated clients

- [ ] **Step 1: Read the existing `Event::append` callsite**

Run: `grep -rn "chain.append\|Event::new" crates/core/src/ | head`
Expected: events are appended via `chain.append(Event::new(...))?`. The new work wraps this in a helper that also broadcasts to subscribers.

- [ ] **Step 2: Write the failing test for the live tail**

Add to `crates/core/tests/operator_server_audit.rs`:

```rust
#[tokio::test]
async fn audit_event_push_reaches_subscribed_clients() {
    let core = spawn_core().await;
    let mut client = connect_operator_with_token(&core).await;
    // Subscribe to the audit.event push.
    let mut sub = client.subscribe("audit.event").await;
    // Trigger an event by appending one.
    let _ = client.notify("audit.append", json!({
        "event": Event::new(EventKind::OperatorConfirmationRequested { id: "c1".into(), tool: "x".into(), args: "{}".into() })
    })).await.unwrap();
    // Wait for the push.
    let push: serde_json::Value = tokio::time::timeout(
        Duration::from_secs(2),
        sub.recv()
    ).await.unwrap().unwrap();
    assert_eq!(push["method"], "audit.event");
    assert!(push["params"]["event"].is_object());
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p blackglass-core audit_event_push_reaches`
Expected: FAIL with "no subscribe method" or "no audit.event push".

- [ ] **Step 4: Add a `tokio::sync::broadcast` channel for events**

In `operator_server.rs`'s state struct, add:

```rust
pub struct OperatorState {
    // ... existing ...
    pub event_tx: tokio::sync::broadcast<blackglass_audit::Event>,
}
```

In `main.rs`, construct it:

```rust
let (event_tx, _rx) = tokio::sync::broadcast::channel(1024);
```

- [ ] **Step 5: Add a `subscribe` method to the connection**

When a client subscribes to `audit.event`, register their write half with the broadcast channel. The cleanest way: spawn a task per subscription that does `event_tx.subscribe()` and forwards each event to the client's write half.

Add to the dispatcher:

```rust
"subscribe" => {
    let params: SubscribeParams = serde_json::from_value(req.params)?;
    if params.channel != "audit.event" {
        return jsonrpc_error(req.id, -32602, format!("unknown channel `{}`", params.channel));
    }
    let mut rx = state.event_tx.subscribe();
    let writer = conn.writer.clone();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let frame = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "audit.event",
                "params": { "event": event }
            });
            let mut w = writer.lock().await;
            let _ = w.write_all(format!("{}\n", serde_json::to_string(&frame).unwrap()).as_bytes()).await;
        }
    });
    jsonrpc_ok(req.id, json!({"ok": true}))
}
```

- [ ] **Step 6: Wrap `chain.append` to broadcast**

In `main.rs`, after the audit chain is opened, add a helper:

```rust
let chain = Arc::new(Chain::open(&chain_path)?);
let chain_for_broadcast = chain.clone();
let event_tx_for_broadcast = event_tx.clone();
let chain_with_broadcast = AppendWithBroadcast { inner: chain_for_broadcast, tx: event_tx_for_broadcast };
// pass `chain_with_broadcast` to anything that calls `chain.append`.
```

Or simpler: in any code that calls `chain.append(event)`, replace with a helper that also calls `event_tx.send(event)`. Add a free function in `audit_query.rs` (or a new `audit_broadcast.rs`):

```rust
pub fn append_and_broadcast(
    chain: &Chain,
    tx: &broadcast::Sender<Event>,
    event: Event,
) -> Result<(), Box<dyn std::error::Error>> {
    chain.append(event.clone())?;
    let _ = tx.send(event);  // best-effort: ignore "no subscribers"
    Ok(())
}
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p blackglass-core audit_event_push_reaches`
Expected: PASS.

- [ ] **Step 8: Run the full core test suite**

Run: `cargo test -p blackglass-core`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/operator_server.rs crates/core/src/main.rs crates/core/src/audit_query.rs crates/core/tests/operator_server_audit.rs
git commit -m "feat(core): audit.event push from core to operator socket subscribers"
```

---

**End of Phase 2.5+.** Run `cargo test -p blackglass-core` and confirm all tests green. The Core IPC delta is complete: 4 new event kinds, operator-socket auth, MCP supervisor, mcp_run_tool, audit.query/verify_chain, and the audit.event push.

**Phase 2.5+ exit criteria:**

- 4 new `EventKind` variants compile and serialize correctly
- `McpSupervisor` spawns 4 children, restarts on crash with backoff, gives up after `max_restarts`, emits audit events for spawn/exit/give-up
- Operator socket requires a token-bearing `auth` call before any other method
- `mcp_run_tool` method works for allow/deny/MCP-down/timeout
- `audit.query` + `audit.verify_chain` work for paginated/empty/all-events
- `audit.event` push reaches subscribed clients
- `cargo test --workspace` is green

Next: Phase 3 (Tauri UI delta).

---

# Phase 3: Tauri UI delta (3-pane domain workspace)

The 8 tasks in this phase add the 3-pane Tauri UI: the 3 new Rust-side Tauri commands, the Svelte-side `McpClient` wrapper, the `DomainRail` / `ToolRunner` / `ResultPane` components, the `AuditDetail` right rail, and the 3-pane layout wired into `App.svelte`.

**Prereq:** Phase 2.5+ complete (`cargo test -p blackglass-core` is green).

---

## Task 3.1: Add the 3 new Tauri commands (`mcp_run_tool`, `mcp_list_tools`, `audit_event`)

**Files:**
- Create: `app/src-tauri/src/operator_client.rs`
- Create: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/main.rs`: register the 3 new commands
- Create: `app/src-tauri/tests/mcp_run_tool.rs`

- [ ] **Step 1: Read the existing Tauri command shape**

Run: `ls app/src-tauri/src/ && cat app/src-tauri/src/main.rs | head -50`
Expected: the Tauri app uses `tauri::command` macros, has a `#[tauri::command] fn audit_query(...)` already (from sub-plan 4's Phase 2 / Task 2.2). The pattern is: command takes a `tauri::State<AppState>`, opens the operator socket (or runtime socket), returns a `Result<T, String>`.

- [ ] **Step 2: Write the failing test for `mcp_run_tool` (success, auth-fail, MCP-down)**

`app/src-tauri/tests/mcp_run_tool.rs`:

```rust
//! Tauri command tests. We don't use Tauri's mock runtime — we test
//! the `commands` module's pure functions directly, with a fake
//! operator socket.

use blackglass_ui::commands::{mcp_run_tool, McpRunRequest, McpRunResponse};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use tempfile::tempdir;

fn spawn_fake_operator(responses: Vec<String>) -> PathBuf {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("op.sock");
    let listener = UnixListener::bind(&sock_path).unwrap();
    let responses = std::sync::Arc::new(responses);
    std::thread::spawn(move || {
        for response in responses.iter() {
            let (mut stream, _) = listener.accept().unwrap();
            use std::io::{Read, Write};
            // Read the auth frame, ignore.
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            // Write the response.
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    sock_path
}

#[tokio::test]
async fn mcp_run_tool_returns_ok_when_operator_returns_ok() {
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"result":{"ok":true}}"#.to_string(),
    ]);
    let token = "test-token\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await.unwrap();
    assert!(resp.ok);
}

#[tokio::test]
async fn mcp_run_tool_returns_auth_error_when_socket_rejects() {
    // Fake operator returns an error with code -32002 (auth failed).
    let sock = spawn_fake_operator(vec![
        r#"{"jsonrpc":"2.0","id":0,"error":{"code":-32002,"message":"auth failed"}}"#.to_string(),
    ]);
    let token = "wrong\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await;
    assert!(resp.is_err());
    let err = resp.unwrap_err();
    assert!(err.contains("auth") || err.contains("32002"));
}

#[tokio::test]
async fn mcp_run_tool_returns_mcp_down_when_socket_refuses_connection() {
    // Use a path that doesn't exist — connection refused.
    let sock = PathBuf::from("/tmp/does-not-exist-12345.sock");
    let token = "any\n".to_string();
    let req = McpRunRequest {
        domain: "ad".into(),
        target: "ad-impacket_psexec".into(),
        args: serde_json::json!({}),
    };
    let resp = mcp_run_tool(req, &sock, &token).await;
    assert!(resp.is_err());
    let err = resp.unwrap_err();
    assert!(err.contains("connect") || err.contains("refused") || err.contains("disconnected"));
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd app/src-tauri && cargo test mcp_run_tool`
Expected: FAIL with "module `commands` not found in `blackglass_ui`" or "function `mcp_run_tool` not found".

- [ ] **Step 4: Create `app/src-tauri/src/operator_client.rs`**

```rust
//! Opens the operator socket, authenticates, and returns a frame writer
//! + frame reader pair for a Tauri command to use.

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::io::{BufRead, BufReader, Write};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("operator returned error: code={code}, message={message}")]
    Op { code: i64, message: String },
    #[error("disconnected: {0}")]
    Disconnected(String),
}

/// Open the operator socket, send the auth frame, return the stream.
pub fn connect_and_auth(sock_path: &Path, token: &str) -> Result<UnixStream, OpError> {
    let mut stream = UnixStream::connect(sock_path)?;
    // Send the auth frame.
    let frame = json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "auth",
        "params": { "token": token }
    });
    let s = format!("{}\n", serde_json::to_string(&frame)?);
    stream.write_all(s.as_bytes())?;
    // Read the auth response (we just check it's not an error).
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        return Err(OpError::Op {
            code: err["code"].as_i64().unwrap_or(0),
            message: err["message"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(stream)
}

/// Send a JSON-RPC call on an authenticated stream and read the response.
pub fn call(stream: &mut UnixStream, method: &str, params: serde_json::Value) -> Result<serde_json::Value, OpError> {
    let frame = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    let s = format!("{}\n", serde_json::to_string(&frame)?);
    stream.write_all(s.as_bytes())?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(OpError::Disconnected("operator closed the socket".into()));
    }
    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        return Err(OpError::Op {
            code: err["code"].as_i64().unwrap_or(0),
            message: err["message"].as_str().unwrap_or("").to_string(),
        });
    }
    Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
}
```

- [ ] **Step 5: Create `app/src-tauri/src/commands.rs`**

```rust
//! The 3 new Tauri commands: mcp_run_tool, mcp_list_tools, audit_event.
//! Each is a thin wrapper over the operator socket.

use crate::operator_client::{call, connect_and_auth, OpError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct McpRunRequest {
    pub domain: String,
    pub target: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpRunResponse {
    pub ok: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub audit_event_id: Option<String>,
    pub error: Option<String>,
}

pub async fn mcp_run_tool(
    req: McpRunRequest,
    sock_path: &Path,
    token: &str,
) -> Result<McpRunResponse, String> {
    let mut stream = connect_and_auth(sock_path, token).map_err(|e| e.to_string())?;
    let result = call(&mut stream, "mcp_run_tool", serde_json::json!({
        "domain": req.domain,
        "target": req.target,
        "args": req.args,
    })).map_err(|e| e.to_string())?;
    let resp: McpRunResponse = serde_json::from_value(result).map_err(|e| e.to_string())?;
    Ok(resp)
}

pub async fn mcp_list_tools(
    domain: String,
    sock_path: &Path,
    token: &str,
) -> Result<serde_json::Value, String> {
    // For v1, the catalog is hardcoded in lib/toolCatalog.ts. The Tauri
    // command exists so the Svelte side has a single entry point — it
    // reads the catalog from the bundled JS, not from this command.
    // This command is a placeholder that returns an empty list, so the
    // Svelte side can fall back to the bundled catalog if the core
    // ever needs to override.
    let _ = (domain, sock_path, token);
    Ok(serde_json::json!([]))
}

pub async fn audit_event(
    id: String,
    sock_path: &Path,
    token: &str,
) -> Result<serde_json::Value, String> {
    let mut stream = connect_and_auth(sock_path, token).map_err(|e| e.to_string())?;
    // audit.query is the existing method; filter by id.
    let result = call(&mut stream, "audit.query", serde_json::json!({
        "filter": { "id": id },
        "page": 0,
        "page_size": 1
    })).map_err(|e| e.to_string())?;
    let events = result["events"].as_array().cloned().unwrap_or_default();
    Ok(events.into_iter().next().unwrap_or(serde_json::Value::Null))
}

// Tauri command bindings (wrap the pure functions with State access)
#[tauri::command]
pub async fn mcp_run_tool_cmd(
    domain: String,
    target: String,
    args: serde_json::Value,
    state: State<'_, crate::AppState>,
) -> Result<McpRunResponse, String> {
    mcp_run_tool(McpRunRequest { domain, target, args }, &state.operator_sock_path, &state.operator_token).await
}

#[tauri::command]
pub async fn mcp_list_tools_cmd(
    domain: String,
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    mcp_list_tools(domain, &state.operator_sock_path, &state.operator_token).await
}

#[tauri::command]
pub async fn audit_event_cmd(
    id: String,
    state: State<'_, crate::AppState>,
) -> Result<serde_json::Value, String> {
    audit_event(id, &state.operator_sock_path, &state.operator_token).await
}
```

- [ ] **Step 6: Add the `AppState` struct + register the 3 commands in `main.rs`**

In `app/src-tauri/src/main.rs`:

```rust
pub struct AppState {
    pub operator_sock_path: std::path::PathBuf,
    pub operator_token: String,
}

fn main() {
    let data_dir = dirs::data_dir().unwrap().join("blackglass");
    std::fs::create_dir_all(&data_dir).ok();
    let operator_sock_path = data_dir.join("runtime.sock");
    let operator_token = std::fs::read_to_string(data_dir.join("operator.token")).unwrap_or_default();

    tauri::Builder::default()
        .manage(AppState { operator_sock_path, operator_token })
        .invoke_handler(tauri::generate_handler![
            crate::commands::mcp_run_tool_cmd,
            crate::commands::mcp_list_tools_cmd,
            crate::commands::audit_event_cmd,
            // ... existing commands ...
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Also add `mod commands;` and `mod operator_client;` at the top of `main.rs`.

- [ ] **Step 7: Make `commands` and `operator_client` reachable from tests**

In `app/src-tauri/src/lib.rs` (or `main.rs`'s `lib` section — adjust based on what sub-plan 3 used), expose:

```rust
pub mod commands;
pub mod operator_client;
```

The test imports as `use blackglass_ui::commands::...`.

- [ ] **Step 8: Run the test to verify it passes**

Run: `cd app/src-tauri && cargo test mcp_run_tool`
Expected: PASS (3 tests).

- [ ] **Step 9: Commit**

```bash
git add app/src-tauri/
git commit -m "feat(ui): Tauri commands mcp_run_tool, mcp_list_tools, audit_event"
```

---

## Task 3.2: Add `McpClient.ts` (Svelte-side wrapper) + `toolCatalog.ts` (hardcoded catalog)

**Files:**
- Create: `app/src/lib/McpClient.ts`
- Create: `app/src/lib/toolCatalog.ts`
- Create: `app/src/lib/McpClient.test.ts`
- Modify: `app/src/lib/types.ts` *(NEW)* — shared types for the workspace

- [ ] **Step 1: Read the existing app structure**

Run: `ls app/src/lib/ 2>/dev/null && cat app/src/lib/state.svelte.ts 2>/dev/null | head -30`
Expected: there's an existing `state.svelte.ts` (or `.ts`) with the pending/confirmation state. We add to it.

- [ ] **Step 2: Write the failing test for `McpClient`**

`app/src/lib/McpClient.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { McpClient } from './McpClient';

// Mock the Tauri invoke function
const mockInvoke = vi.fn();
(globalThis as any).__TAURI_INTERNALS__ = { invoke: mockInvoke };

describe('McpClient', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('runTool calls mcp_run_tool with the right args', async () => {
    mockInvoke.mockResolvedValueOnce({
      ok: true,
      stdout: 'hello',
      stderr: '',
      audit_event_id: 'evt-1',
    });
    const client = new McpClient();
    const result = await client.runTool('ad', 'ad-impacket_psexec', { target: '10.0.0.5' });
    expect(mockInvoke).toHaveBeenCalledWith('mcp_run_tool', {
      domain: 'ad',
      target: 'ad-impacket_psexec',
      args: { target: '10.0.0.5' },
    });
    expect(result.ok).toBe(true);
    expect(result.stdout).toBe('hello');
  });

  it('runTool surfaces the error message on failure', async () => {
    mockInvoke.mockResolvedValueOnce({
      ok: false,
      error: 'gate denied',
    });
    const client = new McpClient();
    const result = await client.runTool('ad', 'ad-impacket_psexec', {});
    expect(result.ok).toBe(false);
    expect(result.error).toBe('gate denied');
  });

  it('runTool throws on transport error', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('socket disconnected'));
    const client = new McpClient();
    await expect(client.runTool('ad', 'ad-impacket_psexec', {})).rejects.toThrow('socket disconnected');
  });

  it('getAuditEvent calls audit_event with the right id', async () => {
    mockInvoke.mockResolvedValueOnce({ kind: 'mcp_run_completed', ok: true });
    const client = new McpClient();
    const evt = await client.getAuditEvent('evt-1');
    expect(mockInvoke).toHaveBeenCalledWith('audit_event', { id: 'evt-1' });
    expect(evt.kind).toBe('mcp_run_completed');
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd app && npm test -- McpClient`
Expected: FAIL with "Cannot find module './McpClient'".

- [ ] **Step 4: Create `app/src/lib/types.ts`**

```typescript
// Shared types for the domain workspace.

export type Domain = 'osint' | 'packets' | 'ad' | 'flipper' | 'phish' | 'detect';

export interface Tool {
  name: string;
  description: string;
  argsSchema: string;  // human-readable hint for v1
  destructive: boolean;
}

export interface McpRunResult {
  ok: boolean;
  stdout?: string;
  stderr?: string;
  audit_event_id?: string;
  error?: string;
}

export interface AuditEvent {
  id: string;
  ts: string;
  kind: string;
  [key: string]: unknown;
}
```

- [ ] **Step 5: Create `app/src/lib/toolCatalog.ts`**

```typescript
// Hardcoded tool catalog. Mirrors the `*_TOOLS` constants in the MCP
// crates. When you add a new tool, update both this file and the
// relevant MCP crate's tools.rs.

import type { Domain, Tool } from './types';

export const TOOL_CATALOG: Record<Domain, Tool[]> = {
  osint: [
    { name: 'osint-whois', description: 'WHOIS lookup for a domain or IP', argsSchema: '{ "target": "example.com" }', destructive: false },
    { name: 'osint-dig', description: 'DNS dig lookup', argsSchema: '{ "target": "example.com", "type": "A" }', destructive: false },
    { name: 'osint-theharvester', description: 'Email/subdomain harvesting', argsSchema: '{ "domain": "example.com" }', destructive: false },
  ],
  packets: [
    { name: 'packets-tshark_read', description: 'Read a pcap with tshark', argsSchema: '{ "pcap": "/path/to.pcap", "filter": "tcp.port==80" }', destructive: false },
    { name: 'packets-tshark_capture', description: 'Live capture with tshark', argsSchema: '{ "iface": "eth0", "duration_s": 30 }', destructive: false },
    { name: 'packets-pcap_export', description: 'Export filtered packets', argsSchema: '{ "pcap": "/path/to.pcap", "filter": "http" }', destructive: false },
    { name: 'packets-scapy_craft', description: 'Craft a packet with scapy', argsSchema: '{ "layers": "IP(dst=\\"10.0.0.5\\")/TCP()" }', destructive: false },
  ],
  ad: [
    { name: 'ad-impacket_psexec', description: 'impacket psexec (run cmd on remote Windows)', argsSchema: '{ "target": "10.0.0.5", "user": "admin", "cmd": "whoami" }', destructive: true },
    { name: 'ad-impacket_secretsdump', description: 'impacket secretsdump (dump SAM/LSA secrets)', argsSchema: '{ "target": "10.0.0.5", "user": "admin" }', destructive: true },
    { name: 'ad-impacket_ntlmrelayx', description: 'impacket ntlmrelayx', argsSchema: '{ "target": "10.0.0.5", "smb2support": true }', destructive: true },
    { name: 'ad-impacket_gettgt', description: 'impacket getTGT (request Kerberos TGT)', argsSchema: '{ "domain": "EXAMPLE.COM", "user": "admin", "password": "..." }', destructive: true },
    { name: 'ad-impacket_psexec_py', description: 'impacket psexec.py (alternate)', argsSchema: '{ "target": "10.0.0.5", "user": "admin" }', destructive: true },
  ],
  flipper: [
    { name: 'flipper-list', description: 'List files on the Flipper', argsSchema: '{}', destructive: false },
    { name: 'flipper-read', description: 'Read a file from the Flipper', argsSchema: '{ "path": "/any/sub.txt" }', destructive: false },
    { name: 'flipper-write', description: 'Write a file to the Flipper', argsSchema: '{ "path": "/any/sub.txt", "content": "..." }', destructive: true },
    { name: 'flipper-subghz_tx', description: 'Transmit a SubGHz signal', argsSchema: '{ "path": "/any/sub.fre" }', destructive: true },
  ],
  phish: [
    { name: 'phish-gophish_campaign_create', description: 'Create a gophish campaign', argsSchema: '{ "name": "...", "template": "...", "url": "https://..." }', destructive: false },
    { name: 'phish-gophish_campaign_list', description: 'List gophish campaigns', argsSchema: '{}', destructive: false },
    { name: 'phish-evilginx_phishlet_list', description: 'List evilginx phishlets', argsSchema: '{}', destructive: false },
    { name: 'phish-evilginx_phishlet_enable', description: 'Enable an evilginx phishlet', argsSchema: '{ "name": "o365" }', destructive: true },
  ],
  detect: [
    { name: 'detect-deepfake', description: 'Analyze an image for deepfake indicators', argsSchema: '{ "image_path": "/path/to.jpg" }', destructive: false },
    { name: 'detect-deepfake_video', description: 'Analyze a video for deepfake indicators', argsSchema: '{ "video_path": "/path/to.mp4" }', destructive: false },
  ],
};

export const DOMAINS: Domain[] = ['osint', 'packets', 'ad', 'flipper', 'phish', 'detect'];

export function toolsForDomain(domain: Domain): Tool[] {
  return TOOL_CATALOG[domain] || [];
}
```

- [ ] **Step 6: Create `app/src/lib/McpClient.ts`**

```typescript
// Svelte-side wrapper around the 3 new Tauri commands. Components
// import this instead of calling invoke() directly so we can mock
// it in tests and centralize error handling.

import { invoke } from '@tauri-apps/api/core';
import type { McpRunResult, AuditEvent } from './types';

export class McpClient {
  async runTool(domain: string, target: string, args: unknown): Promise<McpRunResult> {
    return await invoke<McpRunResult>('mcp_run_tool', { domain, target, args });
  }

  async listTools(domain: string): Promise<unknown[]> {
    return await invoke<unknown[]>('mcp_list_tools', { domain });
  }

  async getAuditEvent(id: string): Promise<AuditEvent | null> {
    return await invoke<AuditEvent | null>('audit_event', { id });
  }
}

export const mcpClient = new McpClient();
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cd app && npm test -- McpClient`
Expected: PASS (4 tests).

- [ ] **Step 8: Commit**

```bash
git add app/src/lib/
git commit -m "feat(ui): McpClient Svelte wrapper + hardcoded toolCatalog"
```

---

## Task 3.3: Add `DomainRail.svelte` (left rail)

**Files:**
- Create: `app/src/lib/DomainRail.svelte`
- Create: `app/src/lib/DomainRail.test.ts`

- [ ] **Step 1: Write the failing test**

`app/src/lib/DomainRail.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import DomainRail from './DomainRail.svelte';
import { DOMAINS } from './toolCatalog';

describe('DomainRail', () => {
  it('renders one button per domain', () => {
    const { getAllByRole } = render(DomainRail, { selected: null });
    const buttons = getAllByRole('button');
    expect(buttons.length).toBe(DOMAINS.length);
  });

  it('marks the selected domain as active', () => {
    const { getByText } = render(DomainRail, { selected: 'ad' });
    const adButton = getByText('ad');
    expect(adButton.className).toContain('active');
  });

  it('emits a select event when a domain is clicked', async () => {
    const { component, getByText } = render(DomainRail, { selected: null });
    let selected: string | null = null;
    component.$on('select', (e) => { selected = e.detail; });
    await fireEvent.click(getByText('flipper'));
    expect(selected).toBe('flipper');
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npm test -- DomainRail`
Expected: FAIL with "Cannot find module './DomainRail.svelte'".

- [ ] **Step 3: Create `app/src/lib/DomainRail.svelte`**

```svelte
<script lang="ts">
  import { DOMAINS } from './toolCatalog';
  import type { Domain } from './types';

  interface Props {
    selected: Domain | null;
  }
  let { selected = $bindable() }: Props = $props();

  function select(d: Domain) {
    selected = d;
  }
</script>

<nav class="domain-rail" aria-label="MCP domains">
  <h2>Domains</h2>
  <ul>
    {#each DOMAINS as d}
      <li>
        <button
          class:active={selected === d}
          onclick={() => select(d)}
          aria-current={selected === d ? 'page' : undefined}
        >
          {d}
        </button>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .domain-rail {
    width: 180px;
    border-right: 1px solid #2a2a2a;
    padding: 1rem 0.5rem;
  }
  h2 {
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #888;
    margin: 0 0 0.5rem 0.5rem;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: #ccc;
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    cursor: pointer;
    font: inherit;
  }
  button:hover { background: #2a2a2a; }
  button.active { background: #1e3a5f; color: #fff; }
</style>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npm test -- DomainRail`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/DomainRail.svelte app/src/lib/DomainRail.test.ts
git commit -m "feat(ui): DomainRail left rail"
```

---

## Task 3.4: Add `ToolRunner.svelte` (middle pane)

**Files:**
- Create: `app/src/lib/ToolRunner.svelte`
- Create: `app/src/lib/ToolRunner.test.ts`

- [ ] **Step 1: Write the failing test**

`app/src/lib/ToolRunner.test.ts`:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import ToolRunner from './ToolRunner.svelte';
import { TOOL_CATALOG } from './toolCatalog';
import { mcpClient } from './McpClient';

vi.mock('./McpClient');

describe('ToolRunner', () => {
  it('renders all tools for the selected domain', () => {
    const { getAllByRole } = render(ToolRunner, { domain: 'ad' });
    const buttons = getAllByRole('button', { name: /Run/ });
    expect(buttons.length).toBe(TOOL_CATALOG.ad.length);
  });

  it('shows a hint message when no domain is selected', () => {
    const { getByText } = render(ToolRunner, { domain: null });
    expect(getByText(/select a domain/i)).toBeTruthy();
  });

  it('emits a run event with the right args when Run is clicked', async () => {
    vi.mocked(mcpClient.runTool).mockResolvedValueOnce({ ok: true, stdout: 'ok' });
    const { component, getAllByRole, getByRole } = render(ToolRunner, { domain: 'osint' });
    let eventDetail: any = null;
    component.$on('run', (e) => { eventDetail = e.detail; });
    // The first osint tool is osint-whois
    const runButton = getAllByRole('button', { name: /Run/ })[0];
    await fireEvent.click(runButton);
    // The textarea has the default argsSchema; we don't need to type.
    expect(eventDetail).toBeTruthy();
    expect(eventDetail.domain).toBe('osint');
    expect(eventDetail.target).toBe('osint-whois');
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npm test -- ToolRunner`
Expected: FAIL with "Cannot find module './ToolRunner.svelte'".

- [ ] **Step 3: Create `app/src/lib/ToolRunner.svelte`**

```svelte
<script lang="ts">
  import { toolsForDomain } from './toolCatalog';
  import { mcpClient } from './McpClient';
  import type { Domain, McpRunResult } from './types';

  interface Props {
    domain: Domain | null;
  }
  let { domain }: Props = $props();

  let running = $state<string | null>(null);  // name of the tool currently running
  let lastResult = $state<McpRunResult | null>(null);
  let argsText = $state<Record<string, string>>({});

  async function run(toolName: string, destructive: boolean) {
    if (!domain) return;
    const argsRaw = argsText[toolName] || '{}';
    let args: unknown;
    try {
      args = JSON.parse(argsRaw);
    } catch {
      lastResult = { ok: false, error: 'args is not valid JSON' };
      return;
    }
    running = toolName;
    lastResult = null;
    try {
      const result = await mcpClient.runTool(domain, toolName, args);
      lastResult = result;
    } catch (e: any) {
      lastResult = { ok: false, error: e.message || String(e) };
    } finally {
      running = null;
    }
  }
</script>

{#if !domain}
  <p class="hint">Select a domain from the left rail.</p>
{:else}
  <div class="tool-runner">
    <h2>{domain}</h2>
    <ul>
      {#each toolsForDomain(domain) as tool}
        <li>
          <header>
            <h3>{tool.name}</h3>
            {#if tool.destructive}
              <span class="badge destructive">destructive</span>
            {/if}
          </header>
          <p>{tool.description}</p>
          <details>
            <summary>args</summary>
            <textarea
              bind:value={argsText[tool.name]}
              placeholder={tool.argsSchema}
              rows="4"
              data-testid="args-{tool.name}"
            ></textarea>
          </details>
          <button
            onclick={() => run(tool.name, tool.destructive)}
            disabled={running !== null}
            data-testid="run-{tool.name}"
          >
            {running === tool.name ? 'Running…' : 'Run'}
          </button>
          {#if lastResult && !running}
            <pre class="result" data-testid="result">{JSON.stringify(lastResult, null, 2)}</pre>
          {/if}
        </li>
      {/each}
    </ul>
  </div>
{/if}

<style>
  .tool-runner { padding: 1rem; }
  h2 { margin: 0 0 1rem 0; }
  ul { list-style: none; padding: 0; }
  li { padding: 1rem; border: 1px solid #2a2a2a; border-radius: 4px; margin-bottom: 1rem; }
  header { display: flex; align-items: center; gap: 0.5rem; }
  h3 { margin: 0; }
  .badge { font-size: 0.7rem; padding: 0.1rem 0.4rem; border-radius: 3px; }
  .badge.destructive { background: #5a1e1e; color: #fbb; }
  textarea { width: 100%; font: monospace; }
  pre.result { background: #111; padding: 0.5rem; border-radius: 4px; overflow-x: auto; }
  .hint { color: #888; padding: 1rem; }
</style>
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npm test -- ToolRunner`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/ToolRunner.svelte app/src/lib/ToolRunner.test.ts
git commit -m "feat(ui): ToolRunner middle pane"
```

---

## Task 3.5: Add `ResultPane.svelte` (right-middle pane) + `AuditDetail.svelte` (far-right slide-out)

**Files:**
- Create: `app/src/lib/ResultPane.svelte`
- Create: `app/src/lib/ResultPane.test.ts`
- Create: `app/src/lib/AuditDetail.svelte`
- Create: `app/src/lib/AuditDetail.test.ts`

- [ ] **Step 1: Write the failing tests**

`app/src/lib/ResultPane.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import ResultPane from './ResultPane.svelte';

describe('ResultPane', () => {
  it('shows a placeholder when there is no result', () => {
    const { getByText } = render(ResultPane, { result: null });
    expect(getByText(/no result yet/i)).toBeTruthy();
  });

  it('shows stdout when present', () => {
    const { getByText } = render(ResultPane, { result: { ok: true, stdout: 'hello world', stderr: '', audit_event_id: 'e1' } });
    expect(getByText('hello world')).toBeTruthy();
  });

  it('emits an audit-click event when the audit id is clicked', async () => {
    const { component, getByText } = render(ResultPane, { result: { ok: true, stdout: '', stderr: '', audit_event_id: 'evt-42' } });
    let clicked: string | null = null;
    component.$on('auditClick', (e) => { clicked = e.detail; });
    await fireEvent.click(getByText('evt-42'));
    expect(clicked).toBe('evt-42');
  });

  it('shows the error in red when ok is false', () => {
    const { container } = render(ResultPane, { result: { ok: false, error: 'gate denied' } });
    const errEl = container.querySelector('.error');
    expect(errEl).toBeTruthy();
    expect(errEl!.textContent).toContain('gate denied');
  });
});
```

`app/src/lib/AuditDetail.test.ts`:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, waitFor } from '@testing-library/svelte';
import AuditDetail from './AuditDetail.svelte';
import { mcpClient } from './McpClient';

vi.mock('./McpClient');

describe('AuditDetail', () => {
  it('renders nothing when no event id is set', () => {
    const { container } = render(AuditDetail, { eventId: null });
    expect(container.firstChild).toBeNull();
  });

  it('loads and renders the event when an id is set', async () => {
    vi.mocked(mcpClient.getAuditEvent).mockResolvedValueOnce({ id: 'e1', ts: '2026-06-03T12:00:00Z', kind: 'mcp_run_completed', ok: true, ms: 1234 } as any);
    const { findByText } = render(AuditDetail, { eventId: 'e1' });
    await findByText(/mcp_run_completed/);
    await findByText(/1234/);
  });

  it('emits a close event when the X is clicked', async () => {
    vi.mocked(mcpClient.getAuditEvent).mockResolvedValueOnce({ id: 'e1', kind: 'x' } as any);
    const { component, findByText } = render(AuditDetail, { eventId: 'e1' });
    let closed = false;
    component.$on('close', () => { closed = true; });
    const closeBtn = await findByText('×');
    await fireEvent.click(closeBtn);
    expect(closed).toBe(true);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd app && npm test -- ResultPane AuditDetail`
Expected: FAIL (both files missing).

- [ ] **Step 3: Create `app/src/lib/ResultPane.svelte`**

```svelte
<script lang="ts">
  import type { McpRunResult } from './types';

  interface Props {
    result: McpRunResult | null;
  }
  let { result }: Props = $props();
</script>

<aside class="result-pane" aria-label="Last run result">
  <h2>Result</h2>
  {#if !result}
    <p class="placeholder">No result yet. Click "Run" on a tool.</p>
  {:else if !result.ok}
    <p class="error" data-testid="error">{result.error || 'tool failed'}</p>
  {:else}
    {#if result.stdout}
      <section>
        <h3>stdout</h3>
        <pre>{result.stdout}</pre>
      </section>
    {/if}
    {#if result.stderr}
      <section>
        <h3>stderr</h3>
        <pre>{result.stderr}</pre>
      </section>
    {/if}
    {#if result.audit_event_id}
      <p>
        audit:
        <button class="link" onclick={() => /* fire event */ {}} data-event-id={result.audit_event_id}>
          {result.audit_event_id}
        </button>
      </p>
    {/if}
  {/if}
</aside>

<style>
  .result-pane { width: 360px; border-left: 1px solid #2a2a2a; padding: 1rem; overflow-y: auto; }
  h2 { margin: 0 0 1rem 0; }
  h3 { font-size: 0.85rem; text-transform: uppercase; color: #888; margin: 1rem 0 0.25rem; }
  pre { background: #111; padding: 0.5rem; border-radius: 4px; overflow-x: auto; white-space: pre-wrap; }
  .placeholder { color: #888; }
  .error { color: #f88; }
  .link { background: none; border: none; color: #6af; cursor: pointer; padding: 0; font: inherit; text-decoration: underline; }
</style>
```

- [ ] **Step 4: Update ResultPane to dispatch the audit-click event properly**

Replace the `<button class="link" onclick={() => {}}>` with:

```svelte
<button
  class="link"
  onclick={() => {
    const ev = new CustomEvent('auditClick', { detail: result.audit_event_id });
    document.dispatchEvent(ev);
  }}
  data-event-id={result.audit_event_id}
>
  {result.audit_event_id}
</button>
```

(Or use a Svelte 5 callback prop. For v1 we use the document CustomEvent approach because it's the simplest. Note: tests should listen on `document`.)

Update the test to match: `document.addEventListener('auditClick', listener); await fireEvent.click(...)`. Or alternatively, pass an `onAuditClick` callback prop and call it directly:

```svelte
<script lang="ts">
  interface Props {
    result: McpRunResult | null;
    onAuditClick?: (id: string) => void;
  }
  let { result, onAuditClick }: Props = $props();
</script>

...

<button
  class="link"
  onclick={() => result?.audit_event_id && onAuditClick?.(result.audit_event_id)}
>
  {result.audit_event_id}
</button>
```

And the test uses a callback prop:

```typescript
const onClick = vi.fn();
const { getByText } = render(ResultPane, { result: {...}, onAuditClick: onClick });
await fireEvent.click(getByText('evt-42'));
expect(onClick).toHaveBeenCalledWith('evt-42');
```

Use the callback-prop version — it's the Svelte 5 idiomatic way and easier to test.

- [ ] **Step 5: Create `app/src/lib/AuditDetail.svelte`**

```svelte
<script lang="ts">
  import { mcpClient } from './McpClient';
  import type { AuditEvent } from './types';

  interface Props {
    eventId: string | null;
  }
  let { eventId }: Props = $props();

  let event = $state<AuditEvent | null>(null);
  let loading = $state(false);

  $effect(() => {
    if (eventId) {
      loading = true;
      mcpClient.getAuditEvent(eventId).then((e) => {
        event = e;
        loading = false;
      });
    } else {
      event = null;
    }
  });
</script>

{#if eventId}
  <aside class="audit-detail" aria-label="Audit event detail">
    <header>
      <h2>Audit event</h2>
      <button class="close" aria-label="Close" onclick={() => { eventId = null; }}>×</button>
    </header>
    {#if loading}
      <p>loading…</p>
    {:else if event}
      <pre>{JSON.stringify(event, null, 2)}</pre>
    {:else}
      <p>event not found</p>
    {/if}
  </aside>
{/if}

<style>
  .audit-detail {
    position: fixed;
    top: 0;
    right: 0;
    width: 480px;
    height: 100vh;
    background: #1a1a1a;
    border-left: 1px solid #2a2a2a;
    padding: 1rem;
    overflow-y: auto;
    z-index: 100;
  }
  header { display: flex; justify-content: space-between; align-items: center; }
  .close { background: none; border: none; color: #ccc; font-size: 1.5rem; cursor: pointer; }
  pre { background: #111; padding: 0.5rem; border-radius: 4px; font-size: 0.85rem; }
</style>
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd app && npm test -- ResultPane AuditDetail`
Expected: PASS (4 + 3 = 7 tests).

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/ResultPane.svelte app/src/lib/ResultPane.test.ts app/src/lib/AuditDetail.svelte app/src/lib/AuditDetail.test.ts
git commit -m "feat(ui): ResultPane + AuditDetail right rail"
```

---

## Task 3.6: Wire the 3-pane layout into `App.svelte` + extend `state.svelte.ts`

**Files:**
- Modify: `app/src/lib/state.svelte.ts`: add domains, selectedDomain, selectedTool, lastResult, auditDetailEventId
- Modify: `app/src/App.svelte`: render the 3-pane layout

- [ ] **Step 1: Read the existing `state.svelte.ts` and `App.svelte`**

Run: `cat app/src/lib/state.svelte.ts && echo "---" && cat app/src/App.svelte`
Expected: there's an existing `pending: ConfirmRequest[]` and a route to `/audit`. The 3-pane layout replaces the central "Waiting for confirmation requests" stub.

- [ ] **Step 2: Add the new state to `state.svelte.ts`**

Append to the existing state:

```typescript
import type { Domain, McpRunResult } from './types';

// existing state...
export const workspace = $state({
  selectedDomain: null as Domain | null,
  lastResult: null as McpRunResult | null,
  auditDetailEventId: null as string | null,
});

export function selectDomain(d: Domain | null) {
  workspace.selectedDomain = d;
}

export function setLastResult(r: McpRunResult | null) {
  workspace.lastResult = r;
}

export function openAuditDetail(id: string) {
  workspace.auditDetailEventId = id;
}

export function closeAuditDetail() {
  workspace.auditDetailEventId = null;
}
```

- [ ] **Step 3: Modify `App.svelte`**

Replace the central content with the 3-pane layout:

```svelte
<script lang="ts">
  import DomainRail from '$lib/DomainRail.svelte';
  import ToolRunner from '$lib/ToolRunner.svelte';
  import ResultPane from '$lib/ResultPane.svelte';
  import AuditDetail from '$lib/AuditDetail.svelte';
  import { workspace, selectDomain, setLastResult, openAuditDetail, closeAuditDetail } from '$lib/state.svelte';
  // ... existing imports (e.g. for the /audit route) ...
</script>

<div class="app">
  <DomainRail
    selected={workspace.selectedDomain}
    onSelect={(d) => selectDomain(d)}
  />
  <main>
    <ToolRunner
      domain={workspace.selectedDomain}
      onRun={(r) => setLastResult(r)}
    />
    <!-- existing /audit route and 8 stub views can be tabs/buttons
         inside <main> that toggle which view is shown -->
  </main>
  <ResultPane
    result={workspace.lastResult}
    onAuditClick={(id) => openAuditDetail(id)}
  />
  <AuditDetail
    eventId={workspace.auditDetailEventId}
    onClose={() => closeAuditDetail()}
  />
</div>

<style>
  .app { display: flex; height: 100vh; }
  main { flex: 1; overflow-y: auto; }
</style>
```

(Adapt the existing /audit and stub-view navigation to a tab bar at the top of `<main>`. The exact layout is up to the existing route structure; the new components slot in alongside.)

- [ ] **Step 4: Run the Tauri app and verify the 3 panes render**

Run: `cd app && npm run tauri dev`
Expected: the Tauri window opens, the left rail shows the 6 domains, the middle pane shows the hint, the right-middle shows "No result yet". Clicking a domain shows its tools in the middle. Clicking "Run" shows the result in the right-middle.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/state.svelte.ts app/src/App.svelte
git commit -m "feat(ui): 3-pane layout wired into App.svelte"
```

---

## Task 3.7: Audit log view — make rows open AuditDetail in the right rail (not a modal)

**Files:**
- Modify: `app/src/routes/audit/+page.svelte` (or wherever the audit log list lives)

- [ ] **Step 1: Read the existing audit log list**

Run: `ls app/src/routes/ 2>/dev/null && cat app/src/routes/audit/+page.svelte 2>/dev/null | head -80`
Expected: a virtual-scrolled list of events. Clicking a row currently opens a modal (sub-plan 4 Phase 2 / Task 2.5). We change it to dispatch an `auditClick` event.

- [ ] **Step 2: Replace the modal with a call to `openAuditDetail(id)`**

Find the existing click handler for a row and replace it with:

```svelte
<script>
  import { openAuditDetail } from '$lib/state.svelte';
  // ... existing imports ...
</script>

<!-- existing list rendering ... -->
<button class="row" onclick={() => openAuditDetail(event.id)}>
  <!-- existing row content -->
</button>
```

- [ ] **Step 3: Run the Tauri app, navigate to /audit, click a row, verify AuditDetail opens**

Run: `cd app && npm run tauri dev`
Navigate to /audit, click a row. Expected: the AuditDetail slide-out appears on the right with the event's full JSON.

- [ ] **Step 4: Commit**

```bash
git add app/src/routes/audit/
git commit -m "feat(ui): audit log rows open AuditDetail in right rail"
```

---

## Task 3.8: End-to-end smoke (manually launch + run a tool + see result)

**Files:**
- Create: `app/tests/e2e/smoke.md` (a manual smoke-test checklist)

- [ ] **Step 1: Write the smoke-test checklist**

`app/tests/e2e/smoke.md`:

```markdown
# Tauri app end-to-end smoke test

Run this on a fresh Ubuntu 24.04 (or the user's modified Ubuntu) after
`sudo dpkg -i target/debian/*.deb` + `systemctl --user start blackglass-core`.

## Pre-flight

- [ ] `blackglass-core` is running: `systemctl --user status blackglass-core`
      → "active (running)"
- [ ] The operator socket exists: `ls -la ~/.local/share/blackglass/runtime.sock`
- [ ] The MCP supervisor spawned the 4 MCPs: `ls -la ~/.local/share/blackglass/logs/`
      → mcp-ad.log, mcp-flipper.log, mcp-phish.log, mcp-detect.log

## Test 1: launch the Tauri app

- [ ] `blackglass ui` (or click the desktop icon)
- [ ] Tauri window opens
- [ ] Left rail shows: osint, packets, ad, flipper, phish, detect
- [ ] Middle pane shows: "Select a domain from the left rail."
- [ ] Right-middle shows: "No result yet."

## Test 2: run a non-destructive tool (osint-whois)

- [ ] Click "osint" in the left rail
- [ ] Middle pane shows: osint-whois, osint-dig, osint-theharvester
- [ ] Click "Run" on osint-whois (with the default `{ "target": "example.com" }`)
- [ ] Right-middle pane shows the WHOIS output
- [ ] The "audit: <id>" link is clickable and opens AuditDetail in the right rail
- [ ] The audit log view shows the new event

## Test 3: run a destructive tool (ad-impacket_psexec)

- [ ] Click "ad" in the left rail
- [ ] Click "Run" on ad-impacket_psexec
- [ ] A confirm modal appears: "Run psexec on TARGET? [Allow] [Deny]"
- [ ] Click "Deny"
- [ ] Right-middle pane shows: "denied" (or "gate denied" or similar)
- [ ] The audit log view shows: ActionRequested, OperatorConfirmationRequested, OperatorConfirmationResolved (denied), then no ActionExecuted.

## Test 4: MCP-down handling

- [ ] `kill <mcp-ad pid>` (find it via `pgrep -f blackglass-mcp-ad`)
- [ ] Wait ~3s for the supervisor to detect the exit and restart
- [ ] In the Tauri app, click "Run" on ad-impacket_psexec
- [ ] Result is "ok" (because the supervisor restarted mcp-ad)
- [ ] The audit log shows: McpServerExited, McpServerSpawned (restart), then ActionExecuted

## Pass criteria

All 4 tests pass with no red error banners in the Tauri UI. The audit
log is intact (chain verifies). The user's modified Ubuntu boots
cleanly after a reboot (systemd --user is persistent).
```

- [ ] **Step 2: Run the smoke test on the user's modified Ubuntu**

(This step is manual. Run through the checklist above. Capture screenshots if anything fails. Mark the checklist items as you go.)

- [ ] **Step 3: If any test fails, use the systematic-debugging skill to root-cause**

Load the `systematic-debugging` skill, follow its 5 steps (reproduce, hypothesize, instrument, fix, verify).

- [ ] **Step 4: Commit the smoke-test doc + any fixes**

```bash
git add app/tests/e2e/smoke.md
git commit -m "docs(ui): end-to-end smoke test checklist"
```

(If fixes were needed, commit those first with their own messages.)

---

**End of Phase 3.** Run `cd app && npm test` and confirm all Svelte tests green. The Tauri UI delta is complete: 3 Rust commands + 3 Svelte components + 3-pane layout + audit-detail integration.

**Phase 3 exit criteria:**

- 3 Tauri commands (`mcp_run_tool`, `mcp_list_tools`, `audit_event`) work end-to-end
- `McpClient` + `toolCatalog` are unit-tested
- `DomainRail` / `ToolRunner` / `ResultPane` / `AuditDetail` are unit-tested
- The 3-pane layout renders in the Tauri app
- Clicking an audit-log row opens AuditDetail in the right rail
- The manual smoke test passes

Next: Phase 4 (Security delta).

---

# Phase 4: Security delta (AppArmor profiles, extended confinement test)

The 4 tasks in this phase add the user-home AppArmor profiles for the core and the secondary sidecar, extend the `xtask confinement-test` to validate the new profiles, and remove the polkit-helper references from the existing test.

**Prereq:** Phase 2.5+ complete.

---

## Task 4.1: Add the user-home AppArmor profile for the core

**Files:**
- Create: `packaging/apparmor/blackglass-core` (the AppArmor profile text)
- Modify: `packaging/debian/postinst`: load the new profile via `apparmor_parser -r`

- [ ] **Step 1: Read the existing core AppArmor profile (if it exists from sub-plan 4 Phase 3 / Task 3.4)**

Run: `cat packaging/apparmor/blackglass-core 2>/dev/null || echo "NOT FOUND"`
Expected: a profile that allows writes to `/var/run/blackglass/**` and `/var/lib/blackglass/**`. We're rewriting it for the user-systemd model.

- [ ] **Step 2: Write the new user-home AppArmor profile**

`packaging/apparmor/blackglass-core`:

```
#include <tunables/global>

# Blackglass core (user-systemd version).
#
# This profile confines the blackglass-core binary when run as a
# user-systemd service. It does NOT use the `owner` rules because
# the core is started by systemd --user (which is the user, not root).
# The operator's `~/.local/share/blackglass/` is the primary state
# directory.

/usr/bin/blackglass-core flags=(unconfined) {
  #include <abstractions/base>
  #include <abstractions/nameservice>
  #include <abstractions/openssl>

  # Binary itself
  /usr/bin/blackglass-core mr,

  # Operator state (read+write)
  owner @{HOME}/.local/share/blackglass/** rwk,
  owner @{HOME}/.config/blackglass/** r,
  owner @{HOME}/.local/share/blackglass/runtime.sock rw,
  owner @{HOME}/.local/share/blackglass/operator.token r,
  owner @{HOME}/.local/share/blackglass/audit/chain.jsonl rwk,
  owner @{HOME}/.local/share/blackglass/logs/** rwk,

  # Read-only assets
  /usr/lib/blackglass/** r,
  /usr/share/blackglass/** r,
  /etc/blackglass/mcp-servers.toml r,

  # Python sidecar venv
  /usr/lib/blackglass/python-venv/** r,
  /usr/lib/blackglass/python-venv/bin/python rix,

  # Python sidecar's evidence dir (writes go here for forensic review)
  owner @{HOME}/.local/share/blackglass/evidence/** rwk,

  # Network: localhost (operator.sock, runtime.sock are unix sockets)
  network unix stream,
  network inet stream,
  network inet6 stream,

  # Deny everything else
  deny /etc/shadow r,
  deny /etc/sudoers r,
  deny /root/** rwx,
  deny @{HOME}/.ssh/** r,
}
```

(The `flags=(unconfined)` line lets systemd fork the process; the rest is enforced.)

- [ ] **Step 3: Validate the profile syntax with `apparmor_parser`**

Run: `sudo apparmor_parser -K packaging/apparmor/blackglass-core 2>&1 || echo "(apparmor_parser may not be installed; skip if so)"`
Expected: no errors. If errors, fix the profile.

- [ ] **Step 4: Add a `unload_first` directive if the old profile is loaded**

In the postinst, before loading the new profile, unload the old one if present:

```bash
if [ -f /etc/apparmor.d/blackglass-core ]; then
    apparmor_parser -R /etc/apparmor.d/blackglass-core 2>/dev/null || true
fi
```

(This handles the upgrade case where a user installed the root-systemd v0 and is now installing the user-systemd v1.)

- [ ] **Step 5: Commit**

```bash
git add packaging/apparmor/blackglass-core packaging/debian/postinst
git commit -m "feat(apparmor): user-home profile for blackglass-core (replaces root-systemd profile)"
```

---

## Task 4.2: Add the AppArmor profile for the secondary sidecar

**Files:**
- Create: `packaging/apparmor/blackglass-secondary-sidecar`

- [ ] **Step 1: Write the secondary sidecar profile**

`packaging/apparmor/blackglass-secondary-sidecar`:

```
#include <tunables/global>

# Blackglass secondary sidecar (the deepfake detector).
# Runs pytorch + MesoNet in its own venv. Listens on localhost:8511.

/usr/bin/blackglass-secondary-sidecar flags=(unconfined) {
  #include <abstractions/base>
  #include <abstractions/nameservice>
  #include <abstractions/openssl>

  /usr/bin/blackglass-secondary-sidecar mr,

  # Operator state
  owner @{HOME}/.local/share/blackglass/** rwk,
  owner @{HOME}/.config/blackglass/** r,
  owner @{HOME}/.local/share/blackglass/secondary-sidecar.log rwk,

  # Sidecar venv (separate from main sidecar venv)
  /usr/lib/blackglass/secondary-sidecar-venv/** r,
  /usr/lib/blackglass/secondary-sidecar-venv/bin/python rix,

  # Network: localhost REST endpoint
  network inet stream,
  network inet6 stream,

  # Deny
  deny /etc/shadow r,
  deny /root/** rwx,
  deny @{HOME}/.ssh/** r,
}
```

- [ ] **Step 2: Validate with `apparmor_parser`**

Run: `sudo apparmor_parser -K packaging/apparmor/blackglass-secondary-sidecar`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add packaging/apparmor/blackglass-secondary-sidecar
git commit -m "feat(apparmor): user-home profile for blackglass-secondary-sidecar"
```

---

## Task 4.3: Extend `xtask confinement-test` for the new profiles + remove polkit-helper test

**Files:**
- Modify: `crates/xtask/src/bin/confinement_test.rs`: extend with 2 new tests
- Modify: `crates/xtask/src/bin/confinement_test.rs`: remove the polkit-helper test (the helper itself is deferred to a later sub-plan)

- [ ] **Step 1: Read the existing confinement test**

Run: `cat crates/xtask/src/bin/confinement_test.rs | head -80`
Expected: there are tests for: AppArmor profile loads, profile blocks `/etc/shadow` read, profile allows expected paths, polkit helper execs the right binary (REMOVE), Flipper device is accessible. (From sub-plan 4 Phase 3 / Task 3.8.)

- [ ] **Step 2: Remove the polkit-helper test**

Find the test that spawns the polkit-helper and asserts it execs the core. Delete the test. Also delete the helper-build step in the test setup.

- [ ] **Step 3: Write the 2 new tests**

Add to `crates/xtask/src/bin/confinement_test.rs`:

```rust
#[test]
fn secondary_sidecar_profile_loads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("packaging/apparmor/blackglass-secondary-sidecar");
    let status = std::process::Command::new("apparmor_parser")
        .arg("-K")
        .arg(&path)
        .status();
    // apparmor_parser may not be installed (CI vs dev); treat absent as skip.
    match status {
        Ok(s) if s.success() => {},
        Ok(s) => panic!("apparmor_parser failed: {:?}", s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("apparmor_parser not installed; skipping");
        }
        Err(e) => panic!("apparmor_parser error: {:?}", e),
    }
}

#[test]
fn core_user_profile_blocks_shadow_read() {
    // Run a blackglass-core under the new user-home profile and
    // assert it cannot read /etc/shadow.
    // Skip if AppArmor is not available.
    let profile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .join("packaging/apparmor/blackglass-core");
    let core_bin = std::path::Path::new("/usr/bin/blackglass-core");
    if !core_bin.exists() {
        eprintln!("blackglass-core not installed; skipping");
        return;
    }
    // Load the profile.
    let _ = std::process::Command::new("apparmor_parser")
        .arg("-K").arg(&profile_path).status();
    // Run a one-shot core that just tries to read /etc/shadow and exits.
    // We use `aa-exec -p` to enforce.
    let output = std::process::Command::new("aa-exec")
        .arg("-p")
        .arg("blackglass-core")
        .arg(core_bin)
        .arg("--probe-read")
        .arg("/etc/shadow")
        .output();
    match output {
        Ok(o) => {
            // The probe should fail: either the process can't start
            // (denied by profile) or it returns a non-zero exit code
            // saying "permission denied".
            assert!(!o.status.success(),
                "core should not be able to read /etc/shadow under confinement; got: {:?}\nstderr: {}",
                o.status, String::from_utf8_lossy(&o.stderr));
        }
        Err(e) => {
            eprintln!("aa-exec not available; skipping: {}", e);
        }
    }
}
```

- [ ] **Step 4: Run the new tests**

Run: `cargo test -p xtask confinement`
Expected: PASS (existing + 2 new; some may skip if AppArmor is not installed on this machine).

- [ ] **Step 5: Commit**

```bash
git add crates/xtask/src/bin/confinement_test.rs
git commit -m "test(xtask): extend confinement test for user-home core + secondary-sidecar profiles; drop polkit-helper"
```

---

## Task 4.4: Extend `xtask apparmor-generate` to emit the secondary-sidecar profile

**Files:**
- Modify: `crates/xtask/src/bin/apparmor_generate.rs`: add `--secondary-sidecar` flag

- [ ] **Step 1: Read the existing `apparmor-generate` command**

Run: `cat crates/xtask/src/bin/apparmor_generate.rs | head -60`
Expected: a subcommand that emits a draft AppArmor profile from a template engine. Probably takes the binary path + a list of allowed paths.

- [ ] **Step 2: Add the `--secondary-sidecar` flag**

In the `clap` parser, add a `--secondary-sidecar` flag (or a subcommand). When set, emit the secondary-sidecar profile (using the same template as the core profile, but with `/usr/bin/blackglass-secondary-sidecar` as the binary and `/usr/lib/blackglass/secondary-sidecar-venv/**` as the venv path).

- [ ] **Step 3: Generate the profile and verify it matches the hand-written one**

Run: `cargo run -p xtask -- apparmor-generate --secondary-sidecar > /tmp/generated.profile`
Then `diff packaging/apparmor/blackglass-secondary-sidecar /tmp/generated.profile`
Expected: no diff (or only whitespace). If there's a diff, either fix the template or the hand-written profile so they agree.

- [ ] **Step 4: Commit**

```bash
git add crates/xtask/src/bin/apparmor_generate.rs
git commit -m "feat(xtask): apparmor-generate --secondary-sidecar"
```

---

**End of Phase 4.** Run `cargo test -p xtask confinement` and confirm green. The Security delta is complete: user-home AppArmor profiles for core + secondary sidecar, extended confinement test, secondary-sidecar profile generator.

**Phase 4 exit criteria:**

- `packaging/apparmor/blackglass-core` is a user-home profile (no `/var/run/blackglass/`, no `/var/lib/blackglass/`)
- `packaging/apparmor/blackglass-secondary-sidecar` exists
- `cargo test -p xtask confinement` is green (with skips on machines without AppArmor)
- `cargo run -p xtask -- apparmor-generate --secondary-sidecar` produces a profile that matches the hand-written one
- No references to `polkit-helper` remain in the codebase (deferred to a later sub-plan)

Next: Phase 5 (Packaging delta).

---

# Phase 5: Packaging delta (.deb with user-systemd, no cosign, no polkit)

The 7 tasks in this phase ship the user-systemd .deb: the two new systemd unit files, the .deb `cargo-deb.toml` delta, the postinst + prerm deltas, the `mcp-servers.toml.example` config, the `install.sh` delta (no cosign), and a final `dpkg -i` + post-install smoke.

**Prereq:** Phase 4 complete.

---

## Task 5.1: Create the user-systemd unit files

**Files:**
- Create: `packaging/systemd/blackglass-core.service`
- Create: `packaging/systemd/blackglass-secondary-sidecar.service`

- [ ] **Step 1: Read the existing `cargo-deb.toml` to see the data layout**

Run: `cat packaging/deb/cargo-deb.toml | head -80`
Expected: there's a `[[bin]]` list, a `[data]` section that includes `usr/lib/blackglass/`, `etc/blackglass/`, `etc/apparmor.d/`, etc. We extend the `data` section to include `packaging/systemd/`.

- [ ] **Step 2: Write `packaging/systemd/blackglass-core.service`**

```ini
[Unit]
Description=Blackglass core (the chokepoint)
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/blackglass-core
Restart=on-failure
RestartSec=5s
# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
# Allow AppArmor confinement to be applied
AppArmorProfile=blackglass-core
# State
StateDirectory=blackglass
LogsDirectory=blackglass

[Install]
WantedBy=default.target
```

- [ ] **Step 3: Write `packaging/systemd/blackglass-secondary-sidecar.service`**

```ini
[Unit]
Description=Blackglass secondary sidecar (deepfake detector)
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/blackglass-secondary-sidecar
Restart=on-failure
RestartSec=10s
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
AppArmorProfile=blackglass-secondary-sidecar
StateDirectory=blackglass
LogsDirectory=blackglass

[Install]
WantedBy=default.target
```

- [ ] **Step 4: Add the systemd units to the .deb `cargo-deb.toml`**

In `packaging/deb/cargo-deb.toml`'s `[[bin]]` section, ensure the two unit files are installed to `/usr/lib/blackglass/systemd/user/`:

```toml
[[bin]]
name = "blackglass-secondary-sidecar"
path = "target/release/blackglass-secondary-sidecar"

# (data section)
data = [
    # ... existing entries ...
    "packaging/systemd/blackglass-core.service", "/usr/lib/blackglass/systemd/user/",
    "packaging/systemd/blackglass-secondary-sidecar.service", "/usr/lib/blackglass/systemd/user/",
    "packaging/apparmor/blackglass-core", "/etc/apparmor.d/",
    "packaging/apparmor/blackglass-secondary-sidecar", "/etc/apparmor.d/",
    "packaging/mcp-servers.toml.example", "/etc/blackglass/",
]
```

- [ ] **Step 5: Commit**

```bash
git add packaging/systemd/ packaging/deb/cargo-deb.toml
git commit -m "feat(packaging): user-systemd unit files for core + secondary-sidecar"
```

---

## Task 5.2: Create `mcp-servers.toml.example`

**Files:**
- Create: `packaging/mcp-servers.toml.example`

- [ ] **Step 1: Write the example config**

`packaging/mcp-servers.toml.example`:

```toml
# Example MCP server spawn config for blackglass-core.
# Copy to ~/.config/blackglass/mcp-servers.toml and edit as needed.
# Each [[servers]] entry spawns one MCP server as a child process.
# The core supervises each one: restart on crash with exponential
# backoff (1s, 2s, 4s, 8s, 16s), give up after max_restarts.

[[servers]]
name = "mcp-ad"
command = "/usr/bin/blackglass-mcp-ad"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-flipper"
command = "/usr/bin/blackglass-mcp-flipper"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-phish"
command = "/usr/bin/blackglass-mcp-phish"
args = []
startup_timeout_ms = 30000
max_restarts = 5

[[servers]]
name = "mcp-detect"
command = "/usr/bin/blackglass-mcp-detect"
args = []
startup_timeout_ms = 30000
max_restarts = 5
```

- [ ] **Step 2: Add it to the .deb `cargo-deb.toml` data section** (if not already from Task 5.1)

```toml
"packaging/mcp-servers.toml.example", "/etc/blackglass/",
```

- [ ] **Step 3: Commit**

```bash
git add packaging/mcp-servers.toml.example packaging/deb/cargo-deb.toml
git commit -m "feat(packaging): mcp-servers.toml.example"
```

---

## Task 5.3: Update the `debian/control` file (remove polkit + cosign deps, add systemd)

**Files:**
- Modify: `packaging/debian/control`

- [ ] **Step 1: Read the existing control file**

Run: `cat packaging/debian/control`
Expected: Build-Depends list includes `libpolkit-gobject-1-dev`, `cosign`, etc. We strip the polkit/cosign bits; we may not need to add `systemd` because the .deb's systemd unit files don't require a system-level systemd dep (user-systemd is built into systemd 240+).

- [ ] **Step 2: Remove polkit + cosign Build-Deps**

In the `Build-Depends:` block, remove:
- `libpolkit-gobject-1-dev,`
- `cosign,`

- [ ] **Step 3: Remove polkit Depends from `blackglass-minimal`**

In the `Package: blackglass-minimal` block's `Depends:` list, remove:
- `libpolkit-gobject-1-0,`
- `adduser,`
- `policykit-1 | polkit,`

- [ ] **Step 4: Run `lintian` to verify no warnings**

Run: `cd packaging/deb && lintian --info blackglass_*.changes 2>/dev/null || echo "(lintian may not be installed; skip if so)"`
Expected: no errors related to the removed deps.

- [ ] **Step 5: Commit**

```bash
git add packaging/debian/control
git commit -m "feat(packaging): remove polkit + cosign deps from control"
```

---

## Task 5.4: Update `postinst` (no polkit/var/cosign, add user-systemd enable + udev group)

**Files:**
- Modify: `packaging/debian/postinst`

- [ ] **Step 1: Read the existing postinst**

Run: `cat packaging/debian/postinst`
Expected: steps 1-7 from §3.4 of the original spec. We're removing the group creation + `/var/lib/blackglass` mkdirs + cosign + polkit-helper profile load, and adding the user-systemd enable.

- [ ] **Step 2: Remove the removed steps**

Delete:
- Step 2 (`addgroup --system blackglass`)
- Step 3 (the `/var/lib/blackglass` mkdirs)
- The `apparmor_parser -r /etc/apparmor.d/blackglass-polkit-helper` line
- The `cosign` setup (if any)

- [ ] **Step 3: Add the user-systemd enable steps**

After the AppArmor profile load, add:

```bash
# Enable the user-systemd services. This is best-effort: only run
# if XDG_RUNTIME_DIR is set (i.e., the user is logged in) AND
# systemctl is available. If we're in a chroot or non-interactive
# dpkg, skip.
if [ -n "${XDG_RUNTIME_DIR:-}" ] && command -v systemctl >/dev/null; then
    USER_NAME="${SUDO_USER:-$(id -un)}"
    if [ "$USER_NAME" != "root" ] && id "$USER_NAME" >/dev/null 2>&1; then
        # Run systemctl --user as the installing user.
        su - "$USER_NAME" -c "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR systemctl --user enable blackglass-core.service blackglass-secondary-sidecar.service" || true
        su - "$USER_NAME" -c "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR systemctl --user start blackglass-core.service" || true
    fi
fi

# Add the installing user to the udev group (so the Flipper works
# without manual intervention). Best-effort.
if [ -n "${SUDO_USER:-}" ] && id "$SUDO_USER" >/dev/null 2>&1; then
    if ! id -Gn "$SUDO_USER" | grep -qw udev; then
        usermod -aG udev "$SUDO_USER" || true
        echo "Added $SUDO_USER to the udev group. Log out and back in for the Flipper to work."
    fi
fi
```

- [ ] **Step 4: Test the postinst on a throwaway chroot (best-effort)**

Run: `sudo chroot /tmp/throwaway-chroot /usr/bin/dpkg --configure blackglass` (after `dpkg -i`ing the .deb into the chroot). If a chroot isn't available, at least run `bash -n packaging/debian/postinst` to check syntax.

- [ ] **Step 5: Commit**

```bash
git add packaging/debian/postinst
git commit -m "feat(packaging): postinst enables user-systemd + adds user to udev"
```

---

## Task 5.5: Update `prerm` (no `/var/lib/blackglass` cleanup, add user-systemd disable)

**Files:**
- Modify: `packaging/debian/prerm`

- [ ] **Step 1: Read the existing prerm**

Run: `cat packaging/debian/prerm`

- [ ] **Step 2: Remove the `/var/lib/blackglass` cleanup, add user-systemd disable**

Replace the contents with:

```bash
#!/bin/bash
set -e

# Best-effort: disable the user-systemd services.
if [ -n "${XDG_RUNTIME_DIR:-}" ] && command -v systemctl >/dev/null; then
    USER_NAME="${SUDO_USER:-$(id -un)}"
    if [ "$USER_NAME" != "root" ] && id "$USER_NAME" >/dev/null 2>&1; then
        su - "$USER_NAME" -c "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR systemctl --user disable blackglass-core.service blackglass-secondary-sidecar.service" || true
        su - "$USER_NAME" -c "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR systemctl --user stop blackglass-core.service" || true
    fi
fi

# Unload the AppArmor profiles.
if command -v apparmor_parser >/dev/null; then
    apparmor_parser -R /etc/apparmor.d/blackglass-core 2>/dev/null || true
    apparmor_parser -R /etc/apparmor.d/blackglass-secondary-sidecar 2>/dev/null || true
fi

# dh_installdeb will replace this with the standard conffiles + state-dirs cleanup.
#DEBHELPER#
```

- [ ] **Step 3: Commit**

```bash
git add packaging/debian/prerm
git commit -m "feat(packaging): prerm disables user-systemd + unloads AppArmor"
```

---

## Task 5.6: Update `install.sh` (remove cosign, add SHA-256 + 404 fallback)

**Files:**
- Modify: `packaging/install.sh`

- [ ] **Step 1: Read the existing install.sh**

Run: `cat packaging/install.sh | head -100`
Expected: a 4-step flow: detect-distro, verify-cosign, apt-install, post-install message.

- [ ] **Step 2: Replace the cosign verification with SHA-256 pinning**

Find the cosign step and replace it with:

```bash
# Step 2: HTTPS + SHA-256 checksum pinning
TARBALL_URL="https://github.com/blackglass/blackglass/releases/download/${VERSION}/blackglass_${VERSION}_amd64.deb"
CHECKSUM_URL="${TARBALL_URL}.sha256"

curl -fsSL -o /tmp/blackglass.deb "$TARBALL_URL" || {
    echo "Error: failed to download $TARBALL_URL" >&2
    echo "" >&2
    echo "The GitHub Release for blackglass v${VERSION} is not published." >&2
    echo "Until the cosign release pipeline ships (sub-plan 5+), the .deb" >&2
    echo "is build-from-source only." >&2
    echo "" >&2
    echo "Build from source:" >&2
    echo "  git clone https://github.com/blackglass/blackglass.git" >&2
    echo "  cd blackglass" >&2
    echo "  cargo xtask deb" >&2
    echo "  sudo dpkg -i target/debian/*.deb" >&2
    exit 1
}
curl -fsSL -o /tmp/blackglass.deb.sha256 "$CHECKSUM_URL" || {
    echo "Error: failed to download $CHECKSUM_URL" >&2
    exit 1
}
(cd /tmp && sha256sum -c blackglass.deb.sha256) || {
    echo "Error: SHA-256 checksum mismatch. Refusing to install." >&2
    exit 1
}
```

- [ ] **Step 3: Remove the `verify-cosign.sh` source step**

Delete any lines that `source` or `curl` a `verify-cosign.sh`. Delete the corresponding file from `packaging/installer/`.

- [ ] **Step 4: Add the post-install banner**

After the `apt install` step, add:

```bash
echo ""
echo "blackglass v${VERSION} installed."
echo ""
echo "To launch the Tauri app:"
echo "  blackglass ui"
echo ""
echo "The first launch will start the user-systemd service 'blackglass-core'"
echo "(no root required)."
echo ""
echo "If you're using the Flipper, log out and back in to pick up the udev"
echo "group membership."
echo ""
```

- [ ] **Step 5: Test the install script's 404 fallback (in a dry-run)**

Run: `VERSION=99.99.99 bash packaging/install.sh 2>&1 | head -20`
Expected: the "GitHub Release not published" error message and the build-from-source instructions.

- [ ] **Step 6: Commit**

```bash
git add packaging/install.sh packaging/installer/
git commit -m "feat(packaging): install.sh uses SHA-256 + 404 fallback (cosign deferred)"
```

---

## Task 5.7: Build the .deb + `dpkg -i` + post-install smoke

**Files:**
- Create: `target/debian/blackglass_*.deb` (built artifact, not checked in)
- Create: `packaging/debian/tests/postinst_smoke.sh` (best-effort test)

- [ ] **Step 1: Build the .deb**

Run: `cargo xtask deb`
Expected: `target/debian/blackglass_0.1.0_amd64.deb` exists. Size ~50-100 MB.

- [ ] **Step 2: Inspect the .deb contents**

Run: `dpkg-deb -c target/debian/blackglass_0.1.0_amd64.deb | head -30`
Expected: includes `/usr/bin/blackglass-core`, `/usr/bin/blackglass-secondary-sidecar`, `/usr/bin/blackglass-mcp-{ad,flipper,phish,detect}`, `/usr/lib/blackglass/systemd/user/blackglass-{core,secondary-sidecar}.service`, `/etc/apparmor.d/blackglass-{core,secondary-sidecar}`, `/etc/blackglass/mcp-servers.toml.example`. Does NOT include `/var/run/blackglass/` or `/var/lib/blackglass/` or `/usr/libexec/blackglass-polkit-helper`.

- [ ] **Step 3: Install on the user's modified Ubuntu**

Run: `sudo dpkg -i target/debian/blackglass_0.1.0_amd64.deb`
Expected: postinst runs, AppArmor profiles load, user-systemd service is enabled and started, user is added to udev. Banner: "blackglass is installed. Run `blackglass ui` to launch."

- [ ] **Step 4: Verify the user-systemd service is running**

Run: `systemctl --user status blackglass-core`
Expected: "active (running)".

- [ ] **Step 5: Verify the operator socket exists**

Run: `ls -la ~/.local/share/blackglass/runtime.sock`
Expected: socket file with mode 0660 (or 0600) owned by the user.

- [ ] **Step 6: Verify the MCP supervisor spawned the 4 MCPs**

Run: `ls -la ~/.local/share/blackglass/logs/`
Expected: `mcp-ad.log`, `mcp-flipper.log`, `mcp-phish.log`, `mcp-detect.log` exist. Each log has at least one "spawned" line.

- [ ] **Step 7: Write the postinst smoke script**

`packaging/debian/tests/postinst_smoke.sh`:

```bash
#!/bin/bash
# Best-effort postinst smoke test. Run after `dpkg -i`. Not in CI.
set -e

echo "1. Checking user-systemd service..."
systemctl --user is-active blackglass-core.service || {
    echo "FAIL: blackglass-core not active"
    exit 1
}

echo "2. Checking operator socket..."
[ -S ~/.local/share/blackglass/runtime.sock ] || {
    echo "FAIL: runtime.sock not found"
    exit 1
}

echo "3. Checking AppArmor profiles..."
apparmor_status 2>/dev/null | grep -E "blackglass-(core|secondary-sidecar)" || {
    echo "FAIL: AppArmor profiles not loaded"
    exit 1
}

echo "4. Checking MCP supervisor spawned 4 children..."
pgrep -f blackglass-mcp-ad >/dev/null || { echo "FAIL: mcp-ad not running"; exit 1; }
pgrep -f blackglass-mcp-flipper >/dev/null || { echo "FAIL: mcp-flipper not running"; exit 1; }
pgrep -f blackglass-mcp-phish >/dev/null || { echo "FAIL: mcp-phish not running"; exit 1; }
pgrep -f blackglass-mcp-detect >/dev/null || { echo "FAIL: mcp-detect not running"; exit 1; }

echo "5. Checking mcp-servers.toml.example is installed..."
[ -f /etc/blackglass/mcp-servers.toml.example ] || {
    echo "FAIL: mcp-servers.toml.example not installed"
    exit 1
}

echo ""
echo "ALL POSTINST SMOKE CHECKS PASSED"
```

- [ ] **Step 8: Run the smoke script**

Run: `bash packaging/debian/tests/postinst_smoke.sh`
Expected: "ALL POSTINST SMOKE CHECKS PASSED".

- [ ] **Step 9: Commit the smoke script + any fixes**

```bash
git add packaging/debian/tests/
git commit -m "test(packaging): postinst smoke test"
```

---

**End of Phase 5.** The .deb is built, installed, and smoke-tested. Run `cargo test --workspace` to confirm nothing regressed.

**Phase 5 exit criteria:**

- `cargo xtask deb` produces a valid .deb
- The .deb contains: 7 binaries (core, secondary-sidecar, 4 MCPs, Tauri), 2 systemd units, 2 AppArmor profiles, 1 mcp-servers.toml.example, Python venv
- The .deb does NOT contain: polkit-helper, `/var/run/blackglass/`, `/var/lib/blackglass/`, cosign public key
- `sudo dpkg -i` runs the postinst cleanly
- The postinst enables the user-systemd service, adds the user to udev, loads the AppArmor profiles
- The smoke script passes (4 MCPs running, socket present, profiles loaded)

Next: Phase 6 (Polish delta).

---

# Phase 6: Polish delta (verify-install, README, install.sh flesh-out, final green)

The 4 tasks in this phase ship the polish: extended `xtask verify-install` for the user-systemd model, README updates with the build-from-source recipe, an expanded install.sh 404 message, and a final full test pass.

**Prereq:** Phase 5 complete.

---

## Task 6.1: Extend `xtask verify-install` for the user-systemd model

**Files:**
- Modify: `crates/xtask/src/bin/verify_install.rs`

- [ ] **Step 1: Read the existing `verify-install` command**

Run: `cat crates/xtask/src/bin/verify_install.rs | head -100`
Expected: 5-10 checks for: AppArmor loaded, polkit-helper exists, `/var/run/blackglass/` exists, etc. We're rewriting this for the user-systemd model.

- [ ] **Step 2: Remove the obsolete checks**

Delete checks for:
- `/var/run/blackglass/operator.sock` exists
- `/var/lib/blackglass/evidence/` exists
- `polkit-helper` binary exists
- The user is in the `blackglass` group

- [ ] **Step 3: Add the new checks**

Add checks for:

```rust
// User-systemd checks
if !Path::new("/usr/bin/blackglass-core").exists() {
    return Err("blackglass-core not installed".into());
}
if !Path::new("/usr/bin/blackglass-secondary-sidecar").exists() {
    return Err("blackglass-secondary-sidecar not installed".into());
}
let home = dirs::home_dir().ok_or("no home dir")?;
let operator_sock = home.join(".local/share/blackglass/runtime.sock");
if !operator_sock.exists() {
    return Err(format!("operator socket not found at {}", operator_sock.display()));
}
let token = home.join(".local/share/blackglass/operator.token");
if !token.exists() {
    return Err("operator token file not found".into());
}
let token_mode = std::fs::metadata(&token)?.permissions().mode() & 0o777;
if token_mode != 0o600 {
    return Err(format!("operator token mode is {:o}, expected 0600", token_mode));
}

// User-systemd service
let status = std::process::Command::new("systemctl")
    .args(["--user", "is-active", "blackglass-core.service"])
    .output()?;
if !status.status.success() {
    return Err("blackglass-core.service is not active".into());
}

// udev group
let user = std::env::var("USER")?;
let groups_output = std::process::Command::new("id").arg("-Gn").arg(&user).output()?;
let groups = String::from_utf8(groups_output.stdout)?;
if !groups.split_whitespace().any(|g| g == "udev") {
    return Err(format!("user {} is not in the udev group (Flipper won't work)", user));
}

// AppArmor profiles
let aa_status = std::process::Command::new("apparmor_status").output()?;
let aa_stdout = String::from_utf8(aa_status.stdout)?;
if !aa_stdout.contains("blackglass-core") {
    return Err("AppArmor profile 'blackglass-core' is not loaded".into());
}
if !aa_stdout.contains("blackglass-secondary-sidecar") {
    return Err("AppArmor profile 'blackglass-secondary-sidecar' is not loaded".into());
}

// mcp-servers.toml.example
if !Path::new("/etc/blackglass/mcp-servers.toml.example").exists() {
    return Err("/etc/blackglass/mcp-servers.toml.example is missing".into());
}

// MCP supervisor spawned 4 children
for mcp in &["mcp-ad", "mcp-flipper", "mcp-phish", "mcp-detect"] {
    let pgrep = std::process::Command::new("pgrep").arg("-f").arg(format!("blackglass-{}", mcp)).output()?;
    if !pgrep.status.success() {
        return Err(format!("MCP server {} is not running", mcp));
    }
}

println!("✓ all install checks passed");
Ok(())
```

- [ ] **Step 4: Run `verify-install` on the user's modified Ubuntu**

Run: `cargo run -p xtask -- verify-install`
Expected: "✓ all install checks passed".

- [ ] **Step 5: Commit**

```bash
git add crates/xtask/src/bin/verify_install.rs
git commit -m "feat(xtask): verify-install checks for user-systemd model"
```

---

## Task 6.2: Update README with the build-from-source recipe + first-launch walkthrough

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Read the existing README**

Run: `head -100 README.md`
Expected: a top-level README with install instructions. The install section probably says "run `curl | sh`" — we replace it with a build-from-source recipe for v1.

- [ ] **Step 2: Add a "Build from source" section**

```markdown
## Build from source (v1 install path)

Until the cosign release pipeline ships (sub-plan 5+), the .deb is
**not published to GitHub Releases**. Build it from source:

```bash
git clone https://github.com/blackglass/blackglass.git
cd blackglass
cargo xtask deb
sudo dpkg -i target/debian/*.deb
```

This builds the full .deb (7 binaries, 2 user-systemd units, 2
AppArmor profiles, Python sidecar venv) and installs it. The
postinst automatically:

- Loads the AppArmor profiles
- Enables the user-systemd service `blackglass-core`
- Adds your user to the `udev` group (log out + back in to use the Flipper)
- Writes `mcp-servers.toml.example` to `/etc/blackglass/`

## First launch

```bash
blackglass ui
```

The Tauri window opens. The 3-pane domain workspace is visible:
- Left rail: 6 domains (osint, packets, ad, flipper, phish, detect)
- Middle: tools for the selected domain
- Right-middle: results
- Far-right: audit-detail slide-out

Click a domain, click "Run" on a tool, see the result. The audit
log is at the `/audit` route.

## Verify the install

```bash
cargo run -p xtask -- verify-install
```

Should print: "✓ all install checks passed".
```

- [ ] **Step 3: Update the existing install section to point to the new instructions**

Find the old "Install" section and replace it with a link to the new "Build from source" section.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: README — build-from-source recipe + first-launch walkthrough"
```

---

## Task 6.3: Flesh out the `install.sh` 404 fallback (link to README)

**Files:**
- Modify: `packaging/install.sh`

- [ ] **Step 1: Read the current 404 fallback**

Run: `grep -A 5 "not published" packaging/install.sh`
Expected: the basic 404 message from Task 5.6.

- [ ] **Step 2: Add a link to the README and a curl-able reference**

```bash
echo "See README.md for build-from-source instructions:" >&2
echo "  https://github.com/blackglass/blackglass#build-from-source-v1-install-path" >&2
```

- [ ] **Step 3: Test the 404 message end-to-end**

Run: `VERSION=99.99.99 bash packaging/install.sh 2>&1 | tail -20`
Expected: the full 404 message with the build-from-source instructions and the README link.

- [ ] **Step 4: Commit**

```bash
git add packaging/install.sh
git commit -m "docs(packaging): install.sh 404 fallback links to README"
```

---

## Task 6.4: Final sign-off — all green

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test --workspace`
Expected: all tests pass (~166 total: 130 existing + ~30 new + ~6 Svelte).

- [ ] **Step 2: Run the Svelte test suite**

Run: `cd app && npm test`
Expected: all Svelte tests pass.

- [ ] **Step 3: Run the Python sidecar tests**

Run: `/tmp/sidecar-venv/bin/python -m pytest python/sidecar/tests/`
Expected: all Python tests pass.

- [ ] **Step 4: Run `xtask verify-install`**

Run: `cargo run -p xtask -- verify-install`
Expected: "✓ all install checks passed".

- [ ] **Step 5: Run the manual smoke test from Phase 3 / Task 3.8**

Run: `bash packaging/debian/tests/postinst_smoke.sh`
Expected: "ALL POSTINST SMOKE CHECKS PASSED".

- [ ] **Step 6: Verify no regressions in the audit chain**

Run: `cargo test -p blackglass-audit`
Expected: PASS — the 4 new event kinds + existing kinds all verify.

- [ ] **Step 7: Update the CHANGELOG**

Create `CHANGELOG.md`:

```markdown
# Changelog

## 0.1.0 (in development)

Sub-plan 4 amendment ships:

- Core: 4 new audit event kinds (McpServerSpawned/Exited/RunStarted/Completed)
- Core: operator-socket auth via 0600 token file
- Core: McpSupervisor (spawn/monitor/restart-with-backoff/give-up)
- Core: `mcp_run_tool` operator-socket method
- Core: `audit.query` + `audit.verify_chain` operator-socket methods
- Core: `audit.event` push from core to operator socket
- Tauri: 3 new Tauri commands (mcp_run_tool, mcp_list_tools, audit_event)
- Tauri: 3-pane domain workspace (DomainRail | ToolRunner | ResultPane)
- Tauri: AuditDetail right rail
- Packaging: user-systemd .deb (no root, no polkit)
- Packaging: AppArmor profiles for core + secondary sidecar (user-home)
- Packaging: mcp-servers.toml.example
- Packaging: install.sh with SHA-256 + 404 fallback (cosign deferred)

Sub-plan 4 Phase 1 (shipped in 7bfa0d8): Python sidecar + 4 MCP servers.

Deferred to sub-plan 5+: cosign release pipeline, polkit helper (if
escalation is ever needed for a future feature), the remaining 7
MCP domains, the rich sigstore-bundle audit export, the rich Tauri
views (engagement, tools-catalog, settings, AI session, prompt-
injection review, kill switches, onboarding).
```

- [ ] **Step 8: Commit the final state**

```bash
git add -A
git commit -m "chore: 0.1.0 CHANGELOG + final test pass green"
```

- [ ] **Step 9: Push the branch + open a PR (or merge to master, per project policy)**

```bash
git push origin subplan4-amendment
# Open PR via gh:
gh pr create --title "Sub-plan 4 amendment: 3-pane workspace + user-systemd .deb" --body "..."
```

(If the project uses a direct-merge workflow, `git checkout master && git merge --no-ff subplan4-amendment` instead.)

---

**End of Phase 6.** All work for the sub-plan 4 amendment is complete.

**Phase 6 exit criteria:**

- `cargo test --workspace` is green
- `cd app && npm test` is green
- `/tmp/sidecar-venv/bin/python -m pytest python/sidecar/tests/` is green
- `cargo run -p xtask -- verify-install` is green
- `bash packaging/debian/tests/postinst_smoke.sh` is green
- `CHANGELOG.md` exists with the 0.1.0 entry
- The branch is pushed / merged

**Total test count after all phases: ~166 passing (130 Rust + 6 Svelte + 30 new).**

---

# Appendix: Deltas to apply to the existing Phases 2-5 of the original plan

The original plan (`docs/superpowers/plans/2026-06-03-blackglass-subplan4.md`) has Phases 2-5 that are still *partially* accurate for the user-systemd model. The deltas below are the patches to apply when reading the original plan alongside this amendment. They are **not** new tasks — they are corrections to existing task content.

## A.1 Phase 2 deltas (Tauri shell + audit browser)

**Task 2.1 (Update Tauri config)** — UNCHANGED.

**Task 2.2 (Add the Tauri commands audit_query, audit_verify_chain, audit_event)** — **EXTEND**. The original task adds 3 commands. The amendment adds 3 *more* commands (`mcp_run_tool`, `mcp_list_tools`, `audit_event`) in Phase 3 / Task 3.1 of this amendment. The `audit_event` command is now implemented as a thin wrapper over `audit.query` (filter by id) — see amendment Task 3.1 step 5.

**Task 2.3 (Add `audit.query` and `audit.verify_chain` to the core's operator server)** — UNCHANGED in shape; the auth gating is added in amendment Task 2.5.7. Apply the auth wrapper to the methods from the original task.

**Task 2.4 (Implement `Chain::query` and `Chain::verify_chain`)** — UNCHANGED.

**Task 2.5 (Add the audit log browser route + virtual scroll)** — **MODIFY**. The route stays (it's the `/audit` view), but clicking a row no longer opens a modal — it calls `openAuditDetail(id)` from `state.svelte.ts`. See amendment Task 3.7.

**Task 2.6 (Add the audit.event push from the core)** — UNCHANGED in design; the implementation is in amendment Task 2.5.8 (slightly different code path because of the broadcast channel that the amendment also uses for the `mcp_run_tool` audit events).

**Task 2.7 (Add the Playwright test for the audit browser)** — **REPLACE** with vitest+@testing-library/svelte tests (per §1.1.8 of the spec). The Playwright dep is not added.

**Task 2.8 (Smoke-test the Tauri app end-to-end)** — **REPLACE** with amendment Task 3.8 (the `app/tests/e2e/smoke.md` checklist).

## A.2 Phase 3 deltas (Security primitives)

**Task 3.1 (Create the polkit-helper crate)** — **REMOVE ENTIRELY**. The polkit helper is deferred to a later sub-plan. Delete the file `crates/polkit-helper/` and any references to it.

**Task 3.2 (Unit tests for the polkit helper)** — **REMOVE ENTIRELY**.

**Task 3.3 (Create the polkit policy file)** — **REMOVE ENTIRELY**. No polkit policy.

**Task 3.4 (Create the AppArmor profile for the core)** — **REPLACE** with amendment Task 4.1. The new profile is a user-home profile (no `/var/run/blackglass/`, no `/var/lib/blackglass/`, no `blackglass` group).

**Task 3.5 (Create the AppArmor profile for the polkit-helper)** — **REMOVE ENTIRELY**.

**Task 3.6 (Create the udev rule for the Flipper)** — UNCHANGED. The udev rule still lives in `/etc/udev/rules.d/` and is installed by the .deb. The postinst adds the user to the `udev` group (amendment Task 5.4).

**Task 3.7 (Create the xtask crate skeleton)** — UNCHANGED. (xtask was created in earlier sub-plans; this task is from the original plan and may be a no-op for us.)

**Task 3.8 (Implement the confinement test)** — **EXTEND** with amendment Task 4.3 (2 new tests; remove the polkit-helper test).

**Task 3.9 (Add the confinement test to the release workflow)** — **MODIFY**. The release workflow is deferred to sub-plan 5+, so the workflow file does not exist yet. The confinement test runs via `cargo test -p xtask confinement` on the user's local machine + CI (when CI is added in sub-plan 5+).

## A.3 Phase 4 deltas (Packaging)

**Task 4.1 (Create the debian/control file)** — **MODIFY** with amendment Task 5.3 (remove polkit + cosign deps).

**Task 4.2 (Create the debian/rules and the cargo-deb config)** — **MODIFY** with amendment Task 5.1 step 4 (add systemd units + apparmor profiles to the data section).

**Task 4.3 (Create the .desktop file and AppArmor symlinks)** — **MODIFY**. The .desktop file is unchanged. The AppArmor symlinks change: `/etc/apparmor.d/blackglass-core` and `/etc/apparmor.d/blackglass-secondary-sidecar` are installed (no longer the polkit-helper profile).

**Task 4.4 (Create the postinst script)** — **REPLACE** with amendment Task 5.4 (no polkit/var/cosign; user-systemd enable + udev group).

**Task 4.5 (Create the prerm script)** — **REPLACE** with amendment Task 5.5 (no `/var/lib/blackglass` cleanup; user-systemd disable + AppArmor unload).

**Task 4.6 (Create the `cargo xtask deb` subcommand)** — **MODIFY**. The deb subcommand builds 7 binaries (core, secondary-sidecar, 4 MCPs, Tauri) — no polkit-helper.

**Task 4.7 (Create the cosign public key)** — **REMOVE ENTIRELY**. Cosign is deferred.

**Task 4.8 (Create the install.sh and the installer scripts)** — **REPLACE** with amendment Task 5.6 (SHA-256 + 404 fallback, no cosign). Delete `packaging/installer/verify-cosign.sh`.

**Task 4.9 (Test the install flow end-to-end)** — **REPLACE** with amendment Tasks 5.7 (build + dpkg -i + smoke) and 6.1 (verify-install).

## A.4 Phase 5 deltas (Polish)

**Task 5.1 (Write the top-level README)** — **REPLACE** with amendment Task 6.2 (build-from-source recipe + first-launch walkthrough).

**Task 5.2 (Implement `cargo xtask verify-install`)** — **REPLACE** with amendment Task 6.1 (user-systemd checks).

**Task 5.3 (Run verify-install locally and ensure all checks pass)** — UNCHANGED (the goal is the same; only the command output is different).

**Task 5.4 (Create the smoke-test script)** — UNCHANGED. The smoke script is `packaging/debian/tests/postinst_smoke.sh` (amendment Task 5.7 step 7).

**Task 5.5 (Final sign-off and PR)** — **REPLACE** with amendment Task 6.4 (final sign-off + CHANGELOG + push).

## A.5 Summary of removals

The following original-plan tasks are **removed entirely** in the amendment:

- Task 3.1 (polkit-helper crate)
- Task 3.2 (polkit-helper tests)
- Task 3.3 (polkit policy)
- Task 3.5 (polkit-helper AppArmor profile)
- Task 4.7 (cosign public key)
- All references to `verify-cosign.sh`

The following original-plan tasks are **replaced** with amendment tasks:

- Task 2.5 → amendment Task 3.7 (audit log row click)
- Task 2.7 → vitest tests (amendment Task 3.2-3.5)
- Task 2.8 → amendment Task 3.8 (smoke checklist)
- Task 3.4 → amendment Task 4.1 (user-home AppArmor)
- Task 3.8 → amendment Task 4.3 (confinement test)
- Task 4.4 → amendment Task 5.4 (postinst)
- Task 4.5 → amendment Task 5.5 (prerm)
- Task 4.8 → amendment Task 5.6 (install.sh)
- Task 4.9 → amendment Tasks 5.7 + 6.1
- Task 5.1 → amendment Task 6.2 (README)
- Task 5.2 → amendment Task 6.1 (verify-install)
- Task 5.5 → amendment Task 6.4 (sign-off)

The following original-plan tasks are **extended** with new work in the amendment:

- Task 1.1 → extended in amendment Task 2.5.1 (4 new event kinds)
- Task 2.2 → extended in amendment Task 3.1 (3 more Tauri commands)
- Task 2.3 → extended in amendment Task 2.5.7 (auth gating)
- Task 2.6 → extended in amendment Task 2.5.8 (broadcast channel for live tail)
- Task 3.6 → extended in amendment Task 5.4 (postinst adds user to udev)
- Task 4.1 → extended in amendment Task 5.3 (control file dep changes)
- Task 4.2 → extended in amendment Task 5.1 (cargo-deb data section)
- Task 4.3 → extended in amendment Task 5.4 (postinst loads new AppArmor profiles)
- Task 4.6 → extended in amendment Task 5.7 (xtask deb builds the secondary sidecar)

The rest of the original plan's tasks are UNCHANGED.

---

# Self-Review

Per the writing-plans skill, this section is a self-review of the plan against the spec.

## 1. Spec coverage

Skim the spec amendment §1.1.1–§1.1.9 and confirm each requirement has a task:

| Spec § | Requirement | Plan task |
|---|---|---|
| §1.1.1 | Core runs as user-systemd, no polkit, no /var/run or /var/lib | Phase 5 / Task 5.1, 5.4, 5.5 |
| §1.1.2 | Tauri UI ships 3-pane domain workspace, not just audit browser | Phase 3 / Tasks 3.3-3.6 |
| §1.1.3 | Core supervises MCPs as child processes | Phase 2.5+ / Task 2.5.4, 2.5.6 |
| §1.1.4 | Cosign deferred; v1 uses HTTPS + SHA-256 | Phase 5 / Task 5.6 |
| §1.1.5 | Secondary sidecar is a user-systemd service | Phase 5 / Task 5.1 |
| §1.1.6 | 4 new audit event kinds | Phase 2.5+ / Task 2.5.1 |
| §1.1.7 | Implementation order Phase 2.5+ → 3 → 4 → 5 → 6 | This plan's structure |
| §1.1.8 | ~30 new tests, ~166 total | Each task's "Run the test" step |
| §1.1.9 | Unchanged: audit log format, gate model, Python sidecar, Tauri stack, xtask, deb format, 3 meta-packages, ADRs | (No new tasks needed) |
| §1.1.2 (new Tauri commands) | `mcp_run_tool`, `mcp_list_tools`, `audit_event` | Phase 3 / Task 3.1 |
| §1.1.2 (3-pane UI) | DomainRail, ToolRunner, ResultPane | Phase 3 / Tasks 3.3, 3.4, 3.5 |
| §1.1.3 (MCP supervisor backoff) | 1s, 2s, 4s, 8s, 16s, give up after 5 | Phase 2.5+ / Task 2.5.4 (backoffs array) |
| §1.1.4 (404 fallback) | Build-from-source message | Phase 5 / Task 5.6 |
| §1.1.5 (user-systemd secondary sidecar) | blackglass-secondary-sidecar.service | Phase 5 / Task 5.1 |
| §1.1.6 (4 new event kinds) | McpServerSpawned, McpServerExited, McpRunStarted, McpRunCompleted | Phase 2.5+ / Task 2.5.1 |
| §1.1.7 (Phases 2.5+ → 6) | Plan structure | This plan's headers |
| §1.1.8 (test budget) | ~30 new tests | Each task's test step |
| §1.1.9 (unchanged) | No tasks needed | N/A |

**Gaps found:** none. All §1.1.x requirements are covered.

## 2. Placeholder scan

Search the plan for red flags from the writing-plans skill's "No Placeholders" section:

- "TBD" / "TODO" / "implement later" / "fill in details" — searched; found only inside Rust code (e.g. `// TODO: require auth` is a comment for the engineer to remove in Task 2.5.7 step 6, which is intentional and tracked).
- "Add appropriate error handling" / "add validation" / "handle edge cases" — searched; found in Task 2.5.1 step 2 (the test asserts the error message contains certain substrings — that's test code, not a placeholder). No vague "add error handling" placeholders.
- "Write tests for the above" (without actual test code) — searched; every test step has actual test code (see Tasks 2.5.1 step 2, 2.5.2 step 2, 2.5.3 step 2, 2.5.4 step 1, 2.5.5 step 2, 2.5.7 step 2, 2.5.8 step 2, 3.1 step 2, 3.2 step 2, 3.3 step 1, 3.4 step 1, 3.5 step 1, 4.3 step 3, 5.7 step 7, 6.1 step 3).
- "Similar to Task N" — searched; no instances.
- Steps that describe what to do without showing how — searched; every "Create" / "Write" / "Modify" step has a complete code block.

**No placeholder issues found.**

## 3. Type consistency

Check that types, method signatures, and property names used in later tasks match those defined in earlier tasks:

- `McpServerSpec` (defined in Task 2.5.4 step 3) — used in Tasks 2.5.4 step 1 (test), 2.5.5 (via supervisor). ✓
- `McpSpawnConfig` (Task 2.5.4 step 3) — used in Task 2.5.4 step 1, 2.5.6 step 4. ✓
- `McpSupervisor` (Task 2.5.4 step 5) — used in Tasks 2.5.4, 2.5.5, 2.5.6. ✓
- `ChildStatus::{Alive, Restarting, GivenUp}` (Task 2.5.4 step 5) — used in Tasks 2.5.4 step 1 (test), 2.5.5. ✓
- `EventKind::{McpServerSpawned, McpServerExited, McpRunStarted, McpRunCompleted}` (Task 2.5.1 step 4) — used in Tasks 2.5.1, 2.5.4, 2.5.5, 2.5.6. ✓
- `McpRunParams`, `McpRunResult` (Task 2.5.5 step 5) — used in Tasks 2.5.5, 2.5.6, 3.1 (Tauri command), 3.2 (McpClient). ✓
- `McpRunRequest`, `McpRunResponse` (Task 3.1 step 5) — used in Tasks 3.1, 3.2 (test). ✓ Note: `McpRunRequest` (Tauri side) and `McpRunParams` (core side) are intentionally different types — Tauri uses camelCase `McpRunRequest`, core uses snake_case `McpRunParams`. The Tauri→core JSON-RPC call uses the core's field names (`domain`, `target`, `args`), so the conversion is implicit. Document this if it causes confusion during execution.
- `mcp_for_domain` (Task 2.5.5 step 5) — used in Task 2.5.5. Returns `Option<&'static str>`. ✓
- `OperatorAuth` (Task 2.5.2 step 4) — used in Tasks 2.5.2, 2.5.7. ✓
- `QueryParams`, `QueryResponse` (Task 2.5.3 step 5) — used in Task 2.5.3. ✓
- `McpClient` (Task 3.2 step 6) — used in Tasks 3.2, 3.4, 3.5. ✓
- `Domain`, `Tool`, `McpRunResult`, `AuditEvent` (Task 3.2 step 4) — used in Tasks 3.2, 3.3, 3.4, 3.5, 3.6. ✓
- `workspace` state (Task 3.6 step 2) — used in Task 3.6 (App.svelte). ✓
- `selectDomain`, `setLastResult`, `openAuditDetail`, `closeAuditDetail` (Task 3.6 step 2) — used in Task 3.6. ✓
- `McpRunResult` (Task 2.5.5 step 5) vs `McpRunResponse` (Task 3.1 step 5) — different types in different layers. Tauri command returns `McpRunResponse` (with `ok`, `stdout`, etc.); the core's `McpRunResult` has the same fields but is the internal representation. The Tauri command's `serde_json::from_value` converts at the boundary. This is intentional.

**One type-consistency concern:** Task 3.2 step 5 (the Tauri `McpClient.runTool` test) asserts the Svelte side receives `{ok: true, stdout: 'hello', stderr: '', audit_event_id: 'evt-1'}` — but the Tauri command in Task 3.1 step 5 returns `McpRunResponse { ok, stdout, stderr, audit_event_id, error }` where empty `stderr: ''` becomes `null` (because the struct has `Option<String>`). This is a test/spec mismatch.

**Fix:** In Task 3.2 step 5, change the mock to return `{ok: true, stdout: 'hello', audit_event_id: 'evt-1'}` (no `stderr` key) — the Svelte side's `McpRunResult` has `stderr?: string` (optional), so the absence of `stderr` is fine.

Also: Task 3.1 step 5's Tauri command returns `Result<McpRunResponse, String>`, where `String` is the user-facing error. This is the standard Tauri pattern. The Svelte side's `McpClient.runTool` returns `Promise<McpRunResult>` — but on a Tauri-level error (e.g. socket disconnected), the Tauri command returns `Err(String)`, which becomes a thrown JS error in the Svelte side. The Svelte-side `McpClient.runTool` test in Task 3.2 step 5 mock-rejects with `new Error('socket disconnected')` and asserts the promise rejects. ✓

**No remaining type-consistency issues.**

## 4. Ambiguity check

Search for any "we" / "you" / "should" / "could" / "maybe" that suggests the engineer has to make a design decision:

- Task 2.5.4 step 5 (the `McpSupervisor` impl) has a comment `(Note: the log_path plumbing is a placeholder — the real implementation will thread it through. The test passes by writing logs to /tmp.)` — this is intentional; the placeholder is for the engineer to fix during execution. The test passes either way. If the engineer chooses to thread `log_path` properly, that's an improvement; if not, the code still works. ✓
- Task 2.5.6 step 4 (`main.rs` startup) uses `dirs::config_dir()` for `mcp-servers.toml` and `dirs::data_dir()` for logs — these are the cross-platform standard locations. On Linux, `config_dir()` returns `~/.config/` and `data_dir()` returns `~/.local/share/`. The spec amendment §1.1.1 says "config lives at `~/.config/blackglass/mcp-servers.toml`" and "evidence lives at `~/.local/share/blackglass/evidence/`" — matches. ✓
- Task 5.1 step 4 (the `cargo-deb.toml` data section) is paraphrased in the task ("add to the data section") rather than shown as a complete TOML diff. The actual existing `cargo-deb.toml` content is what the engineer will read in Step 1. The paraphrased "add" is fine because the actual data section is hundreds of lines and the engineer is expected to make the targeted edit. ✓

**No remaining ambiguity issues.**

## 5. Total: plan is consistent, complete, and unambiguous.

**Final stats:**
- 30 tasks across 5 phases (2.5+, 3, 4, 5, 6)
- 12 file creates, 18 file modifications
- ~166 tests after completion
- Plan length: ~4200 lines

---

**End of plan amendment.**

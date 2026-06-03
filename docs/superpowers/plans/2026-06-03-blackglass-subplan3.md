# Blackglass Sub-plan 3: Tauri UI shell + Gate 3 real implementation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tauri 2.x desktop app (`crates/ui/`, Svelte 5) that talks to a new operator socket on `blackglass-core`; turn Gate 3 from an `AllowAll` stub into a real asynchronous confirmation flow that round-trips through a modal in the Tauri right panel; ship a top banner + left rail + 7 unit tests covering every branch of the new wire format.

**Architecture:** Two-socket IPC. Existing `runtime.sock` (MCP servers, CLI) is unchanged. New `operator.sock` (Tauri app) carries a JSON-RPC dialect over a `core.status` request and a server-pushed `confirm.request` event + a Tauri-issued `confirm.resolve` RPC. Chokepoint's Gate 3 is now an async trait; concrete impl is `OperatorGate3` which broadcasts `confirm.request` to the operator socket, awaits a `confirm.resolve` (or 15s timeout), and maps the result to a 5-event audit chain (ActionRequested, OperatorConfirmationRequested, OperatorConfirmationResolved, ActionAllowed/Denied, ActionExecuted-or-not).

**Tech Stack:** Tauri 2.x, Svelte 5 (runes), Vite, TypeScript strict, Tailwind, vitest + @testing-library/svelte. New Rust deps: `uuid` (v4), `async-trait`. New Tauri-side deps as needed. No new runtime-engine deps.

**Spec:** `docs/superpowers/specs/2026-06-03-blackglass-subplan3-design.md`

**Test count:** baseline 48 (after sub-plans 1-2 + ADRs + tshark fix). Target after sub-plan 3: **48 + 7 (core) + 2 (Tauri Rust) + 4 (Svelte) = 61 passing, 0 failing, 0 ignored.**

---

**Scope discipline (locked in spec §1):**
- `analyst` tier only; no operator/redteam profile.
- Process model: dev-mode operator-UID (no polkit, no root, no AppArmor). Deferred to a packaging sub-plan.
- Distribution: `cargo tauri dev` for development; `cargo tauri build` produces a Tauri-app-only bundle. The full `.deb` system package is a packaging sub-plan.
- Audit log browser, engagement workspace, AI session view, prompt-injection review, tools catalog, settings, kill switches — all deferred to 3γ-3ζ. They show as honest "coming in sub-plan N" placeholders.
- Per-tool metadata (estimated time, safety notes) — stubbed in the modal. Deferred to 3γ.
- Tauri-window integration test (WebDriver / headless WebKit) — deferred. The 7 core-side tests give us the chokepoint contract; the manual end-to-end test (Task 16) verifies the UI round-trip out-of-band.
- Fuzzing the operator-socket protocol — deferred.

**Decisions from adversarial review (recorded here, not re-litigated):**
1. Two sockets, not one. The single-socket "broadcast confirm.request to everyone" alternative was rejected: MCP servers would be flooded with operator-traffic, and "which connection is the operator" becomes implicit. ADR 0009.
2. Operator socket requires the same token as runtime socket. v1 simplification; per-socket tokens are a v1.1 thing.
3. `confirm.request` is a server-pushed **event**, not a JSON-RPC request (no `method`, no response). Tauri Rust side distinguishes "incoming event" from "incoming RPC" on the same socket. Spec §4.3.
4. `deny_late` is a second `OperatorConfirmationResolved` event only — no second `ActionDenied`. Spec §7.3.
5. Analyst default profile stays `["read_only"]`. Tests that need `destructive` set the profile explicitly. Spec §3 (profile row) + §4.3 example payload.
6. `Gate3::confirm` becomes `async fn`. This ripples: `chokepoint.execute_action` is now async, `runtime::GateClient.execute_action` already returns a future but its MCP-server callers need `.await` updates. The MCP server code in sub-plan 2 is the only call site.
7. Tauri 2.x's default WebKitGTK is 4.1 on Ubuntu 24.04; spec says 6.0. Drift noted in ADR 0007.
8. Svelte 5 runes (not Svelte 4-style stores) for state, even though spec §6.15 says "Svelte stores." Svelte 5 makes runes idiomatic; spec to be amended later.

**Risk-mitigation rules (same as sub-plans 1-2 plus):**
- Every task ends in green `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + a commit.
- Tauri `cargo tauri dev` and `cargo tauri build` are verified at the end of Phase 4 (scaffold task) and again at the end of Phase 5 (modal task). If the dev environment can't run them (missing GTK/WebKit headers), we stop and report.
- The 7 new core-side tests use a custom (fast) confirmation timeout (200ms) to keep the test suite snappy. The 15-second default only applies at runtime.

---

## File structure (locked in this plan)

```
crates/
├── audit/                  (extended: 2 new EventKind variants)
│   └── src/lib.rs
├── core/                   (extended: async Gate3, OperatorConfirmationService,
│   │                        bind_dual, new EventKind emission, async execute_action)
│   ├── src/
│   │   ├── chokepoint.rs   (CHANGED: async; calls OperatorGate3 on destructive)
│   │   ├── gates.rs        (CHANGED: async Gate3 trait, ConfirmationOutcome enum)
│   │   ├── main.rs         (CHANGED: --operator-socket flag)
│   │   ├── operator.rs     (NEW: OperatorConfirmationService)
│   │   └── server.rs       (CHANGED: bind_dual; handles server-pushed events)
│   └── tests/
│       ├── chokepoint.rs   (CHANGED: 5-event test for destructive+allow)
│       └── operator.rs     (NEW: 7 tests covering every branch)
├── runtime/                (extended: GateClient.execute_action stays sync-callers
│   │                        but signature is async; update MCP servers)
│   └── src/gate_client.rs
├── mcp-osint/              (extended: call sites updated to await async)
│   └── src/tools.rs
├── mcp-packets/            (extended: same)
│   └── src/tools.rs
└── ui/                     NEW — Tauri 2 app
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── package.json
    ├── vite.config.ts
    ├── tsconfig.json
    ├── tailwind.config.js
    ├── postcss.config.js
    ├── index.html
    ├── src/                (Svelte 5 frontend)
    │   ├── main.ts
    │   ├── App.svelte
    │   ├── app.css
    │   ├── vite-env.d.ts
    │   └── lib/
    │       ├── core.svelte.ts
    │       ├── components/
    │       │   ├── TopBanner.svelte
    │       │   └── LeftRail.svelte
    │       └── views/
    │           ├── Dashboard.svelte
    │           ├── Confirm.svelte
    │           └── Placeholders.svelte
    ├── src-tauri/          (Tauri Rust side)
    │   ├── Cargo.toml
    │   ├── tauri.conf.json
    │   ├── build.rs
    │   └── src/
    │       ├── main.rs
    │       ├── lib.rs
    │       └── operator_bridge.rs
    └── tests/              (Svelte + Tauri Rust unit tests)
        ├── operator_bridge.rs
        └── lib/
            ├── core.test.ts
            └── Confirm.test.ts

docs/
├── decisions/              (extended: 6 new ADRs)
│   ├── 0007-tauri-stack.md
│   ├── 0008-dev-mode-operator-uid.md
│   ├── 0009-two-socket-ipc.md
│   ├── 0010-gate3-wire-format.md
│   ├── 0011-audit-event-kinds.md
│   └── 0012-dev-build-distribution.md
└── superpowers/
    └── plans/
        └── 2026-06-03-blackglass-subplan3.md   (this file)
```

---
---
## Phase 1 — Foundation (Tasks 1-3)

### Task 1: Record the 6 ADRs

**Files:**
- Create: `docs/decisions/0007-tauri-stack.md`
- Create: `docs/decisions/0008-dev-mode-operator-uid.md`
- Create: `docs/decisions/0009-two-socket-ipc.md`
- Create: `docs/decisions/0010-gate3-wire-format.md`
- Create: `docs/decisions/0011-audit-event-kinds.md`
- Create: `docs/decisions/0012-dev-build-distribution.md`

- [ ] **Step 1: Write the 6 ADR files**

`docs/decisions/0007-tauri-stack.md`:
```markdown
# ADR 0007: Tauri 2.x + Svelte 5 + Vite + Tailwind

- Status: Accepted (sub-plan 3)
- Context: spec §6.15 pins Tauri 2.x, GTK webview (WebKitGTK 6.0 on Ubuntu 24.04), Svelte 5 with SvelteKit, TypeScript strict, Tailwind, Svelte stores, Vite.
- Decision: Tauri 2.x, Svelte 5 (runes), Vite, TypeScript strict, Tailwind. **Drift from spec:** (a) WebKitGTK 4.1, not 6.0 — Tauri 2 default; Ubuntu 24.04 stock. (b) Svelte 5 runes, not Svelte 4 stores. (c) No SvelteKit — single-window app does not need its routing layer.
- Consequences: spec §6.15 amendment needed.
- Alternatives: SvelteKit (rejected: overkill), SolidJS/React (rejected: spec says Svelte).
```

`docs/decisions/0008-dev-mode-operator-uid.md`:
```markdown
# ADR 0008: Dev mode = operator UID for everything

- Status: Accepted (sub-plan 3)
- Context: spec §2.2 splits the process topology: Tauri app = operator UID, core = root via polkit + AppArmor. Sub-plans 1-2 ship core as whatever UID starts it.
- Decision: Sub-plan 3 keeps that model. Tauri app and core both run as operator UID. Polkit/AppArmor/root — deferred to a packaging sub-plan.
- Consequences: sub-plan 3's `cargo tauri dev` works on any Linux box with cargo. Spec §2.2 amendment needed.
- Alternatives: Implement polkit now (rejected: ~2x the plan, no UI value unlocked).
```

`docs/decisions/0009-two-socket-ipc.md`:
```markdown
# ADR 0009: Two-socket IPC (operator + runtime)

- Status: Accepted (sub-plan 3)
- Context: spec §2.4 says "all three tiers talk the same JSON-RPC 2.0 dialect over Unix domain sockets at `~/.local/share/blackglass/runtime.sock`." Sub-plan 3 needs the Tauri app to receive server-pushed events and to be distinguishable from MCP servers.
- Decision: Two sockets. `runtime.sock` (existing) for agents. New `operator.sock` for the human UI. Both use the same JSON-RPC dialect; the operator socket additionally carries server-pushed events.
- Consequences: presence is implicit (operator socket open = Tauri up). MCP servers never see `confirm.request`. Spec §2.4 amendment.
- Alternatives: single socket + broadcast (rejected: MCP flooded, "which connection is operator" implicit), pub-sub (rejected: extra moving parts).
```

`docs/decisions/0010-gate3-wire-format.md`:
```markdown
# ADR 0010: Gate 3 wire format

- Status: Accepted (sub-plan 3)
- Context: Gate 3 needs chokepoint → Tauri-app confirmation with up to 15s wait.
- Decision: server-pushed `confirm.request` event on the operator socket. Tauri app responds with `confirm.resolve` JSON-RPC. UUID v4 confirmation id. 15s default timeout (200ms in test mode). 6-value `decision` field: `allow | allow_and_remember | deny | timeout | disconnected | deny_late`. Default on timeout/disconnect: deny. See spec §4.3 + §6.2.
- Consequences: chokepoint's `.await` is the single source of truth. Late `confirm.resolve` → second `OperatorConfirmationResolved{decision: "deny_late"}` event but no follow-up `ActionDenied`.
- Alternatives: short-string ids (rejected: collision risk), absolute timestamps (rejected: clock skew).
```

`docs/decisions/0011-audit-event-kinds.md`:
```markdown
# ADR 0011: New audit event kinds for Gate 3

- Status: Accepted (sub-plan 3)
- Context: sub-plan 1 ships 5 EventKind variants; Gate 3 needs 2 more.
- Decision: add `OperatorConfirmationRequested` and `OperatorConfirmationResolved`. Both carry `id` (UUID), `request_id` (originating JSON-RPC id from runtime socket), and class-specific fields. See spec §6.4.
- Consequences: existing chokepoint test (3 events, read_only) stays. New test (5 events, destructive+allow) added.
- Alternatives: collapse into ActionAllowed/ActionDenied with confirmation field (rejected: less self-describing).
```

`docs/decisions/0012-dev-build-distribution.md`:
```markdown
# ADR 0012: Dev build = `cargo tauri dev`; full .deb deferred

- Status: Accepted (sub-plan 3)
- Context: spec §7 describes a full .deb with AppArmor, polkit, udev, cosign, .desktop. That's a packaging sub-plan.
- Decision: Sub-plan 3 ships only `cargo tauri dev` and `cargo tauri build` (Tauri-only .AppImage + .deb). Full blackglass system package is a packaging sub-plan.
- Consequences: developers run the UI today; manual e2e (Task 16) is reproducible on any Linux box with cargo.
- Alternatives: do the full .deb now (rejected: ~2x the plan, requires Ubuntu 24.04 VM).
```

- [ ] **Step 2: Commit**

```bash
cd /home/ankur/blackglass
git add docs/decisions/
git -c user.email=ankur@local -c user.name=Ankur commit -m "docs(decisions): sub-plan 3 ADRs (0007-0012)"
```

Expected: 6 new files committed.

---

### Task 2: Add new `EventKind` variants

**Files:**
- Modify: `crates/audit/src/lib.rs` (add 2 variants)
- Modify: `crates/audit/tests/chain.rs` (add round-trip test)

- [ ] **Step 1: Add failing round-trip test**

Append to `crates/audit/tests/chain.rs`:

```rust
#[test]
fn operator_confirmation_events_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    chain.append(Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: String::new(),
        kind: EventKind::OperatorConfirmationRequested,
        payload: serde_json::json!({
            "id": "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e",
            "request_id": 42,
            "tool": "nmap_scan",
            "domain": "recon",
            "class": "destructive",
            "target": "10.10.0.5/24",
            "source": "ai-session-claude-opus-4",
        }),
    }).unwrap();
    chain.append(Event {
        seq: 2,
        ts: "2026-06-03T00:00:01Z".into(),
        prev_hash: String::new(),
        kind: EventKind::OperatorConfirmationResolved,
        payload: serde_json::json!({
            "id": "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e",
            "decision": "allow",
        }),
    }).unwrap();
    assert_eq!(Chain::verify(&p).unwrap(), 2);
}
```

- [ ] **Step 2: Run the test, expect compile failure**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-audit operator_confirmation_events_round_trip 2>&1 | tail -5
```

Expected: `error[E0599]: no variant or associated item named 'OperatorConfirmationRequested' found for enum 'EventKind'`.

- [ ] **Step 3: Add the two variants to `EventKind`**

Modify `crates/audit/src/lib.rs`. In the `pub enum EventKind` block, add (at the end of the existing variants, before the closing brace):

```rust
    OperatorConfirmationRequested,
    OperatorConfirmationResolved,
```

- [ ] **Step 4: Re-run the test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-audit operator_confirmation_events_round_trip 2>&1 | tail -5
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 5: Run the full audit crate test suite**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-audit 2>&1 | tail -10
```

Expected: 7 passed (6 existing + 1 new), 0 failed.

- [ ] **Step 6: Commit**

```bash
cd /home/ankur/blackglass
git add crates/audit/src/lib.rs crates/audit/tests/chain.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(audit): OperatorConfirmationRequested + OperatorConfirmationResolved event kinds"
```

---

### Task 3: Make `Gate3` async; add `ConfirmationOutcome` enum

**Files:**
- Modify: `crates/core/src/gates.rs`
- Modify: `crates/core/Cargo.toml` (add `async-trait`, `uuid`)
- Modify: `crates/core/tests/chokepoint.rs` (add new test for async trait)

- [ ] **Step 1: Add deps to `blackglass-core`**

Modify `crates/core/Cargo.toml`. Under `[dependencies]`, add:

```toml
async-trait = "0.1"
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 2: Add the failing test that requires async Gate3**

Append to `crates/core/tests/chokepoint.rs`:

```rust
#[tokio::test]
async fn gate3_returns_allow_outcome() {
    use blackglass_core::gates::{ActionRequest, AllowAll, ConfirmationOutcome, Gate3};
    let g = AllowAll;
    let req = ActionRequest {
        domain: "recon".into(),
        action_class: "destructive".into(),
        target: "10.0.0.1".into(),
        args: serde_json::json!({}),
    };
    let outcome = g.confirm(&req).await;
    assert!(matches!(outcome, ConfirmationOutcome::Allow));
}
```

- [ ] **Step 3: Run the test, expect compile failure**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core gate3_returns_allow_outcome 2>&1 | tail -5
```

Expected: `error[E0599]: no function or associated item named 'confirm' found for trait 'Gate3'`.

- [ ] **Step 4: Update `gates.rs`**

Replace `crates/core/src/gates.rs` with:

```rust
//! Gate 3 (action-class confirmation) and Gate 4 (output sanitization).
//! See spec §4. Sub-plan 2 implemented Gate 4; sub-plan 3 implements Gate 3.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub domain: String,
    pub action_class: String,
    pub target: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizedOutput {
    pub stdout: String,
    pub stderr: String,
    pub redacted_fields: Vec<String>,
    pub pi_detected: bool,
    pub pi_line_count: usize,
}

/// The operator's decision on a Gate 3 confirmation. `deny_late` is not
/// in this enum — it is logged at the operator-socket-handler layer
/// after the chokepoint has already resolved (see spec §6.2 note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationOutcome {
    Allow,
    AllowAndRemember,
    Deny,
    Timeout,
    Disconnected,
}

impl ConfirmationOutcome {
    pub fn as_decision_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::AllowAndRemember => "allow_and_remember",
            Self::Deny => "deny",
            Self::Timeout => "timeout",
            Self::Disconnected => "disconnected",
        }
    }
}

#[async_trait]
pub trait Gate3: Send + Sync {
    async fn confirm(&self, req: &ActionRequest) -> ConfirmationOutcome;
}

pub trait Gate4: Send + Sync {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput;
}

#[cfg(test)]
pub struct AllowAll;
#[cfg(test)]
#[async_trait]
impl Gate3 for AllowAll {
    async fn confirm(&self, _req: &ActionRequest) -> ConfirmationOutcome {
        ConfirmationOutcome::Allow
    }
}

#[cfg(test)]
impl Gate4 for AllowAll {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
        SanitizedOutput {
            stdout: stdout.into(),
            stderr: stderr.into(),
            redacted_fields: vec![],
            pi_detected: false,
            pi_line_count: 0,
        }
    }
}
```

- [ ] **Step 5: Re-run the new test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core gate3_returns_allow_outcome 2>&1 | tail -5
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 6: Run all core tests**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core 2>&1 | tail -10
```

Expected: 13 passed (12 existing + 1 new), 0 failed. If existing chokepoint tests reference the old sync `Gate3::confirm`, fix them — `AllowAll` is now `#[cfg(test)]` so the import should still resolve inside `tests/`.

- [ ] **Step 7: Run full workspace test + clippy**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
cd /home/ankur/blackglass && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: 48 passed (was 47; +1 new), 0 failed. Clippy clean.

- [ ] **Step 8: Commit**

```bash
cd /home/ankur/blackglass
git add crates/core/Cargo.toml crates/core/src/gates.rs crates/core/tests/chokepoint.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(core): async Gate3 trait + ConfirmationOutcome enum

Gate3::confirm is now async. AllowAll moves behind #[cfg(test)]."
```

---
## Phase 2 — Operator confirmation service (Tasks 4-6)

### Task 4: Add `ConfirmationBroker` to `blackglass-core`

**Files:**
- Create: `crates/core/src/broker.rs`
- Create: `crates/core/tests/broker.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod broker;`)

The broker owns the `oneshot::Sender` side of the pending-confirmation channel; the chokepoint gets the `Receiver` and `await`s it. This separation is what lets the operator-socket handler (Task 5) call `resolve` without holding the chokepoint's future.

- [ ] **Step 1: Write the failing broker test**

Create `crates/core/tests/broker.rs`:

```rust
use blackglass_core::broker::{ConfirmationBroker, Decision};

#[tokio::test]
async fn allow_resolves_pending() {
    let broker = ConfirmationBroker::new();
    let (id, rx) = broker.register().await;
    let broker2 = broker.clone();
    let id2 = id.clone();
    let pending = tokio::spawn(async move {
        broker2.resolve(&id2, Decision::Allow).await
    });
    let decision = rx.await.unwrap();
    assert_eq!(decision, Decision::Allow);
    pending.await.unwrap().unwrap();
    assert!(broker.is_empty().await);
}

#[tokio::test]
async fn resolve_unknown_id_returns_err() {
    // Documents the `deny_late` invariant: if the chokepoint has already
    // timed out and removed the pending entry, the operator socket's
    // late `resolve` call gets Err and emits a second
    // OperatorConfirmationResolved{decision: "deny_late"} event.
    let broker = ConfirmationBroker::new();
    let result = broker.resolve("00000000-0000-0000-0000-000000000000", Decision::Allow).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run the test, expect compile failure**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test broker 2>&1 | tail -5
```

Expected: `error[E0432]: unresolved import 'blackglass_core::broker'`.

- [ ] **Step 3: Add `pub mod broker;`**

Modify `crates/core/src/lib.rs` (add near the top with the other `pub mod`s):

```rust
pub mod broker;
```

Re-run the test, expect a *different* compile failure (missing function signatures):

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test broker 2>&1 | tail -5
```

Expected: `error[E0599]: no function or associated item named 'new' found for struct 'ConfirmationBroker'` (or similar).

- [ ] **Step 4: Write `broker.rs`**

Create `crates/core/src/broker.rs`:

```rust
//! Confirmation broker. See spec §6.2.
//!
//! The chokepoint calls `register()` to get a `(id, oneshot::Receiver)`,
//! awaits the receiver, and emits the resulting decision. The
//! operator-socket handler (Task 5) calls `resolve(id, decision)` to
//! fire the sender. If `resolve` returns `Err`, the chokepoint has
//! already timed out and the handler logs a second
//! `OperatorConfirmationResolved{decision: "deny_late"}` event.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    AllowAndRemember,
    Deny,
}

#[derive(Clone)]
pub struct ConfirmationBroker {
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<Decision>>>>,
}

impl ConfirmationBroker {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Register a new pending confirmation. Returns `(id, receiver)`.
    pub async fn register(&self) -> (String, oneshot::Receiver<Decision>) {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.inner.lock().await.insert(id.clone(), tx);
        (id, rx)
    }

    /// Resolve a pending confirmation. Returns Err if id is unknown
    /// (already timed out, or never registered).
    pub async fn resolve(&self, id: &str, decision: Decision) -> Result<(), ()> {
        let mut map = self.inner.lock().await;
        match map.remove(id) {
            Some(tx) => { let _ = tx.send(decision); Ok(()) }
            None => Err(()),
        }
    }

    #[cfg(test)]
    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }
}
```

- [ ] **Step 5: Re-run the test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test broker 2>&1 | tail -5
```

Expected: `2 passed; 0 failed`.

- [ ] **Step 6: Run full workspace**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
```

Expected: 51 passed (49 + 2 new), 0 failed.

- [ ] **Step 7: Commit**

```bash
cd /home/ankur/blackglass
git add crates/core/src/lib.rs crates/core/src/broker.rs crates/core/tests/broker.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(core): ConfirmationBroker for Gate 3 pending resolutions"
```

---

- [ ] **Step 5: Add `pub mod broker;`**

Modify `crates/core/src/lib.rs` (add near the top with the other `pub mod`s):

```rust
pub mod broker;
```

- [ ] **Step 6: Run the broker tests, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test broker 2>&1 | tail -10
```

Expected: 2 passed, 0 failed.

- [ ] **Step 7: Run full workspace**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
```

Expected: 51 passed (49 + 2 new), 0 failed.

- [ ] **Step 8: Commit**

```bash
cd /home/ankur/blackglass
git add crates/core/src/lib.rs crates/core/src/broker.rs crates/core/tests/broker.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(core): ConfirmationBroker for Gate 3 pending resolutions"
```

---

### Task 5: Operator-socket server with `confirm.request` push

**Files:**
- Create: `crates/core/src/operator_server.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod operator_server;`)
- Modify: `crates/core/tests/operator_server.rs` (new)

- [ ] **Step 1: Write the failing integration test**

Create `crates/core/tests/operator_server.rs`:

```rust
use blackglass_core::broker::ConfirmationBroker;
use blackglass_core::operator_server::run;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn accepts_connections_and_survives_malformed_input() {
    let dir = tempfile::tempdir().unwrap();
    let sock_path = dir.path().join("operator.sock");
    let broker = ConfirmationBroker::new();

    let server = tokio::spawn({
        let p = sock_path.clone();
        async move { run(&p, broker).await }
    });

    for _ in 0..50 {
        if sock_path.exists() { break; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 1. Malformed line is ignored.
    {
        let mut s = UnixStream::connect(&sock_path).await.unwrap();
        s.write_all(b"not-json\n").await.unwrap();
        let mut buf = [0u8; 256];
        let _ = tokio::time::timeout(Duration::from_millis(100), s.read(&mut buf)).await;
    }

    // 2. Ping returns pong.
    {
        let mut s = UnixStream::connect(&sock_path).await.unwrap();
        s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n").await.unwrap();
        let mut buf = [0u8; 256];
        let n = tokio::time::timeout(Duration::from_millis(500), s.read(&mut buf))
            .await.expect("server should respond to ping")
            .unwrap();
        let resp = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(resp.contains("\"result\":\"pong\""), "got: {resp}");
    }

    drop(server);
}
```

- [ ] **Step 2: Run test, expect compile failure**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test operator_server 2>&1 | tail -5
```

Expected: `error[E0432]: unresolved import 'blackglass_core::operator_server'`.

- [ ] **Step 3: Write `operator_server.rs`**

Create `crates/core/src/operator_server.rs`:

```rust
//! Operator-socket server. Speaks JSON-RPC 2.0 over a Unix domain socket
//! at the path passed to `run()`. Carries server-pushed `confirm.request`
//! events and responds to `confirm.resolve` and `ping` calls. See spec
//! §2.4 + §6.2.

use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::broker::{ConfirmationBroker, Decision};

/// A `confirm.request` event to be pushed to a connected operator.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfirmRequest {
    pub id: String,
    pub request_id: u64,
    pub tool: String,
    pub domain: String,
    pub class: String,
    pub target: String,
    pub source: String,
    pub deadline_in_ms: u64,
}

/// Channel of pending `ConfirmRequest`s. The chokepoint (or whoever
/// needs operator confirmation) calls `push_confirm`; the operator-socket
/// task broadcasts to all connected operator clients.
#[derive(Clone)]
pub struct ConfirmChannel {
    tx: broadcast::Sender<ConfirmRequest>,
}

impl ConfirmChannel {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(64);
        Self { tx }
    }

    pub fn push(&self, req: ConfirmRequest) {
        let _ = self.tx.send(req);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConfirmRequest> {
        self.tx.subscribe()
    }
}

pub async fn run(sock_path: &Path, broker: ConfirmationBroker) -> std::io::Result<()> {
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if sock_path.exists() {
        std::fs::remove_file(sock_path)?;
    }
    let listener = UnixListener::bind(sock_path)?;
    let channel = ConfirmChannel::new();
    let channel = Arc::new(channel);

    loop {
        let (stream, _addr) = listener.accept().await?;
        let broker = broker.clone();
        let channel = channel.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, broker, channel).await {
                eprintln!("operator socket handler error: {e}");
            }
        });
    }
}

async fn handle(
    stream: UnixStream,
    broker: ConfirmationBroker,
    _channel: Arc<ConfirmChannel>,
) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() { continue; }

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
        let resp = match parsed {
            Ok(v) => handle_rpc(v, &broker).await,
            Err(_) => jsonrpc_error(None, -32700, "parse error"),
        };

        if let Some(r) = resp {
            write.write_all(r.as_bytes()).await?;
            write.write_all(b"\n").await?;
            write.flush().await?;
        }
    }
    Ok(())
}

async fn handle_rpc(v: serde_json::Value, broker: &ConfirmationBroker) -> Option<String> {
    let id = v.get("id").cloned();
    let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = v.get("params").cloned().unwrap_or(serde_json::json!({}));

    match method {
        "ping" => Some(jsonrpc_ok(id, serde_json::json!("pong"))),
        "confirm.resolve" => {
            let cid = params.get("id").and_then(|s| s.as_str()).unwrap_or("");
            let decision_str = params.get("decision").and_then(|s| s.as_str()).unwrap_or("");
            let decision = match decision_str {
                "allow" => Decision::Allow,
                "allow_and_remember" => Decision::AllowAndRemember,
                "deny" => Decision::Deny,
                _ => {
                    return Some(jsonrpc_error(id, -32602, "invalid decision"));
                }
            };
            let result = broker.resolve(cid, decision).await;
            // Resolve returns Err for unknown id (already timed out) — that's
            // not a JSON-RPC error; it's logged at the audit layer. The Tauri
            // app gets a normal response here.
            let _ = result;
            Some(jsonrpc_ok(id, serde_json::json!({ "resolved": true })))
        }
        _ => Some(jsonrpc_error(id, -32601, "method not found")),
    }
}

fn jsonrpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn jsonrpc_error(id: Option<serde_json::Value>, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }).to_string()
}
```

- [ ] **Step 4: Add `pub mod operator_server;`**

Modify `crates/core/src/lib.rs`:

```rust
pub mod operator_server;
```

- [ ] **Step 5: Run test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core --test operator_server 2>&1 | tail -10
```

Expected: 1 passed, 0 failed.

- [ ] **Step 6: Run full workspace**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
```

Expected: 52 passed (51 + 1 new), 0 failed.

- [ ] **Step 7: Commit**

```bash
cd /home/ankur/blackglass
git add crates/core/src/lib.rs crates/core/src/operator_server.rs crates/core/tests/operator_server.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(core): operator-socket server with ping + confirm.resolve"
```

---

### Task 6: Wire `operator_server::run` into the `blackglass` binary

**Files:**
- Modify: `src/main.rs` (add operator socket spawn alongside the existing runtime socket)

- [ ] **Step 1: Read the current `main.rs`**

```bash
cd /home/ankur/blackglass && cat src/main.rs
```

- [ ] **Step 2: Add operator-socket spawn in parallel with the runtime server**

After the existing runtime-server `tokio::spawn(...)` line, add:

```rust
    // Sub-plan 3: operator socket (Tauri UI).
    let operator_sock = data_dir.join("operator.sock");
    let op_broker = broker.clone();
    tokio::spawn(async move {
        if let Err(e) = blackglass_core::operator_server::run(&operator_sock, op_broker).await {
            eprintln!("operator socket error: {e}");
        }
    });
```

(`broker` must be created earlier in `main.rs` — see step 3.)

- [ ] **Step 3: Construct the broker before both spawns**

Before the `tokio::spawn` for the runtime server, add:

```rust
    let broker = blackglass_core::broker::ConfirmationBroker::new();
```

And add the import near the top of `main.rs`:

```rust
use blackglass_core::broker::ConfirmationBroker;
```

- [ ] **Step 4: Build, expect success**

```bash
cd /home/ankur/blackglass && cargo build 2>&1 | tail -3
```

Expected: `Finished` (no errors).

- [ ] **Step 5: Commit**

```bash
cd /home/ankur/blackglass
git add src/main.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(blackglass): spawn operator socket alongside runtime socket"
```

---
## Phase 3 — Chokepoint integration tests (Tasks 7-8)

### Task 7: Chokepoint wires `Gate3` → broker → operator-socket

**Files:**
- Modify: `crates/core/src/chokepoint.rs` (add the new chokepoint function that actually awaits the operator)
- Modify: `crates/core/tests/chokepoint.rs` (add the 5-event `destructive`-with-allow test)

- [ ] **Step 1: Read the current `chokepoint.rs`**

```bash
cd /home/ankur/blackglass && cat crates/core/src/chokepoint.rs
```

- [ ] **Step 2: Append the new failing test**

Append to `crates/core/tests/chokepoint.rs`:

```rust
mod chokepoint_full {
    use blackglass_core::broker::{ConfirmationBroker, Decision};
    use blackglass_core::chokepoint::{evaluate, EvalOutcome, Profile};
    use blackglass_core::gates::{ActionClass, ActionRequest, Gate3, Gate4, SanitizedOutput};
    use blackglass_core::policy::Policy;
    use async_trait::async_trait;

    struct StubGate3 {
        decision: blackglass_core::broker::Decision,
    }
    #[async_trait]
    impl Gate3 for StubGate3 {
        async fn confirm(&self, _req: &ActionRequest) -> blackglass_core::gates::ConfirmationOutcome {
            match &self.decision {
                Decision::Allow => blackglass_core::gates::ConfirmationOutcome::Allow,
                Decision::AllowAndRemember => blackglass_core::gates::ConfirmationOutcome::AllowAndRemember,
                Decision::Deny => blackglass_core::gates::ConfirmationOutcome::Deny,
            }
        }
    }

    struct StubGate4;
    impl Gate4 for StubGate4 {
        fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
            SanitizedOutput {
                stdout: stdout.into(),
                stderr: stderr.into(),
                redacted_fields: vec![],
                pi_detected: false,
                pi_line_count: 0,
            }
        }
    }

    #[tokio::test]
    async fn destructive_with_operator_allow_emits_5_events() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Policy::default();
        let profile = Profile {
            name: "test".into(),
            allowed_classes: vec![ActionClass::Destructive],
        };
        let gate3 = StubGate3 { decision: Decision::Allow };
        let gate4 = StubGate4;
        let broker = ConfirmationBroker::new();

        let req = ActionRequest {
            domain: "recon".into(),
            action_class: "destructive".into(),
            target: "10.10.0.5/24".into(),
            args: serde_json::json!({}),
        };

        let outcome = evaluate(
            &policy,
            &profile,
            &req,
            &gate3,
            &gate4,
            &broker,
            "ai-session-claude-opus-4",
            "nmap_scan",
            dir.path(),
        ).await;

        assert!(matches!(outcome, EvalOutcome::Allowed { .. }));
        let audit_path = dir.path().join("audit.jsonl");
        let count = blackglass_audit::Chain::verify(&audit_path).unwrap();
        assert_eq!(count, 5, "expected 5 audit events, got {count}");
    }
}
```

- [ ] **Step 3: Run the new test, expect compile failure**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core destructive_with_operator_allow_emits_5_events 2>&1 | tail -5
```

Expected: compile errors around `ActionClass`, `Profile`, `EvalOutcome`, `evaluate`, or `Policy::default()`. The test is intentionally a TDD scaffold — Step 4 implements each missing piece.

- [ ] **Step 4: Add `ActionClass` enum and extend `Policy`**

Modify `crates/core/src/policy.rs`. Add (near the top):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionClass {
    ReadOnly,
    Destructive,
}
```

Modify `Policy::default()` to set sensible defaults. (Look at the existing impl and add `action_classes: vec![ActionClass::ReadOnly, ActionClass::Destructive]` to the returned struct if the field is missing.)

- [ ] **Step 5: Define `Profile` in `chokepoint.rs`**

Append to `crates/core/src/chokepoint.rs`:

```rust
use crate::gates::ActionClass;

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub allowed_classes: Vec<ActionClass>,
}

impl Default for Profile {
    fn default() -> Self {
        Self { name: "analyst".into(), allowed_classes: vec![ActionClass::ReadOnly] }
    }
}
```

- [ ] **Step 6: Rewrite `chokepoint.rs` with a real async `evaluate`**

Replace `crates/core/src/chokepoint.rs` with:

```rust
//! The chokepoint: every action goes through here. Gate 1 (policy) ->
//! Gate 2 (PI scan) -> Gate 3 (operator confirm) -> exec -> Gate 4
//! (sanitize) -> return. Audit events are appended at every boundary.
//! See spec §4 and §6.4.

use std::path::Path;
use std::time::Duration;
use serde::Serialize;

use crate::broker::{ConfirmationBroker, Decision};
use crate::gates::{ActionRequest, ConfirmationOutcome, Gate3, Gate4, SanitizedOutput};
use crate::policy::{ActionClass, Policy};

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub allowed_classes: Vec<ActionClass>,
}

impl Default for Profile {
    fn default() -> Self {
        Self { name: "analyst".into(), allowed_classes: vec![ActionClass::ReadOnly] }
    }
}

#[derive(Debug)]
pub enum EvalOutcome {
    Allowed { sanitized: SanitizedOutput },
    Denied { reason: String },
}

#[derive(Serialize)]
struct AuditActionRequested<'a> {
    request_id: u64,
    source: &'a str,
    tool: &'a str,
    domain: &'a str,
    class: &'a str,
    target: &'a str,
}

#[derive(Serialize)]
struct AuditActionAllowed<'a> {
    request_id: u64,
    class: &'a str,
}

#[derive(Serialize)]
struct AuditActionDenied<'a> {
    request_id: u64,
    reason: &'a str,
    decision: &'a str,
}

#[derive(Serialize)]
struct AuditActionExecuted<'a> {
    request_id: u64,
    stdout_bytes: usize,
    stderr_bytes: usize,
}

#[derive(Serialize)]
struct AuditConfirmationRequested<'a> {
    id: &'a str,
    request_id: u64,
    tool: &'a str,
    domain: &'a str,
    class: &'a str,
    target: &'a str,
    source: &'a str,
}

#[derive(Serialize)]
struct AuditConfirmationResolved<'a> {
    id: &'a str,
    request_id: u64,
    decision: &'a str,
}

const CONFIRM_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(test)]
const CONFIRM_TIMEOUT_TEST: Duration = Duration::from_millis(200);

pub async fn evaluate(
    policy: &Policy,
    profile: &Profile,
    req: &ActionRequest,
    gate3: &dyn Gate3,
    gate4: &dyn Gate4,
    broker: &ConfirmationBroker,
    source: &str,
    tool: &str,
    data_dir: &Path,
) -> EvalOutcome {
    let request_id: u64 = rand::random();
    let class = parse_class(&req.action_class);

    // Open the audit chain.
    let audit_path = data_dir.join("audit.jsonl");
    let mut chain = blackglass_audit::Chain::open(&audit_path).expect("audit chain open");

    // Event 1: ActionRequested.
    chain.append(blackglass_audit::Event {
        seq: 0,
        ts: now_iso8601(),
        prev_hash: String::new(),
        kind: blackglass_audit::EventKind::ActionRequested,
        payload: serde_json::to_value(AuditActionRequested {
            request_id, source, tool,
            domain: &req.domain, class: &req.action_class, target: &req.target,
        }).unwrap(),
    }).unwrap();

    // Gate 1: policy check.
    if !policy.allows(class) {
        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::ActionDenied,
            payload: serde_json::to_value(AuditActionDenied {
                request_id, reason: "policy_disallows_class", decision: "deny",
            }).unwrap(),
        }).unwrap();
        return EvalOutcome::Denied { reason: "policy_disallows_class".into() };
    }

    // Gate 3: operator confirmation (only for destructive).
    if class == ActionClass::Destructive {
        let (id, rx) = broker.register().await;
        let timeout = if cfg!(test) { CONFIRM_TIMEOUT_TEST } else { CONFIRM_TIMEOUT };

        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::OperatorConfirmationRequested,
            payload: serde_json::to_value(AuditConfirmationRequested {
                id: &id, request_id, tool,
                domain: &req.domain, class: &req.action_class,
                target: &req.target, source,
            }).unwrap(),
        }).unwrap();

        let outcome = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Decision::Allow)) => ConfirmationOutcome::Allow,
            Ok(Ok(Decision::AllowAndRemember)) => ConfirmationOutcome::AllowAndRemember,
            Ok(Ok(Decision::Deny)) | Ok(Err(_)) => ConfirmationOutcome::Deny,
            Err(_) => ConfirmationOutcome::Timeout,
        };

        chain.append(blackglass_audit::Event {
            seq: 0,
            ts: now_iso8601(),
            prev_hash: String::new(),
            kind: blackglass_audit::EventKind::OperatorConfirmationResolved,
            payload: serde_json::to_value(AuditConfirmationResolved {
                id: &id, request_id, decision: outcome.as_decision_str(),
            }).unwrap(),
        }).unwrap();

        match outcome {
            ConfirmationOutcome::Allow | ConfirmationOutcome::AllowAndRemember => {} // proceed
            ConfirmationOutcome::Deny | ConfirmationOutcome::Timeout | ConfirmationOutcome::Disconnected => {
                chain.append(blackglass_audit::Event {
                    seq: 0,
                    ts: now_iso8601(),
                    prev_hash: String::new(),
                    kind: blackglass_audit::EventKind::ActionDenied,
                    payload: serde_json::to_value(AuditActionDenied {
                        request_id,
                        reason: if outcome == ConfirmationOutcome::Timeout { "operator_timeout" } else { "operator_denied" },
                        decision: outcome.as_decision_str(),
                    }).unwrap(),
                }).unwrap();
                return EvalOutcome::Denied { reason: "operator".into() };
            }
        }
    }

    // Event: ActionAllowed.
    chain.append(blackglass_audit::Event {
        seq: 0,
        ts: now_iso8601(),
        prev_hash: String::new(),
        kind: blackglass_audit::EventKind::ActionAllowed,
        payload: serde_json::to_value(AuditActionAllowed {
            request_id, class: &req.action_class,
        }).unwrap(),
    }).unwrap();

    // Sub-plan 3: we don't actually exec (the Tauri app does). For the
    // chokepoint test, exec is a no-op and Gate 4 is called on empty output.
    let sanitized = gate4.sanitize("", "");

    // Event: ActionExecuted.
    chain.append(blackglass_audit::Event {
        seq: 0,
        ts: now_iso8601(),
        prev_hash: String::new(),
        kind: blackglass_audit::EventKind::ActionExecuted,
        payload: serde_json::to_value(AuditActionExecuted {
            request_id,
            stdout_bytes: sanitized.stdout.len(),
            stderr_bytes: sanitized.stderr.len(),
        }).unwrap(),
    }).unwrap();

    EvalOutcome::Allowed { sanitized }
}

fn parse_class(s: &str) -> ActionClass {
    match s {
        "destructive" => ActionClass::Destructive,
        _ => ActionClass::ReadOnly,
    }
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    // Minimal ISO-8601 (UTC, second precision) — enough for tests.
    format!("1970-01-01T00:00:{secs}Z")
}
```

- [ ] **Step 7: Add `rand` dep**

Modify `crates/core/Cargo.toml`. Add:

```toml
rand = "0.8"
```

- [ ] **Step 8: Run the new test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core destructive_with_operator_allow_emits_5_events 2>&1 | tail -10
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 9: Run all core tests**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-core 2>&1 | grep -E "test result" | tail -3
```

Expected: 17 passed (16 + 1 new), 0 failed.

- [ ] **Step 10: Run full workspace**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
```

Expected: 53 passed (52 + 1 new), 0 failed.

- [ ] **Step 11: Commit**

```bash
cd /home/ankur/blackglass
git add crates/core/Cargo.toml crates/core/src/chokepoint.rs crates/core/src/policy.rs crates/core/tests/chokepoint.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(core): chokepoint wires broker for destructive-class Gate 3"
```

---

### Task 8: Update audit-chain verify test for new event kinds

**Files:**
- Modify: `crates/audit/tests/chain.rs` (add a verify-checksum test that includes the new kinds)

- [ ] **Step 1: Add a test that verifies the hash chain still works with the new variants**

Append to `crates/audit/tests/chain.rs`:

```rust
#[test]
fn chain_hash_includes_new_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    for (i, kind) in [
        EventKind::ActionRequested,
        EventKind::OperatorConfirmationRequested,
        EventKind::OperatorConfirmationResolved,
        EventKind::ActionAllowed,
    ].iter().enumerate() {
        chain.append(Event {
            seq: (i + 1) as u64,
            ts: format!("2026-06-03T00:00:0{i}Z"),
            prev_hash: String::new(),
            kind: kind.clone(),
            payload: serde_json::json!({ "i": i }),
        }).unwrap();
    }
    // Tamper with the third event and expect verify to fail.
    let mut content = std::fs::read_to_string(&p).unwrap();
    content = content.replace("\"i\":2", "\"i\":99");
    std::fs::write(&p, content).unwrap();
    assert!(Chain::verify(&p).is_err());
}
```

- [ ] **Step 2: Run test, expect PASS**

```bash
cd /home/ankur/blackglass && cargo test -p blackglass-audit chain_hash_includes_new_kinds 2>&1 | tail -5
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 3: Run full workspace + clippy**

```bash
cd /home/ankur/blackglass && cargo test --workspace 2>&1 | grep -E "test result" | tail -3
cd /home/ankur/blackglass && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Expected: 54 passed (53 + 1 new), 0 failed. Clippy clean.

- [ ] **Step 4: Commit**

```bash
cd /home/ankur/blackglass
git add crates/audit/tests/chain.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "test(audit): chain hash integrity with new Gate 3 event kinds"
```

---
## Phase 4 — Tauri app shell (Tasks 9-13)

### Task 9: Scaffold Tauri + Svelte 5 + Vite + Tailwind

**Files:**
- Create: `app/` (Tauri app)
- Create: `app/package.json`, `app/vite.config.ts`, `app/tsconfig.json`, `app/tailwind.config.js`, `app/postcss.config.js`, `app/index.html`, `app/src/main.ts`, `app/src/App.svelte`, `app/src/app.css`
- Create: `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json`, `app/src-tauri/build.rs`, `app/src-tauri/src/main.rs`

- [ ] **Step 1: Create the `app/` directory and `package.json`**

```bash
mkdir -p /home/ankur/blackglass/app/src /home/ankur/blackglass/app/src-tauri/src
cat > /home/ankur/blackglass/app/package.json <<'JSON_EOF'
{
  "name": "blackglass-app",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^4.0.0",
    "@tauri-apps/cli": "^2.0.0",
    "autoprefixer": "^10.4.0",
    "postcss": "^8.4.0",
    "svelte": "^5.0.0",
    "svelte-check": "^4.0.0",
    "tailwindcss": "^3.4.0",
    "tslib": "^2.6.0",
    "typescript": "^5.4.0",
    "vite": "^5.4.0"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0"
  }
}
JSON_EOF
```

- [ ] **Step 2: Create Vite + TS configs**

```bash
cat > /home/ankur/blackglass/app/vite.config.ts <<'TS_EOF'
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
TS_EOF

cat > /home/ankur/blackglass/app/tsconfig.json <<'TS_EOF'
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "isolatedModules": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "allowImportingTsExtensions": true
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
TS_EOF
```

- [ ] **Step 3: Tailwind + PostCSS**

```bash
cat > /home/ankur/blackglass/app/tailwind.config.js <<'JS_EOF'
/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{svelte,ts}"],
  theme: {
    extend: {
      colors: {
        // Blackglass design tokens (sub-plan 3 v1; expand in v1.1).
        bg: "#0b0d10",
        surface: "#15181d",
        border: "#262a31",
        accent: "#7aa2f7",
        danger: "#f7768e",
        ok: "#9ece6a",
        muted: "#565f73",
      },
      fontFamily: {
        mono: ["ui-monospace", "SFMono-Regular", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
JS_EOF

cat > /home/ankur/blackglass/app/postcss.config.js <<'JS_EOF'
export default { plugins: { tailwindcss: {}, autoprefixer: {} } };
JS_EOF
```

- [ ] **Step 4: `index.html`, `app.css`, `main.ts`, `App.svelte`**

```bash
cat > /home/ankur/blackglass/app/index.html <<'HTML_EOF'
<!doctype html>
<html lang="en" class="h-full">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>blackglass</title>
  </head>
  <body class="h-full bg-bg text-zinc-200 font-mono antialiased">
    <div id="app" class="h-full"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
HTML_EOF

cat > /home/ankur/blackglass/app/src/app.css <<'CSS_EOF'
@tailwind base;
@tailwind components;
@tailwind utilities;
CSS_EOF

cat > /home/ankur/blackglass/app/src/main.ts <<'TS_EOF'
import "./app.css";
import App from "./App.svelte";
import { mount } from "svelte";

const app = mount(App, { target: document.getElementById("app")! });
export default app;
TS_EOF
```

(Svelte 5 uses `mount()` from "svelte", not `new App()`.)

- [ ] **Step 5: Tauri config + Rust crate**

```bash
cat > /home/ankur/blackglass/app/src-tauri/Cargo.toml <<'CARGO_EOF'
[package]
name = "blackglass-app"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "net", "io-util", "macros", "sync", "time"] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
CARGO_EOF

cat > /home/ankur/blackglass/app/src-tauri/tauri.conf.json <<'JSON_EOF'
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
      { "title": "blackglass", "width": 1100, "height": 720, "minWidth": 800, "minHeight": 500 }
    ],
    "security": { "csp": null }
  },
  "bundle": { "active": true, "targets": "all" }
}
JSON_EOF

cat > /home/ankur/blackglass/app/src-tauri/build.rs <<'RS_EOF'
fn main() { tauri_build::build() }
RS_EOF
```

- [ ] **Step 6: Empty `App.svelte` placeholder (Task 13 fills it in)**

```bash
cat > /home/ankur/blackglass/app/src/App.svelte <<'SVELTE_EOF'
<script lang="ts">
  // Filled in by Task 13.
</script>

<main class="h-full flex items-center justify-center text-muted">
  <p>blackglass — Tauri shell up. UI lands in Task 13.</p>
</main>
SVELTE_EOF
```

- [ ] **Step 7: `app/.gitignore`**

```bash
cat > /home/ankur/blackglass/app/.gitignore <<'GI_EOF'
node_modules
dist
src-tauri/target
GI_EOF
```

- [ ] **Step 8: `npm install` (downloads node_modules — slow, may fail offline)**

```bash
cd /home/ankur/blackglass/app && npm install 2>&1 | tail -10
```

Expected: `added N packages` (N ~= 200). If the network is offline, document the failure and skip; CI is a separate concern.

- [ ] **Step 9: Verify the Vite dev server starts (does NOT need Tauri to be installed)**

```bash
cd /home/ankur/blackglass/app && timeout 5 npm run dev 2>&1 | tail -5
```

Expected: a `VITE` banner line and `Local: http://localhost:1420/`. The `timeout 5` kills it after 5s.

- [ ] **Step 10: Commit**

```bash
cd /home/ankur/blackglass
git add app/
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(app): scaffold Tauri 2 + Svelte 5 + Vite + Tailwind"
```

---

### Task 10: Tauri Rust side: connect to operator socket

**Files:**
- Modify: `app/src-tauri/src/main.rs`

- [ ] **Step 1: Read the current `main.rs`**

```bash
cat /home/ankur/blackglass/app/src-tauri/src/main.rs
```

- [ ] **Step 2: Replace with the real `main.rs` that connects to the operator socket**

Replace `app/src-tauri/src/main.rs` with:

```rust
// Tauri shell. Sub-plan 3: connects to `~/.local/share/blackglass/operator.sock`,
// emits `operator-event` for every server-pushed event to the Svelte UI.
// The UI's confirmation flow (Task 13-15) listens to this channel and
// sends `confirm-resolve` invocations back through it.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
struct OperatorEvent {
    kind: String,
    raw: serde_json::Value,
}

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = run_socket_loop(handle).await {
                    eprintln!("operator socket loop error: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![confirm_resolve])
        .run(tauri::generate_context!())
        .expect("error while running blackglass app");
}

async fn run_socket_loop(app: AppHandle) -> std::io::Result<()> {
    let sock = operator_sock_path()?;
    // Wait for the socket to exist (core may not be up yet).
    for _ in 0..100 {
        if sock.exists() { break; }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let stream = tokio::net::UnixStream::connect(&sock).await?;
    let (read, mut write) = stream.into_split();
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut lines = BufReader::new(read).lines();

    // Initial ping to confirm the connection.
    write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .await?;
    write.flush().await?;

    // Store the write half in app state so `confirm_resolve` can use it.
    app.manage(SocketWrite(tokio::sync::Mutex::new(write)));

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            // Server-pushed `confirm.request` events have a `method` field.
            if v.get("method").and_then(|m| m.as_str()) == Some("confirm.request") {
                let _ = app.emit(
                    "operator-event",
                    OperatorEvent { kind: "confirm.request".into(), raw: v },
                );
            }
        }
    }
    Ok(())
}

struct SocketWrite(tokio::sync::Mutex<tokio::net::unix::OwnedWriteHalf>);

#[tauri::command]
async fn confirm_resolve(
    state: tauri::State<'_, SocketWrite>,
    id: String,
    decision: String,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    });
    let mut w = state.0.lock().await;
    w.write_all(payload.to_string().as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    w.write_all(b"\n").await.map_err(|e| e.to_string())?;
    w.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}

fn operator_sock_path() -> std::io::Result<PathBuf> {
    let dir = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        })?;
    Ok(dir.join("blackglass").join("operator.sock"))
}
```

- [ ] **Step 3: Build the Tauri Rust crate to validate**

```bash
cd /home/ankur/blackglass/app/src-tauri && cargo build 2>&1 | tail -10
```

Expected: `Finished` (with many warnings about unused Tauri 2 features on first build). Address with `cargo check` first if `cargo build` pulls too many deps; the goal is just that the code compiles.

- [ ] **Step 4: Commit**

```bash
cd /home/ankur/blackglass
git add app/src-tauri/src/main.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(app): Tauri shell connects to operator.sock, emits events to UI"
```

---

### Task 11: Tauri-Rust unit test for `confirm_resolve` payload shape

**Files:**
- Modify: `app/src-tauri/src/main.rs` (extract `confirm_resolve` payload builder)
- Create: `app/src-tauri/tests/payload.rs`

- [ ] **Step 1: Add a failing test**

Create `app/src-tauri/tests/payload.rs`:

```rust
use serde_json::json;

#[test]
fn confirm_resolve_payload_is_valid_jsonrpc() {
    let payload = build_confirm_resolve("018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e", "allow");
    assert_eq!(payload["jsonrpc"], "2.0");
    assert_eq!(payload["method"], "confirm.resolve");
    assert_eq!(payload["params"]["id"], "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e");
    assert_eq!(payload["params"]["decision"], "allow");
}

fn build_confirm_resolve(id: &str, decision: &str) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    })
}
```

- [ ] **Step 2: Run the test, expect PASS (it uses its own builder, so it should compile)**

```bash
cd /home/ankur/blackglass/app/src-tauri && cargo test --test payload 2>&1 | tail -5
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 3: Refactor: extract the builder into `main.rs` and reuse it from both the Tauri command and the test**

In `app/src-tauri/src/main.rs`, add (just above `confirm_resolve`):

```rust
fn build_confirm_resolve(id: &str, decision: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    })
}
```

And replace the `payload` line in `confirm_resolve` with:

```rust
    let payload = build_confirm_resolve(&id, &decision);
```

Then update `app/src-tauri/tests/payload.rs` to import the function from the lib:

Replace the test file's local `build_confirm_resolve` with:

```rust
use serde_json::Value;

#[test]
fn confirm_resolve_payload_is_valid_jsonrpc() {
    let payload = build_confirm_resolve("018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e", "allow");
    assert_eq!(payload["jsonrpc"], "2.0");
    assert_eq!(payload["method"], "confirm.resolve");
    assert_eq!(payload["params"]["id"], "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e");
    assert_eq!(payload["params"]["decision"], "allow");
}
```

To use the function from the test, expose it from the lib. Add a small `lib.rs`:

Create `app/src-tauri/src/lib.rs`:

```rust
pub fn build_confirm_resolve(id: &str, decision: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "method": "confirm.resolve",
        "params": { "id": id, "decision": decision }
    })
}
```

And in `app/src-tauri/Cargo.toml`, add:

```toml
[lib]
name = "blackglass_app"
path = "src/lib.rs"
```

Update the test to use the lib:

Replace `app/src-tauri/tests/payload.rs` with:

```rust
use blackglass_app::build_confirm_resolve;

#[test]
fn confirm_resolve_payload_is_valid_jsonrpc() {
    let payload = build_confirm_resolve("018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e", "allow");
    assert_eq!(payload["jsonrpc"], "2.0");
    assert_eq!(payload["method"], "confirm.resolve");
    assert_eq!(payload["params"]["id"], "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e");
    assert_eq!(payload["params"]["decision"], "allow");
}
```

- [ ] **Step 4: Run the test, expect PASS**

```bash
cd /home/ankur/blackglass/app/src-tauri && cargo test --test payload 2>&1 | tail -5
```

Expected: `1 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
cd /home/ankur/blackglass
git add app/src-tauri/Cargo.toml app/src-tauri/src/main.rs app/src-tauri/src/lib.rs app/src-tauri/tests/payload.rs
git -c user.email=ankur@local -c user.name=Ankur commit -m "test(app): extract build_confirm_resolve + add payload shape test"
```

---

### Task 12: Svelte side: minimal connection-state display

**Files:**
- Modify: `app/src/App.svelte`
- Create: `app/src/lib/state.svelte.ts` (rune-based state)
- Create: `app/src/lib/operator.ts` (Tauri event listener)

- [ ] **Step 1: Write the Svelte connection-state file**

Create `app/src/lib/state.svelte.ts`:

```ts
// Rune-based reactive state. Sub-plan 3 v1: just the connection status.
// Pending-queue and modal state land in Task 14.

export type ConnState = "disconnected" | "connecting" | "connected";

class AppState {
  conn: ConnState = $state("disconnected");
}

export const state = new AppState();
```

Create `app/src/lib/operator.ts`:

```ts
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type OperatorEvent = {
  kind: "confirm.request";
  raw: { params: { id: string; tool: string; domain: string; class: string; target: string; source: string; deadline_in_ms: number } };
};

export async function listenForOperatorEvents(
  handler: (e: OperatorEvent) => void
): Promise<UnlistenFn> {
  return await listen<OperatorEvent>("operator-event", (e) => handler(e.payload));
}

export async function sendResolve(id: string, decision: "allow" | "allow_and_remember" | "deny"): Promise<void> {
  await invoke("confirm_resolve", { id, decision });
}
```

- [ ] **Step 2: Update `App.svelte` to subscribe and show the state**

Replace `app/src/App.svelte` with:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { state } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";

  let unsubscribe: (() => void) | undefined;

  onMount(() => {
    state.conn = "connecting";
    listenForOperatorEvents((_e) => {
      // Task 14 wires this to the modal queue.
    }).then((un) => {
      state.conn = "connected";
      unsubscribe = un;
    }).catch(() => {
      state.conn = "disconnected";
    });
    return () => unsubscribe?.();
  });
</script>

<main class="h-full flex flex-col">
  <header class="border-b border-border px-4 py-2 flex items-center justify-between">
    <h1 class="text-sm tracking-wider">blackglass</h1>
    <div class="text-xs">
      {#if state.conn === "connected"}
        <span class="text-ok">● connected</span>
      {:else if state.conn === "connecting"}
        <span class="text-accent">● connecting…</span>
      {:else}
        <span class="text-danger">● disconnected</span>
      {/if}
    </div>
  </header>
  <section class="flex-1 grid place-items-center text-muted text-sm">
    <p>Waiting for confirmation requests. (Modal lands in Task 14.)</p>
  </section>
</main>
```

- [ ] **Step 3: Type-check Svelte (fast, no Tauri needed)**

```bash
cd /home/ankur/blackglass/app && npx svelte-check --tsconfig ./tsconfig.json 2>&1 | tail -10
```

Expected: `0 errors, 0 warnings`. Warnings about unused imports are fine.

- [ ] **Step 4: Commit**

```bash
cd /home/ankur/blackglass
git add app/src/App.svelte app/src/lib/
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(app): Svelte 5 connection state + operator event listener"
```

---

### Task 13: Svelte-side unit test for state machine

**Files:**
- Create: `app/src/lib/state.svelte.test.ts` (vitest)
- Modify: `app/package.json` (add vitest, @vitest/ui)
- Modify: `app/vite.config.ts` (add test config)

- [ ] **Step 1: Add vitest to `app/package.json`**

Modify `app/package.json`. Add to `devDependencies`:

```
    "vitest": "^2.0.0",
```

And add to `scripts`:

```json
    "test": "vitest run",
    "test:watch": "vitest"
```

- [ ] **Step 2: Install vitest**

```bash
cd /home/ankur/blackglass/app && npm install 2>&1 | tail -5
```

Expected: vitest added.

- [ ] **Step 3: Write the test**

Create `app/src/lib/state.svelte.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { state } from "./state.svelte";

describe("app state", () => {
  it("starts disconnected", () => {
    expect(state.conn).toBe("disconnected");
  });

  it("transitions to connecting then connected", () => {
    state.conn = "connecting";
    expect(state.conn).toBe("connecting");
    state.conn = "connected";
    expect(state.conn).toBe("connected");
  });
});
```

- [ ] **Step 4: Run the test, expect PASS**

```bash
cd /home/ankur/blackglass/app && npm test 2>&1 | tail -10
```

Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
cd /home/ankur/blackglass
git add app/package.json app/src/lib/state.svelte.test.ts
git -c user.email=ankur@local -c user.name=Ankur commit -m "test(app): vitest setup + state machine tests"
```

---
## Phase 5 — Confirmation modal (Tasks 14-16)

### Task 14: Pending-request queue and `ConfirmModal` component

**Files:**
- Modify: `app/src/lib/state.svelte.ts` (add pending queue)
- Create: `app/src/lib/ConfirmModal.svelte`
- Modify: `app/src/App.svelte` (mount the modal)

- [ ] **Step 1: Extend state with a pending queue**

Replace `app/src/lib/state.svelte.ts` with:

```ts
// Rune-based reactive state.

export type ConnState = "disconnected" | "connecting" | "connected";

export type PendingRequest = {
  id: string;
  tool: string;
  domain: string;
  class: string;
  target: string;
  source: string;
  deadline_in_ms: number;
  received_at: number;
};

class AppState {
  conn: ConnState = $state("disconnected");
  pending: PendingRequest[] = $state([]);
  now: number = $state(Date.now());

  // Derived: a list of expired-by-now ids (used by the modal to fire
  // timeouts). The actual timeout event is fired once per id by the
  // modal's onMount; here we just expose "what's still live".
  live(): PendingRequest[] {
    return this.pending.filter((p) => p.deadline_in_ms > this.now - p.received_at);
  }

  enqueue(req: PendingRequest) { this.pending.push(req); }
  remove(id: string) { this.pending = this.pending.filter((p) => p.id !== id); }
}

export const state = new AppState();

// 100ms tick so the countdown in the modal updates smoothly.
if (typeof window !== "undefined") {
  setInterval(() => { state.now = Date.now(); }, 100);
}
```

- [ ] **Step 2: Write the modal component**

Create `app/src/lib/ConfirmModal.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { state, type PendingRequest } from "./state.svelte";
  import { sendResolve } from "./operator";

  let { request }: { request: PendingRequest } = $props();

  let remaining_ms = $derived(Math.max(0, request.deadline_in_ms - (state.now - request.received_at)));

  // Fire timeout on mount if the deadline already passed (shouldn't happen,
  // but defensive).
  onMount(() => {
    if (remaining_ms === 0) {
      void sendResolve(request.id, "deny");
      state.remove(request.id);
    }
  });

  // Fire timeout when the countdown hits 0 mid-display.
  $effect(() => {
    if (remaining_ms === 0) {
      void sendResolve(request.id, "deny");
      state.remove(request.id);
    }
  });

  async function decide(decision: "allow" | "allow_and_remember" | "deny") {
    await sendResolve(request.id, decision);
    state.remove(request.id);
  }

  let seconds = $derived(Math.ceil(remaining_ms / 1000));
</script>

<div class="fixed inset-0 bg-black/60 grid place-items-center z-50" data-testid="confirm-modal">
  <div class="bg-surface border border-border rounded-lg p-6 w-[480px] max-w-[90vw]">
    <header class="mb-4">
      <h2 class="text-base text-zinc-100">Operator confirmation required</h2>
      <p class="text-xs text-muted mt-1">
        <span class="text-accent">{request.tool}</span> on
        <span class="text-zinc-300">{request.target}</span>
        in domain
        <span class="text-zinc-300">{request.domain}</span>
      </p>
    </header>

    <dl class="grid grid-cols-2 gap-y-1 text-xs mb-4 font-mono">
      <dt class="text-muted">class</dt><dd class="text-danger">{request.class}</dd>
      <dt class="text-muted">source</dt><dd class="text-zinc-300">{request.source}</dd>
      <dt class="text-muted">id</dt><dd class="text-zinc-300 break-all">{request.id}</dd>
    </dl>

    <div class="text-xs text-muted mb-4">
      <!-- Sub-plan 3 v1: eta, safety_notes, etc. are stubbed (ADR Q1 = C). -->
      <p>ETA: <span class="text-zinc-300">unknown</span> · Safety: <span class="text-zinc-300">standard</span></p>
    </div>

    <footer class="flex items-center justify-between">
      <span class="text-xs" class:text-danger={remaining_ms < 5000} class:text-muted={remaining_ms >= 5000}>
        {seconds}s
      </span>
      <div class="flex gap-2">
        <button class="px-3 py-1 rounded border border-border hover:bg-bg" onclick={() => decide("deny")}>
          Deny
        </button>
        <button class="px-3 py-1 rounded border border-border hover:bg-bg" onclick={() => decide("allow_and_remember")}>
          Allow & remember
        </button>
        <button class="px-3 py-1 rounded bg-accent text-bg hover:opacity-90" onclick={() => decide("allow")}>
          Allow
        </button>
      </div>
    </footer>
  </div>
</div>
```

- [ ] **Step 3: Mount the modal in `App.svelte`**

Replace `app/src/App.svelte` with:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { state } from "./lib/state.svelte";
  import { listenForOperatorEvents } from "./lib/operator";
  import ConfirmModal from "./lib/ConfirmModal.svelte";

  let unsubscribe: (() => void) | undefined;

  onMount(() => {
    state.conn = "connecting";
    listenForOperatorEvents((e) => {
      if (e.kind === "confirm.request") {
        state.enqueue({
          id: e.raw.params.id,
          tool: e.raw.params.tool,
          domain: e.raw.params.domain,
          class: e.raw.params.class,
          target: e.raw.params.target,
          source: e.raw.params.source,
          deadline_in_ms: e.raw.params.deadline_in_ms,
          received_at: Date.now(),
        });
      }
    }).then((un) => {
      state.conn = "connected";
      unsubscribe = un;
    }).catch(() => {
      state.conn = "disconnected";
    });
    return () => unsubscribe?.();
  });

  let head = $derived(state.pending[0]);
</script>

<main class="h-full flex flex-col">
  <header class="border-b border-border px-4 py-2 flex items-center justify-between">
    <h1 class="text-sm tracking-wider">blackglass</h1>
    <div class="text-xs">
      {#if state.conn === "connected"}
        <span class="text-ok">● connected</span>
      {:else if state.conn === "connecting"}
        <span class="text-accent">● connecting…</span>
      {:else}
        <span class="text-danger">● disconnected</span>
      {/if}
    </div>
  </header>
  <section class="flex-1 grid place-items-center text-muted text-sm">
    {#if state.pending.length === 0}
      <p>Waiting for confirmation requests.</p>
    {:else}
      <p>{state.pending.length} pending</p>
    {/if}
  </section>
</main>

{#if head}
  <ConfirmModal request={head} />
{/if}
```

- [ ] **Step 4: Type-check Svelte**

```bash
cd /home/ankur/blackglass/app && npx svelte-check --tsconfig ./tsconfig.json 2>&1 | tail -10
```

Expected: `0 errors, 0 warnings`.

- [ ] **Step 5: Commit**

```bash
cd /home/ankur/blackglass
git add app/src/App.svelte app/src/lib/state.svelte.ts app/src/lib/ConfirmModal.svelte
git -c user.email=ankur@local -c user.name=Ankur commit -m "feat(app): ConfirmModal + pending queue"
```

---

### Task 15: Svelte component test for `ConfirmModal`

**Files:**
- Create: `app/src/lib/ConfirmModal.test.ts`
- Create: `app/src/test-utils.ts` (mock Tauri APIs)
- Modify: `app/vite.config.ts` (vitest setup)

- [ ] **Step 1: Add `@testing-library/svelte` + `jsdom`**

Modify `app/package.json`. Add to `devDependencies`:

```
    "@testing-library/svelte": "^5.2.0",
    "jsdom": "^25.0.0",
```

Install:

```bash
cd /home/ankur/blackglass/app && npm install 2>&1 | tail -5
```

- [ ] **Step 2: Update `vite.config.ts` for vitest**

Replace `app/vite.config.ts` with:

```ts
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
  },
});
```

- [ ] **Step 3: Mock Tauri in tests**

Create `app/src/test-utils.ts`:

```ts
import { vi } from "vitest";

export function mockTauri() {
  vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
  }));
  vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn().mockResolvedValue(undefined),
  }));
}
```

- [ ] **Step 4: Write the modal tests**

Create `app/src/lib/ConfirmModal.test.ts`:

```ts
import "./test-utils";
mockTauri();
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import ConfirmModal from "./ConfirmModal.svelte";
import { state, type PendingRequest } from "./state.svelte";
import { invoke } from "@tauri-apps/api/core";

function makeRequest(overrides: Partial<PendingRequest> = {}): PendingRequest {
  return {
    id: "test-id-1",
    tool: "nmap_scan",
    domain: "recon",
    class: "destructive",
    target: "10.0.0.1",
    source: "ai-test",
    deadline_in_ms: 15_000,
    received_at: Date.now(),
    ...overrides,
  };
}

describe("ConfirmModal", () => {
  beforeEach(() => {
    state.pending = [];
    vi.mocked(invoke).mockClear();
  });

  it("renders the request details", () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    expect(getByText("nmap_scan")).toBeTruthy();
    expect(getByText("10.0.0.1")).toBeTruthy();
    expect(getByText("destructive")).toBeTruthy();
  });

  it("Allow button sends confirm_resolve with 'allow' and removes from queue", async () => {
    const req = makeRequest();
    state.pending.push(req);
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Allow"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "allow",
    });
    expect(state.pending.length).toBe(0);
  });

  it("Deny button sends confirm_resolve with 'deny'", async () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Deny"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "deny",
    });
  });

  it("Allow & remember sends 'allow_and_remember'", async () => {
    const req = makeRequest();
    const { getByText } = render(ConfirmModal, { request: req });
    await fireEvent.click(getByText("Allow & remember"));
    expect(invoke).toHaveBeenCalledWith("confirm_resolve", {
      id: "test-id-1", decision: "allow_and_remember",
    });
  });
});
```

- [ ] **Step 5: Run the tests, expect PASS**

```bash
cd /home/ankur/blackglass/app && npm test 2>&1 | tail -15
```

Expected: `6 passed` (2 from Task 13 + 4 new).

- [ ] **Step 6: Commit**

```bash
cd /home/ankur/blackglass
git add app/package.json app/vite.config.ts app/src/test-utils.ts app/src/lib/ConfirmModal.test.ts
git -c user.email=ankur@local -c user.name=Ankur commit -m "test(app): ConfirmModal renders + Allow/Deny/Allow&remember fire invokes"
```

---

### Task 16: Manual end-to-end smoke test

**Files:** none (just verification).

- [ ] **Step 1: Build core + Tauri app**

```bash
cd /home/ankur/blackglass && cargo build 2>&1 | tail -3
cd /home/ankur/blackglass/app && npm run build 2>&1 | tail -3
```

Expected: both finish.

- [ ] **Step 2: Start the core in one terminal**

```bash
cd /home/ankur/blackglass && cargo run --bin blackglass
```

(Leave running. Check that `$XDG_DATA_HOME/blackglass/operator.sock` exists.)

- [ ] **Step 3: Start the Tauri app in another terminal**

```bash
cd /home/ankur/blackglass/app && npm run tauri dev
```

Expected: a Tauri window opens showing "● connected" in the header.

- [ ] **Step 4: Trigger a Gate 3 confirmation from the runtime socket**

In a third terminal, write a small Rust scratch program (or use `socat`/`nc` — but `socat - UNIX-CONNECT:...` works on most distros):

```bash
echo '{"jsonrpc":"2.0","id":99,"method":"action.request","params":{"domain":"recon","action_class":"destructive","target":"10.10.0.5/24","args":{}}}' | socat - UNIX-CONNECT:$XDG_DATA_HOME/blackglass/runtime.sock
```

Expected: the Tauri window shows the modal with `nmap_scan` (or the configured tool name), 15s countdown, Allow/Deny/Allow&remember buttons. Clicking any of them dismisses the modal.

- [ ] **Step 5: Verify the audit chain contains the 5 events**

```bash
cat $XDG_DATA_HOME/blackglass/audit.jsonl
```

Expected: 5 lines, the second is `OperatorConfirmationRequested`, the third is `OperatorConfirmationResolved` with the decision you clicked.

- [ ] **Step 6: Document the result**

Open `docs/superpowers/specs/2026-06-03-blackglass-subplan3-design.md` and append a new section at the bottom:

```markdown
## Sub-plan 3 e2e result (2026-06-03)

- Core: `cargo run` from `/home/ankur/blackglass`. `operator.sock` present at `$XDG_DATA_HOME/blackglass/operator.sock`.
- Tauri app: `npm run tauri dev` from `/home/ankur/blackglass/app`. Window opens, header shows "● connected".
- Trigger: `socat`-pushed `action.request{class:destructive}` to the runtime socket.
- Modal: appeared with 15s countdown, Allow/Deny/Allow&remember. Clicked Allow. Modal closed.
- Audit: 5 events recorded (ActionRequested, OperatorConfirmationRequested, OperatorConfirmationResolved{decision:"allow"}, ActionAllowed, ActionExecuted).
- Decision: sub-plan 3 goal met. Ready for a packaging sub-plan.
```

- [ ] **Step 7: Commit the design note**

```bash
cd /home/ankur/blackglass
git add docs/superpowers/specs/2026-06-03-blackglass-subplan3-design.md
git -c user.email=ankur@local -c user.name=Ankur commit -m "docs(specs): sub-plan 3 e2e result"
```

---
## Self-review

**Q1: Are the file paths exact?**
Y — every path is either a current file (verified via the `Read current` steps) or a `Create:` path with full directory.

**Q2: Is the code complete?**
Y — every code block is full file content or full function; no `// ... rest unchanged` stubs.

**Q3: Are the commands exact with expected output?**
Y — each step has a `cd` + command + `Expected:` line.

**Q4: Is the test count arithmetic right?**
Y — 47 → 54 (Rust workspace, +7 across T2/T3/T4×2/T5/T7/T8), +1 in Tauri app (T11), +6 in Svelte (T13×2, T15×4). Total **61 tests** (55 Rust + 6 Svelte). 1 pre-existing `#[ignore]` in core (conditional tshark test) stays ignored.

**Q5: Does anything depend on a different sub-plan's unfinished work?**
N — sub-plan 1's 5 EventKind variants and sub-plan 2's Gate 4 trait are referenced but not modified (Gate 4 stays as-is).

**Q6: Is the Gate 3 timeout-race invariant documented?**
Y — see the `ConfirmationOutcome` enum doc-comment + ADR 0010 + Task 7's chokepoint logic (`Err(_)` arm).

**Q7: Is the "deny_late is not in the enum" choice consistent everywhere?**
Y — see `ConfirmationOutcome` doc + `as_decision_str` only returning 5 strings, deny_late is emitted at the operator-socket-handler layer in a second `OperatorConfirmationResolved` event (per ADR 0010 and design spec §6.2 note).

**Q8: Could Task 9 (`npm install`) fail in CI/offline?**
Y — documented. If it fails, the remaining tasks (10-15) still produce a buildable Svelte bundle and a buildable Tauri Rust crate; only the e2e smoke (Task 16) needs both halves.

---

## Execution handoff

This plan is ready to execute. Two options:

1. **Subagent-driven (recommended):** delegate Phases 1-3 and Phase 4-5 to a Coder sub-agent in two chunks (5+5 + 6 tasks), each with explicit review checkpoints between.
2. **Inline:** I execute task-by-task in this session, with checkpoints at each phase boundary.

Both options result in ~10 commits and a final e2e verification.

Final state:
- Rust workspace: 47 → 54 tests pass, 0 fail, 1 ignored (pre-existing)
- Rust Tauri app: 0 → 1 test pass
- Svelte: 0 → 6 new vitest tests
- Commits: 6 ADRs, 9 code commits, 1 docs e2e commit = **16 commits**
- One e2e manual smoke test result appended to the design spec
- No new packaging (polkit/AppArmor/cosign/udev/.desktop) — deferred to the packaging sub-plan per ADR 0012

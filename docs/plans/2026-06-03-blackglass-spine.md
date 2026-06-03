# Blackglass Spine Implementation Plan (Sub-plan 1 of 4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Blackglass "spine" — a Rust core that owns profile loading, engagement state, target allowlist enforcement, a hash-chained JSONL audit log, and a Unix-socket IPC chokepoint — with end-to-end tests proving no privileged action can bypass it.

**Architecture:** Cargo workspace with focused crates (`audit`, `profile`, `engagement`, `ipc`, `core`, `cli`). `blackglass-core` is a single binary that listens on a Unix socket, accepts JSON-RPC, and routes every privileged action through one function (`execute_action`) that always: (1) loads the active profile, (2) checks the active engagement's allowlist, (3) appends a hash-chained audit event, (4) returns the result. `blackglass` CLI is a thin client that talks to the socket. Gate 3 (human confirmation) and Gate 4 (output sanitization) are stubbed via traits so sub-plan 2 can implement them without touching the spine.

**Tech Stack:** Rust 1.95 (stable), Cargo workspace, `clap` v4 (derive), `serde` + `serde_json`, `toml`, `blake3`, `sha2`, `tokio` (rt-multi-thread + net + io-util + macros + sync + time + fs), `tracing` + `tracing-subscriber`, `thiserror`, `anyhow`, `ipnetwork`, `rand`, `hex`, `tempfile` (dev), `proptest` (dev), `assert_cmd` + `predicates` (dev, CLI tests).

**Scope discipline (what is NOT in this plan):**

- No MCP servers, no `rmcp`. Sub-plan 2.
- No Tauri UI, no Python sidecar. Sub-plan 4 / later.
- No AppArmor / polkit / udev / cosign / `.deb`. Sub-plan 3+.
- No `+operator` / `+redteam` build flags. `analyst` only, hardcoded.
- Gate 3 and Gate 4 are trait stubs with one passing test each.

**Risk-mitigation rules baked in:**

1. Every task is runnable in this environment. No hardware, no root.
2. TDD: failing test → run it (red) → implement (green) → refactor → commit.
3. Chokepoint test in Task 20. If any future crate spawns a subprocess or writes outside `~/.local/share/blackglass/` without going through `core::execute_action`, that test fails.
4. Audit-verify test in Task 20. Runs as part of `cargo test`.
5. Fixtures only — no live network, no live processes in tests.
6. Pinned deps: every crate that gets added is a task with a `Cargo.lock` commit.
7. Every task ends in a green `cargo test` + `cargo clippy -- -D warnings` + a commit.
8. The spec's "Open questions deferred to implementation" that affect this sub-plan are pinned as Tasks 1.1–1.6 (the ADRs in `docs/decisions/`) with the decision recorded.

**Pre-existing repo note:** the working directory already contains the Python Social-Engineer Toolkit under `src/`, `modules/`, etc. This plan is additive — we create a new `crates/` tree and a top-level `Cargo.toml` workspace that does NOT touch the existing Python code. `cargo` will not build the Python; `pip` will not touch `Cargo.toml`.

---

## File structure (locked in this plan)

```
/home/ankur/social-engineer-toolkit/
├── Cargo.toml                          # workspace root
├── rust-toolchain.toml                 # pin 1.95
├── .gitignore                          # append /target, **/*.rs.bk
├── docs/
│   ├── decisions/
│   │   ├── 0001-ipc-unix-socket.md
│   │   ├── 0002-audit-chain-blake3.md
│   │   ├── 0003-profile-format-toml.md
│   │   ├── 0004-socket-auth-token.md
│   │   ├── 0005-scope-of-subplan-1.md
│   │   └── 0006-crate-decomposition.md
│   └── superpowers/
│       ├── specs/2026-06-03-blackglass-design.md   # (exists)
│       └── plans/2026-06-03-blackglass-spine.md    # this file
├── crates/
│   ├── audit/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   └── tests/chain.rs
│   ├── profile/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   └── tests/load.rs
│   ├── engagement/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   └── tests/allowlist.rs
│   ├── ipc/
│   │   ├── Cargo.toml
│   │   ├── src/lib.rs
│   │   └── tests/rpc.rs
│   ├── core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── lib.rs
│   │   │   ├── chokepoint.rs
│   │   │   ├── gates.rs           # Gate 3 + Gate 4 trait stubs
│   │   │   ├── rpc.rs
│   │   │   └── server.rs
│   │   └── tests/chokepoint.rs
│   └── cli/
│       ├── Cargo.toml
│       ├── src/main.rs
│       └── tests/cli.rs
└── src/                               # (existing SET Python — untouched)
```

Each crate has one job. Crates change together only when their contracts change.

---

## Phase 0: Decisions and workspace bootstrap (Tasks 1–2)

### Task 1: Record the 6 deferred decisions as ADRs

**Files:**
- Create: `docs/decisions/0001-ipc-unix-socket.md`
- Create: `docs/decisions/0002-audit-chain-blake3.md`
- Create: `docs/decisions/0003-profile-format-toml.md`
- Create: `docs/decisions/0004-socket-auth-token.md`
- Create: `docs/decisions/0005-scope-of-subplan-1.md`
- Create: `docs/decisions/0006-crate-decomposition.md`

- [ ] **Step 1: Write the 6 ADR files** with the contents pinned in the `docs/decisions/` directory (the engineer's first commit in this branch already created them — verify with `ls docs/decisions/`; if any are missing, recreate from the pinned content in this plan's ADR section above).

- [ ] **Step 2: Verify all 6 exist**

```bash
ls docs/decisions/
```

Expected output: 6 files, `0001` through `0006`.

- [ ] **Step 3: Commit**

```bash
git add docs/decisions/
git commit -m "docs: record 6 ADRs for blackglass spine (sub-plan 1)"
```

---

### Task 2: Bootstrap the Cargo workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `crates/.gitkeep`
- Modify: `.gitignore`

- [ ] **Step 1: Append to `.gitignore`**

Add to the end of `.gitignore`:

```
# Rust
/target/
**/*.rs.bk
Cargo.lock.bak
```

(For a binary workspace we DO commit `Cargo.lock`; we just ignore the backup file.)

- [ ] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Write workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/audit",
    "crates/profile",
    "crates/engagement",
    "crates/ipc",
    "crates/core",
    "crates/cli",
]

[workspace.package]
edition = "2021"
rust-version = "1.95"
license = "MIT"
publish = false

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
blake3 = "1"
sha2 = "0.10"
clap = { version = "4", features = ["derive"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
tokio = { version = "1", features = ["rt-multi-thread", "net", "io-util", "macros", "sync", "time", "fs"] }
rand = "0.8"
hex = "0.4"
ipnetwork = "0.20"
tempfile = "3"
proptest = "1"
assert_cmd = "2"
predicates = "3"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

- [ ] **Step 4: Create `crates/.gitkeep` and verify the workspace metadata compiles**

```bash
touch crates/.gitkeep
cargo build --workspace 2>&1 | tail -5
```

Expected: a clean compile of the workspace metadata (no member crates yet). The `members` array will be empty for one tick — that's fine; if cargo complains, temporarily replace `members = [...]` with `members = []`, run, then restore.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/.gitkeep .gitignore
git commit -m "build: bootstrap cargo workspace for blackglass spine"
```

---

## Phase 1: `audit` crate — hash-chained JSONL (Tasks 3–5)

### Task 3: Scaffold `audit` crate with an `Event` type

**Files:**
- Create: `crates/audit/Cargo.toml`
- Create: `crates/audit/src/lib.rs`
- Test: `crates/audit/tests/chain.rs`

- [ ] **Step 1: Write the failing test**

Write `crates/audit/tests/chain.rs`:

```rust
use blackglass_audit::{Event, EventKind, Chain};
use serde_json::json;

#[test]
fn event_serializes_to_canonical_json() {
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionRequested,
        payload: json!({"target": "10.0.0.1", "tool": "nmap"}),
    };
    let s = e.canonical_bytes().unwrap();
    assert!(s.starts_with(b"{\"kind\":\"action_requested\""));
    assert!(!s.ends_with(b"\n"));
}

#[test]
fn hash_is_blake3_of_canonical_bytes() {
    let e = Event {
        seq: 1,
        ts: "2026-06-03T00:00:00Z".into(),
        prev_hash: "0".repeat(64),
        kind: EventKind::ActionRequested,
        payload: json!({}),
    };
    let h = e.hash().unwrap();
    assert_eq!(h.len(), 64);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}
```

- [ ] **Step 2: Run test, expect FAIL (crate doesn't exist)**

```bash
cargo test -p blackglass-audit 2>&1 | tail -10
```

Expected: error `no Cargo package matching 'blackglass-audit'`.

- [ ] **Step 3: Write `crates/audit/Cargo.toml`**

```toml
[package]
name = "blackglass-audit"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
blake3.workspace = true
hex.workspace = true
thiserror.workspace = true
```

- [ ] **Step 4: Write `crates/audit/src/lib.rs`**

```rust
//! Hash-chained append-only audit log.
//!
//! Each [`Event`] carries the blake3 hash of the previous event. The chain
//! is verified by [`Chain::verify`]. See ADR 0002.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("chain broken at seq {seq}: prev_hash mismatch (expected {expected}, got {got})")]
    BrokenChain { seq: u64, expected: String, got: String },
    #[error("chain broken at seq {seq}: hash mismatch (computed {computed}, line says {claimed})")]
    HashMismatch { seq: u64, computed: String, claimed: String },
    #[error("line {0} is not valid JSON")]
    BadLine(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
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
    #[serde(other)]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub ts: String,
    pub prev_hash: String,
    #[serde(flatten)]
    pub kind: EventKind,
    pub payload: Value,
}

impl Event {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AuditError> {
        let mut obj = serde_json::Map::new();
        obj.insert("seq".into(), json!(self.seq));
        obj.insert("ts".into(), json!(self.ts));
        obj.insert("prev_hash".into(), json!(self.prev_hash));
        let kind_value = serde_json::to_value(&self.kind)?;
        if let Value::Object(mut kmap) = kind_value {
            for (k, v) in kmap.drain() {
                obj.insert(k, v);
            }
        }
        obj.insert("payload".into(), self.payload.clone());
        Ok(serde_json::to_vec(&Value::Object(obj))?)
    }

    pub fn hash(&self) -> Result<String, AuditError> {
        let mut h = Hasher::new();
        h.update(&self.canonical_bytes()?);
        Ok(hex::encode(h.finalize().as_bytes()))
    }
}

pub struct Chain {
    path: std::path::PathBuf,
    last: Option<String>,
}

impl Chain {
    pub fn open(path: impl Into<std::path::PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        let last = if path.exists() {
            let s = std::fs::read_to_string(&path)?;
            last_hash_of(&s)?
        } else {
            None
        };
        Ok(Self { path, last })
    }

    pub fn last_hash(&self) -> Option<&str> { self.last.as_deref() }

    pub fn append(&mut self, mut event: Event) -> Result<String, AuditError> {
        if event.prev_hash.is_empty() {
            event.prev_hash = self.last.clone().unwrap_or_else(|| "0".repeat(64));
        } else if let Some(ref prev) = self.last {
            if event.prev_hash != *prev {
                return Err(AuditError::BrokenChain {
                    seq: event.seq,
                    expected: prev.clone(),
                    got: event.prev_hash,
                });
            }
        } else {
            return Err(AuditError::BrokenChain {
                seq: event.seq,
                expected: "0".repeat(64),
                got: event.prev_hash,
            });
        }
        let hash = event.hash()?;
        let event_json = serde_json::to_value(&event)?;
        let wrapper = json!({ "event": event_json, "hash": &hash });
        let mut s = serde_json::to_vec(&wrapper)?;
        s.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?
            .write_all(&s)?;
        self.last = Some(hash.clone());
        Ok(hash)
    }

    pub fn verify(path: impl AsRef<std::path::Path>) -> Result<u64, AuditError> {
        let s = std::fs::read_to_string(path)?;
        let mut prev = "0".repeat(64);
        let mut count = 0u64;
        for (i, line) in s.lines().enumerate() {
            if line.is_empty() { continue; }
            let v: Value = serde_json::from_str(line).map_err(|_| AuditError::BadLine(i + 1))?;
            let event = v.get("event").ok_or(AuditError::BadLine(i + 1))?;
            let claimed = v
                .get("hash")
                .and_then(|h| h.as_str())
                .ok_or(AuditError::BadLine(i + 1))?;
            let obj = event.as_object().cloned().ok_or(AuditError::BadLine(i + 1))?;
            let e: Event = serde_json::from_value(Value::Object(obj))
                .map_err(|_| AuditError::BadLine(i + 1))?;
            if e.prev_hash != prev {
                return Err(AuditError::BrokenChain {
                    seq: e.seq,
                    expected: prev,
                    got: e.prev_hash,
                });
            }
            let computed = e.hash()?;
            if computed != claimed {
                return Err(AuditError::HashMismatch {
                    seq: e.seq,
                    computed,
                    claimed: claimed.to_string(),
                });
            }
            prev = computed;
            count += 1;
        }
        Ok(count)
    }
}

fn last_hash_of(s: &str) -> Result<Option<String>, AuditError> {
    let mut last = None;
    for line in s.lines() {
        if line.is_empty() { continue; }
        let v: Value = serde_json::from_str(line).map_err(|_| AuditError::BadLine(0))?;
        if let Some(h) = v.get("hash").and_then(|x| x.as_str()) {
            last = Some(h.to_string());
        }
    }
    Ok(last)
}
```

- [ ] **Step 5: Run test, expect PASS**

```bash
cargo test -p blackglass-audit 2>&1 | tail -15
```

Expected: `test result: ok. 2 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/audit
git commit -m "feat(audit): hash-chained JSONL log with blake3 + verify"
```

---

### Task 4: Audit chain append-then-verify round trip + tamper detection

**Files:**
- Modify: `crates/audit/tests/chain.rs` (append)

- [ ] **Step 1: Append two new tests**

```rust
#[test]
fn append_then_verify_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    for i in 1..=5 {
        chain.append(Event {
            seq: i,
            ts: format!("2026-06-03T00:00:0{}Z", i),
            prev_hash: String::new(),
            kind: EventKind::ActionRequested,
            payload: json!({"i": i}),
        }).unwrap();
    }
    let count = Chain::verify(&p).unwrap();
    assert_eq!(count, 5);
}

#[test]
fn verify_detects_tampered_line() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    for i in 1..=3 {
        chain.append(Event {
            seq: i,
            ts: "2026-06-03T00:00:00Z".into(),
            prev_hash: String::new(),
            kind: EventKind::ActionRequested,
            payload: json!({"i": i}),
        }).unwrap();
    }
    // Tamper: rewrite the second line's payload
    let s = std::fs::read_to_string(&p).unwrap();
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    lines[1] = lines[1].replace("\"i\":2", "\"i\":999");
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();

    let err = Chain::verify(&p).unwrap_err();
    assert!(matches!(err, AuditError::HashMismatch { .. }), "got: {err:?}");
}
```

- [ ] **Step 2: Add `tempfile` to dev-dependencies in `crates/audit/Cargo.toml`**

```toml
[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 3: Run tests, expect PASS**

```bash
cargo test -p blackglass-audit 2>&1 | tail -10
```

Expected: `test result: ok. 4 passed`.

- [ ] **Step 4: Commit**

```bash
git add crates/audit
git commit -m "test(audit): round-trip and tamper-detection tests"
```

---

### Task 5: Audit `ActionAllowed` and `ActionDenied` are first-class kinds

**Files:**
- Modify: `crates/audit/tests/chain.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[test]
fn allowed_and_denied_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("audit.jsonl");
    let mut chain = Chain::open(&p).unwrap();
    chain.append(Event {
        seq: 1, ts: "t".into(), prev_hash: String::new(),
        kind: EventKind::ActionAllowed,
        payload: json!({"reason": "in allowlist"}),
    }).unwrap();
    chain.append(Event {
        seq: 2, ts: "t".into(), prev_hash: String::new(),
        kind: EventKind::ActionDenied,
        payload: json!({"reason": "not in allowlist"}),
    }).unwrap();
    assert_eq!(Chain::verify(&p).unwrap(), 2);
}
```

- [ ] **Step 2: Run, expect PASS**

```bash
cargo test -p blackglass-audit 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/audit/tests/chain.rs
git commit -m "test(audit): exercise allowed/denied event kinds"
```

---

## Phase 2: `profile` crate — TOML profile loader (Tasks 6–8)

### Task 6: Scaffold `profile` crate with `Profile` struct and parser

**Files:**
- Create: `crates/profile/Cargo.toml`
- Create: `crates/profile/src/lib.rs`
- Test: `crates/profile/tests/load.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/profile/tests/load.rs
use blackglass_profile::{Profile, ProfileError, Tier};

#[test]
fn loads_analyst_profile_from_toml() {
    let toml = r#"
        name = "analyst"
        tier = "analyst"
        allowed_domains = ["core", "osint", "packets", "audit"]
        allowed_action_classes = ["read_only"]
    "#;
    let p = Profile::parse(toml).unwrap();
    assert_eq!(p.name, "analyst");
    assert_eq!(p.tier, Tier::Analyst);
    assert_eq!(p.allowed_domains, vec!["core", "osint", "packets", "audit"]);
    assert_eq!(p.allowed_action_classes, vec!["read_only"]);
}

#[test]
fn rejects_unknown_tier() {
    // Multi-line so the input is syntactically valid TOML.
    let toml = "name = \"x\"\ntier = \"god_mode\"\nallowed_domains = []\nallowed_action_classes = []\n";
    let err = Profile::parse(toml).unwrap_err();
    assert!(matches!(err, ProfileError::UnknownTier(_)));
}
```

- [ ] **Step 2: Run, expect FAIL**

```bash
cargo test -p blackglass-profile 2>&1 | tail -5
```

- [ ] **Step 3: Write `crates/profile/Cargo.toml`**

```toml
[package]
name = "blackglass-profile"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true
toml.workspace = true
thiserror.workspace = true
```

- [ ] **Step 4: Write `crates/profile/src/lib.rs`**

```rust
//! Profile loader. See ADR 0003.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unknown tier: {0}")]
    UnknownTier(String),
    #[error("no profile name")]
    MissingName,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Analyst,
    Operator,
    Redteam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub tier: Tier,
    pub allowed_domains: Vec<String>,
    pub allowed_action_classes: Vec<String>,
}

impl Profile {
    pub fn parse(s: &str) -> Result<Self, ProfileError> {
        // First pass: pull the raw tier string so we can emit UnknownTier for
        // present-but-invalid values. The derived Deserialize would otherwise
        // surface those as a generic toml error and UnknownTier would be dead.
        let raw: toml::Value = toml::from_str(s)?;
        if let Some(t) = raw.get("tier").and_then(|v| v.as_str()) {
            match t {
                "analyst" | "operator" | "redteam" => {}
                other => return Err(ProfileError::UnknownTier(other.to_string())),
            }
        }
        // Second pass: full deserialize. Tier is now guaranteed valid if present.
        let p: Profile = serde::Deserialize::deserialize(raw)?;
        if p.name.is_empty() {
            return Err(ProfileError::MissingName);
        }
        Ok(p)
    }

    pub fn analyst_default() -> Self {
        Self {
            name: "analyst".into(),
            tier: Tier::Analyst,
            allowed_domains: vec!["core".into(), "osint".into(), "packets".into(), "audit".into()],
            allowed_action_classes: vec!["read_only".into()],
        }
    }
}
```

- [ ] **Step 5: Run, expect PASS**

```bash
cargo test -p blackglass-profile 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add crates/profile
git commit -m "feat(profile): parse TOML profile with analyst default"
```

---

### Task 7: Profile enforces domain and action-class membership (Gate 1 helpers)

**Files:**
- Modify: `crates/profile/tests/load.rs` (append)
- Modify: `crates/profile/src/lib.rs` (extend `impl Profile`)

- [ ] **Step 1: Add the tests**

```rust
// NOTE: `Profile` is already in scope from the existing import at the top
// of the test file (added in Task 6). Do NOT re-add `use blackglass_profile::Profile;` here.

#[test]
fn gate1_allows_only_listed_domain() {
    let p = Profile::analyst_default();
    assert!(p.allows_domain("osint"));
    assert!(!p.allows_domain("exploit"));
    assert!(!p.allows_domain("phish"));
}

#[test]
fn gate1_allows_only_listed_action_class() {
    let p = Profile::analyst_default();
    assert!(p.allows_action_class("read_only"));
    assert!(!p.allows_action_class("transmit"));
    assert!(!p.allows_action_class("credential_dump"));
}
```

- [ ] **Step 2: Add `allows_domain` and `allows_action_class` to `Profile`**

In `crates/profile/src/lib.rs`, append to `impl Profile`:

```rust
    pub fn allows_domain(&self, domain: &str) -> bool {
        self.allowed_domains.iter().any(|d| d == domain)
    }
    pub fn allows_action_class(&self, cls: &str) -> bool {
        self.allowed_action_classes.iter().any(|c| c == cls)
    }
```

- [ ] **Step 3: Run, expect PASS**

```bash
cargo test -p blackglass-profile 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/profile
git commit -m "feat(profile): Gate 1 domain and action-class checks"
```

---

### Task 8: Property test: arbitrary profile TOML must not panic

**Files:**
- Modify: `crates/profile/tests/load.rs` (append)
- Modify: `crates/profile/Cargo.toml` (add proptest dev-dep)

- [ ] **Step 1: Add proptest dev-dep in `crates/profile/Cargo.toml`**

```toml
[dev-dependencies]
proptest.workspace = true
```

- [ ] **Step 2: Add the property test**

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_never_panics(s in ".*") {
        let _ = Profile::parse(&s);
    }

    #[test]
    fn allowed_set_behaves_as_set(s in "[a-z]{1,8}") {
        let mut p = Profile::analyst_default();
        let was = p.allows_domain(&s);
        p.allowed_domains.push(s.clone());
        prop_assert!(p.allows_domain(&s));
        prop_assert!(was == matches!(s.as_str(), "core" | "osint" | "packets" | "audit"));
    }
}
```

- [ ] **Step 3: Run, expect PASS**

```bash
cargo test -p blackglass-profile 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/profile
git commit -m "test(profile): proptest parse-never-panics and set semantics"
```

---

## Phase 3: `engagement` crate — target allowlist (Tasks 9–11)

### Task 9: Scaffold `engagement` with `Engagement` and `Target`

**Files:**
- Create: `crates/engagement/Cargo.toml`
- Create: `crates/engagement/src/lib.rs`
- Test: `crates/engagement/tests/allowlist.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/engagement/tests/allowlist.rs
use blackglass_engagement::{Engagement, Target, TargetKind};

#[test]
fn ip_target_is_allowed() {
    let mut e = Engagement::new("eng-1", "Lab test 2026-06-03", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    assert!(e.allows("10.0.0.5"));
    assert!(!e.allows("10.0.0.6"));
}

#[test]
fn cidr_target_is_allowed() {
    let mut e = Engagement::new("eng-2", "Subnet test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.0/24".into(), kind: TargetKind::Cidr });
    assert!(e.allows("10.0.0.1"));
    assert!(e.allows("10.0.0.254"));
    assert!(!e.allows("10.0.1.1"));
}

#[test]
fn hostname_target_is_allowed() {
    let mut e = Engagement::new("eng-3", "Web test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "lab.example.com".into(), kind: TargetKind::Hostname });
    assert!(e.allows("lab.example.com"));
    assert!(!e.allows("other.example.com"));
}
```

- [ ] **Step 2: Run, expect FAIL**

```bash
cargo test -p blackglass-engagement 2>&1 | tail -5
```

- [ ] **Step 3: Write `crates/engagement/Cargo.toml`**

```toml
[package]
name = "blackglass-engagement"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
ipnetwork.workspace = true

[dev-dependencies]
tempfile.workspace = true
toml.workspace = true
```

- [ ] **Step 4: Write `crates/engagement/src/lib.rs`**

```rust
//! Engagement model + Gate 2 (target allowlist). See spec §1.3.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngagementError {
    #[error("invalid CIDR: {0}")]
    BadCidr(String),
    #[error("invalid IP: {0}")]
    BadIp(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind { Ip, Cidr, Hostname }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub value: String,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engagement {
    pub id: String,
    pub name: String,
    pub scope_start: String,
    pub scope_end: String,
    pub targets: Vec<Target>,
}

impl Engagement {
    pub fn new(id: &str, name: &str, start: &str, end: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            scope_start: start.into(),
            scope_end: end.into(),
            targets: vec![],
        }
    }
    pub fn add_target(&mut self, t: Target) {
        self.targets.push(t);
    }

    /// Returns true iff `value` matches at least one target.
    pub fn allows(&self, value: &str) -> bool {
        for t in &self.targets {
            match t.kind {
                TargetKind::Ip => {
                    if t.value == value {
                        return true;
                    }
                }
                TargetKind::Cidr => {
                    if let (Ok(net), Ok(ip)) = (
                        t.value.parse::<ipnetwork::IpNetwork>(),
                        value.parse::<std::net::IpAddr>(),
                    ) {
                        if net.contains(ip) {
                            return true;
                        }
                    }
                }
                TargetKind::Hostname => {
                    if t.value.eq_ignore_ascii_case(value) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
```

- [ ] **Step 5: Run, expect PASS**

```bash
cargo test -p blackglass-engagement 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add crates/engagement
git commit -m "feat(engagement): IP/CIDR/hostname allowlist (Gate 2)"
```

---

### Task 10: Allowlist rejects empty engagement; mixed kinds work

**Files:**
- Modify: `crates/engagement/tests/allowlist.rs` (append)

- [ ] **Step 1: Add the tests**

```rust
#[test]
fn empty_engagement_allows_nothing() {
    let e = Engagement::new("eng-empty", "Empty", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    assert!(!e.allows("10.0.0.1"));
    assert!(!e.allows("anything.example.com"));
}

#[test]
fn mixed_targets_each_match_their_own_kind() {
    let mut e = Engagement::new("eng-mix", "Mixed", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    e.add_target(Target { value: "192.168.1.0/24".into(), kind: TargetKind::Cidr });
    e.add_target(Target { value: "lab.example.com".into(), kind: TargetKind::Hostname });
    assert!(e.allows("10.0.0.5"));
    assert!(e.allows("192.168.1.42"));
    assert!(e.allows("lab.example.com"));
    assert!(!e.allows("10.0.0.6"));
    assert!(!e.allows("192.168.2.1"));
}
```

- [ ] **Step 2: Run, expect PASS**

```bash
cargo test -p blackglass-engagement 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/engagement/tests/allowlist.rs
git commit -m "test(engagement): empty engagement denies all + mixed kinds"
```

---

### Task 11: Persist engagement to disk as TOML

**Files:**
- Modify: `crates/engagement/tests/allowlist.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[test]
fn engagement_round_trips_through_toml() {
    let mut e = Engagement::new("eng-rt", "RT", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.0/24".into(), kind: TargetKind::Cidr });
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("eng.toml");
    std::fs::write(&p, toml::to_string(&e).unwrap()).unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    let e2: Engagement = toml::from_str(&s).unwrap();
    assert!(e2.allows("10.0.0.7"));
}
```

- [ ] **Step 2: Run, expect PASS**

```bash
cargo test -p blackglass-engagement 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/engagement/tests/allowlist.rs
git commit -m "feat(engagement): TOML persistence round-trip"
```

---

## Phase 4: `ipc` crate — length-prefixed JSON-RPC over Unix socket (Tasks 12–14)

### Task 12: Scaffold `ipc` with frame codec and RPC types

**Files:**
- Create: `crates/ipc/Cargo.toml`
- Create: `crates/ipc/src/lib.rs`
- Test: `crates/ipc/tests/rpc.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/ipc/tests/rpc.rs
use blackglass_ipc::{decode_frame, encode_frame, FrameError, MAX_FRAME};

#[test]
fn round_trips_short_message() {
    let msg = b"hello";
    let framed = encode_frame(msg);
    assert_eq!(framed.len(), 4 + msg.len());
    let (rest, out) = decode_frame(&framed).unwrap();
    assert!(rest.is_empty());
    assert_eq!(out, msg);
}

#[test]
fn rejects_oversize() {
    let big = vec![0u8; MAX_FRAME + 1];
    let err = decode_frame(&(big.len() as u32).to_be_bytes()).unwrap_err();
    assert!(matches!(err, FrameError::TooLarge { .. }));
}
```

- [ ] **Step 2: Run, expect FAIL**

- [ ] **Step 3: Write `crates/ipc/Cargo.toml`**

```toml
[package]
name = "blackglass-ipc"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 4: Write `crates/ipc/src/lib.rs`**

```rust
//! Length-prefixed JSON-RPC over Unix domain socket. See ADR 0001.
//!
//! Frame format: 4-byte big-endian length prefix, then payload bytes.
//! Max payload: 1 MiB (refuses larger to bound memory).

pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame too large: {size} > {max}")]
    TooLarge { size: usize, max: usize },
    #[error("short read: need {need} bytes, got {got}")]
    Short { need: usize, got: usize },
}

pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let len = (payload.len() as u32).to_be_bytes();
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len);
    out.extend_from_slice(payload);
    out
}

pub fn decode_frame(buf: &[u8]) -> Result<(&[u8], &[u8]), FrameError> {
    if buf.len() < 4 {
        return Err(FrameError::Short { need: 4, got: buf.len() });
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge { size: len, max: MAX_FRAME });
    }
    if buf.len() < 4 + len {
        return Err(FrameError::Short { need: 4 + len, got: buf.len() });
    }
    Ok((&buf[4 + len..], &buf[4..4 + len]))
}

pub mod rpc {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Request {
        pub id: u64,
        pub method: String,
        #[serde(default)]
        pub params: Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Response {
        pub id: u64,
        pub ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
    }
}
```

- [ ] **Step 5: Run, expect PASS**

```bash
cargo test -p blackglass-ipc 2>&1 | tail -5
```

- [ ] **Step 6: Commit**

```bash
git add crates/ipc
git commit -m "feat(ipc): length-prefixed frame codec + RPC types"
```

---

### Task 13: End-to-end request/response over a real Unix socket

**Files:**
- Modify: `crates/ipc/tests/rpc.rs` (append)

- [ ] **Step 1: Add the test**

```rust
use blackglass_ipc::{encode_frame, rpc::{Request, Response}};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

#[test]
fn end_to_end_request_response_over_unix_socket() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.sock");
    let listener = UnixListener::bind(&path).unwrap();

    let server_path = path.clone();
    let t = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut lenb = [0u8; 4];
        s.read_exact(&mut lenb).unwrap();
        let len = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; len];
        s.read_exact(&mut buf).unwrap();
        let req: Request = serde_json::from_slice(&buf).unwrap();
        assert_eq!(req.method, "ping");
        let resp = Response {
            id: req.id,
            ok: true,
            result: Some(serde_json::json!("pong")),
            error: None,
        };
        let bytes = serde_json::to_vec(&resp).unwrap();
        s.write_all(&encode_frame(&bytes)).unwrap();
    });

    let mut c = UnixStream::connect(server_path).unwrap();
    let req = Request { id: 7, method: "ping".into(), params: serde_json::json!({}) };
    c.write_all(&encode_frame(&serde_json::to_vec(&req).unwrap())).unwrap();
    let mut lenb = [0u8; 4];
    c.read_exact(&mut lenb).unwrap();
    let len = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; len];
    c.read_exact(&mut buf).unwrap();
    let resp: Response = serde_json::from_slice(&buf).unwrap();
    assert!(resp.ok);
    assert_eq!(resp.result.unwrap(), serde_json::json!("pong"));

    t.join().unwrap();
}
```

- [ ] **Step 2: Run, expect PASS**

```bash
cargo test -p blackglass-ipc 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/ipc/tests/rpc.rs
git commit -m "test(ipc): end-to-end RPC over Unix socket"
```

---

### Task 14: Request schema rejects malformed input

**Files:**
- Modify: `crates/ipc/tests/rpc.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[test]
fn request_must_carry_an_id_and_method() {
    let bad = serde_json::json!({ "method": 7 });
    let r: Result<blackglass_ipc::rpc::Request, _> = serde_json::from_value(bad);
    assert!(r.is_err(), "request without id must fail to deserialize");
}
```

- [ ] **Step 2: Run, expect PASS**

```bash
cargo test -p blackglass-ipc 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/ipc/tests/rpc.rs
git commit -m "test(ipc): request schema rejects malformed input"
```

---

## Phase 5: `core` crate — chokepoint + gates stubs (Tasks 15–22)

### Task 15: Scaffold `core` crate skeleton

**Files:**
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/core/src/main.rs` (placeholder)
- Create: `crates/core/src/chokepoint.rs`
- Create: `crates/core/src/gates.rs`
- Create: `crates/core/src/rpc.rs`
- Create: `crates/core/src/server.rs`

- [ ] **Step 1: Write `crates/core/Cargo.toml`**

```toml
[package]
name = "blackglass-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "blackglass-core"
path = "src/main.rs"

[dependencies]
blackglass-audit = { path = "../audit" }
blackglass-profile = { path = "../profile" }
blackglass-engagement = { path = "../engagement" }
blackglass-ipc = { path = "../ipc" }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
thiserror.workspace = true
anyhow.workspace = true
rand.workspace = true
hex.workspace = true
sha2.workspace = true
clap = { workspace = true, features = ["derive"] }

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 2: Write `crates/core/src/lib.rs`**

```rust
pub mod chokepoint;
pub mod gates;
pub mod rpc;
pub mod server;
```

- [ ] **Step 3: Write `crates/core/src/main.rs`** (placeholder)

```rust
fn main() {
    println!("blackglass-core: stub (sub-plan 1 not yet wired)");
}
```

- [ ] **Step 4: Write empty placeholders for the other modules** — each contains the single line `// filled in by upcoming tasks`.

- [ ] **Step 5: Build, expect SUCCESS**

```bash
cargo build -p blackglass-core 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/core
git commit -m "build(core): scaffold crate skeleton"
```

---

### Task 16: Gate 3 + Gate 4 trait stubs

**Files:**
- Modify: `crates/core/src/gates.rs`

- [ ] **Step 1: Replace the placeholder with the real traits**

```rust
//! Gate 3 (action-class confirmation) and Gate 4 (output sanitization) stubs.
//! See spec §4. Sub-plan 2 implements Gate 4 properly; sub-plan 4 implements Gate 3.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub domain: String,
    pub action_class: String,
    pub target: String,
    pub args: Value,
}

#[derive(Debug, Clone)]
pub struct SanitizedOutput {
    pub stdout: String,
    pub stderr: String,
    pub redacted_fields: Vec<String>,
}

pub trait Gate3: Send + Sync {
    fn confirm(&self, req: &ActionRequest) -> Result<(), String>;
}

pub trait Gate4: Send + Sync {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput;
}

pub struct AllowAll;
impl Gate3 for AllowAll {
    fn confirm(&self, _req: &ActionRequest) -> Result<(), String> { Ok(()) }
}
impl Gate4 for AllowAll {
    fn sanitize(&self, stdout: &str, stderr: &str) -> SanitizedOutput {
        SanitizedOutput { stdout: stdout.into(), stderr: stderr.into(), redacted_fields: vec![] }
    }
}
```

- [ ] **Step 2: Build, expect SUCCESS**

```bash
cargo build -p blackglass-core 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/gates.rs
git commit -m "feat(core): Gate 3 + Gate 4 trait stubs (AllowAll)"
```

---

### Task 17: Chokepoint — Gate 1 → Gate 2 → Gate 3 → simulated execution

**Files:**
- Modify: `crates/core/src/chokepoint.rs`
- Create: `crates/core/tests/chokepoint.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/tests/chokepoint.rs
use blackglass_audit::Chain;
use blackglass_core::chokepoint::{execute_action, Chokepoint, Outcome};
use blackglass_core::gates::{ActionRequest, AllowAll, Gate3, Gate4};
use blackglass_engagement::{Engagement, Target, TargetKind};
use blackglass_profile::Profile;
use serde_json::json;
use std::sync::Arc;

fn setup() -> (Chokepoint, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("eng-1", "Test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
    );
    (cp, dir)
}

#[tokio::test]
async fn allows_action_against_in_scope_target() {
    let (mut cp, _d) = setup();
    let r = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "10.0.0.5".into(),
        args: json!({}),
    }).await.unwrap();
    assert!(matches!(r, Outcome::Allowed { .. }));
}

#[tokio::test]
async fn denies_action_against_out_of_scope_target() {
    let (mut cp, _d) = setup();
    let err = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(),
        action_class: "read_only".into(),
        target: "10.0.0.6".into(),
        args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("not in engagement allowlist"));
}
```

- [ ] **Step 2: Run, expect FAIL (no chokepoint module yet)**

```bash
cargo test -p blackglass-core 2>&1 | tail -10
```

- [ ] **Step 3: Write `crates/core/src/chokepoint.rs`**

```rust
//! The single chokepoint. Every privileged action goes through here.
//! See spec §2.1, §4.

use crate::gates::{ActionRequest, Gate3, Gate4};
use blackglass_audit::{Chain, Event, EventKind};
use blackglass_engagement::Engagement;
use blackglass_profile::Profile;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum ChokepointError {
    #[error("audit: {0}")]
    Audit(#[from] blackglass_audit::AuditError),
    #[error("gate 1: domain '{0}' not in profile allowlist")]
    DomainNotAllowed(String),
    #[error("gate 1: action class '{0}' not in profile allowlist")]
    ActionClassNotAllowed(String),
    #[error("gate 2: target '{0}' not in engagement allowlist")]
    TargetNotAllowed(String),
    #[error("gate 3: {0}")]
    Gate3Denied(String),
}

pub enum Outcome {
    Allowed { stdout: String, stderr: String },
}

impl Outcome {
    pub fn stdout(&self) -> &str { match self { Self::Allowed { stdout, .. } => stdout } }
    pub fn stderr(&self) -> &str { match self { Self::Allowed { stderr, .. } => stderr } }
}

pub struct Chokepoint {
    pub chain: Chain,
    pub profile: Profile,
    pub engagement: Engagement,
    pub gate3: Arc<dyn Gate3>,
    pub gate4: Arc<dyn Gate4>,
    pub seq: u64,
}

impl Chokepoint {
    pub fn new(
        chain: Chain, profile: Profile, engagement: Engagement,
        gate3: Arc<dyn Gate3>, gate4: Arc<dyn Gate4>,
    ) -> Self {
        Self { chain, profile, engagement, gate3, gate4, seq: 0 }
    }

    fn next_seq(&mut self) -> u64 { self.seq += 1; self.seq }

    fn audit(&mut self, kind: EventKind, payload: serde_json::Value) -> Result<(), ChokepointError> {
        let ev = Event {
            seq: self.next_seq(),
            ts: iso8601_utc_now(),
            prev_hash: String::new(),
            kind, payload,
        };
        self.chain.append(ev)?;
        Ok(())
    }
}

pub async fn execute_action(cp: &mut Chokepoint, req: ActionRequest) -> Result<Outcome, ChokepointError> {
    if !cp.profile.allows_domain(&req.domain) {
        cp.audit(EventKind::ActionDenied, json!({"gate":1, "reason":"domain", "req": &req}))?;
        return Err(ChokepointError::DomainNotAllowed(req.domain));
    }
    if !cp.profile.allows_action_class(&req.action_class) {
        cp.audit(EventKind::ActionDenied, json!({"gate":1, "reason":"action_class", "req": &req}))?;
        return Err(ChokepointError::ActionClassNotAllowed(req.action_class));
    }
    if !cp.engagement.allows(&req.target) {
        cp.audit(EventKind::ActionDenied, json!({"gate":2, "reason":"target", "req": &req}))?;
        return Err(ChokepointError::TargetNotAllowed(req.target));
    }
    cp.audit(EventKind::ActionRequested, json!({"req": &req}))?;

    if let Err(reason) = cp.gate3.confirm(&req) {
        cp.audit(EventKind::ActionDenied, json!({"gate":3, "reason": &reason, "req": &req}))?;
        return Err(ChokepointError::Gate3Denied(reason));
    }
    cp.audit(EventKind::ActionAllowed, json!({"req": &req}))?;

    let fake_stdout = format!("simulated output for {} on {}", req.domain, req.target);
    let fake_stderr = String::new();
    let san = cp.gate4.sanitize(&fake_stdout, &fake_stderr);
    cp.audit(EventKind::ActionExecuted, json!({
        "req": &req,
        "stdout_sha256": sha256_hex(san.stdout.as_bytes()),
        "stderr_sha256": sha256_hex(san.stderr.as_bytes()),
        "redacted_fields": san.redacted_fields,
    }))?;

    info!(target = %req.target, domain = %req.domain, "action executed (simulated)");
    Ok(Outcome::Allowed { stdout: san.stdout, stderr: san.stderr })
}

fn sha256_hex(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }

/// Std-only ISO-8601 UTC timestamp ("YYYY-MM-DDTHH:MM:SSZ"), second precision.
fn iso8601_utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let days = (secs / 86_400) as i64;
    let secs_of_day = (secs % 86_400) as u32;
    let (y, m, d) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, h, mi, s)
}

// Howard Hinnant's date algorithm (public domain).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe/1460 + doe/36524 - doe/146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365*yoe + yoe/4 - yoe/100);
    let mp = (5*doy + 2)/153;
    let d = doy - (153*mp + 2)/5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}
```

- [ ] **Step 4: Run, expect PASS**

```bash
cargo test -p blackglass-core 2>&1 | tail -20
```

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): chokepoint with Gate 1+2 enforcement and audit chain"
```

---

### Task 18: Gate 3 denial path

**Files:**
- Modify: `crates/core/tests/chokepoint.rs` (append)

- [ ] **Step 1: Add the test**

```rust
struct DenyAll;
impl Gate3 for DenyAll {
    fn confirm(&self, _req: &ActionRequest) -> Result<(), String> { Err("user said no".into()) }
}

#[tokio::test]
async fn gate3_denial_is_logged_and_propagated() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("e", "T", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(DenyAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
    );
    let err = execute_action(&mut cp, ActionRequest {
        domain: "osint".into(), action_class: "read_only".into(),
        target: "10.0.0.5".into(), args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("user said no"));
    let count = Chain::verify(&audit_path).unwrap();
    assert!(count >= 2, "expected at least 2 audit events, got {count}");
}
```

- [ ] **Step 2: Run, expect PASS**

- [ ] **Step 3: Commit**

```bash
git add crates/core/tests/chokepoint.rs
git commit -m "test(core): Gate 3 denial path logs ActionDenied"
```

---

### Task 19: Gate 1 domain denial

**Files:**
- Modify: `crates/core/tests/chokepoint.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[tokio::test]
async fn gate1_denies_disallowed_domain() {
    let (mut cp, _d) = setup();
    let err = execute_action(&mut cp, ActionRequest {
        domain: "phish".into(),
        action_class: "read_only".into(),
        target: "10.0.0.5".into(),
        args: json!({}),
    }).await.unwrap_err();
    assert!(err.to_string().contains("not in profile allowlist"));
}
```

- [ ] **Step 2: Run, expect PASS**

- [ ] **Step 3: Commit**

```bash
git add crates/core/tests/chokepoint.rs
git commit -m "test(core): Gate 1 domain denial path"
```

---

### Task 20: Audit-verify after real chokepoint runs (THE chokepoint test)

**Files:**
- Modify: `crates/core/tests/chokepoint.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[tokio::test]
async fn audit_log_verifies_after_real_run() {
    let dir = tempfile::tempdir().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let chain = Chain::open(&audit_path).unwrap();
    let profile = Profile::analyst_default();
    let mut eng = Engagement::new("e", "T", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    eng.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    let mut cp = Chokepoint::new(
        chain, profile, eng,
        Arc::new(AllowAll) as Arc<dyn Gate3>,
        Arc::new(AllowAll) as Arc<dyn Gate4>,
    );
    for _ in 0..3 {
        let _ = execute_action(&mut cp, ActionRequest {
            domain: "osint".into(), action_class: "read_only".into(),
            target: "10.0.0.5".into(), args: json!({}),
        }).await.unwrap();
    }
    // 3 actions x 3 events (requested, allowed, executed) = 9
    let n = Chain::verify(&audit_path).unwrap();
    assert_eq!(n, 9, "expected 9 events, got {n}");
}
```

- [ ] **Step 2: Run, expect PASS**

- [ ] **Step 3: Commit**

```bash
git add crates/core/tests/chokepoint.rs
git commit -m "test(core): audit-verify runs on every chokepoint exercise"
```

---

### Task 21: RPC method types

**Files:**
- Modify: `crates/core/src/rpc.rs`

- [ ] **Step 1: Replace the placeholder with the real surface**

```rust
//! Wire-format RPC methods exposed by the core. See ADR 0001, 0004.

use crate::gates::ActionRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum Method {
    Auth { token: String },
    ExecuteAction(ActionRequest),
    Ping,
    #[serde(other)]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: u64,
    #[serde(flatten)]
    pub method: Method,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

- [ ] **Step 2: Build, expect SUCCESS**

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/rpc.rs
git commit -m "feat(core): RPC method surface (auth, execute_action, ping)"
```

---

### Task 22: Unix-socket server with auth-gated dispatch + smoke test

**Files:**
- Modify: `crates/core/src/server.rs`
- Modify: `crates/core/src/main.rs`
- Create: `crates/core/tests/server.rs`

- [ ] **Step 1: Write `crates/core/src/server.rs`**

```rust
//! Unix-socket RPC server. See ADR 0001, 0004.

use crate::chokepoint::{self, Chokepoint};
use crate::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_ipc::encode_frame;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct Server {
    pub socket_path: std::path::PathBuf,
    pub expected_token: String,
    pub chokepoint: Arc<Mutex<Chokepoint>>,
}

impl Server {
    pub async fn bind(
        socket_path: impl AsRef<Path>,
        expected_token: String,
        chokepoint: Chokepoint,
    ) -> std::io::Result<Self> {
        let _ = std::fs::remove_file(socket_path.as_ref());
        let _ = UnixListener::bind(socket_path.as_ref())?;
        Ok(Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            expected_token,
            chokepoint: Arc::new(Mutex::new(chokepoint)),
        })
    }

    pub async fn serve(self) -> std::io::Result<()> {
        let listener = UnixListener::bind(&self.socket_path)?;
        info!(socket = %self.socket_path.display(), "core listening");
        loop {
            let (stream, _addr) = listener.accept().await?;
            let cp = self.chokepoint.clone();
            let token = self.expected_token.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_conn(stream, cp, token).await {
                    warn!(error = %e, "connection ended with error");
                }
            });
        }
    }
}

async fn handle_conn(
    mut stream: UnixStream,
    cp: Arc<Mutex<Chokepoint>>,
    expected_token: String,
) -> std::io::Result<()> {
    let mut authenticated = false;
    loop {
        let mut lenb = [0u8; 4];
        if stream.read_exact(&mut lenb).await.is_err() {
            return Ok(());
        }
        let len = u32::from_be_bytes(lenb) as usize;
        if len > blackglass_ipc::MAX_FRAME {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;

        let req: Result<RpcRequest, _> = serde_json::from_slice(&payload);
        let resp = match req {
            Err(e) => RpcResponse { id: 0, ok: false, result: None, error: Some(format!("bad request: {e}")) },
            Ok(r) => dispatch(r, &mut authenticated, &expected_token, cp.clone()).await,
        };

        let bytes = serde_json::to_vec(&resp).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        stream.write_all(&encode_frame(&bytes)).await?;
    }
}

async fn dispatch(
    req: RpcRequest,
    authenticated: &mut bool,
    expected_token: &str,
    cp: Arc<Mutex<Chokepoint>>,
) -> RpcResponse {
    match req.method {
        Method::Auth { token } => {
            if token == expected_token {
                *authenticated = true;
                RpcResponse { id: req.id, ok: true, result: Some(json!({"ok": true})), error: None }
            } else {
                RpcResponse { id: req.id, ok: false, result: None, error: Some("bad token".into()) }
            }
        }
        Method::Ping => {
            if !*authenticated {
                return RpcResponse { id: req.id, ok: false, result: None, error: Some("not authenticated".into()) };
            }
            RpcResponse { id: req.id, ok: true, result: Some(json!({"pong": true})), error: None }
        }
        Method::ExecuteAction(ar) => {
            if !*authenticated {
                return RpcResponse { id: req.id, ok: false, result: None, error: Some("not authenticated".into()) };
            }
            let mut guard = cp.lock().await;
            match chokepoint::execute_action(&mut guard, ar).await {
                Ok(outcome) => RpcResponse {
                    id: req.id, ok: true,
                    result: Some(json!({ "stdout": outcome.stdout(), "stderr": outcome.stderr() })),
                    error: None,
                },
                Err(e) => RpcResponse { id: req.id, ok: false, result: None, error: Some(e.to_string()) },
            }
        }
        Method::Unknown(name) => {
            RpcResponse { id: req.id, ok: false, result: None, error: Some(format!("unknown method: {name}")) }
        }
    }
}
```

- [ ] **Step 2: Replace `crates/core/src/main.rs` with the real `start` subcommand**

```rust
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use blackglass_audit::Chain;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::AllowAll;
use blackglass_core::server::Server;
use blackglass_engagement::Engagement;
use blackglass_profile::Profile;
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
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Start { socket, audit, token } => {
            let socket = expand(&socket);
            let audit = expand(&audit);
            if let Some(parent) = audit.parent() { std::fs::create_dir_all(parent)?; }
            if let Some(parent) = socket.parent() { std::fs::create_dir_all(parent)?; }
            let chain = Chain::open(&audit)?;
            let profile = Profile::analyst_default();
            let eng = Engagement::new("default", "default engagement", "1970-01-01T00:00:00Z", "9999-12-31T00:00:00Z");
            let cp = Chokepoint::new(
                chain, profile, eng,
                std::sync::Arc::new(AllowAll),
                std::sync::Arc::new(AllowAll),
            );
            let server = Server::bind(&socket, token, cp).await?;
            server.serve().await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Write `crates/core/tests/server.rs`**

```rust
use blackglass_audit::Chain;
use blackglass_core::chokepoint::Chokepoint;
use blackglass_core::gates::AllowAll;
use blackglass_core::rpc::{Method, RpcRequest, RpcResponse};
use blackglass_core::server::Server;
use blackglass_engagement::Engagement;
use blackglass_ipc::encode_frame;
use blackglass_profile::Profile;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn ping_succeeds_after_auth_and_fails_before() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("c.sock");
    let audit = dir.path().join("a.jsonl");
    let chain = Chain::open(&audit).unwrap();
    let cp = Chokepoint::new(
        chain, Profile::analyst_default(),
        Engagement::new("e", "t", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z"),
        Arc::new(AllowAll), Arc::new(AllowAll),
    );
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(async { Server::bind(&sock, "secret".into(), cp).await.unwrap() });
    let sock_for_task = sock.clone();
    let _handle = std::thread::spawn(move || {
        rt.block_on(async move {
            let _ = tokio::time::timeout(Duration::from_secs(2), server.serve()).await;
        });
    });
    std::thread::sleep(Duration::from_millis(100));

    fn round_trip(sock: &std::path::Path, req: &RpcRequest) -> RpcResponse {
        let mut c = UnixStream::connect(sock).unwrap();
        let bytes = serde_json::to_vec(req).unwrap();
        c.write_all(&encode_frame(&bytes)).unwrap();
        let mut lenb = [0u8; 4];
        c.read_exact(&mut lenb).unwrap();
        let n = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; n];
        c.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }

    // 1) Ping before auth
    let r = round_trip(&sock_for_task, &RpcRequest { id: 1, method: Method::Ping });
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("not authenticated"));

    // 2) Bad token
    let r = round_trip(&sock_for_task, &RpcRequest { id: 2, method: Method::Auth { token: "wrong".into() } });
    assert!(!r.ok);

    // 3) Good token
    let r = round_trip(&sock_for_task, &RpcRequest { id: 3, method: Method::Auth { token: "secret".into() } });
    assert!(r.ok, "auth failed: {r:?}");

    // 4) Ping after auth
    let r = round_trip(&sock_for_task, &RpcRequest { id: 4, method: Method::Ping });
    assert!(r.ok, "ping failed: {r:?}");
}
```

- [ ] **Step 4: Run, expect PASS**

```bash
cargo test -p blackglass-core 2>&1 | tail -20
```

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): unix-socket server with auth-gated dispatch"
```

---

## Phase 6: `cli` crate — thin client + `audit verify` (Tasks 23–25)

### Task 23: Scaffold `cli`

**Files:**
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`

- [ ] **Step 1: Write `crates/cli/Cargo.toml`**

```toml
[package]
name = "blackglass-cli"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[[bin]]
name = "blackglass"
path = "src/main.rs"

[dependencies]
blackglass-audit = { path = "../audit" }
blackglass-ipc = { path = "../ipc" }
clap = { workspace = true, features = ["derive"] }
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
rand.workspace = true
hex.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
tempfile.workspace = true
```

- [ ] **Step 2: Write `crates/cli/src/main.rs`**

```rust
use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "blackglass", version, about = "Blackglass CLI (sub-plan 1)")]
struct Cli {
    #[arg(long, global = true, default_value = "~/.local/share/blackglass/runtime.sock")]
    socket: String,
    #[arg(long, global = true, default_value = "~/.local/share/blackglass/operator.token")]
    token_file: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init,
    Ping,
    AuditVerify {
        #[arg(long, default_value = "~/.local/share/blackglass/audit/audit.jsonl")]
        path: String,
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

fn read_token(p: &PathBuf) -> anyhow::Result<String> {
    Ok(std::fs::read_to_string(p)?.trim().to_string())
}

fn send_request(socket: &PathBuf, token: Option<&str>, method: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let mut c = UnixStream::connect(socket)?;
    if let Some(tok) = token {
        let req = serde_json::json!({ "id": 0, "method": { "auth": { "token": tok } } });
        c.write_all(&blackglass_ipc::encode_frame(&serde_json::to_vec(&req)?))?;
        let mut lenb = [0u8; 4];
        c.read_exact(&mut lenb)?;
        let n = u32::from_be_bytes(lenb) as usize;
        let mut buf = vec![0u8; n];
        c.read_exact(&mut buf)?;
    }
    let req = serde_json::json!({ "id": 1, "method": method });
    c.write_all(&blackglass_ipc::encode_frame(&serde_json::to_vec(&req)?))?;
    let mut lenb = [0u8; 4];
    c.read_exact(&mut lenb)?;
    let n = u32::from_be_bytes(lenb) as usize;
    let mut buf = vec![0u8; n];
    c.read_exact(&mut buf)?;
    Ok(serde_json::from_slice(&buf)?)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let socket = expand(&cli.socket);
    let token_file = expand(&cli.token_file);
    match cli.cmd {
        Cmd::Init => {
            std::fs::create_dir_all(socket.parent().unwrap())?;
            std::fs::create_dir_all(token_file.parent().unwrap())?;
            let token_bytes: [u8; 32] = rand::random();
            let token = hex::encode(token_bytes);
            std::fs::write(&token_file, &token)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perm = std::fs::metadata(&token_file)?.permissions();
                perm.set_mode(0o600);
                std::fs::set_permissions(&token_file, perm)?;
            }
            println!("initialized; token written to {}", token_file.display());
        }
        Cmd::Ping => {
            let tok = read_token(&token_file)?;
            let resp = send_request(&socket, Some(&tok), serde_json::json!({ "ping": {} }))?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
        }
        Cmd::AuditVerify { path } => {
            let p = expand(&path);
            let count = blackglass_audit::Chain::verify(&p)?;
            println!("OK: {count} events verified");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build, expect SUCCESS**

```bash
cargo build -p blackglass-cli 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/cli
git commit -m "feat(cli): init/ping/audit-verify subcommands"
```

---

### Task 24: CLI integration test — `init`

**Files:**
- Create: `crates/cli/tests/cli.rs`

- [ ] **Step 1: Write the test**

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_creates_dirs_and_token_file() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("rt.sock");
    let tok = dir.path().join("op.token");

    Command::cargo_bin("blackglass").unwrap()
        .arg("--socket").arg(&sock)
        .arg("--token-file").arg(&tok)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("initialized"));

    assert!(sock.parent().unwrap().is_dir());
    assert!(tok.parent().unwrap().is_dir());
    assert!(tok.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tok).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "token file mode is {:o}, expected 0600", mode);
    }
    let token = std::fs::read_to_string(&tok).unwrap();
    assert_eq!(token.trim().len(), 64);
}
```

- [ ] **Step 2: Run, expect PASS**

- [ ] **Step 3: Commit**

```bash
git add crates/cli/tests/cli.rs
git commit -m "test(cli): init creates dirs and 0600 token"
```

---

### Task 25: CLI integration test — `audit verify`

**Files:**
- Modify: `crates/cli/tests/cli.rs` (append)

- [ ] **Step 1: Add the test**

```rust
#[test]
fn audit_verify_succeeds_on_clean_log_and_fails_on_tampered() {
    use blackglass_audit::{Chain, Event, EventKind};
    use serde_json::json;

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("a.jsonl");
    let sock = dir.path().join("rt.sock");
    let tok = dir.path().join("op.token");

    let mut chain = Chain::open(&log).unwrap();
    for i in 1..=3 {
        chain.append(Event {
            seq: i, ts: "2026-06-03T00:00:00Z".into(), prev_hash: String::new(),
            kind: EventKind::ActionRequested, payload: json!({"i": i}),
        }).unwrap();
    }
    drop(chain);

    Command::cargo_bin("blackglass").unwrap()
        .arg("--socket").arg(&sock)
        .arg("--token-file").arg(&tok)
        .arg("audit-verify")
        .arg("--path").arg(&log)
        .assert()
        .success()
        .stdout(predicate::str::contains("OK: 3"));

    let s = std::fs::read_to_string(&log).unwrap();
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    lines[1] = lines[1].replace("\"i\":2", "\"i\":999");
    std::fs::write(&log, lines.join("\n") + "\n").unwrap();

    Command::cargo_bin("blackglass").unwrap()
        .arg("--socket").arg(&sock)
        .arg("--token-file").arg(&tok)
        .arg("audit-verify")
        .arg("--path").arg(&log)
        .assert()
        .failure();
}
```

- [ ] **Step 2: Run, expect PASS**

- [ ] **Step 3: Commit**

```bash
git add crates/cli/tests/cli.rs
git commit -m "test(cli): audit-verify end-to-end on real and tampered log"
```

---

## End-of-plan checklist (run before declaring sub-plan 1 done)

- [ ] **All workspace tests pass**

```bash
cargo test --workspace 2>&1 | tail -20
```

- [ ] **Clippy is clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```

- [ ] **No reverse deps from spine crates into `core`**

```bash
cargo tree -p blackglass-audit | grep blackglass-core
cargo tree -p blackglass-profile | grep blackglass-core
cargo tree -p blackglass-engagement | grep blackglass-core
cargo tree -p blackglass-ipc | grep blackglass-core
```

Expected: each command produces no output.

- [ ] **`Cargo.lock` is committed**

```bash
git add Cargo.lock
```

- [ ] **Final commit**

```bash
git add -A
git commit --allow-empty -m "chore: sub-plan 1 (spine) complete and green"
```

---

## What this plan does NOT cover (handed to future sub-plans)

- **Sub-plan 2** (`docs/superpowers/plans/2026-06-03-blackglass-osint-packets.md`): implement Gate 4 properly with redaction fixtures; add `mcp-osint` and `mcp-packets` servers via `rmcp`; end-to-end test against a docker-compose test target.
- **Sub-plan 3** (`docs/superpowers/plans/2026-06-03-blackglass-operator.md`): `+operator` build flag, signed config, Gate 3 Tauri modal, `mcp-recon` and `mcp-network` (nmap, nuclei), AppArmor profile.
- **Sub-plan 4** (TBD): remaining 11 domains, Tauri desktop shell, Python sidecar, packaging.

The chokepoint test in Task 17 / 20, the Gate-denial tests in 18 / 19, and the server auth test in Task 22 are the contract that every future sub-plan must not break.

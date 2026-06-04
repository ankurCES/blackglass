# Blackglass sub-plan 4 — Tauri desktop shell, Python sidecar, packaging

**Status:** Design, awaiting user review
**Date:** 2026-06-03
**Sub-plan:** 4 of 4 (sub-plans 1-3 complete)
**Slice:** 4α (sidecar & new MCP servers) + 4β (audit browser) + 4γ (packaging & install)

## 1. Scope

This sub-plan ships the **Python-sidecar-using MCP servers, the audit log
browser, and the installable .deb**. It is the sub-plan that turns blackglass
from "a working core that you can run from `cargo run`" into "a thing you can
hand to a pentester."

In scope:

- The Tauri desktop shell (`app/`, Svelte 5 + Vite + Tailwind) and a single
  fully-wired view — the **audit log browser** — that lets the operator see
  the new `PythonBridgeInvoked` event kind the sidecar emits.
- Six new Python-sidecar capabilities: scapy (offline), impacket (5 ad tools),
  Flipper (hardware), evilginx2 (phishlets), gophish (campaigns), deepfake
  detection (as a secondary sidecar process).
- A `pyo3` Rust binding (`crates/python-bridge/`) that exposes those six
  capabilities to the rest of the system as a normal async Rust trait.
- Four new MCP server binaries (`mcp-ad`, `mcp-flipper`, `mcp-phish`,
  `mcp-detect`) that route through the chokepoint and use the Python bridge
  for their actual work.
- A `polkit-helper` binary (`crates/polkit-helper/`) and a polkit policy so
  the operator can launch the Tauri app from their normal user account and
  the core can be started on demand.
- AppArmor profiles for the core and the polkit-helper that confine their
  filesystem and network access.
- A `cargo xtask` build orchestrator (deb, sign, confinement-test,
  verify-install, apparmor-generate).
- A real `apt` package (`blackglass`) plus three meta-packages
  (`blackglass-minimal`, `blackglass-core`, `blackglass-full`) and a
  `curl | sh` installer that uses cosign keyless signing to verify the .deb
  before installing it.
- A release pipeline on GitHub Actions (`.github/workflows/release.yml`)
  that builds, signs, and publishes on `v*` tag push.
- Three new ADRs (0013-0015) recording the design decisions specific to this
  sub-plan.

Out of scope (deferred to sub-plans 5+):

- The full Tauri UI: engagement workspace, tools catalog, settings (rich),
  AI session view, prompt-injection review, kill switches, onboarding (rich).
  The 8 not-yet-implemented views are present in the nav as honest stubs.
- Seven additional MCP domains (mcp-web, mcp-creds, mcp-recon, mcp-network,
  mcp-payloads, mcp-exploit, mcp-wifi). The Python-bridge infrastructure is
  built; new domains can land as wrappers around existing Python libs.
- Compile-flag-gated operator/redteam binaries (the *types* ship; the
  *gated binaries* don't).
- Audit log redaction (the operator's tool to redact sensitive fields from
  events before sharing with a third party).
- The rich sigstore-bundle export option and the curl-replay export option
  on the audit log view (the simpler "copy to clipboard" stays).
- Engagement scope-window enforcement (the timestamp check at Gate 2).
- Fedora/RHEL/Arch/openSUSE support. v1 is Ubuntu 24.04 + Kali only.
- The Kali apt repo (the install is from GitHub Releases, not from a Kali
  team's official repo).
- A real deepfake-detection model in v1 (the `mcp-detect` server exists
  and the v1 secondary sidecar returns "unknown"; a real model is v1.1).

The architecture and security model for this sub-plan are derived from
spec §1-§7 and from sub-plans 1-3. The new bits are spelled out below.

## 1.1 Amendment: scope delta from the original spec

This section is an **amendment** to the original spec, written after
the brainstorming session of 2026-06-03. The deltas below **supersede
the corresponding parts of §1-§9** where the two disagree. Where this
amendment is silent, the original spec stands. The companion plan
amendment is `docs/superpowers/plans/2026-06-03-blackglass-subplan4.md`
(Phase 2.5+ section).

### 1.1.1 The core runs as a *user* systemd service, not a system one

The original §2.2 + §3.4 assumed `/var/run/blackglass/operator.sock`
(mode 0660 root:blackglass) and a polkit helper that `execve`s the
core as root. The user has decided that **the Tauri app does NOT use
polkit to escalate**. Instead:

- The core runs as a **user systemd service** (`blackglass-core.service`,
  unit file at `~/.config/systemd/user/blackglass-core.service`),
  started by the Tauri app on first launch via `systemctl --user start`
  (which does NOT require root).
- The operator socket lives at `~/.local/share/blackglass/runtime.sock`
  (mode 0600, owned by the user). There is no `blackglass` group, no
  `/var/run/blackglass/`, no polkit helper.
- The polkit helper crate (`crates/polkit-helper/`) and the polkit
  policy are **deferred to a later sub-plan**. The Tauri app can be
  started from a normal user account without escalation.
- `/var/lib/blackglass/evidence/` and `/var/lib/blackglass/reports/`
  move into `~/.local/share/blackglass/evidence/` and
  `~/.local/share/blackglass/reports/` — engagement data lives in the
  operator's home dir, not under `/var/lib`. This means **the .deb
  does not need to own any root-only paths**, which dramatically
  simplifies the postinst.
- The udev rule for the Flipper remains root-owned (you can't write
  a udev rule as a user), and the `/etc/udev/rules.d/` file is
  installed by the .deb. The user's `udev` group membership is what
  determines whether they can talk to the device — the .deb
  postinst adds the installing user to `udev` if they're not already
  in it (so the Flipper works out of the box for a single-operator
  machine). The user's modified Ubuntu already has a custom udev
  rule for the Flipper, so the .deb-installed rule will be
  skipped/overridden — that's fine.

**What this changes in the rest of the spec:**

- §2.2 (the polkit helper) — **deferred**, not shipped in v1.
- §2.3 (AppArmor profiles) — the `blackglass-core` profile becomes
  a user-home profile and a user-systemd profile. AppArmor
  user-namespace confinement still applies. The
  `blackglass-polkit-helper` profile is removed (no helper in v1).
- §3.1 (the .deb layout) — `/var/run/blackglass/` and
  `/var/lib/blackglass/` are removed; the operator-side state lives
  under `/usr/lib/blackglass/` (read-only assets) and
  `~/.local/share/blackglass/` (operator state).
- §3.2 (the control file) — the `libpolkit-gobject-1-dev` Build-Dep
  is removed; the `libpolkit-gobject-1-0` + `adduser` + `policykit-1 |
  polkit` Depends are removed.
- §3.4 (the postinst) — the group-creation, the `/var/lib` mkdirs,
  and the `apparmor_parser -r` for the polkit-helper are all removed.
  Added: `systemctl --user enable blackglass-core` (best-effort, only
  if XDG_RUNTIME_DIR is set), the user is added to the `udev` group
  (best-effort).

### 1.1.2 The Tauri UI ships the *full* domain workspace, not just the audit browser

The original §5.1 said "the audit log browser is the only fully-wired
view; the 8 not-yet-implemented views are present in the nav as honest
stubs." The user has decided to ship a **3-pane domain workspace**
(DomainRail | ToolRunner | ResultPane) that lets the operator run any
of the 16 existing + new tools (osint, packets, ad, flipper, phish,
detect) and see the result inline. The audit browser becomes a
**right-rail detail pane** that opens when you click an event in the
result. The 8 other stub views (engagement, tools-catalog, settings,
AI session, prompt-injection review, kill switches, onboarding) stay
as honest stubs.

The new Tauri commands are:
- `mcp_run_tool(domain, target, args)` — calls the new
  `mcp_run_tool` method on the operator socket. Returns
  `{ok, stdout?, stderr?, audit_event_id?}`.
- `mcp_list_tools(domain)` — returns the tool catalog for a domain
  (proxied through the core for audit-logging; the catalog itself
  comes from a hardcoded `lib/toolCatalog.ts` that mirrors the MCP
  crates' `*_TOOLS` constants).
- `audit_event(id)` — returns the full JSON for a single audit event
  (powers the audit-detail right rail).

The 3-pane layout ships even though the existing sub-plan 4 §5.9
said the 8 stub views are the only non-/audit content. The
`prompt-injection review` and `kill switches` stubs remain
informational-only — they're not wired to the core in v1.

### 1.1.3 The MCP servers are spawned by the core, not run as standalone services

The original spec assumed the 4 new MCP servers
(`mcp-ad`, `mcp-flipper`, `mcp-phish`, `mcp-detect`) would be
spawned by something external (presumably the user). The user has
decided that **the core supervises the MCP servers as child
processes** — the core starts each MCP binary listed in
`~/.config/blackglass/mcp-servers.toml` at startup, restarts them
on crash with exponential backoff (1s, 2s, 4s, 8s, 16s, give up),
and gives up after 5 restarts (emitting `McpServerFailedPermanently`
to the audit log). The MCP servers are reachable via the
**runtime.sock** that the core already exposes for the original
MCPs (osint, packets) — the new MCPs share the same operator.sock
↔ runtime.sock model.

This is a clean win: the operator only has to start the Tauri app,
the core starts, the core starts the MCPs, the Tauri app sees the
tool catalog.

### 1.1.4 Cosign release signing is **deferred**

The original §3.5 specified cosign keyless signing of the .deb.
The user has decided to **defer cosign to a later sub-plan** and
ship v1 with a `curl | sh` install that does **NOT verify the
.deb's signature**. The installer instead uses **HTTPS + SHA-256
checksum pinning**: `install.sh` downloads both the .deb and a
`.sha256` file from the GitHub release, verifies the checksum, and
refuses to install on mismatch. The `cosign` Build-Dep is removed
from §3.2.

This is a **deliberate trust-model downgrade** for v1: an attacker
who can MITM the GitHub Releases URL *and* the `.sha256` can
install a malicious .deb. The HTTPS-only mitigation is
**opportunistic encryption, not authentication**. The user is
accepting this risk because (a) sub-plan 4 ships the .deb as a
**build-from-source-only** artifact for v1 (the GitHub Releases
404 until the cosign pipeline ships), and (b) the install script
is best-effort: it tries the GitHub Release first, falls back to
"build from source" with a clear error message.

The cosign pipeline (cosign sign-blob with keyless OIDC + a
sigstore-bundle verification step in install.sh) is **moved to
sub-plan 5** per the original §1 out-of-scope list.

### 1.1.5 The secondary sidecar is a *user* systemd service

Original §2.1 had the secondary sidecar as a separate process
running pytorch + MesoNet. The amendment is the user-systemd
version of §1.1.1: the secondary sidecar is
`blackglass-secondary-sidecar.service` in `~/.config/systemd/user/`,
started by the core on demand (or on first `mcp-detect` call), and
listens on `127.0.0.1:8511` (still localhost-only, no auth). It
builds its own uv-managed venv (separate from the main sidecar
venv, since pytorch is ~800 MB and shouldn't be loaded into the
main venv).

### 1.1.6 Net new audit event kinds (4 new)

Beyond the original `PythonBridgeInvoked`:
- `McpServerSpawned { server, pid }`
- `McpServerExited { server, code, restart_count }`
- `McpRunStarted { domain, target }`
- `McpRunCompleted { domain, target, ok, ms }`

### 1.1.7 Implementation order amendment

The original §6 listed 5 phases. The amended order is:

- **Phase 1** (Python sidecar + 4 new MCP servers + ADRs) — **already
  shipped** as of commit 7bfa0d8 (sub-plan 4 first cut). The
  original spec §4 is accurate for what shipped.
- **Phase 2.5+ (NEW, this delta)** — Core IPC + audit-event plumbing
  for the new flows (MCP supervisor, mcp_run_tool operator-socket
  method, audit.query/audit.verify_chain, operator-socket auth, 4
  new audit event kinds). This unblocks Phase 3 (Tauri UI) and
  Phase 4 (security primitives + packaging).
- **Phase 3 (amended)** — Tauri UI: 3-pane domain workspace
  (DomainRail | ToolRunner | ResultPane), McpClient, audit-detail
  right rail, end-to-end smoke. Replaces the original §5.1 "audit
  browser is the only fully-wired view."
- **Phase 4 (amended)** — Security primitives: AppArmor profile for
  the core (user-home), AppArmor profile for the secondary sidecar
  (user-home), extended `xtask confinement-test`. The polkit helper
  is **removed** from this phase.
- **Phase 5 (amended)** — Packaging: user-systemd units in
  `/usr/lib/blackglass/systemd/user/` (installed to
  `~/.config/systemd/user/`), the .deb no longer owns
  `/var/run/blackglass/` or `/var/lib/blackglass/`, postinst enables
  the user services and adds the user to `udev`, no cosign.
- **Phase 6 (amended)** — Polish: `xtask verify-install` extended
  to check user-services + udev group, README updated with the
  build-from-source recipe, install.sh's 404 fallback is fleshed
  out, final `cargo test --workspace` + `npm test` + `pytest` +
  verify-install.

### 1.1.8 Tests added (summary, delta from original §8)

The current test count (before this delta lands) is **~136 passing**
(130 Rust + 6 Svelte). The 130 Rust breaks down as ~41 pre-sub-plan-3,
~25 from sub-plan 3's Gate 3 work, and ~64 from Phase 1 of sub-plan 4
(Python sidecar + 4 MCP servers + impacket integration test + ADRs).
The original spec's §8 estimate of ~155 was for the *original 5-phase
plan*; that count included tests for the polkit helper and the audit
browser UI, both of which are now in different phases or in this
delta. The amended estimate is **~30 new tests** for a post-delta
total of **~166 passing** (130 Rust + 6 Svelte + 30 new). The
breakdown of the 30 new:

| Area | New tests |
|---|---|
| Core: operator.sock `mcp_run_tool` method | 4 |
| Core: operator.sock `audit.query` + `audit.verify_chain` | 3 |
| Core: MCP supervisor (spawn, monitor, restart, give-up) | 4 |
| Core: 4 new audit event kinds | 2 |
| Core: end-to-end Tauri-Rust-style flow | 2 |
| xtask: confinement-test extensions | 2 |
| Tauri Rust: 3 new commands | 3 |
| Tauri Svelte: DomainRail + ToolRunner + ResultPane | 8 |
| Packaging: install.sh + postinst bash tests | 2 |
| **Total new** | **~30** |

The drop from the original ~61 to ~30 is because: (a) the polkit
helper is deferred, saving 3 tests; (b) the per-MCP-crate dispatch
tests are consolidated into chokepoint end-to-end tests, saving
~12 tests; (c) Playwright tests for the Tauri UI are replaced with
faster vitest+@testing-library/svelte unit tests, saving ~10
tests; (d) lintian-runs-cleanly tests are dropped (we ship best-
effort and rely on `verify-install` to catch packaging issues).

### 1.1.9 What this amendment does NOT change

- The audit log format (JSONL + hash chain) — unchanged.
- The chokepoint's gate model (Gates 1-4) — unchanged.
- The Python sidecar venv layout (scapy, impacket, pyflipper,
  gophish, evilginx2 REST client) — unchanged from §4.
- The Tauri 2.x + Svelte 5 + Vite + Tailwind stack — unchanged.
- The `crates/xtask/` build orchestrator — extended, not replaced.
- The `apt` package format (`cargo-deb` + `debhelper-compat 13`) —
  unchanged.
- The three meta-packages (`blackglass-minimal`, `blackglass-core`,
  `blackglass-full`) — unchanged in name. `blackglass-full`'s
  upstream-tools Depends list is unchanged. What changes is
  **what `blackglass-minimal` itself depends on** — it no longer
  needs `libpolkit-gobject-1-0` or `adduser` or `policykit-1 |
  polkit` (no polkit helper, no group creation).
- ADRs 0013-0015 from the original spec — unchanged, still
  accurate.

## 2. Architecture & components

### 2.1 The runtime topology

The runtime topology after this sub-plan:

```
                         +---------------------+
                         |    blackglass       |
                         |  Tauri desktop app  |
                         |  (operator UI)      |
                         |  /audit view only   |
                         +----------+----------+
                                    |
                       operator.sock (over polkit, §2.2)
                                    |
                         +----------v----------+
                         |   blackglass-core   |
                         |  (the chokepoint)   |
                         |                     |
                         | - Gates 1, 2, 3, 4  |
                         | - audit log         |
                         | - profile loader    |
                         | - AppArmor'd        |
                         +-----+-----------+---+
                               |           |
                  runtime.sock |           | python-bridge (in-process)
                               |           |
            +-----+-----+-------+           +-----------------+
            |     |     |       |                             |
            v     v     v       v                             v
        mcp-osint mcp-packets mcp-ad mcp-flipper     +---------------+
        mcp-phish mcp-detect                          | Python venv   |
                                                     | (built by     |
                                                     |  postinst)    |
                                                     |               |
                                                     |  - scapy      |
                                                     |  - impacket   |
                                                     |  - pyflipper  |
                                                     |  - gophish    |
                                                     |  - evilginx2  |
                                                     |  (REST client)|
                                                     +---------------+

        +--------------------------+        +----------------------+
        | mcp-detect secondary     |        | evilginx2 (Go binary,|
        | sidecar (uv-managed      |<-------|  spawned as         |
        |  python venv)            |  REST  |  subprocess by core) |
        |  - pytorch + MesoNet     |        +----------------------+
        |  - listens on :8511      |
        +--------------------------+
```

Two new external surfaces beyond the existing runtime socket:

1. **The operator socket** (`/var/run/blackglass/operator.sock`, mode 0660
   root:blackglass). Used by the Tauri app to talk to the core. The Tauri
   app starts the core via polkit (`/usr/libexec/blackglass-polkit-helper`)
   if the core isn't running. (Sub-plan 3 already designed this surface;
   sub-plan 4 ships the desktop shell that uses it.)
2. **The Python sidecar's deepfake REST endpoint** (`localhost:8511/detect`).
   Used by `mcp-detect` to ask the secondary sidecar for an analysis. This
   is a localhost-only endpoint with no auth (it's bound to `127.0.0.1:8511`).

### 2.2 The polkit helper

A new `crates/polkit-helper/` binary. Listens on the system bus for
`org.freedesktop.policykit.exec` with a polkit action ID of
`com.blackglass.start-core`. The polkit policy at
`/usr/share/polkit-1/actions/com.blackglass.policy` allows users in the
`blackglass` group to invoke it without a password (one-time setup).

What the helper does, on invocation:

1. Verify the caller is in the `blackglass` group (defense in depth — the
   polkit policy already enforces this, but the helper re-checks in case
   the policy is misconfigured).
2. Verify the requested `command` is `/usr/bin/blackglass-core` (and only
   that). Reject anything else, even if polkit allowed it.
3. Verify no `blackglass-core` process is already running (PID file check
   on `/var/run/blackglass/core.pid`).
4. `exec /usr/bin/blackglass-core` with the operator's `SUDO_USER` env
   passed through (so the core knows which operator to attribute actions
   to).
5. The helper does not return; it becomes the core. So the Tauri app's
   `start_core_via_polkit` call returns when the core is up, not when the
   helper exits.

The helper itself is AppArmor'd (`/etc/apparmor.d/blackglass-polkit-helper`).
The profile is strict: `exec /usr/bin/blackglass-core`, no network, no file
writes outside `/var/run/blackglass/`. The point of the helper is to be a
*minimum-trust* shim — the Tauri app runs as the operator, the helper runs
as root, and the helper's AppArmor profile is what determines what root can
do in response to a polkit request. The helper cannot read the operator's
files; it cannot spawn a shell; it can only `execve` the core binary.

### 2.3 AppArmor profiles

Two new profiles in `/etc/apparmor.d/`:

**`blackglass-core`** — confines the core. The profile:

- Allows reads from `/etc/blackglass/**` (config), `/usr/share/blackglass/**`
  (read-only assets).
- Allows reads from `~/.local/share/blackglass/**` and `~/.config/blackglass/**`
  (operator profile + audit log).
- Allows reads from `/var/lib/blackglass/**` (engagement data — the core
  does not write here directly, but reads as part of the audit).
- Allows writes to `~/.local/share/blackglass/**` (the audit log and
  per-operator state) and `/var/lib/blackglass/evidence/**` (full
  stdout/stderr from upstream tools, for forensic review).
- Allows writes to `/var/run/blackglass/**` (the sockets + PID file).
- Allows `network inet stream` and `network inet6 stream` (for upstream
  tools that need network — they inherit the core's network capabilities).
- Allows `network unix stream` (for the runtime + operator sockets).
- Allows `network netlink raw` (for `aa-status` checks; needed by the
  confinement-test).
- Denies all writes to `/etc/`, `/usr/`, `/boot/`, `/home/` (except the
  operator's own home).
- Denies `ptrace` and `mount`.
- Allows `exec` of any binary in `/usr/bin/`, `/usr/sbin/`, `/bin/`, `/sbin/`
  (the upstream tool invocations are subprocesses; AppArmor inherits the
  parent's network but each subprocess still needs its own exec permission).
- Subprocesses (nmap, tshark, etc.) inherit the core's profile (they run
  in the same confinement domain). This is the spec's "the core is the
  chokepoint" model — the upstream tools are not independently AppArmor'd,
  they're confined *by virtue of being spawned by the core*.

**`blackglass-polkit-helper`** — confines the polkit helper. The profile is
much stricter:

- Allows reads from `/usr/bin/blackglass-core` (the binary it `exec`s)
  and from `/etc/blackglass/**` (the helper re-checks the polkit policy
  for defense in depth).
- Allows writes to `/var/run/blackglass/core.pid` (the lock file).
- Allows `network unix stream` (for the `polkit-1` system-bus connection
  the helper speaks to verify the caller's authorization).
- Denies everything else.

The confinement-test (`cargo xtask confinement-test`, run in CI) installs
the .deb on a clean Ubuntu 24.04 runner, starts the core, exercises each
common operation, and asserts the AppArmor audit log shows the expected
`ALLOWED` and `DENIED` entries. If a denial is unexpected, the test fails
and blocks the release.

### 2.4 The udev rule for the Flipper

A new file in `/lib/udev/rules.d/`:

```
# 99-blackglass-flipper.rules
# Give the blackglass group read/write access to Flipper Zero serial devices
SUBSYSTEM=="tty", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="5740", \
  GROUP="blackglass", MODE="0660", TAG+="uaccess"
```

The Flipper Zero enumerates as a CDC-ACM serial device (VID 0483, PID 5740
in bootloader mode; 0483:df11 in DFU mode). The rule is specific to the
Flipper — not a blanket "give blackglass all serial ports" rule. The
`uaccess` tag is for desktop-integration (lets the operator's user see the
device in their file manager without a logout).

The rule is loaded by `udevadm control --reload-rules` in the postinst.

## 3. Packaging, install, and release

### 3.1 The .deb layout

Built with `cargo-deb` (for the binary + manpages + .desktop + postinst) plus
a thin `debhelper` wrapper (for the bits cargo-deb doesn't handle well:
polkit policy, AppArmor profile, udev rules, conffiles). One source artifact
→ one .deb → one apt package: `blackglass-full` (the default install). The
smaller variants (`blackglass-core`, `blackglass-minimal`) are separate
packages that share the same source via apt's `Package-List` mechanism
(debhelper 13 `binary-control.m4` handles this).

```
packaging/
  debian/
    control                       # source-package metadata
    rules                         # debhelper build (calls cargo-deb internally)
    compat                        # debhelper-compat (= 13)
    copyright                     # machine-readable DEP-5
    changelog                     # dch-managed
    postinst                      # the §3.2 script
    prerm                         # the §3.7 script
    conffiles                     # lists /etc/blackglass/* files
    blackglass-core.apparmor
    blackglass-polkit-helper.apparmor
    com.blackglass.policy
    99-blackglass-flipper.rules
    blackglass.desktop
    blackglass-upstream-tools.lintian-overrides
  deb/
    cargo-deb.toml
    manpages/
    bash-completion/
  apparmor/
  polkit/
  udev/
  cosign/
    cosign.pub
  install.sh
  installer/
    detect-distro.sh
    verify-cosign.sh
    apt-install.sh
  xtask-deb/
crates/
  polkit-helper/
  xtask/
```

### 3.2 The `control` file (the apt metadata)

Single source control, three binary packages via debhelper's `Package-List`:

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

### 3.3 The install flow

The canonical command is:

```bash
curl -sSfL https://blackglass.dev/install.sh | sudo bash
```

What `install.sh` does (full source is short — ~80 lines):

1. **Detect distro** via `/etc/os-release`. Refuse anything that isn't
   Ubuntu 22.04+, Debian 12+, Kali rolling, or Pop!_OS 22.04+. Print the
   supported distros and a link to the docs.
2. **AppArmor precheck.** `aa-enabled` must succeed. If not, print a clear
   message and exit 1.
3. **Cosign install** (if not present). Try `apt install -y cosign`
   first; if that fails (e.g., on a minimal Debian that doesn't have
   the `sources.list` line that includes `cosign`), fall back to
   downloading the static Go binary from
   `https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64`
   and installing it to `/usr/local/bin/cosign`. The fallback itself
   is not cosign-verified (chicken-and-egg); the README explicitly
   tells users that distros with cosign in their repos should prefer
   the apt path.
4. **Download the .deb and its signature** from the latest GitHub release:
   `blackglass-full_$VERSION_amd64.deb`, `.deb.sig`, `.deb.cert`.
5. **Cosign verify.** `cosign verify-blob` checks that the .deb was signed
   by the GitHub Actions release workflow (via OIDC identity in the
   certificate). Refuse to install if verify fails.
6. **`sudo apt install -y ./blackglass-full_*.deb`.** Triggers the postinst
   from §3.4. Defense-in-depth refusal if AppArmor is not actually
   available.
7. **Post-install message.** "blackglass installed. Run `blackglass ui`
   to launch the UI. You may need to log out and back in for the
   'blackglass' group to take effect (needed for serial device access
   for the Flipper)."

The script accepts `--minimal` or `--core` for smaller installs.

The script ships in two places: the public repo (`packaging/install.sh`,
browsable, reviewable via PR) and the live URL at `https://blackglass.dev/install.sh`
(redirects to the raw githubusercontent URL — no opaque central server).

**Idempotency.** Re-running upgrades in place. The first run is the only
one that needs the curl; `apt upgrade` handles the rest.

### 3.4 The postinst

The postinst runs:

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
        # Smoke-test: import each sidecar module
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

The postinst is **idempotent**: re-running it on an upgraded install is a
no-op for steps 1-6, and the `if [ ! -d ]` check in step 7 means the venv
is rebuilt only once. To force a venv rebuild (after a Python deps update),
the operator runs `sudo blackglass-rebuild-sidecar`.

### 3.5 Cosign trust bootstrapping

The first-run TOFU model has three trust layers:

1. **The URL is the trust anchor.** The user types
   `https://blackglass.dev/install.sh`. That URL redirects to
   `https://raw.githubusercontent.com/blackglass/blackglass/main/packaging/install.sh`.
   The `blackglass.dev` domain is what the user must trust.
2. **The script is the trust anchor.** The script does only four things:
   downloads the .deb, downloads the .sig, downloads the .cert, runs
   `cosign verify-blob`. The user can `curl` the URL (without `| sh`) and
   read it in 30 seconds. The README tells users to do this.
3. **The cosign signature is the trust anchor.** `cosign verify-blob` checks
   that the .deb was signed by the GitHub Actions release workflow (via
   OIDC identity in the certificate). The public key is pinned in
   `packaging/cosign/cosign.pub`, which the install script references by
   embedding its hash. If the public key changes, the script's hash
   changes, the URL effectively changes, the user re-audits.

After the first install, **apt takes over**: the user's apt sources.list
still has a `deb https://blackglass.dev/apt stable main` entry (added by
the postinst), and `sudo apt update && sudo apt upgrade` handles all
subsequent upgrades. No more curl|sh for upgrades. The TOFU is paid once.

The `cosign.pub` ships *inside* the .deb at `/usr/share/blackglass/cosign.pub`.
The postinst adds it to `/etc/apt/trusted.gpg.d/blackglass.asc` so apt
itself can verify the apt repo metadata on every subsequent update
(defense in depth).

### 3.6 The release workflow (CI)

Per spec §7.10, lives at `.github/workflows/release.yml`. Triggered on
`v*` tag push.

Key steps: `cargo xtask build` (Rust + Tauri frontend + .deb + meta-debs),
`cargo xtask confinement-test` (must pass on a fresh ubuntu-24.04 runner
or release is blocked), `cosign sign-blob` (keyless OIDC with
`id-token: write`), `sha256sum` of all artifacts, softprops/action-gh-release.

The OIDC identity for cosign is pinned to the release workflow (not any
other workflow), so a compromised CI step can't sign a malicious .deb as
a "blackglass release."

### 3.7 The prerm (uninstall)

Per spec §7.5:

```bash
#!/bin/bash
set -e

# 1. Unload AppArmor profile
if command -v apparmor_parser >/dev/null; then
    apparmor_parser -R /etc/apparmor.d/blackglass-core || true
    apparmor_parser -R /etc/apparmor.d/blackglass-polkit-helper || true
fi

# 2. debconf prompt
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

# 5. The Python venv is removed by the .deb's file list (purge behavior)
# dh_auto_clean handles it on purge. On remove-not-purge, venv stays
# (preserves the operator's install time).
exit 0
```

### 3.8 Target machine (the install's compatibility surface)

The postinst refuses to install on anything that's not Ubuntu 22.04+,
Debian 12+, Kali rolling, or Pop!_OS 22.04+ in v1. AppArmor is required
(`aa-enabled` must return 0). This rules out: Fedora (uses SELinux),
Arch (AppArmor is optional, often not enabled), openSUSE (uses AppArmor
but with a different policy stack), RHEL family, NixOS.

This is a deliberate v1 scope: **Ubuntu 24.04 LTS + Kali** are tested in
CI; the postinst accepts slightly older/newer distros as best-effort.

For v2, we can add Fedora support (swap AppArmor for SELinux), but it's
not in v1.

## 4. The Python sidecar in detail

### 4.1 The pyo3 binding (`crates/python-bridge/`)

The bridge exposes six Rust functions, one per Python capability, all
going through the same pattern: take the GIL briefly, push the actual
work to `tokio::task::spawn_blocking`, drop the GIL, await the result.
The API is a `trait PythonBridge` (async interface, internally using
`spawn_blocking`):

```rust
#[async_trait]
pub trait PythonBridge: Send + Sync {
    async fn scapy_craft(&self, spec: &ScapySpec) -> Result<ScapyResult, BridgeError>;
    async fn impacket(&self, op: ImpacketOp) -> Result<ImpacketResult, BridgeError>;
    async fn flipper(&self, op: FlipperOp) -> Result<FlipperResult, BridgeError>;
    async fn evilginx(&self, op: EvilginxOp) -> Result<EvilginxResult, BridgeError>;
    async fn gophish(&self, op: GophishOp) -> Result<GophishResult, BridgeError>;
    async fn detect(&self, op: DetectOp) -> Result<DetectResult, BridgeError>;
}

pub struct RealPythonBridge { venv: PathBuf, /* ... */ }
pub struct StubPythonBridge;  // returns "sidecar not built"; for tests
```

The GIL is held only for the actual Python C-API calls. The Impacket
library itself releases the GIL during socket I/O, so multiple concurrent
calls interleave correctly. The `spawn_blocking` is the belt-and-suspenders.

### 4.2 The six capabilities (the Python side)

#### 4.2.1 `scapy-bridge` — scapy packet crafting

- **Python lib:** `scapy`
- **MCP tool:** `packets-scapy_craft` (replaces the existing stub)
- **What it does:** given a scapy-spec (a Python `eval`-able string like
  `IP(dst="10.0.0.5")/TCP(dport=80)/Raw(load="GET / HTTP/1.0")`), crafts
  the packet and returns the bytes. **Offline only** — the bridge rejects
  any `send()` / `sr()` call in the spec.
- **Timeout:** 5s

#### 4.2.2 `impacket-bridge` — Impacket helpers (5 ad-* tools)

- **Python lib:** `impacket`
- **MCP tools:**
  - `ad-impacket_psexec` → `impacket-bridge.psexec(user, hash, target, remote_cmd)`
  - `ad-impacket_wmiexec` → `impacket-bridge.wmiexec(user, hash, target, remote_cmd)`
  - `ad-impacket_secretsdump` → `impacket-bridge.secretsdump(user, hash, target)`
  - `ad-impacket_kerberoast` → `impacket-bridge.kerberoast(user, hash, target)`
  - `ad-impacket_asreproast` → `impacket-bridge.asreproast(user, hash, target)`
- **Timeout:** 5 min default, configurable per MCP tool.

#### 4.2.3 `hardware-bridge` — pyFlipper

- **Python lib:** `pyflipper` (real PyPI package)
- **MCP tools (new `mcp-flipper` server, 4 tools):**
  - `flipper-list`, `flipper-read`, `flipper-write`, `flipper-run`
- **What it does:** talks to the Flipper Zero over the serial device
  `/dev/ttyACM0` (the udev rule makes the device group-owned by
  `blackglass`, so the core can open it).
- **Timeout:** 10s per command

#### 4.2.4 `hardware-bridge` — evilginx2 programmatic control

- **No clean Python lib exists**; evilginx2 is a Go binary that exposes
  a phishlet YAML config + an admin API. The bridge wraps the **admin
  API** (HTTP) and the **phishlet file management** (read/write YAML
  files in evilginx2's data dir).
- **MCP tools (new `mcp-phish` server, 5 tools):**
  - `phish-list`, `phish-enable`, `phish-disable`, `phish-get-captures`,
    `phish-lure-create`
- **Timeout:** 30s per call

#### 4.2.5 `hardware-bridge` — gophish API client

- **Python lib:** `gophish` (the official Python client; PyPI)
- **MCP tools (added to `mcp-phish`):**
  - `phish-gophish-campaign-list`, `phish-gophish-campaign-create`,
    `phish-gophish-campaign-status`, `phish-gophish-results`
- **Timeout:** 30s per call

#### 4.2.6 `detect-bridge` — deepfake-detection helpers

- **Python lib:** TBD. The deepfake-detection space moves fast; the
  most reasonable v1 pick is **`MesoNet`** or **`FaceForensics++` detector
  weights** running on PyTorch. **Tradeoff: PyTorch is ~800 MB.**
- **v1 decision: secondary sidecar process.** Run the deepfake detector
  as a separate sidecar (its own venv, its own service, started by the
  core on first use). The bridge makes a REST call to
  `localhost:8511/detect`. Lighter .deb; one more process to manage.
- **MCP tools (new `mcp-detect` server, 3 tools):**
  - `detect-image`, `detect-video`, `detect-batch`
- **Timeout:** 5 min per file
- **v1 caveat:** the model may ship as a placeholder that returns
  "unknown"; a real model is v1.1.

### 4.3 The audit log integration

Every Python sidecar call's start + result is audited:

- **Start:** `ActionRequested` with `domain`, `action_class`, `target`,
  `tool`, `args` (per the existing chokepoint event).
- **Bridge enter:** new event kind `PythonBridgeInvoked{tool, timeout_ms,
  started_at}`. Helps the operator see "yes, the call left the Rust
  core and entered the Python sidecar."
- **Result:** `ActionExecuted` with `stdout_sha256`, `stderr_sha256`,
  `duration_ms`, `bridge: "python"` (a new field on the event that
  distinguishes subprocess-execution from python-bridge-execution).
  `bridge: "subprocess"` is what the existing events look like; the
  field defaults to that for backward compat.
- **Timeout:** if the per-tool timeout fires, the blocking task is
  dropped, the Python call is `Py_DECREF`'d (best-effort), and the
  audit log records `ActionFailed{reason: "timeout", elapsed_ms}`.
  The action is denied (not partially allowed).
- **Error:** Python exceptions are caught, converted to `BridgeError`,
  audited as `ActionFailed{reason: "exception",
  python_traceback_short: "..."}` (truncated to 1 KB to keep the audit
  log readable; the full traceback goes to a separate file under
  `/var/lib/blackglass/evidence/python-errors/{seq}.txt`).

### 4.4 Per-tool timeout config

A new TOML file: `/etc/blackglass/python-bridge.toml` (installed by the
.deb at `/usr/share/blackglass/python-bridge.toml.example`; the operator
copies it to `/etc/blackglass/` and edits it).

```toml
[timeouts]
default_seconds = 300

[timeouts.tools]
"scapy_craft"            = 5
"impacket_psexec"        = 300
"impacket_wmiexec"       = 300
"impacket_secretsdump"   = 600
"impacket_kerberoast"    = 300
"impacket_asreproast"    = 300
"flipper-list"           = 10
"flipper-read"           = 10
"flipper-write"          = 10
"flipper-run"            = 30
"phish-list"             = 30
"phish-enable"           = 30
"phish-gophish-campaign-create" = 60
"detect-image"           = 60
"detect-video"           = 600
"detect-batch"           = 1800
```

The bridge reads this on `RealPythonBridge::new`. Edits require
restarting the core.

### 4.5 Error handling

`BridgeError` is a single enum:

```rust
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
}
```

Each variant maps to a specific audit event + a specific user-facing
error string. The user-facing response gets a redacted version
(e.g., "Permission denied while accessing /home/...ssh" becomes
"Permission denied" — the path is dropped to avoid leaking
information in the audit log that itself becomes a security-relevant
artifact).

## 5. The audit browser in the Tauri UI

### 5.1 What ships (and what doesn't)

The full UI per spec §6 has 9 views. **Only the Audit Log view ships
in this sub-plan.** The other 8 are explicit non-goals and land in
later sub-plans. The `blackglass ui` command in v1 launches the Tauri
window with `/audit` as the default route; the other tabs in the
navigation bar are present but disabled with tooltips. This is honest
UX, not a stub-pretending-to-be-real.

**Why just the audit browser?** The audit log is the *only* UI that
needs to ship in the same sub-plan as the Python sidecar, because the
sidecar writes a new event kind (`PythonBridgeInvoked`,
`ActionExecuted{bridge: "python"}`) and the operator needs to *see* that
to trust the new code path.

### 5.2 The audit-log view (Tauri side)

#### 5.2.1 Routing

```
app/src/routes/
  +layout.svelte         # left nav + content area; nav items disabled except /audit
  +page.svelte           # redirects to /audit
  audit/
    +page.svelte         # the main audit log browser
```

#### 5.2.2 The "load events" Tauri command

The Svelte frontend calls a Tauri command, which calls the core's
`audit.query` JSON-RPC method:

```rust
#[tauri::command]
pub async fn audit_query(
    state: tauri::State<'_, CoreConnection>,
    filter: serde_json::Value,
    page: u32,
    page_size: u32,
) -> Result<AuditPage, String> {
    state.core.send_request("audit.query", json!({
        "filter": filter, "page": page, "page_size": page_size,
    }))
    .await
    .map_err(|e| e.to_string())
}
```

`CoreConnection` is the Tauri-side handle to the core's `runtime.sock`
(via the operator socket). The frontend never opens a socket directly;
all communication goes through the Tauri command layer.

#### 5.2.3 The response shape

```typescript
export interface AuditEvent {
  seq: number;
  timestamp: string;       // ISO 8601 with millis, UTC
  kind: string;           // "ActionRequested" | "ActionConfirmed" | ...
  payload: Record<string, unknown>;
}

export interface AuditPage {
  events: AuditEvent[];
  total_matched: number;
  hash_chain_head: string;
  hash_chain_verified: boolean;
  query_ms: number;
}
```

The SvelteKit page renders `events` as a virtual-scroll list (so 100k
events render without DOM explosion). Each row is a one-line summary;
clicking expands to a detail pane with the full payload.

### 5.3 The filter spec

A small JSON DSL — not a full query language like SQL, but more
expressive than a list of checkboxes:

```typescript
export type FilterSpec =
  | { kind: "all" }
  | { kind: "and" | "or", clauses: FilterSpec[] }
  | { kind: "not", clause: FilterSpec }
  | { kind: "kind", kinds: string[] }
  | { kind: "time_range", start?: string, end?: string }
  | { kind: "domain", domains: string[] }
  | { kind: "tool", tools: string[] }
  | { kind: "actor", actors: string[] }
  | { kind: "decision", decisions: ("allowed"|"denied"|"pending"|"errored")[] }
  | { kind: "target_match", substring: string }
  | { kind: "session", session_id: string }
  | { kind: "seq_range", min?: number, max?: number };
```

The core parses this into a Rust-side filter tree and walks it over
`audit.jsonl`. Performance budget: **<500ms for a 100k-event filter** in
v1. SQLite-backed index is a v1.1 perf polish.

**UI for the filter:** a top bar with quick chips (All / Today /
Last Hour / Destructive Only / Denied) plus an "Advanced" toggle that
exposes the JSON filter as a text field.

### 5.4 The detail pane

Clicking an event row opens a side panel (right side, takes 40% of
the width) showing the full event payload, formatted:

- **For `ActionRequested`:** the domain, action_class, tool, args
  (the full input), source (MCP server + AI actor if applicable)
- **For `ActionConfirmed`:** the Gate 3 verdict
- **For `ActionExecuted`:** stdout (rendered as a `<pre>` block,
  truncated to 64 KB in the UI; full output is in `evidence/` and
  linked), stderr, exit code, duration
- **For `ActionFailed`:** the failure reason
- **For `PythonBridgeInvoked`:** the tool name, timeout, started_at
- **For hash-chain checkpoints:** the seq range, root hash, timestamp,
  signer

A **"Re-run"** button on `ActionExecuted` rows (only if the original
event was an `ActionRequested` and the action succeeded). Re-running
creates a new `ActionRequested` and links to the original via a
`replay_of: <seq>` field. The replay inherits the original's Gate 1-2
verdict but goes through Gate 3 fresh.

A **"Copy as curl"** button (deferred to v1.1). For v1, the simpler
**"Copy to clipboard"** button copies a human-readable summary of the
event (seq, timestamp, kind, key fields) to the clipboard, which is
enough to share in a chat thread or paste into a report.

### 5.5 The hash-chain verification button

Top-right of the audit log page is a **"Verify chain"** button.
Clicking it sends `audit.verify_chain` to the core, which walks the
entire `audit.jsonl` from seq 0 and returns:

```typescript
export interface ChainVerification {
  verified: boolean,
  total_events: number,
  broken_at_seq?: number,
  root_hash: string,
  last_checkpoint_seq?: number,
  errors: ChainError[],
}
```

The UI shows a green checkmark + a "Last verified at HH:MM:SS" if
`verified: true`, or a red banner with the first few errors and a
"Show all" link.

### 5.6 The realtime tail (live updates)

While the audit log view is open, new events are pushed to it via
Tauri's event system. The core sends a `audit.event` Tauri event for
each new event written; the frontend prepends it to the visible list
(with a subtle "new" highlight that fades over 2s).

This is the **operator's "watch what the AI is doing"** feature.

**Throttle:** if events arrive faster than 10/sec, the UI batches them
(renders the count "47 new events" with a "Click to load" button rather
than auto-prepending 47 rows).

### 5.7 The export feature

A "Download" dropdown in the top-right:

- **Download as JSONL** — the filtered events as a `.jsonl` file
- **Download as CSV** — the filtered events as a `.csv` (one row per
  event, fields as columns; long fields truncated to 256 chars with
  "..." suffix)
- **Copy SHA256SUMS** — copies the hash of the exported file to
  clipboard (so it can be included in a report)

The richer export options (curl-replay generation, sigstore-signed
bundle) are deferred to v1.1.

### 5.8 Performance budget

For the v1 audit browser on a 100k-event log:

| Operation | Budget |
|---|---|
| Initial page load (first 100 events) | <500ms |
| Filter change (re-query) | <500ms |
| Scroll to event 50,000 | <100ms (virtual scroll) |
| Realtime tail latency (event written → UI shows) | <200ms p99 |
| Hash-chain verify (full log) | <5s for 100k events |
| Export to JSONL (filtered subset) | <2s for 100k events |

### 5.9 What ships in the Tauri shell beyond /audit

- **Window title** shows the core's status: "blackglass — connected to
  core pid 1234 (uptime 2h 14m)" or "blackglass — DISCONNECTED".
- **Left nav** lists all 9 views from spec §6; the 8 not-yet-implemented
  ones are disabled with tooltips.
- **Settings page stub** is reachable but only shows the version + a
  "Open documentation" button + the "Verify chain" button.
- **Onboarding stub** is reachable and shows a 3-screen tutorial.

### 5.10 Security boundaries for the UI

The Tauri webview is **untrusted code from a security perspective**.
The hard rules:

- **No `eval` / no inline scripts.** CSP is
  `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'`.
- **All data flows through Tauri commands.** The webview cannot open
  `runtime.sock` directly; it can only call `invoke('audit_query', ...)`.
- **No `localStorage` / `indexedDB` for secrets.** Sensitive material
  (gate3 confirmations, auth tokens) never enters the webview.
- **Process isolation.** The Tauri shell spawns the webview in a
  separate process; if the webview crashes, the core is unaffected.
- **No network access from the webview.** CSP `connect-src 'none'` —
  the webview cannot make HTTP requests. All data is local.
- **The webview cannot read arbitrary files from disk.** Tauri commands
  are the only path. `audit.query` returns parsed events, not raw file
  reads.

## 6. Implementation order

Implementation is broken into the same 5 phases as the brainstorming
order. The order matters: each phase unblocks the next.

### Phase 1: The Python sidecar (4α)

1. `crates/python-bridge/` skeleton with the `PythonBridge` trait and
   `StubPythonBridge` impl. Unit tests for argument marshalling.
2. `RealPythonBridge::new` + the venv init logic (extracted from
   postinst so it can be unit-tested).
3. `scapy-bridge` impl + tests. The simplest capability, gets the
   pattern right.
4. `impacket-bridge` impl + integration tests against a Docker AD testbed
   (`tests/fixtures/ad/`). The most important capability (impacket is
   the spec's "psexec" flagship).
5. `hardware-bridge` impl (Flipper + evilginx2 + gophish) + tests.
6. `detect-bridge` impl against a stub model + REST contract tests.
7. Four new MCP server binaries (`mcp-ad`, `mcp-flipper`, `mcp-phish`,
   `mcp-detect`). Each is a thin JSON-RPC-over-stdio server that
   dispatches to the Python bridge.
8. Wire the new tools into the chokepoint dispatch table.
9. Audit log changes: new `PythonBridgeInvoked` event kind, new
   `bridge: "python"` field on `ActionExecuted`.

**Exit criteria:** all tests green, all six capabilities smoke-tested
manually, chokepoint dispatches the new tools, audit log shows the new
event kind.

### Phase 2: The Tauri shell + audit browser (4β)

1. `app/` Tauri 2.x project skeleton. Build + run on a dev box.
2. `crates/ui/` removed (sub-plan 3's experimental Tauri shell is
   superseded by `app/`).
3. `+layout.svelte` with the disabled-nav stub.
4. `+page.svelte` redirecting to `/audit`.
5. `audit/+page.svelte` — virtual-scroll list, top filter chips,
   detail pane, hash-chain verify button, realtime tail.
6. `app/src-tauri/src/commands.rs` — `audit_query`,
   `audit_verify_chain`, `audit_event` (push).
7. Tauri config: CSP, no `localStorage` for secrets, process isolation,
   bundle the SvelteKit dist into the .deb's `/usr/lib/blackglass/blackglass-ui/`.

**Exit criteria:** `blackglass ui` launches the Tauri window, the
audit log view loads events, the realtime tail works, the hash-chain
verify button works, all 8 stub views are visibly disabled.

### Phase 3: Security (4γ)

1. `crates/polkit-helper/` binary + AppArmor profile + polkit policy +
   D-Bus config. Confined to `exec /usr/bin/blackglass-core` only.
2. AppArmor profile for the core (the §2.3 profile). Manually
   generated; the `xtask apparmor-generate` command produces a draft
   but the canonical profile is checked in.
3. Udev rule for the Flipper (§2.4). `udevadm` test in the postinst.
4. `cargo xtask confinement-test` — the CI step that proves the
   AppArmor profile actually confines on a fresh ubuntu-24.04 runner.

**Exit criteria:** the confinement-test passes in CI on a fresh
ubuntu-24.04 runner, the polkit helper successfully starts the core
from a non-root user's session, the Flipper is accessible to the
operator's user.

### Phase 4: Packaging (4γ continued)

1. `packaging/debian/` directory with `control`, `rules`, `changelog`,
   `postinst`, `prerm`, `conffiles`, etc.
2. `packaging/deb/cargo-deb.toml` with per-binary deb config.
3. `xtask deb` subcommand: builds all three .debs.
4. `xtask sign` subcommand: cosign sign-blob with keyless OIDC.
5. `packaging/install.sh` with the four-step verify-and-install flow.
6. `packaging/installer/*.sh` (detect-distro, verify-cosign, apt-install).
7. `.github/workflows/release.yml` with the build → sign → publish steps.

**Exit criteria:** the .deb builds locally, installs cleanly on
ubuntu-24.04, the install script downloads + verifies + installs the
.deb on a fresh box, the release pipeline publishes a signed .deb on
a `v0.1.0-rc1` tag.

### Phase 5: Polish

1. The rich postinst message, the desktop entry, the icon, the manpages.
2. The lintian overrides for the warnings we intentionally accept.
3. The `xtask verify-install` command (an operator-friendly "is my
   install healthy?" check).
4. The CHANGELOG and release notes template.

## 7. ADRs

Three new ADRs (in `docs/superpowers/adrs/`):

- **0013-pyo3-gil-pattern.md** — the GIL-acquire / spawn_blocking /
  GIL-release pattern for the Python bridge. Records why we use
  `spawn_blocking` even though pyo3 can release the GIL around blocking
  calls (consistency + Tokio-safety).
- **0014-deepfake-secondary-sidecar.md** — the decision to run the
  deepfake detection model as a secondary sidecar process (separate
  venv, separate service, REST contract on `localhost:8511`) rather
  than as a library in the main Python venv. Records the disk-weight
  tradeoff (~800 MB PyTorch vs. one more process to manage).
- **0015-deb-tiers-and-cosign-tofu.md** — the decision to ship three
  meta-packages (`minimal`/`core`/`full`) and to use cosign keyless
  signing for the install flow. Records the TOFU model (URL → script
  → signature → apt-pinned key) and the deliberate v1 scope
  (Ubuntu 24.04 + Kali only).

## 8. Tests added (summary)

- `crates/python-bridge/tests/`: 6+ unit tests for argument marshalling,
  1 integration test per capability (24 total: scapy 1, impacket 5,
  flipper 4, evilginx 5, gophish 4, detect 3) + 2 timeout tests.
- `crates/mcp-{ad,flipper,phish,detect}/tests/`: 2-3 dispatch tests
  each (tool not found, args invalid, args valid → Python bridge
  invoked).
- `crates/polkit-helper/tests/`: 3 tests (caller in group, caller not
  in group, requested command is the core binary).
- `crates/core/tests/`: 1 end-to-end test per new Python capability
  (the chokepoint → Python bridge → audit log round-trip).
- `app/src-tauri/tests/`: 2 integration tests for the Tauri command
  layer (`audit_query` happy path, `audit_verify_chain` happy path).
- `app/tests/`: 1 Playwright test for the audit browser (open the
  view, click "Verify chain", assert the green check appears).
- `xtask/src/bin/confinement_test.rs`: 5+ tests (AppArmor profile
  loads, profile blocks `/etc/shadow` read, profile allows the
  expected paths, polkit helper execs the right binary, Flipper
  device is accessible).
- `packaging/debian/tests/`: 2 lintian-runs-cleanly tests.

Total: ~61 new tests across the stack. Brings the v1 test count to
**~155** (60 existing Rust + 6 existing Svelte + 25 Gate 3 tests from
sub-plan 3 + 61 new Rust + 1 new Svelte + 2 packaging).

## 9. What this sub-plan does NOT change

- The .deb build tool is `cargo-deb` (per spec §7.2). No change.
- The .deb format is `debhelper-compat (= 13)` (per spec §7.2). No change.
- The architecture is `amd64` only in v1 (per spec §7.2). No change.
- The release channel is GitHub Releases only (per spec §7.1). No
  Kali repo in v1; no PPA.
- The audit log format is the same JSONL + hash chain as in
  sub-plans 1-3. New event kinds are additive.
- The existing MCP servers (`mcp-osint`, `mcp-packets`) are unchanged.
  The `packets-scapy_craft` tool moves from being a stub to going
  through the Python bridge; that's the only behavioral change to
  an existing tool.

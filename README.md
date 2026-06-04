# blackglass

A local-first, audit-logged security tool platform.

Every upstream pentest tool goes through a chokepoint that writes to a
tamper-evident, hash-chained audit log. The Tauri desktop app is the only
UI. The Python sidecar handles tools that need raw sockets. AppArmor
confinement + user-systemd for the core (no root daemon) + udev rules for
the Flipper.

## Quickstart (Ubuntu 24.04, Kali, Debian 12+)

```bash
# 1. Install from a GitHub release
curl -sSfL https://raw.githubusercontent.com/ankurCES/blackglass/master/packaging/install.sh | sudo bash -s -- --full
# (see "Install script" below for what --full actually does)

# 2. Log out and back in — this puts you in the `udev` group so the
#    Flipper works without sudo.

# 3. First launch
blackglass-core --init    # one-time: write ~/.local/share/blackglass/operator.token
blackglass ui             # opens the Tauri desktop app
```

The audit log is at `~/.local/share/blackglass/audit/audit.jsonl`.

> The `curl | sudo bash` pattern is by design. `packaging/install.sh`
> is ~100 lines of bash, browsable on GitHub. It downloads the matching
> `.deb` from the GitHub release pinned by SHA-256, hands off to `apt`,
> and configures the user-systemd services. It does not run anything
> outside of what `apt` would have done; it is a thin wrapper.
>
> If the release artifacts are not yet published, the script exits
> with a 404 and a link to the build-from-source recipe below.

## Install script

`packaging/install.sh` is a 100-line bash file. It:

1. Detects the distro (`/etc/os-release`) and refuses to run on
   anything not derived from Debian.
2. Downloads the latest release's `blackglass_*_amd64.deb` and the
   pinned `SHA256SUMS` from the GitHub release.
3. Verifies the `.deb` against `SHA256SUMS`.
4. Installs via `apt install -y ./blackglass_*.deb`.
5. **404 fallback** — if the release has not been published yet
   (common in dev), it prints a link to the build-from-source
   recipe below and exits non-zero.

Override the distro check with `--ubuntu`, `--kali`, or `--debian`.

## Build from source

The .deb is the only official install path, but until the first
release is cut, you can build it from a clean checkout:

### Prereqs (Debian / Ubuntu 24.04+ / Kali rolling)

```bash
sudo apt install -y \
  build-essential pkg-config libssl-dev \
  cargo rustc nodejs npm pnpm \
  libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  python3 python3-venv python3-pip \
  libpcap-dev tshark nmap \
  apparmor-utils udev sudo
sudo usermod -aG udev "$USER"   # log out and back in
```

### Build

```bash
git clone https://github.com/ankurCES/blackglass
cd blackglass
cargo build --workspace
( cd app && pnpm install && pnpm build )
cargo run -p xtask -- deb --variants full
```

The .deb lands at `target/debian/blackglass_*_amd64.deb`.

### Install the local .deb

```bash
sudo apt install -y ./target/debian/blackglass_*_amd64.deb
```

The .deb's postinst enables the user-systemd services and starts
the core. Verify with:

```bash
systemctl --user status blackglass-core
sudo -E cargo run -p xtask -- verify-install
```

## First launch walkthrough

1. **Initialize the operator token** (one-time, per operator):
   ```bash
   blackglass-core --init
   ```
   This writes `~/.local/share/blackglass/operator.token` (mode 0600)
   and prints the public half. The Tauri app reads it on start.

2. **Start the core** (if the .deb postinst hasn't already):
   ```bash
   systemctl --user enable --now blackglass-core
   systemctl --user enable --now blackglass-secondary-sidecar
   systemctl --user status blackglass-core
   ```
   You should see `Active: active (running)`. If `apparmor="DENIED"`
   lines appear, the AppArmor profile isn't loaded — run:
   ```bash
   sudo apparmor_parser -r /etc/apparmor.d/blackglass-core \
     /etc/apparmor.d/blackglass-secondary-sidecar
   ```

3. **Configure the MCPs** (one-time):
   ```bash
   cp /etc/blackglass/mcp-servers.toml.example \
      ~/.config/blackglass/mcp-servers.toml
   systemctl --user restart blackglass-core
   ```
   The 4 supervised MCPs (`mcp-ad`, `mcp-flipper`, `mcp-phish`,
   `mcp-detect`) come up. Verify with:
   ```bash
   pgrep -fa blackglass-mcp
   ```

4. **Initialize your first profile** (operator scope):
   ```bash
   blackglass profile init
   blackglass engagement init
   ```
   Edit `~/.config/blackglass/profile.toml` to whitelist the
   domains you want (`osint`, `packets`, etc.).

5. **Open the UI**:
   ```bash
   blackglass ui
   ```
   The 3-pane layout (DomainRail | ToolRunner | ResultPane) and
   the AuditLog rail are visible. Every tool run gets written to
   the hash-chained audit log.

6. **Verify the install**:
   ```bash
   cargo run -p xtask -- verify-install
   ```
   All 11 checks should pass.

7. **Verify the audit chain** (any time):
   ```bash
   blackglass audit verify
   ```

## Status

| Sub-plan | Status | What it ships |
|---|---|---|
| 1 — spine | ✅ complete | cargo workspace, `blackglass-{audit,profile,engagement,ipc,core,cli,runtime}`, 4-gate chokepoint, hash-chained audit, JSON-RPC over Unix socket, CLI |
| 2 — Gate 4 + mcp-{osint,packets} | ✅ complete | prompt-injection sanitizer wired into the chokepoint, `osint-{whois,dig}`, `packets-{tshark_read,tshark_capture,pcap_export,scapy_craft_stub}` |
| 3 — Gate 3 + operator server | ✅ complete | operator confirmation chokepoint wired end-to-end |
| 4 — desktop + sidecar + .deb | ✅ complete | Tauri UI (3-pane + audit rail), 4 new MCP server crates, Python sidecar (scapy/impacket/hardware/detect), deepfake secondary sidecar, AppArmor profiles, user-systemd .deb (no polkit, no cosign), 230 tests passing |

## Architecture

```
┌────────────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  mcp-{osint,packets,   │    │  blackglass-core │    │  Python sidecar  │
│   ad,flipper,phish,    ├───►│  (Rust, the gate)├────►│  (scapy, impacket│
│   detect}              │    │  runs as user-   │    │   pyflipper,     │
│  6 thin MCP clients    │    │  systemd service │    │   gophish,       │
└────────────────────────┘    │  at ~/.local/    │    │   detect_bridge) │
        │                     │  share/blackglass│    └────────┬─────────┘
        │                     └──────────────────┘             │ loopback HTTP
        │                       │                             ▼
        │                       │                  ┌──────────────────┐
        │              ┌──────────────────┐         │  secondary side- │
        │              │  audit chain     │         │  car (FastAPI    │
        │              │  (JSONL+blake3)  │         │   placeholder)   │
        │              └──────────────────┘         └──────────────────┘
        │                       │
        │                       ▼
        │              ┌──────────────────┐
        │              │  Tauri UI        │
        │              │  (audit browser) │
        │              └──────────────────┘
        │
        └──────────────► nmap, tshark, ... ◄───────────┘
                        (upstream tool binaries)
```

All operator state lives under `~/.local/share/blackglass/`:

```
~/.local/share/blackglass/
  operator.token          # 0600; one per operator
  runtime.sock            # core ↔ operator IPC
  audit/
    audit.jsonl           # hash-chained log
    audit.index           # offsets
  evidence/<id>/          # per-action captures
  logs/                   # rotated
```

### The four gates

1. **Gate 1 — profile.** Profile lists allowed domains (e.g., `core`, `osint`, `packets`, `audit`) and allowed action classes (e.g., `read_only`, `destructive`). Reject before the request gets a footprint.
2. **Gate 2 — engagement scope.** Engagement file (TOML) lists allowed targets as IP / CIDR / hostname. Reject anything out of scope. This is the chokepoint that stops an LLM-driven agent from scanning the wrong network.
3. **Gate 3 — operator confirmation.** "This action is destructive — confirm in the UI" prompts. The operator_server binary surfaces the prompt over a Unix socket; the Tauri UI consumes it.
4. **Gate 4 — output sanitization.** Downstream tool output passes through PI-strip, length-truncation, and wrap-with-delimiters. Every redacted line is logged as `PromptInjectionSuspected` evidence.

Every gate decision is written to the hash-chained audit log **before** the request continues. Tampering with the log invalidates the chain.

## Development

```bash
# Build everything
cargo build --workspace
( cd app && pnpm install && pnpm build )

# Run the test suite
cargo test --workspace
( cd app && pnpm test )

# Build a .deb
cargo run -p xtask -- deb --variants full

# Verify a fresh install meets the security prerequisites
sudo cargo run -p xtask -- verify-install

# Run the confinement test
sudo cargo run -p xtask -- confinement-test
```

## Layout

```
crates/
  audit/                # blake3 hash-chained JSONL log + Chain::verify
  profile/              # TOML profile + Gate 1 helpers
  engagement/           # TOML engagement + Gate 2 allowlist (IP/CIDR/hostname)
  ipc/                  # 4-byte-BE length-prefixed JSON-RPC codec
  core/                 # chokepoint, gates, RPC dispatch, unix-socket server
  cli/                  # init | ping | audit-verify
  runtime/              # GateClient — async auth + execute_action over Unix socket
  mcp-osint/            # osint-whois, osint-dig
  mcp-packets/          # tshark_read, tshark_capture, pcap_export, scapy_craft
  mcp-ad/               # 5 ad-* tools (impacket psexec/wmiexec/secretsdump/kerberoast/asreproast)
  mcp-flipper/          # flipper-{list,read,write,run}
  mcp-phish/            # phish-* (evilginx2 + gophish)
  mcp-detect/           # detect-{image,video,batch} (routes to secondary sidecar)
  python-bridge/        # pyo3-gated trait + stub for sidecar calls
  secondary-sidecar/    # launcher for the FastAPI deepfake placeholder
  xtask/                # build orchestrator: build, deb, verify-install, confinement-test, apparmor-generate
python/
  sidecar/              # blackglass_sidecar Python package (scapy, impacket, hardware, detect bridges)
  secondary-sidecar/    # blackglass_secondary FastAPI package (deepfake placeholder)
packaging/
  debian/               # control, rules, .desktop, postinst, prerm, tests/
  apparmor/             # profiles for core + secondary-sidecar
  udev/                 # 99-blackglass-flipper.rules
  systemd/user/         # blackglass-core.service + blackglass-secondary-sidecar.service
  install.sh            # one-line installer (100 lines, browsable)
  installer/            # detect-distro, verify-sha256, apt-install helpers
scripts/
  smoke-test.sh         # install-time smoke test
```

## Security

Read `docs/specs/2026-06-03-blackglass-design.md` for the full design
(threat model, kill-switch list, secure-update mechanism, the four gates,
the audit chain format, the IPC wire protocol). The 15 ADRs in
`docs/decisions/` capture the per-decision rationale (two-socket IPC,
pyo3 GIL pattern, secondary sidecar, deb tiers, etc.).

There is no privileged daemon. The core runs as a *user*-systemd
service under your uid. AppArmor confines what the core can touch;
the Python sidecar is in a separate profile; udev grants the Flipper
without `sudo`. There is no `polkit`, no `setuid` helper, no
`/var/lib/blackglass`.

The install is integrity-checked at install time by SHA-256 against
a pinned `SHA256SUMS` file in the GitHub release. Future versions
will add a detached cosign signature (see ADR-0015).

## License

MIT.

This is offensive-security software. Read the spec before you run it.

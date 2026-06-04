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

| Sub-plan | Status | What it ships |
|---|---|---|
| 1 — spine | ✅ complete | cargo workspace, `blackglass-{audit,profile,engagement,ipc,core,cli,runtime}`, 4-gate chokepoint, hash-chained audit, JSON-RPC over Unix socket, CLI |
| 2 — Gate 4 + mcp-{osint,packets} | ✅ complete | prompt-injection sanitizer wired into the chokepoint, `osint-{whois,dig}`, `packets-{tshark_read,tshark_capture,pcap_export,scapy_craft_stub}` |
| 3 — Gate 3 + operator server | ✅ complete | operator confirmation chokepoint wired end-to-end; 66 tests passing |
| 4 — desktop + sidecar + .deb | ✅ complete | Tauri UI foundation, Python sidecar scaffold, AppArmor profiles, polkit helper, cosign-signed .deb pipeline, 90 tests passing |

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
cd app && npm install && npm run build

# Run the test suite
cargo test --workspace

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
  mcp-packets/          # tshark_read, tshark_capture, pcap_export, scapy_craft_stub
  python-bridge/        # pyo3-gated trait for scapy/impacket/pyflipper calls
  polkit-helper/        # root-only exec shim that re-checks every polkit grant
  xtask/                # build orchestrator: build, deb, sign, verify-install, confinement-test
packaging/
  debian/               # control, rules, .desktop, postinst, prerm
  apparmor/             # profiles for core + polkit-helper
  polkit/               # com.blackglass.start-core policy
  udev/                 # 99-blackglass-flipper.rules
  cosign/               # pinned public-key for curl|sh install
  install.sh            # one-line installer
  installer/            # detect-distro, verify-cosign, apt-install helpers
scripts/
  smoke-test.sh         # 7-criterion install smoke test
```

## Security

Read `docs/security.md` for the threat model, the kill-switch list, and
the secure-update mechanism. Read `docs/spec.md` for the full design.

The install flow uses cosign keyless signing (OIDC, tied to the
release GitHub Actions workflow). The pinned public-key fingerprint
lives at `packaging/cosign/cosign.pub`.

## License

MIT.

This is offensive-security software. Read the spec before you run it.

# blackglass

A local-first, audit-logged security tool platform.

Every upstream pentest tool goes through a chokepoint that writes to a
tamper-evident, hash-chained audit log. The Tauri desktop app is the only
UI. The Python sidecar handles tools that need raw sockets. AppArmor
confinement + polkit privilege drop + udev rules for the Flipper.

## Quickstart

```bash
# 1. Install (Ubuntu 24.04, Kali, Debian 12+)
curl -sSfL https://raw.githubusercontent.com/ankurCES/blackglass/master/packaging/install.sh | sudo bash -s -- --full

# 2. Initialize your first profile
blackglass profile init

# 3. Launch the UI
blackglass ui
```

That's it. The audit log is at `~/.local/share/blackglass/audit/audit.jsonl`.

> **Note:** the `curl | sudo bash` pattern is by design — the installer is
> a 100-line bash file that lives in the repo (`packaging/install.sh`)
> and is browsable on GitHub before you run it. It downloads the cosign
> binary, fetches the release metadata, verifies the .deb with cosign
> keyless signing, and then hands off to apt. There is no `--insecure`
> flag; the install refuses to proceed if cosign verification fails.

## Status

| Sub-plan | Status | What it ships |
|---|---|---|
| 1 — spine | ✅ complete | cargo workspace, `blackglass-{audit,profile,engagement,ipc,core,cli,runtime}`, 4-gate chokepoint, hash-chained audit, JSON-RPC over Unix socket, CLI |
| 2 — Gate 4 + mcp-{osint,packets} | ✅ complete | prompt-injection sanitizer wired into the chokepoint, `osint-{whois,dig}`, `packets-{tshark_read,tshark_capture,pcap_export,scapy_craft_stub}` |
| 3 — Gate 3 + operator server | ✅ complete | operator confirmation chokepoint wired end-to-end; 66 tests passing |
| 4 — desktop + sidecar + .deb | ✅ complete | Tauri UI foundation, 4 new MCP server crates (`mcp-ad`, `mcp-flipper`, `mcp-phish`, `mcp-detect`), Python sidecar with scapy/impacket/hardware/detect bridges, deepfake secondary sidecar (FastAPI placeholder), AppArmor profiles, polkit helper, cosign-signed .deb pipeline, 134 tests passing |

## Architecture

```
┌────────────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  mcp-{osint,packets,   │    │  blackglass-core │    │  Python sidecar  │
│   ad,flipper,phish,    ├───►│  (Rust, the gate)├────►│  (scapy, impacket│
│   detect}              │    │                  │    │   pyflipper,     │
│  6 thin MCP clients    │    │                  │    │   gophish,       │
└────────────────────────┘    └──────────────────┘    │   detect_bridge) │
        │                       │                      └────────┬─────────┘
        │                       │                               │ loopback HTTP
        │                       │                               ▼
        │              ┌──────────────────┐         ┌──────────────────┐
        │              │  audit chain     │         │  secondary side- │
        │              │  (JSONL+blake3)  │         │  car (FastAPI    │
        │              └──────────────────┘         │   placeholder)   │
        │                       │                  └──────────────────┘
        │                       ▼
        │              ┌──────────────────┐
        │              │  Tauri UI        │
        │              │  (audit browser) │
        │              └──────────────────┘
        │
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
  mcp-packets/          # tshark_read, tshark_capture, pcap_export, scapy_craft
  mcp-ad/               # 5 ad-* tools (impacket psexec/wmiexec/secretsdump/kerberoast/asreproast)
  mcp-flipper/          # flipper-{list,read,write,run}
  mcp-phish/            # phish-* (evilginx2 + gophish)
  mcp-detect/           # detect-{image,video,batch} (routes to secondary sidecar)
  python-bridge/        # pyo3-gated trait + stub for sidecar calls
  polkit-helper/        # root-only exec shim that re-checks every polkit grant
  secondary-sidecar/    # launcher for the FastAPI deepfake placeholder
  xtask/                # build orchestrator: build, deb, sign, verify-install, confinement-test
python/
  sidecar/              # blackglass_sidecar Python package (scapy, impacket, hardware, detect bridges)
  secondary-sidecar/    # blackglass_secondary FastAPI package (deepfake placeholder)
packaging/
  debian/               # control, rules, .desktop, postinst, prerm
  apparmor/             # profiles for core + polkit-helper + sidecars
  polkit/               # com.blackglass.start-core policy
  udev/                 # 99-blackglass-flipper.rules
  cosign/               # pinned public-key for curl|sh install
  install.sh            # one-line installer (100 lines, browsable)
  installer/            # detect-distro, verify-cosign, apt-install helpers
scripts/
  smoke-test.sh         # 7-criterion install smoke test
```

## Security

Read `docs/specs/2026-06-03-blackglass-design.md` for the full design
(threat model, kill-switch list, secure-update mechanism, the four gates,
the audit chain format, the IPC wire protocol). The 15 ADRs in
`docs/decisions/` capture the per-decision rationale (cosign keyless
signing, two-socket IPC, pyo3 GIL pattern, secondary sidecar, deb tiers,
etc.).

The install flow uses cosign keyless signing (OIDC, tied to the release
GitHub Actions workflow). The pinned public-key fingerprint lives at
`packaging/cosign/cosign.pub`.

## License

MIT.

This is offensive-security software. Read the spec before you run it.

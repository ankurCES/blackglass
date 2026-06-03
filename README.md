# blackglass

Linux-native, AI-invokable, audit-first red-team platform.

A single authorized operator gets a unified desktop UI and a guarded Model
Context Protocol (MCP) surface over the modern offensive-security toolchain,
with tiered capability profiles so the public release can be widely distributed
without it becoming a script-kiddie weapon.

## Status

**Sub-plans 1 (spine) and 2 (Gate 4 + first MCP servers) are complete and green.**

| Sub-plan | Status | What it ships |
|---|---|---|
| 1 — spine | ✅ complete | cargo workspace, `blackglass-{audit,profile,engagement,ipc,core,cli,runtime}`, 4-gate chokepoint, hash-chained audit, JSON-RPC over Unix socket, CLI |
| 2 — Gate 4 + mcp-{osint,packets} | ✅ complete | prompt-injection sanitizer wired into the chokepoint, `osint-{whois,dig}`, `packets-{tshark_read,tshark_capture,pcap_export,scapy_craft_stub}` |
| 3 | ⏳ next | TBD |
| 4 | ⏳ | TBD |

## Architecture

```
                                    ┌──────────────────┐
                                    │   operator UI    │
                                    │  (Tauri, future) │
                                    └────────┬─────────┘
                                             │ JSON-RPC over Unix socket
                                             │ (auth → execute_action)
                                             ▼
┌──────────────────────────────────────────────────────────────┐
│                          blackglass-core                     │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  chokepoint:  execute_action(req)                       │ │
│  │     ├─ Gate 1:  profile.allowed_domain × action.domain   │ │
│  │     ├─ Gate 2:  engagement.target ∈ allowlist            │ │
│  │     ├─ Gate 3:  operator confirmation (stub AllowAll)    │ │
│  │     └─ Gate 4:  sanitize downstream output (PI-strip)    │ │
│  │            ↓                                             │ │
│  │     simulate_execute  (stub: real tools in sub-plan 4)   │ │
│  └─────────────────────────────────────────────────────────┘ │
│             ↓ ↓ ↓                                            │
│  ┌────────────────────┐    every action emits 3 events:     │
│  │  blackglass-audit  │ ◀──── ActionRequested               │
│  │  blake3 hash-chain │       ActionAllowed | ActionDenied  │
│  │  tamper-evident    │       ActionExecuted                │
│  └────────────────────┘                                       │
└──────────────────────────────────────────────────────────────┘
                                             ▲
                                             │ JSON-RPC (auth-gated)
                                             │
                ┌────────────────────────────┴────────────────────────┐
                │                                                     │
        ┌───────┴────────┐                                   ┌────────┴───────┐
        │ blackglass-mcp │                                   │  blackglass-   │
        │     -osint     │                                   │    packets     │
        │ whois, dig     │                                   │ tshark, pcap   │
        └────────────────┘                                   └────────────────┘
```

## The four gates

1. **Gate 1 — profile.** Profile lists allowed domains (e.g., `core`, `osint`, `packets`, `audit`) and allowed action classes (e.g., `read_only`, `destructive`). Reject before the request gets a footprint.
2. **Gate 2 — engagement scope.** Engagement file (TOML) lists allowed targets as IP / CIDR / hostname. Reject anything out of scope. This is the chokepoint that stops an LLM-driven agent from scanning the wrong network.
3. **Gate 3 — operator confirmation.** Reserved for "this action is destructive — confirm in the UI" prompts. Stubbed as `AllowAll` in sub-plan 1.
4. **Gate 4 — output sanitization.** Downstream tool output passes through PI-strip, length-truncation, and wrap-with-delimiters. Every redacted line is logged as `PromptInjectionSuspected` evidence.

Every gate decision is written to the hash-chained audit log **before** the request continues. Tampering with the log invalidates the chain.

## Build & test

```sh
rustup show                           # 1.95 per rust-toolchain.toml
cargo test --workspace                # 47 passed, 0 failed, 1 ignored (live tshark)
cargo clippy --workspace --all-targets -- -D warnings
```

## Layout

```
crates/
  audit/         # blake3 hash-chained JSONL log + Chain::verify
  profile/       # TOML profile + Gate 1 helpers
  engagement/    # TOML engagement + Gate 2 allowlist (IP/CIDR/hostname)
  ipc/           # 4-byte-BE length-prefixed JSON-RPC codec
  core/          # chokepoint, gates, RPC dispatch, unix-socket server
  cli/           # init | ping | audit-verify
  runtime/       # GateClient — async auth + execute_action over Unix socket
  mcp-osint/     # osint-whois, osint-dig
  mcp-packets/   # tshark_read, tshark_capture, pcap_export, scapy_craft_stub
docs/
  specs/         # design spec
  plans/         # sub-plan 1 implementation plan
```

## Security

See `docs/specs/2026-06-03-blackglass-design.md` for the full threat model.
The local socket auth model (ADR 0004) is "first RPC on every connection must
be `auth` carrying a 32-byte token; server refuses all other methods until
auth succeeds." The token lives in a 0600 file generated by `blackglass init`.

This is offensive-security software. Read the spec before you run it.

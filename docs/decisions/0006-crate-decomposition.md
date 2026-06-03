# ADR 0006: Crate decomposition

- Status: Accepted (sub-plan 1). Amended by sub-plan 2 — see "Amendments" below.
- Context: A single `blackglass-core` crate mixing audit, profile, engagement, IPC, RPC, gates, server, and CLI would be a 3000-line monster by sub-plan 4. Per the writing-plans skill: "Files that change together should live together. Split by responsibility, not by technical layer."
- Decision: Six crates, one job each:

  | Crate | Job | Depends on |
  |---|---|---|
  | `blackglass-audit` | Hash-chained JSONL log; `Chain::append`, `Chain::verify`. | serde, blake3, hex, thiserror |
  | `blackglass-profile` | TOML profile loader; Gate 1 helpers. | serde, toml, thiserror |
  | `blackglass-engagement` | Engagement model; target allowlist (Gate 2). | serde, ipnetwork, thiserror |
  | `blackglass-ipc` | Length-prefixed frame codec; `Request`/`Response` types. | tokio, serde, serde_json, thiserror |
  | `blackglass-core` | The chokepoint, gates, RPC, Unix-socket server, `blackglass-core` binary. | audit, profile, engagement, ipc, tokio, tracing, clap |
  | `blackglass-cli` | The `blackglass` binary: `init`, `start`, `status`, `audit verify`, etc. | ipc, core (re-exports), clap |

- Consequences:
  - Each crate is small enough to hold in context and to TDD in isolation.
  - `audit`, `profile`, `engagement`, `ipc` are pure-Rust, no `tokio` runtime required (except `ipc` for the socket test) — fast to test.
  - The workspace forces explicit dependency edges; "I just imported `core` from `audit`" is a compile error.
- Alternatives: One crate with modules (rejected: spec calls for ≥4 distinct binaries over time; one crate makes that painful), more crates (rejected: premature).

## Amendments

### Sub-plan 2 (2026-06-03) — added 3 crates

Sub-plan 2 (`docs/plans/2026-06-03-blackglass-osint-packets.md`) added the
following crates on top of the original six. ADR 0006 is **not** reversed —
the principle "one crate, one job" still holds — but the table above is
incomplete.

| Crate | Job | Depends on |
|---|---|---|
| `blackglass-runtime` | `GateClient` — async auth + `execute_action` over the Unix socket, used by every MCP server. | ipc, audit, tokio |
| `blackglass-mcp-osint` | MCP server: `osint-whois`, `osint-dig`. | runtime, rmcp |
| `blackglass-mcp-packets` | MCP server: `packets-tshark_read`, `packets-tshark_capture`, `packets-pcap_export`, `packets-scapy_craft` (stub). | runtime, rmcp |

Total workspace crates: 9.

### Future sub-plans

Sub-plan 3 (TBD) is expected to add at least one more crate (the Tauri
desktop shell, or a `mcp-core` crate, depending on direction chosen). Each
new crate will land with an amendment to this ADR listing it.

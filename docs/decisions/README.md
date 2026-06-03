# Architecture Decision Records

Six ADRs were pinned at the start of sub-plan 1 (see
[`docs/plans/2026-06-03-blackglass-spine.md`](../plans/2026-06-03-blackglass-spine.md))
and were the source of the deferred decisions in the design spec's
"Open questions deferred to implementation" section.

| ADR | Decision |
|---|---|
| [0001](0001-ipc-unix-socket.md) | IPC = Unix domain socket, length-prefixed JSON-RPC. |
| [0002](0002-audit-chain-blake3.md) | Audit chain hash = blake3. |
| [0003](0003-profile-format-toml.md) | Profile format = TOML. |
| [0004](0004-socket-auth-token.md) | Local socket auth = 0600 token file, per-connection auth RPC. |
| [0005](0005-scope-of-subplan-1.md) | Sub-plan 1 scope (the spine, analyst-only). |
| [0006](0006-crate-decomposition.md) | Crate decomposition (originally 6 crates; amended after sub-plan 2). |

New decisions get a new ADR file. Do not edit an existing ADR to reverse a
decision — write a new one that supersedes it.

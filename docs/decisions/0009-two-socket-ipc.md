# ADR 0009: Two-socket IPC (operator + runtime)

- Status: Accepted (sub-plan 3)
- Context: spec §2.4 says "all three tiers talk the same JSON-RPC 2.0 dialect over Unix domain sockets at `~/.local/share/blackglass/runtime.sock`." Sub-plan 3 needs the Tauri app to receive server-pushed events and to be distinguishable from MCP servers.
- Decision: Two sockets. `runtime.sock` (existing) for agents. New `operator.sock` for the human UI. Both use the same JSON-RPC dialect; the operator socket additionally carries server-pushed events.
- Consequences: presence is implicit (operator socket open = Tauri up). MCP servers never see `confirm.request`. Spec §2.4 amendment.
- Alternatives: single socket + broadcast (rejected: MCP flooded, "which connection is operator" implicit), pub-sub (rejected: extra moving parts).

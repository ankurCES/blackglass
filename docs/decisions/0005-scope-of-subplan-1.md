# ADR 0005: Sub-plan 1 scope

- Status: Accepted
- Context: spec §1.3 / §1.4 / §5 collectively cover a 13-domain, 47-tool, 3-profile, AppArmor+polkit+udev+cosign-platform. That is many months of work. The spec was not broken into sub-specs; we are decomposing it into sub-plans.
- Decision: Sub-plan 1 ("the spine") ships ONLY the following:

  **In scope:**
  - Cargo workspace bootstrap.
  - `blackglass-audit` crate: hash-chained JSONL log with blake3, `Chain::append`, `Chain::verify`.
  - `blackglass-profile` crate: TOML loader for `analyst` profile; Gate 1 helpers (`allows_domain`, `allows_action_class`).
  - `blackglass-engagement` crate: engagement model, IP / CIDR / hostname target allowlist (Gate 2).
  - `blackglass-ipc` crate: length-prefixed frame codec + JSON-RPC `Request` / `Response` types; loopback Unix-socket smoke test.
  - `blackglass-core` crate: the chokepoint (`execute_action`) that runs Gates 1 → 2 → 3(stub) → 4(stub), with a simulated downstream; Gate 3 / Gate 4 are `AllowAll` trait implementations.
  - `blackglass-cli` crate: `init`, `start`, `status`, `audit verify` subcommands (CLI itself is a thin client over the socket).
  - Hash-chained audit log end-to-end test that runs in `cargo test`.

  **Out of scope (deferred):**
  - Any real upstream tool (nmap, nuclei, hydra, impacket, evilginx2, gophish, scapy, theharvester, subfinder, dig, whois, tcpdump, tshark, …).
  - Any MCP server (`mcp-core`, `mcp-osint`, …) and the `rmcp` dependency.
  - Tauri desktop UI, Tauri webview, the Gate 3 confirmation modal.
  - Python sidecar (pyo3 binding, `pyFlipper`, Impacket helpers).
  - AppArmor profile, polkit policy, udev rules, `cosign`, `.deb` packaging.
  - `+operator` and `+redteam` build flags. Sub-plan 1 is `analyst`-only, hardcoded.
  - Engagement scope-window enforcement (start/end timestamps) — the field is parsed and stored but not enforced yet.
  - Audit log redaction (spec §8.5 risk 16) — Gate 4 is identity in sub-plan 1.

- Consequences:
  - Sub-plan 1 can be implemented, tested, and reviewed in days, not months.
  - Every future sub-plan has a verified chokepoint to build on.
  - The chokepoint test in Task 17 / 30 makes bypassing the core a test failure, not a code-review finding.

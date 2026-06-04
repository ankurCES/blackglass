# Changelog

All notable changes to blackglass are documented here.
The format is loosely [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- **Install model rewrite.** The core no longer runs as a root
  polkit-activated system service. It runs as a *user*-systemd
  service under the operator's uid. All operator state moved
  from `/var/{lib,run}/blackglass/` to
  `~/.local/share/blackglass/`. The `blackglass` group, the
  polkit-helper, the `cosign.pub`-pinned install path, and
  `/var/lib/blackglass/evidence/` are gone. See the **Migration**
  section of `docs/specs/2026-06-03-blackglass-design.md` if
  upgrading from a v0 install.

### Removed
- `polkit-helper` crate and runtime install.
- `cosign.pub` install asset + the cosign branch of `install.sh`.
- `adduser blackglass` from the .deb postinst.
- `debian/` deps: `libpolkit-gobject-1-dev`, `libpolkit-gobject-1-0`,
  `policykit-1` / `polkit`, `adduser`, `cosign`.
- `/var/lib/blackglass/evidence/`, `/var/run/blackglass/operator.sock`.
- `com.blackglass.start-core` polkit policy.
- `blackglass-polkit-helper` AppArmor profile.

### Added
- `packaging/systemd/user/blackglass-core.service` and
  `packaging/systemd/user/blackglass-secondary-sidecar.service`.
- `packaging/debian/postinst` symlinks the .service into the
  operator's `~/.config/systemd/user/` and `daemon-reload`s.
- `packaging/debian/prerm` stops and disables the user-systemd
  service.
- `packaging/debian/tests/postinst_smoke.sh` (best-effort, manual).
- `packaging/apparmor/blackglass-secondary-sidecar` profile
  (confines the deepfake-detector sidecar separately).
- `/etc/blackglass/mcp-servers.toml.example` (operator copies to
  `~/.config/blackglass/mcp-servers.toml` to enable supervised MCPs).
- `xtask verify-install` rewritten for the user-systemd model
  (11 checks: binaries, AppArmor, operator state/socket/token
  mode-0600, 2 user-systemd services, udev group + rules,
  mcp-servers.toml.example, MCP supervisor children, Python venv).
- `xtask apparmor-generate --secondary-sidecar` — produces the
  secondary-sidecar profile from a template (5 unit tests).
- `xtask confinement-test` extended: drops polkit-helper exec
  check, adds `aa-exec` probe asserting the sidecar cannot read
  `operator.token` or the audit dir (2 unit tests).
- README — `Build from source` recipe + `First launch walkthrough`
  replace the obsolete `Quickstart`.
- LICENSE (MIT) — packaged with the .deb.

### Security
- The operator token file (`~/.local/share/blackglass/operator.token`)
  is verified to be mode `0600` by `xtask verify-install`.
- The Python sidecar venv is verified to import the 5 bridge
  modules by `xtask verify-install` (a broken venv is caught
  here, not at tool-run time).
- The secondary-sidecar AppArmor profile denies reads of
  `~/.local/share/blackglass/audit/**` and `operator.token`.

## [0.1.0] — 2026-06-04 — Sub-plan 4 amendment

First end-to-end working release. The .deb builds (8.7 MB) and
ships 10 binaries, 2 user-systemd units, 2 AppArmor profiles, the
mcp-servers.toml example, the Python sidecar source, the Flipper
udev rule, and the desktop entry. No polkit. No /var/lib/blackglass.

### Highlights

- **4 gates wired end-to-end.** Gate 1 (profile), Gate 2
  (engagement scope), Gate 3 (operator confirmation via
  `blackglass-core` → Unix socket → Tauri UI), Gate 4 (PI-strip
  + length-truncation + delimiters). Every gate decision is
  written to the blake3 hash-chained audit log **before** the
  request continues.
- **6 MCP domains** (`mcp-osint`, `mcp-packets`, `mcp-ad`,
  `mcp-flipper`, `mcp-phish`, `mcp-detect`) — 27 tools total,
  each routed through the chokepoint.
- **Python sidecar** at `/usr/lib/blackglass/python-venv/` with
  scapy, impacket, pyflipper, gophish, and detect_bridge modules.
- **Secondary sidecar** (FastAPI placeholder) for deepfake
  detection. Routes `detect-{image,video,batch}` over loopback
  HTTP from the primary sidecar. Verdict is `unknown` until the
  model is wired up.
- **Tauri UI** — 3-pane layout (DomainRail | ToolRunner |
  ResultPane) + AuditLog rail + AuditDetail right rail.
- **231 tests** (175 Rust + 56 Svelte, 0 svelte-check errors).
- **AppArmor** — user-home core profile + secondary-sidecar
  profile, both deny access to `operator.token` and the audit
  dir from outside the core.

### Known limitations

- The MCP supervisor's per-MCP sub-chain is at
  `chain.jsonl` next to `audit.jsonl` — `blackglass audit verify`
  only checks the main chain. A `chain verify --all` is on the
  post-v0.1 roadmap.
- `cosign`-keyed install is deferred. v0.1 uses SHA-256 pinning
  against `SHA256SUMS` in the GitHub release. Future versions
  will add a detached cosign signature (see ADR-0015).
- The `mcp-packets::scapy_craft` tool is a stub — the real scapy
  call goes through the Python sidecar. The Rust stub exists for
  test parity only.
- The Tauri UI is read-only for audit events. Bulk export and
  filter presets are on the v0.2 roadmap.

[Unreleased]: https://github.com/ankurCES/blackglass/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ankurCES/blackglass/releases/tag/v0.1.0

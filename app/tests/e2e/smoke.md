# Tauri app end-to-end smoke test

Run this on a fresh Ubuntu 24.04 (or the user's modified Ubuntu) after
`sudo dpkg -i target/debian/*.deb` + `systemctl --user start blackglass-core`.

## Pre-flight

- [ ] `blackglass-core` is running: `systemctl --user status blackglass-core`
      → "active (running)"
- [ ] The operator socket exists: `ls -la ~/.local/share/blackglass/runtime.sock`
- [ ] The MCP supervisor spawned the 4 MCPs: `ls -la ~/.local/share/blackglass/logs/`
      → mcp-ad.log, mcp-flipper.log, mcp-phish.log, mcp-detect.log

## Test 1: launch the Tauri app

- [ ] `blackglass ui` (or click the desktop icon)
- [ ] Tauri window opens
- [ ] Left rail shows: osint, packets, ad, flipper, phish, detect
- [ ] Middle pane shows: "Select a domain from the left rail."
- [ ] Right-middle shows: "No result yet."

## Test 2: run a non-destructive tool (osint-whois)

- [ ] Click "osint" in the left rail
- [ ] Middle pane shows: osint-whois, osint-dig, osint-theharvester
- [ ] Click "Run" on osint-whois (with the default `{ "target": "example.com" }`)
- [ ] Right-middle pane shows the WHOIS output
- [ ] The "audit: <id>" link is clickable and opens AuditDetail in the right rail
- [ ] The audit log view shows the new event

## Test 3: run a destructive tool (ad-impacket_psexec)

- [ ] Click "ad" in the left rail
- [ ] Click "Run" on ad-impacket_psexec
- [ ] A confirm modal appears: "Run psexec on TARGET? [Allow] [Deny]"
- [ ] Click "Deny"
- [ ] Right-middle pane shows: "denied" (or "gate denied" or similar)
- [ ] The audit log view shows: ActionRequested, OperatorConfirmationRequested, OperatorConfirmationResolved (denied), then no ActionExecuted.

## Test 4: MCP-down handling

- [ ] `kill <mcp-ad pid>` (find it via `pgrep -f blackglass-mcp-ad`)
- [ ] Wait ~3s for the supervisor to detect the exit and restart
- [ ] In the Tauri app, click "Run" on ad-impacket_psexec
- [ ] Result is "ok" (because the supervisor restarted mcp-ad)
- [ ] The audit log shows: McpServerExited, McpServerSpawned (restart), then ActionExecuted

## Pass criteria

All 4 tests pass with no red error banners in the Tauri UI. The audit
log is intact (chain verifies). The user's modified Ubuntu boots
cleanly after a reboot (systemd --user is persistent).

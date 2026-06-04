#!/usr/bin/env bash
# smoke-test.sh — 7-criterion smoke test for a fresh install.
# Run as the operator (in the `blackglass` group).

set -euo pipefail
PASS=0
FAIL=0

pass() { echo "  ✓ $1"; PASS=$((PASS+1)); }
fail() { echo "  ✗ $1"; FAIL=$((FAIL+1)); }

# 1. The audit log is readable
echo "1. audit log readable"
AUDIT=~/.local/share/blackglass/audit/audit.jsonl
if [ -r "$AUDIT" ]; then pass "audit.jsonl readable"; else fail "audit.jsonl missing or unreadable"; fi

# 2. The core binary is in PATH
echo "2. core binary in PATH"
if command -v blackglass-core >/dev/null; then pass "blackglass-core in PATH"; else fail "blackglass-core not in PATH"; fi

# 3. The Tauri binary is in PATH
echo "3. Tauri binary in PATH"
if command -v blackglass >/dev/null; then pass "blackglass in PATH"; else fail "blackglass not in PATH"; fi

# 4. The polkit helper is installed
echo "4. polkit helper installed"
if [ -x /usr/libexec/blackglass-polkit-helper ]; then pass "polkit-helper installed"; else fail "polkit-helper missing"; fi

# 5. AppArmor profiles are loaded
echo "5. AppArmor profiles loaded"
if aa-status 2>/dev/null | grep -q blackglass-core; then pass "blackglass-core profile loaded"; else fail "blackglass-core profile NOT loaded"; fi
if aa-status 2>/dev/null | grep -q blackglass-polkit-helper; then pass "blackglass-polkit-helper profile loaded"; else fail "blackglass-polkit-helper profile NOT loaded"; fi

# 6. The Python venv exists and imports cleanly
echo "6. Python venv"
VENV=/usr/lib/blackglass/python-venv/bin/python
if [ -x "$VENV" ]; then
    if "$VENV" -c "import blackglass_sidecar.scapy_bridge, blackglass_sidecar.impacket_bridge, blackglass_sidecar.hardware_bridge, blackglass_sidecar.audit_types" 2>/dev/null; then
        pass "sidecar venv imports"
    else
        fail "sidecar venv import failed"
    fi
else
    fail "sidecar venv missing"
fi

# 7. A test run produces an audit event
echo "7. test run produces an audit event"
EVENT_BEFORE=$(wc -l < "$AUDIT" 2>/dev/null || echo 0)
# Run a known-bad op that should be denied and logged
blackglass core op osint_whois --target "127.0.0.1" --note "smoke-test" 2>/dev/null || true
EVENT_AFTER=$(wc -l < "$AUDIT" 2>/dev/null || echo 0)
if [ "$EVENT_AFTER" -gt "$EVENT_BEFORE" ]; then
    pass "test run produced audit event ($EVENT_BEFORE → $EVENT_AFTER)"
else
    fail "test run did NOT produce an audit event"
fi

echo ""
echo "Passed: $PASS / $((PASS+FAIL))"
[ "$FAIL" -eq 0 ]

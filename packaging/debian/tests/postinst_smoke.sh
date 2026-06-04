#!/bin/bash
# Best-effort postinst smoke test. Run after `dpkg -i`. Not in CI.
# This script checks that the user-systemd install actually started
# the core, that the operator socket exists, and that the AppArmor
# profiles are loaded.
#
# Usage:
#   sudo bash packaging/debian/tests/postinst_smoke.sh
#
# Exit code 0 = all checks pass; non-zero = at least one check failed.

set -e

echo "1. Checking user-systemd service..."
if ! systemctl --user is-active blackglass-core.service >/dev/null 2>&1; then
    echo "FAIL: blackglass-core.service is not active"
    echo "  hint: systemctl --user status blackglass-core"
    exit 1
fi
echo "  ✓ blackglass-core is running"

echo "2. Checking operator socket..."
SOCK="${HOME}/.local/share/blackglass/runtime.sock"
if [ ! -S "$SOCK" ]; then
    echo "FAIL: $SOCK not found"
    exit 1
fi
echo "  ✓ operator socket exists"

echo "3. Checking AppArmor profiles..."
if ! command -v apparmor_status >/dev/null 2>&1; then
    echo "  · apparmor_status not available; skipping"
else
    if ! apparmor_status 2>/dev/null | grep -qE "blackglass-(core|secondary-sidecar)"; then
        echo "FAIL: AppArmor profiles not loaded"
        echo "  hint: sudo apparmor_parser -r /etc/apparmor.d/blackglass-core"
        exit 1
    fi
    echo "  ✓ AppArmor profiles loaded"
fi

echo "4. Checking mcp-servers.toml.example is installed..."
if [ ! -f /etc/blackglass/mcp-servers.toml.example ]; then
    echo "FAIL: /etc/blackglass/mcp-servers.toml.example not found"
    exit 1
fi
echo "  ✓ mcp-servers.toml.example is installed"

echo "5. Checking secondary-sidecar service..."
if ! systemctl --user is-active blackglass-secondary-sidecar.service >/dev/null 2>&1; then
    echo "  · blackglass-secondary-sidecar.service not active (non-fatal: it can be down without breaking the core)"
else
    echo "  ✓ blackglass-secondary-sidecar is running"
fi

echo "6. Checking the 4 MCPs are supervised (best-effort)..."
for mcp in mcp-ad mcp-flipper mcp-phish mcp-detect; do
    if pgrep -f "blackglass-${mcp}" >/dev/null 2>&1; then
        echo "  ✓ ${mcp} is running"
    else
        echo "  · ${mcp} not running (may not be in mcp-servers.toml)"
    fi
done

echo ""
echo "ALL POSTINST SMOKE CHECKS PASSED"

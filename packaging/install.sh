#!/usr/bin/env bash
# install.sh — blackglass one-line installer.
# Source: https://github.com/blackglass/blackglass/blob/main/packaging/install.sh
# This script is browsable. Auditing it is the point.

set -euo pipefail

# Parse args
VARIANT="full"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --minimal|--core|--full)
            VARIANT="${1#--}"
            shift
            ;;
        *)
            echo "unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

# 1. Detect distro
DISTRO=$(. /etc/os-release && echo "${ID:-}")
case "$DISTRO" in
    ubuntu|kali|debian) ;;
    *) echo "unsupported distro: $DISTRO (need Ubuntu 24.04+, Kali, or Debian 12+)" >&2; exit 1 ;;
esac
echo "✓ detected distro: $DISTRO"

# 2. AppArmor precheck
if ! command -v aa-enabled >/dev/null; then
    echo "AppArmor is not installed. Install apparmor and apparmor-utils first." >&2
    exit 1
fi
if ! aa-enabled --quiet 2>/dev/null; then
    echo "AppArmor is not enabled. blackglass requires AppArmor." >&2
    exit 1
fi
echo "✓ AppArmor is enabled"

# 3. Ensure cosign is available
if ! command -v cosign >/dev/null; then
    echo "Installing cosign..."
    if command -v apt-get >/dev/null; then
        apt-get install -y cosign 2>/dev/null || {
            echo "cosign not in repos; falling back to static binary"
            curl -sSfL -o /usr/local/bin/cosign \
              https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64
            chmod +x /usr/local/bin/cosign
        }
    fi
fi
echo "✓ cosign is available"

# 4. Fetch the latest release metadata
echo "Fetching latest release info..."
release_json=$(curl -sSfL https://api.github.com/repos/blackglass/blackglass/releases/latest)
version=$(echo "$release_json" | jq -r .tag_name)
asset_base="https://github.com/blackglass/blackglass/releases/download/$version"

# 5. Download the .deb and its signature
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
deb_basename="blackglass-${VARIANT}_${version#v}_amd64.deb"
echo "Downloading $deb_basename..."
curl -sSfL -o "$tmpdir/$deb_basename"   "$asset_base/$deb_basename"
curl -sSfL -o "$tmpdir/$deb_basename.sig"   "$asset_base/$deb_basename.sig"
curl -sSfL -o "$tmpdir/$deb_basename.cert"  "$asset_base/$deb_basename.cert"

# 6. Verify the .deb is what we built
echo "Verifying cosign signature..."
. /usr/lib/blackglass/installer/verify-cosign.sh 2>/dev/null || . "$(dirname "$0")/installer/verify-cosign.sh"
verify_cosign_blob "$tmpdir/$deb_basename" "$tmpdir/$deb_basename.sig" "$tmpdir/$deb_basename.cert"
echo "✓ signature verified"

# 7. Install with apt
echo "Installing with apt..."
. "$(dirname "$0")/installer/apt-install.sh"
apt_install_deb "$tmpdir/$deb_basename"

# 8. Print the summary
cat <<EOF

blackglass ${version} installed.
  UI:           blackglass ui
  Profile:      blackglass profile init
  Audit log:    ~/.local/share/blackglass/audit/audit.jsonl
  Re-install:   curl -sSfL https://blackglass.dev/install.sh | sudo bash
You may need to log out and back in for the 'blackglass' group to take effect.
EOF

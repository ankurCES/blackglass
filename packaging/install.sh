#!/usr/bin/env bash
# install.sh — blackglass one-line installer.
# Source: https://raw.githubusercontent.com/ankurCES/blackglass/master/packaging/install.sh
# This script is browsable. Auditing it is the point.

set -euo pipefail

# Parse args
VARIANT="full"
DISTRO_OVERRIDE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --minimal|--core|--full)
            VARIANT="${1#--}"
            shift
            ;;
        --ubuntu|--kali|--debian)
            # Force the distro identity past the /etc/os-release check.
            # Use this when you've customised /etc/os-release (e.g. a
            # modified Ubuntu where ID is no longer "ubuntu") but you
            # know the system is still deb-based and AppArmor-enabled.
            # You are asserting the install is safe to run as if it
            # were a stock ${1#--} install. The downstream apt-get
            # calls will fail loudly if your assertion is wrong.
            DISTRO_OVERRIDE="${1#--}"
            shift
            ;;
        *)
            echo "unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

# 1. Detect distro
if [[ -n "$DISTRO_OVERRIDE" ]]; then
    DISTRO="$DISTRO_OVERRIDE"
    echo "⚠ distro forced to '$DISTRO' via --$DISTRO_OVERRIDE (skipping /etc/os-release check)"
    echo "  you are responsible for ensuring this is a real $DISTRO system with AppArmor + apt"
else
    DISTRO=$(. /etc/os-release && echo "${ID:-}")
    case "$DISTRO" in
        ubuntu|kali|debian) ;;
        *) echo "unsupported distro: $DISTRO (need Ubuntu 24.04+, Kali, or Debian 12+)" >&2
           echo "  hint: if you have a customised /etc/os-release but are still on a deb-based"
           echo "  system, re-run with --ubuntu / --kali / --debian to override." >&2
           exit 1 ;;
    esac
    echo "✓ detected distro: $DISTRO"
fi

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

# Set up a tmpdir for downloads and bind cleanup to EXIT so we don't
# leak .deb artifacts on failure paths. Created here (before the API
# call) so the release.json can be saved into it for inspection.
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# 4. Fetch the latest release metadata
echo "Fetching latest release info..."
http_code=$(curl -sS -o "$tmpdir/release.json" -w "%{http_code}" \
    https://api.github.com/repos/ankurCES/blackglass/releases/latest || echo "000")
release_json_file="$tmpdir/release.json"
if [[ "$http_code" != "200" ]]; then
    cat >&2 <<EOF
✗ could not find a published release for ankurCES/blackglass (HTTP $http_code).

  The GitHub API returns 404 when the repository has zero published
  releases. The blackglass release pipeline is not yet wired up
  end-to-end (xtask's 'sign' subcommand is a Phase-4 stub), so no
  cosign-signed .deb has been attached to a release yet.

  What you can do:
    1. Watch for the first release:
         https://github.com/ankurCES/blackglass/releases
       (re-run this installer once a release is cut)

    2. Build from source instead:
         git clone https://github.com/ankurCES/blackglass
         cd blackglass
         sudo apt-get install -y cargo rustc nodejs npm cargo-deb
         cargo run -p xtask -- deb --variants full
         sudo apt-get install -y ./target/debian/blackglass-full_*_amd64.deb

  Either path gets you the same code, but option 2 skips the cosign
  verification because there is no signature to verify yet.
EOF
    exit 1
fi
version=$(jq -r .tag_name < "$release_json_file")
asset_base="https://github.com/ankurCES/blackglass/releases/download/$version"

# 5. Download the .deb and its signature
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
  Re-install:   curl -sSfL https://raw.githubusercontent.com/ankurCES/blackglass/master/packaging/install.sh | sudo bash -s -- --full
You may need to log out and back in for the 'blackglass' group to take effect.
EOF

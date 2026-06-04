#!/usr/bin/env bash
# install.sh — blackglass one-line installer.
# Source: https://raw.githubusercontent.com/ankurCES/blackglass/master/packaging/install.sh
# This script is browsable. Auditing it is the point.
#
# v1 (this file): HTTPS + SHA-256 checksum pinning. The cosign
# release-signing pipeline is deferred to a later sub-plan, so
# the cosign branch of v0 has been removed.
#
# If the release pipeline hasn't published a .deb yet, the script
# prints a "build from source" recipe and exits cleanly. (The v0
# cosign step would have hard-exited on 404 with a cosign error,
# which was confusing for first-time users.)

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

# 1. Detect distro.
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

# 2. AppArmor precheck.
if ! command -v aa-enabled >/dev/null; then
    echo "AppArmor is not installed. Install apparmor and apparmor-utils first." >&2
    exit 1
fi
if ! aa-enabled --quiet 2>/dev/null; then
    echo "AppArmor is not enabled. blackglass requires AppArmor." >&2
    exit 1
fi
echo "✓ AppArmor is enabled"

# 3. Set up a tmpdir for downloads and bind cleanup to EXIT so we don't
#    leak .deb artifacts on failure paths.
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# 4. Fetch the latest release metadata.
echo "Fetching latest release info..."
http_code=$(curl -sS -o "$tmpdir/release.json" -w "%{http_code}" \
    https://api.github.com/repos/ankurCES/blackglass/releases/latest || echo "000")
if [[ "$http_code" != "200" ]]; then
    cat >&2 <<EOF
✗ could not find a published release for ankurCES/blackglass (HTTP $http_code).

  The GitHub API returns 404 when the repository has zero published
  releases. The blackglass release pipeline is not yet wired up
  end-to-end (the cosign release-signing sub-plan is deferred), so
  no SHA-256-pinned .deb has been attached to a release yet.

  What you can do:
    1. Watch for the first release:
         https://github.com/ankurCES/blackglass/releases
       (re-run this installer once a release is cut)

    2. Build from source instead — see the README, section
       "Build from source":
         https://github.com/ankurCES/blackglass#build-from-source
       The short version:
         git clone https://github.com/ankurCES/blackglass
         cd blackglass
         sudo apt-get install -y cargo rustc nodejs npm cargo-deb
         cargo build --workspace
         ( cd app && npm install && npm run build )
         cargo run -p xtask -- deb --variants full
         sudo apt-get install -y ./target/debian/blackglass-full_*_amd64.deb

       Then follow the "First launch walkthrough" section in the
       README:
         https://github.com/ankurCES/blackglass#first-launch-walkthrough

  Both paths get you the same code. Option 2 skips the SHA-256 check
  because the build-from-source artifact is trusted by construction
  (you built it).
EOF
    exit 1
fi
version=$(jq -r .tag_name < "$tmpdir/release.json")
asset_base="https://github.com/ankurCES/blackglass/releases/download/$version"

# 5. Download the .deb and its SHA-256.
deb_basename="blackglass-${VARIANT}_${version#v}_amd64.deb"
echo "Downloading $deb_basename + checksum..."
curl -sSfL -o "$tmpdir/$deb_basename"      "$asset_base/$deb_basename"
curl -sSfL -o "$tmpdir/$deb_basename.sha256" "$asset_base/$deb_basename.sha256"

# 6. SHA-256 verify.
echo "Verifying SHA-256..."
(cd "$tmpdir" && sha256sum -c "$(basename "$deb_basename").sha256") || {
    echo "SHA-256 checksum mismatch — refusing to install." >&2
    exit 1
}
echo "✓ checksum verified"

# 7. Install with apt.
echo "Installing with apt..."
. "$(dirname "$0")/installer/apt-install.sh"
apt_install_deb "$tmpdir/$deb_basename"

# 8. Print the summary.
cat <<EOF

blackglass ${version} installed.
  UI:           blackglass ui
  Audit log:    ~/.local/share/blackglass/audit/audit.jsonl
  Socket:       ~/.local/share/blackglass/runtime.sock
  Re-install:   curl -sSfL https://blackglass.dev/install.sh | sudo bash

If you have a Flipper, log out and back in for the udev group to
take effect (the postinst added you to it).

To verify the install, run (from a source checkout):
  cargo run -p xtask -- verify-install
  sudo cargo run -p xtask -- confinement-test

To verify the audit chain:
  blackglass audit verify
EOF

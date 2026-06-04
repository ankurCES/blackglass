#!/usr/bin/env bash
# verify-cosign.sh — verify a .deb with cosign keyless signing.

set -euo pipefail

verify_cosign_blob() {
    local deb="$1"
    local sig="$2"
    local cert="$3"

    cosign verify-blob \
      --signature "$sig" \
      --certificate "$cert" \
      --certificate-identity-regexp 'https://github.com/blackglass/blackglass/.github/workflows/release.yml@refs/tags/v.*' \
      --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
      "$deb"
}

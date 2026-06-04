#!/usr/bin/env bash
# detect-distro.sh — refuse to install on unsupported systems.

set -euo pipefail

. /etc/os-release

case "${ID:-}-${VERSION_ID:-}" in
    ubuntu-24.*|ubuntu-25.*)
        echo "ubuntu"
        ;;
    kali-*)
        echo "kali"
        ;;
    debian-12|debian-13)
        echo "debian"
        ;;
    *)
        echo ""
        ;;
esac

#!/usr/bin/env bash
# apt-install.sh — install a .deb with apt.

set -euo pipefail

apt_install_deb() {
    local deb="$1"
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y "$deb"
}

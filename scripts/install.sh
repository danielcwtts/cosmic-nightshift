#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Builds and installs cosmic-nightshift:
#   - the privileged helper -> /usr/local/bin/cosmic-nightshift-helper
#   - the polkit rule        -> /etc/polkit-1/rules.d/
#   - (optionally) the GUI    -> /usr/local/bin/cosmic-nightshift
#
# The helper is the only component that runs as root; the GUI and daemon
# run as your user and call the helper through pkexec.
#
# Usage:
#   ./scripts/install.sh           # build (release) then sudo-install
#   ./scripts/install.sh --gui     # also build + install the libcosmic GUI

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

build_gui=0
[[ "${1:-}" == "--gui" ]] && build_gui=1

echo ">> Building nightshift-helper (release)..."
cargo build --release -p nightshift-helper

helper_bin="$repo_root/target/release/cosmic-nightshift-helper"
[[ -x "$helper_bin" ]] || { echo "helper binary not found at $helper_bin" >&2; exit 1; }

if [[ "$build_gui" -eq 1 ]]; then
    echo ">> Building cosmic-nightshift GUI (release)... (this pulls libcosmic and is slow)"
    cargo build --release -p cosmic-nightshift
fi

echo ">> Installing (requires sudo)..."
sudo install -o root -g root -m 0755 "$helper_bin" /usr/local/bin/cosmic-nightshift-helper
sudo install -o root -g root -m 0644 \
    "$repo_root/polkit/49-cosmic-nightshift.rules" \
    /etc/polkit-1/rules.d/49-cosmic-nightshift.rules

if [[ "$build_gui" -eq 1 ]]; then
    sudo install -o root -g root -m 0755 \
        "$repo_root/target/release/cosmic-nightshift" /usr/local/bin/cosmic-nightshift
    sudo install -o root -g root -m 0644 \
        "$repo_root/data/io.github.cosmic_nightshift.desktop" \
        /usr/share/applications/io.github.cosmic_nightshift.desktop
    sudo update-desktop-database /usr/share/applications 2>/dev/null || true
fi

echo
echo "Installed:"
echo "  /usr/local/bin/cosmic-nightshift-helper"
echo "  /etc/polkit-1/rules.d/49-cosmic-nightshift.rules"
if [[ "$build_gui" -eq 1 ]]; then
    echo "  /usr/local/bin/cosmic-nightshift"
    echo "  /usr/share/applications/io.github.cosmic_nightshift.desktop"
    echo
    echo 'Search "Night Shift" in the COSMIC launcher to open the GUI.'
fi
echo
echo "Test it (will briefly flip VTs and warm the screen):"
echo "  pkexec /usr/local/bin/cosmic-nightshift-helper --temp 3500"
echo "Reset:"
echo "  pkexec /usr/local/bin/cosmic-nightshift-helper --off"
echo
echo "If the polkit rule isn't picked up: sudo systemctl restart polkit"

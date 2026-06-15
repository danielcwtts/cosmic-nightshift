#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
#
# Uninstalls cosmic-nightlight components installed by install.sh
#
# Usage:
#   ./scripts/uninstall.sh

set -euo pipefail

echo ">> Uninstalling cosmic-nightlight components (requires sudo)..."

# Helper and polkit rule
if [[ -f "/usr/local/bin/cosmic-nightlight-helper" ]]; then
    sudo rm -f "/usr/local/bin/cosmic-nightlight-helper"
    echo "Removed: /usr/local/bin/cosmic-nightlight-helper"
fi

if [[ -f "/etc/polkit-1/rules.d/49-cosmic-nightlight.rules" ]]; then
    sudo rm -f "/etc/polkit-1/rules.d/49-cosmic-nightlight.rules"
    echo "Removed: /etc/polkit-1/rules.d/49-cosmic-nightlight.rules"
fi

# GUI components
if [[ -f "/usr/local/bin/cosmic-nightlight" ]]; then
    sudo rm -f "/usr/local/bin/cosmic-nightlight"
    echo "Removed: /usr/local/bin/cosmic-nightlight"
fi

if [[ -f "/usr/share/applications/io.github.cosmic_nightlight.desktop" ]]; then
    sudo rm -f "/usr/share/applications/io.github.cosmic_nightlight.desktop"
    echo "Removed: /usr/share/applications/io.github.cosmic_nightlight.desktop"
fi

if [[ -f "/usr/share/applications/io.github.cosmic_nightlight.settings.desktop" ]]; then
    sudo rm -f "/usr/share/applications/io.github.cosmic_nightlight.settings.desktop"
    echo "Removed: /usr/share/applications/io.github.cosmic_nightlight.settings.desktop"
fi

# Update desktop database if it exists
if command -v update-desktop-database >/dev/null 2>&1; then
    sudo update-desktop-database /usr/share/applications 2>/dev/null || true
fi

echo
echo "Uninstallation complete."
echo "If the polkit rule is still active, you may need to: sudo systemctl restart polkit"

#!/bin/bash
# fix-ndev-on-boot.sh - Fix network interface naming on boot

set -euo pipefail

# Wait for interfaces to appear
sleep 2

# Fix wlan0 naming if needed
if ip link show wlan0 2>/dev/null; then
    # Check if it's actually the WiFi interface
    if ethtool -i wlan0 2>/dev/null | grep -q "brcmfmac"; then
        # Rename to wlan0 if it has a different name
        CURRENT_NAME=$(ip link show | grep -E "^[0-9]+: wl" | head -1 | awk -F': ' '{print $2}' | awk '{print $1}')
        if [[ "$CURRENT_NAME" != "wlan0" ]]; then
            ip link set "$CURRENT_NAME" down
            ip link set "$CURRENT_NAME" name wlan0
            ip link set wlan0 up
        fi
    fi
fi

# Ensure wlan0mon doesn't exist from previous run
if ip link show wlan0mon 2>/dev/null; then
    ip link set wlan0mon down
    ip link set wlan0mon name wlan0 2>/dev/null || true
fi

exit 0
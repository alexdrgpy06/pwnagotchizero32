#!/bin/bash
# bt-keepalive.sh - Maintain Bluetooth PAN connection

set -euo pipefail

CONFIG_FILE="/etc/pwnagotchi/config.toml"
PHONE_MAC=""
CHECK_INTERVAL=30
RECONNECT_DELAY=10

# Read phone MAC from config
if [[ -f "$CONFIG_FILE" ]]; then
    PHONE_MAC=$(grep -E '^\s*phone_mac\s*=' "$CONFIG_FILE" | sed -E 's/.*=\s*"?([^"]+)"?/\1/' | tr -d ' ')
fi

# If not in config, try to find from paired devices
if [[ -z "$PHONE_MAC" ]]; then
    PHONE_MAC=$(bt-device -l 2>/dev/null | grep -E '^\[.*\]' | head -1 | sed -E 's/\[([^\]]+)\].*/\1/')
fi

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [bt-keepalive] $*"
}

check_bt_interface() {
    # Check if bnep0 exists and is up
    if ip link show bnep0 &>/dev/null; then
        if ip link show bnep0 | grep -q "state UP"; then
            # Check if we have an IP
            if ip -4 addr show bnep0 | grep -q "inet "; then
                return 0
            fi
        fi
    fi
    return 1
}

connect_bt_pan() {
    local mac=$1
    log "Connecting to $mac via PAN..."
    
    # Use bt-network for BlueZ 5.x
    bt-network -c "$mac" nap &
    local pid=$!
    
    # Wait for connection
    for i in {1..10}; do
        sleep 1
        if check_bt_interface; then
            log "BT PAN connected successfully"
            return 0
        fi
    done
    
    # Kill background process if still running
    kill $pid 2>/dev/null || true
    return 1
}

disconnect_bt_pan() {
    log "Disconnecting BT PAN..."
    # Find and kill bt-network processes
    pkill -f "bt-network.*nap" 2>/dev/null || true
    
    # Bring down interface
    ip link set bnep0 down 2>/dev/null || true
}

main() {
    log "Starting BT keepalive (phone: ${PHONE_MAC:-auto-discover})"
    
    while true; do
        if check_bt_interface; then
            # Interface is up, check connectivity
            if ping -c 1 -W 2 8.8.8.8 &>/dev/null; then
                # Connected and working
                sleep $CHECK_INTERVAL
                continue
            else
                log "BT interface up but no internet, reconnecting..."
                disconnect_bt_pan
            fi
        else
            log "BT PAN not connected"
        fi
        
        # Try to connect
        if [[ -n "$PHONE_MAC" ]]; then
            if connect_bt_pan "$PHONE_MAC"; then
                sleep $CHECK_INTERVAL
                continue
            fi
        else
            log "No phone MAC configured, scanning..."
            # Could implement auto-discovery here
        fi
        
        log "Reconnect failed, waiting ${RECONNECT_DELAY}s..."
        sleep $RECONNECT_DELAY
    done
}

# Handle signals
trap 'log "Shutting down..."; disconnect_bt_pan; exit 0' SIGTERM SIGINT

main
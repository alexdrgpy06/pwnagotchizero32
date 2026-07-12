#!/bin/bash
# pisugar-watchdog.sh - PiSugar battery monitor and watchdog

set -euo pipefail

I2C_BUS=1
I2C_ADDR=0x24
CHECK_INTERVAL=30
LOW_BATTERY_THRESHOLD=10
CRITICAL_BATTERY_THRESHOLD=5

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

# Read register from PiSugar
read_register() {
    local reg=$1
    i2cget -y $I2C_BUS $I2C_ADDR $reg 2>/dev/null || echo "0x00"
}

# Get battery percentage
get_battery() {
    local reg_val=$(read_register 0x0A)
    # Register 0x0A returns battery percentage directly
    printf "%d" "$reg_val" 2>/dev/null || echo "0"
}

# Get charging status
get_charging() {
    local reg_val=$(read_register 0x0B)
    # Bit 0 = charging, Bit 1 = full
    local charging=$((reg_val & 0x01))
    echo $charging
}

# Get button state
get_button() {
    local reg_val=$(read_register 0x0C)
    # Bit 0 = pressed
    local pressed=$((reg_val & 0x01))
    echo $pressed
}

# Main watchdog loop
log "Starting PiSugar watchdog (interval: ${CHECK_INTERVAL}s)"

while true; do
    BATTERY=$(get_battery)
    CHARGING=$(get_charging)
    BUTTON=$(get_button)
    
    log "Battery: ${BATTERY}%, Charging: $CHARGING, Button: $BUTTON"
    
    # Check battery level
    if [[ $BATTERY -le $CRITICAL_BATTERY_THRESHOLD && $CHARGING -eq 0 ]]; then
        log "CRITICAL: Battery at ${BATTERY}%, initiating safe shutdown"
        systemctl poweroff
        exit 0
    elif [[ $BATTERY -le $LOW_BATTERY_THRESHOLD && $CHARGING -eq 0 ]]; then
        log "WARNING: Battery low (${BATTERY}%)"
        # Could trigger LED warning or notification
    fi
    
    # Check for long button press (handled by hardware interrupt ideally)
    # This is a fallback
    if [[ $BUTTON -eq 1 ]]; then
        log "Button pressed, starting hold timer..."
        HOLD_COUNT=0
        while [[ $(get_button) -eq 1 && $HOLD_COUNT -lt 30 ]]; do
            sleep 1
            HOLD_COUNT=$((HOLD_COUNT + 1))
        done
        if [[ $HOLD_COUNT -ge 10 ]]; then  # 10 seconds = long press
            log "Long button press detected, initiating safe shutdown"
            systemctl poweroff
            exit 0
        fi
    fi
    
    # Pet the hardware watchdog if enabled
    if [[ -e /dev/watchdog ]]; then
        echo > /dev/watchdog
    fi
    
    sleep $CHECK_INTERVAL
done
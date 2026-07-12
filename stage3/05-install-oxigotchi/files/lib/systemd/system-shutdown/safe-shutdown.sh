#!/bin/bash
# safe-shutdown.sh - Safe shutdown handler for pwnagotchi-zero
# Called on shutdown/reboot/halt and on button long-press/low battery

set -euo pipefail

LOG_FILE="/etc/pwnagotchi/log/safe-shutdown.log"
ZRAM_LOG="/etc/pwnagotchi/log"
ZRAM_DATA="/var/tmp/pwnagotchi"
PERSISTENT_LOG="/var/lib/pwnagotchi/log"
PERSISTENT_DATA="/var/lib/pwnagotchi/data"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

log "=== Safe shutdown initiated ==="

# Stop oxigotchi daemon gracefully
log "Stopping oxigotchi service..."
systemctl stop oxigotchi.service 2>/dev/null || true

# Wait for clean exit
sleep 2

# Force kill if still running
if systemctl is-active oxigotchi.service &>/dev/null; then
    log "Oxigotchi still running, sending SIGKILL..."
    pkill -9 oxigotchi 2>/dev/null || true
    sleep 1
fi

# Sync zram to persistent storage
log "Syncing zram mounts to disk..."

# Sync logs
if mountpoint -q "$ZRAM_LOG"; then
    log "Syncing log directory..."
    rsync -a --delete "$ZRAM_LOG/" "$PERSISTENT_LOG/" 2>&1 | tee -a "$LOG_FILE" || true
fi

# Sync data
if mountpoint -q "$ZRAM_DATA"; then
    log "Syncing data directory..."
    rsync -a --delete "$ZRAM_DATA/" "$PERSISTENT_DATA/" 2>&1 | tee -a "$LOG_FILE" || true
fi

# Sync handshakes (already on disk, but ensure)
log "Syncing handshakes..."
rsync -a /etc/pwnagotchi/handshakes/ /var/lib/pwnagotchi/handshakes/ 2>&1 | tee -a "$LOG_FILE" || true

# Sync config backups
log "Syncing config backups..."
rsync -a /etc/pwnagotchi/backups/ /var/lib/pwnagotchi/backups/ 2>&1 | tee -a "$LOG_FILE" || true

# Flush filesystem caches
log "Flushing filesystem caches..."
sync

# Turn off e-ink display (show shutdown face)
if [[ -x /usr/local/bin/epd-shutdown.sh ]]; then
    /usr/local/bin/epd-shutdown.sh 2>&1 | tee -a "$LOG_FILE" || true
fi

# Unmount zram devices
log "Unmounting zram devices..."
systemctl stop zram-log.service 2>/dev/null || true
systemctl stop zram-data.service 2>/dev/null || true

# Final sync
sync
sync
sync

log "=== Safe shutdown complete ==="
exit 0
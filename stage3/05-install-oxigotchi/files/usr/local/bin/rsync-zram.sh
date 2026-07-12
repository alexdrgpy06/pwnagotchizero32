#!/bin/bash
# rsync-zram.sh - Sync zram mounts to persistent storage

set -euo pipefail

LOG_DIR="/etc/pwnagotchi/log"
DATA_DIR="/var/tmp/pwnagotchi"
PERSISTENT_LOG="/var/lib/pwnagotchi/log"
PERSISTENT_DATA="/var/lib/pwnagotchi/data"

# Create persistent directories
mkdir -p "$PERSISTENT_LOG"
mkdir -p "$PERSISTENT_DATA"

# Sync log directory
if [[ -d "$LOG_DIR" ]]; then
    rsync -a --delete "$LOG_DIR/" "$PERSISTENT_LOG/" 2>/dev/null || true
fi

# Sync data directory
if [[ -d "$DATA_DIR" ]]; then
    rsync -a --delete "$DATA_DIR/" "$PERSISTENT_DATA/" 2>/dev/null || true
fi

exit 0
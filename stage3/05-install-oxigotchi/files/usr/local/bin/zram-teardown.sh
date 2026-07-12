#!/bin/bash
# zram-teardown.sh - Tear down zram device
# Usage: zram-teardown.sh <name>

set -euo pipefail

NAME="$1"

if [[ -z "$NAME" ]]; then
    echo "Usage: $0 <name>"
    exit 1
fi

# Determine zram device number
if [[ "$NAME" == "log" ]]; then
    ZRAM_NUM=0
elif [[ "$NAME" == "data" ]]; then
    ZRAM_NUM=1
else
    echo "Unknown zram name: $NAME"
    exit 1
fi

ZRAM_DEV="/dev/zram$ZRAM_NUM"
MOUNTPOINT=""

# Find mountpoint
if [[ "$NAME" == "log" ]]; then
    MOUNTPOINT="/etc/pwnagotchi/log"
elif [[ "$NAME" == "data" ]]; then
    MOUNTPOINT="/var/tmp/pwnagotchi"
fi

echo "Tearing down zram$ZRAM_NUM ($NAME)"

# Sync to disk first
if [[ -n "$MOUNTPOINT" && -d "$MOUNTPOINT" ]]; then
    rsync -a --delete "$MOUNTPOINT/" "/var/lib/pwnagotchi/$NAME/" 2>/dev/null || true
fi

# Unmount
umount "$MOUNTPOINT" 2>/dev/null || true

# Reset zram device
echo 1 > "/sys/block/zram$ZRAM_NUM/reset" 2>/dev/null || true

echo "zram$ZRAM_NUM torn down"
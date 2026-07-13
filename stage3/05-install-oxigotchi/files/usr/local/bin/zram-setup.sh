#!/bin/bash
# zram-setup.sh - Set up zram device for RAM-backed storage
# Usage: zram-setup.sh <name> <size> <mountpoint>

set -euo pipefail

NAME="$1"
SIZE="$2"
MOUNTPOINT="$3"

if [[ -z "$NAME" || -z "$SIZE" || -z "$MOUNTPOINT" ]]; then
    echo "Usage: $0 <name> <size> <mountpoint>"
    echo "Example: $0 log 50M /etc/pwnagotchi/log"
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

echo "Setting up zram$ZRAM_NUM ($NAME, $SIZE) at $MOUNTPOINT"

# Load zram module if not loaded
modprobe zram num_devices=2

# Reset device
echo 1 > "/sys/block/zram$ZRAM_NUM/reset" 2>/dev/null || true

# Set compression algorithm
echo zstd > "/sys/block/zram$ZRAM_NUM/comp_algorithm"

# Set size
echo "$SIZE" > "/sys/block/zram$ZRAM_NUM/disksize"

# Format as ext4
mkfs.ext4 -F -L "pwnagotchi-$NAME" "$ZRAM_DEV" >/dev/null 2>&1

# Mount with noatime to reduce writes
mkdir -p "$MOUNTPOINT"
mount -o noatime,nodiratime,discard "$ZRAM_DEV" "$MOUNTPOINT"

# Set permissions
chmod 755 "$MOUNTPOINT"

echo "zram$ZRAM_NUM mounted at $MOUNTPOINT"
#!/bin/bash
# buffer-cleaner.sh - Clean kernel ring buffer and logs periodically
# Reduces RAM usage from log spam

set -euo pipefail

# Clear dmesg buffer (keep last 1000 lines)
dmesg -c > /dev/null 2>&1 || true

# Clear journal if too large
JOURNAL_SIZE=$(journalctl --disk-usage 2>/dev/null | grep -oE '[0-9.]+[GMK]' | head -1)
if [[ -n "$JOURNAL_SIZE" ]]; then
    # If journal > 50M, vacuum
    NUM=$(echo "$JOURNAL_SIZE" | grep -oE '[0-9.]+')
    UNIT=$(echo "$JOURNAL_SIZE" | grep -oE '[GMK]')
    if [[ "$UNIT" == "G" ]] || [[ "$UNIT" == "M" && $(echo "$NUM > 50" | bc -l) -eq 1 ]]; then
        journalctl --vacuum-size=20M > /dev/null 2>&1 || true
    fi
fi

# Clear old log files in zram
find /etc/pwnagotchi/log -name "*.log.*" -mtime +3 -delete 2>/dev/null || true
find /var/tmp/pwnagotchi -type f -mtime +1 -delete 2>/dev/null || true

exit 0
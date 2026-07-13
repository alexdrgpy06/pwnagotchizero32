#!/bin/bash -e
# 05-install-oxigotchi/00-run.sh - Install the daemon, configs, units, plugins.
#
# The files/ subtree mirrors the target filesystem layout, so it is copied
# wholesale into the image rootfs. Everything then runs against ${ROOTFS_DIR}
# or inside the chroot via on_chroot.

# Runtime directories the daemon expects.
install -d -m 755 \
    "${ROOTFS_DIR}/etc/pwnagotchi/conf.d" \
    "${ROOTFS_DIR}/etc/pwnagotchi/custom-plugins/faces" \
    "${ROOTFS_DIR}/etc/pwnagotchi/handshakes" \
    "${ROOTFS_DIR}/etc/pwnagotchi/log" \
    "${ROOTFS_DIR}/etc/pwnagotchi/backups" \
    "${ROOTFS_DIR}/etc/pwnagotchi/sessions" \
    "${ROOTFS_DIR}/var/tmp/pwnagotchi" \
    "${ROOTFS_DIR}/var/lib/pwnagotchi/log" \
    "${ROOTFS_DIR}/var/lib/pwnagotchi/data" \
    "${ROOTFS_DIR}/usr/local/share/pwnagotchi/custom-plugins"

# Copy the whole overlay (etc/, usr/, lib/) into the rootfs. Use rsync with
# --keep-dirlinks so the overlay's lib/ directory is merged into the rootfs's
# /lib symlink target (/usr/lib on merged-usr bookworm) instead of trying to
# overwrite the symlink itself.
rsync -a --keep-dirlinks files/ "${ROOTFS_DIR}/"

# Make the daemon binary and helper scripts executable.
chmod 755 "${ROOTFS_DIR}/usr/local/bin/oxigotchi"
chmod 755 "${ROOTFS_DIR}"/usr/local/bin/*.sh 2>/dev/null || true
chmod 755 "${ROOTFS_DIR}/usr/local/bin/bt-pan-connect" 2>/dev/null || true
chmod 755 "${ROOTFS_DIR}/usr/local/bin/bt-pan-disconnect" 2>/dev/null || true
chmod 755 "${ROOTFS_DIR}/lib/systemd/system-shutdown/safe-shutdown.sh" 2>/dev/null || true

# logrotate for the on-zram logs.
cat > "${ROOTFS_DIR}/etc/logrotate.d/oxigotchi" << 'EOF'
/etc/pwnagotchi/log/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 644 root root
}
EOF

# Periodic zram->disk sync.
cat > "${ROOTFS_DIR}/etc/cron.d/rsync-zram" << 'EOF'
* * * * * root /usr/local/bin/rsync-zram.sh >> /var/log/rsync-zram.log 2>&1
EOF

# Enable the core daemon. The zram/bt-agent/nm-watchdog/epd-startup helper
# units are installed but NOT enabled for this image: several reference
# scripts that aren't in the overlay yet or contain bugs, and enabling them
# would only add failed units at boot. The daemon itself brings up the display
# and epoch loop, so the image is fully functional without them. Re-enable
# them as their scripts are fixed.
on_chroot << 'EOF'
set +e
systemctl enable oxigotchi.service && echo "enabled oxigotchi.service" || echo "oxigotchi: could not enable oxigotchi.service"
systemctl enable NetworkManager.service 2>/dev/null || true
EOF

echo "oxigotchi installation complete"

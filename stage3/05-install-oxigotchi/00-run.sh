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

# NetworkManager refuses connection profiles that aren't root-owned mode 0600.
if [ -f "${ROOTFS_DIR}/etc/NetworkManager/system-connections/usb0.nmconnection" ]; then
    chmod 600 "${ROOTFS_DIR}/etc/NetworkManager/system-connections/usb0.nmconnection"
    chown 0:0 "${ROOTFS_DIR}/etc/NetworkManager/system-connections/usb0.nmconnection"
fi

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

# Enable the daemon and the supporting units whose scripts are now in place.
# epd-startup.service and safe-shutdown.service are intentionally left disabled:
# the daemon draws the boot/shutdown faces itself, and shutdown sync runs via
# the /lib/systemd/system-shutdown hook. bt-pan@.service is a template started
# per-device at runtime.
on_chroot << 'EOF'
set +e
for unit in usb-gadget.service oxigotchi.service zram-log.service zram-data.service \
            rsync-zram.timer bt-agent.service nm-watchdog.service; do
    systemctl enable "$unit" && echo "enabled $unit" || echo "oxigotchi: could not enable $unit (continuing)"
done
systemctl enable NetworkManager.service 2>/dev/null || true
# Ensure SSH is up so the device is reachable over the USB-gadget network.
systemctl enable ssh.service 2>/dev/null || systemctl enable sshd.service 2>/dev/null || true
EOF

echo "oxigotchi installation complete"

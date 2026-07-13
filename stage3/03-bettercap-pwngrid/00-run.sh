#!/bin/bash -e
# 03-bettercap-pwngrid/00-run.sh - Keep bettercap installed but disabled.
#
# The oxigotchi daemon drives attacks itself; bettercap must not grab the
# radio at boot. Write a compatibility config and disable its service.

install -d -m 755 "${ROOTFS_DIR}/etc/bettercap"
cat > "${ROOTFS_DIR}/etc/bettercap/bettercap.conf" << 'EOF'
# Bettercap config for oxigotchi (service disabled; kept for compatibility)
wifi.interface wlan0mon
wifi.hop true
wifi.channels 1,2,3,4,5,6,7,8,9,10,11,12,13
events.ignore ble.device.new,ble.device.lost,ble.device.service.discovered,ble.device.characteristic.discovered,ble.device.disconnected,ble.device.connected,ble.connection.timeout,wifi.client.new,wifi.client.lost,wifi.client.probe,wifi.ap.new,wifi.ap.lost,mod.started
EOF

on_chroot << 'EOF'
systemctl disable bettercap.service 2>/dev/null || true
EOF

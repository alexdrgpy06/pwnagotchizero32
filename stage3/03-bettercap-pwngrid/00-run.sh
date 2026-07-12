#!/bin/bash -e
# 03-bettercap-pwngrid/00-run.sh - Configure bettercap (disabled on device, used for pwngrid)

# bettercap is installed but we disable the service on the Pi
# The oxigotchi daemon uses AngryOxide for attacks instead
systemctl disable bettercap || true
systemctl stop bettercap || true

# Install pwngrid agent for peer discovery (optional)
# go install github.com/evilsocket/pwngrid/cmd/pwngrid@latest

# Create bettercap config directory
mkdir -p /etc/bettercap
cat > /etc/bettercap/bettercap.conf << 'EOF'
# Bettercap config for pwnagotchi-zero
# Attacks are handled by AngryOxide, this is for compatibility
wifi.interface wlan0mon
wifi.hop true
wifi.channels 1,2,3,4,5,6,7,8,9,10,11,12,13
events.ignore ble.device.new,ble.device.lost,ble.device.service.discovered,ble.device.characteristic.discovered,ble.device.disconnected,ble.device.connected,ble.connection.timeout,wifi.client.new,wifi.client.lost,wifi.client.probe,wifi.ap.new,wifi.ap.lost,mod.started
EOF
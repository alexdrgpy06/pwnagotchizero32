#!/bin/bash -e
# 05-install-oxigotchi/00-run.sh - Install oxigotchi daemon, systemd units, configs

# Create directories
mkdir -p /usr/local/bin
mkdir -p /etc/pwnagotchi/conf.d
mkdir -p /etc/pwnagotchi/custom-plugins/faces
mkdir -p /etc/pwnagotchi/handshakes
mkdir -p /etc/pwnagotchi/log
mkdir -p /etc/pwnagotchi/backups
mkdir -p /etc/pwnagotchi/sessions
mkdir -p /var/tmp/pwnagotchi
mkdir -p /usr/local/share/pwnagotchi/custom-plugins

# Copy oxigotchi binary (built separately and placed in files/usr/local/bin/)
cp /stage3/05-install-oxigotchi/files/usr/local/bin/oxigotchi /usr/local/bin/oxigotchi
chmod +x /usr/local/bin/oxigotchi

# Copy config.toml
cp /stage3/05-install-oxigotchi/files/etc/pwnagotchi/config.toml /etc/pwnagotchi/config.toml

# Copy angryoxide-v5.toml overlay
cp /stage3/05-install-oxigotchi/files/etc/pwnagotchi/conf.d/angryoxide-v5.toml /etc/pwnagotchi/conf.d/angryoxide-v5.toml

# Copy NetworkManager config for BT PAN
cp /stage3/05-install-oxigotchi/files/etc/NetworkManager/conf.d/99-bt-pan.conf /etc/NetworkManager/conf.d/99-bt-pan.conf

# Copy PiSugar config
cp /stage3/05-install-oxigotchi/files/etc/pwnagotchi/pisugar-config.json /etc/pwnagotchi/pisugar-config.json

# Copy systemd units
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/oxigotchi.service /etc/systemd/system/oxigotchi.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/bt-agent.service /etc/systemd/system/bt-agent.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/bt-pan@.service /etc/systemd/system/bt-pan@.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/epd-startup.service /etc/systemd/system/epd-startup.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/zram-log.service /etc/systemd/system/zram-log.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/zram-data.service /etc/systemd/system/zram-data.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/safe-shutdown.service /etc/systemd/system/safe-shutdown.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/nm-watchdog.service /etc/systemd/system/nm-watchdog.service
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/rsync-zram.timer /etc/systemd/system/rsync-zram.timer
cp /stage3/05-install-oxigotchi/files/etc/systemd/system/rsync-zram.service /etc/systemd/system/rsync-zram.service

# Copy shutdown script
cp /stage3/05-install-oxigotchi/files/lib/systemd/system-shutdown/safe-shutdown.sh /lib/systemd/system-shutdown/safe-shutdown.sh
chmod +x /lib/systemd/system-shutdown/safe-shutdown.sh

# Copy helper scripts
cp /stage3/05-install-oxigotchi/files/usr/local/bin/fix-ndev-on-boot.sh /usr/local/bin/fix-ndev-on-boot.sh
cp /stage3/05-install-oxigotchi/files/usr/local/bin/buffer-cleaner.sh /usr/local/bin/buffer-cleaner.sh
cp /stage3/05-install-oxigotchi/files/usr/local/bin/pisugar-watchdog.sh /usr/local/bin/pisugar-watchdog.sh
cp /stage3/05-install-oxigotchi/files/usr/local/bin/usb0-fallback.sh /usr/local/bin/usb0-fallback.sh
cp /stage3/05-install-oxigotchi/files/usr/local/bin/bt-keepalive.sh /usr/local/bin/bt-keepalive.sh
chmod +x /usr/local/bin/*.sh

# Copy Lua plugins
cp /stage3/05-install-oxigotchi/files/usr/local/share/pwnagotchi/custom-plugins/*.lua /usr/local/share/pwnagotchi/custom-plugins/
cp -r /stage3/05-install-oxigotchi/files/usr/local/share/pwnagotchi/custom-plugins/faces/* /usr/local/share/pwnagotchi/custom-plugins/faces/

# Enable services
systemctl enable oxigotchi.service
systemctl enable bt-agent.service
systemctl enable epd-startup.service
systemctl enable zram-log.service
systemctl enable zram-data.service
systemctl enable safe-shutdown.service
systemctl enable nm-watchdog.service
systemctl enable rsync-zram.timer

# Disable pwnagotchi legacy service
systemctl disable pwnagotchi.service || true

# Set up logrotate for zram sync logs
cat > /etc/logrotate.d/oxigotchi << 'EOF'
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

# Configure dhcpcd for BT PAN and USB fallback
cat >> /etc/dhcpcd.conf << 'EOF'

# BT PAN interface
interface bnep*
static ip_address=192.168.44.1/24
nohook wpa_supplicant

# USB gadget fallback
interface usb0
static ip_address=192.168.42.1/24
nohook wpa_supplicant
EOF

# Set up cron for rsync zram to disk
cat > /etc/cron.d/rsync-zram << 'EOF'
# Sync zram to disk every 60 seconds
* * * * * root /usr/local/bin/rsync-zram.sh >> /var/log/rsync-zram.log 2>&1
# Sync on shutdown
@reboot root sleep 30 && /usr/local/bin/rsync-zram.sh >> /var/log/rsync-zram.log 2>&1
EOF

# Create rsync-zram script
cat > /usr/local/bin/rsync-zram.sh << 'EOF'
#!/bin/bash
# Sync zram mounts to persistent storage
rsync -a --delete /etc/pwnagotchi/log/ /var/lib/pwnagotchi/log/ 2>/dev/null || true
rsync -a --delete /var/tmp/pwnagotchi/ /var/lib/pwnagotchi/data/ 2>/dev/null || true
EOF
chmod +x /usr/local/bin/rsync-zram.sh

# Create persistent storage directories
mkdir -p /var/lib/pwnagotchi/log
mkdir -p /var/lib/pwnagotchi/data

# Set permissions
chown -R root:root /etc/pwnagotchi
chmod 755 /etc/pwnagotchi
chmod 644 /etc/pwnagotchi/config.toml
chmod 755 /etc/pwnagotchi/handshakes
chmod 755 /etc/pwnagotchi/custom-plugins
chmod 755 /usr/local/share/pwnagotchi/custom-plugins

echo "oxigotchi installation complete"
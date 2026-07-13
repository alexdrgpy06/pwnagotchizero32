#!/bin/bash -e
# 01-pwn-packages/00-run.sh - Install runtime dependencies inside the image.

# Core packages that must be present for the daemon to run. A failure here
# should fail the build.
on_chroot << 'EOF'
set -e
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
    libpcap0.8 \
    bluez \
    bluez-tools \
    dbus \
    network-manager \
    i2c-tools \
    iw \
    wireless-tools \
    lua5.4 \
    rsync \
    cron \
    logrotate \
    ca-certificates \
    curl \
    wget
EOF

# Optional packages: nice to have but not required to boot. Install each on a
# best-effort basis so one missing package can't abort the whole image build.
on_chroot << 'EOF'
export DEBIAN_FRONTEND=noninteractive
for pkg in hcxtools hcxdumptool bettercap zram-tools minisign openssh-client unzip xz-utils spi-tools; do
    apt-get install -y --no-install-recommends "$pkg" || echo "oxigotchi: optional package '$pkg' unavailable, skipping"
done
apt-get clean
rm -rf /var/lib/apt/lists/*
EOF

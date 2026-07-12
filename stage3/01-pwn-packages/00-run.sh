#!/bin/bash -e
# 01-pwn-packages/00-run.sh - Install pwnagotchi dependencies

apt-get update
apt-get install -y \
    bettercap \
    hcxtools \
    hcxdumptool \
    libpcap-dev \
    libbluetooth-dev \
    bluez \
    bluez-tools \
    dbus \
    network-manager \
    dhcpcd5 \
    i2c-tools \
    spi-tools \
    python3-pip \
    lua5.4 \
    liblua5.4-dev \
    git \
    curl \
    wget \
    unzip \
    xz-utils \
    rsync \
    openssh-client \
    minisign \
    cron \
    logrotate

# Install bettercap from source for latest features (optional)
# go install github.com/bettercap/bettercap/v2@latest

# Install Python dependencies for any legacy plugins
pip3 install --break-system-packages \
    paho-mqtt \
    requests \
    psutil \
    netifaces \
    scapy \
    python-dateutil \
    pyyaml \
    toml

# Clean up apt cache
apt-get clean
rm -rf /var/lib/apt/lists/*
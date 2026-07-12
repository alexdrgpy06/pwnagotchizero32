#!/bin/bash -e
# 00-pre-pwn/00-run.sh - Pre-pwnagotchi setup

# Enable SPI and I2C
sed -i 's/^#dtparam=spi=on/dtparam=spi=on/' /boot/config.txt
sed -i 's/^#dtparam=i2c_arm=on/dtparam=i2c_arm=on/' /boot/config.txt

# Enable UART for Bluetooth
echo "dtoverlay=disable-bt" >> /boot/config.txt
echo "dtoverlay=pi3-miniuart-bt" >> /boot/config.txt

# Increase GPU memory for display
echo "gpu_mem=64" >> /boot/config.txt

# Disable swap to reduce SD writes
systemctl disable dphys-swapfile || true

# Set up zram for logs and temp data
cat > /etc/systemd/zram-generator.conf << 'EOF'
[zram0]
zram-size = min(ram / 2, 2048)
compression-algorithm = zstd
EOF

systemctl enable systemd-zram-setup@zram0.service
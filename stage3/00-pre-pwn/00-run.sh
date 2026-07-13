#!/bin/bash -e
# 00-pre-pwn/00-run.sh - Enable SPI/I2C/UART and base tweaks in the image.
#
# pi-gen runs this on the build host, so all changes must target the image
# rootfs (${ROOTFS_DIR}) or run inside the chroot (on_chroot).

# Bookworm keeps the boot config on the firmware partition (/boot/firmware);
# older layouts use /boot. Pick whichever exists in the rootfs.
BOOT_DIR="${ROOTFS_DIR}/boot/firmware"
[ -d "${BOOT_DIR}" ] || BOOT_DIR="${ROOTFS_DIR}/boot"
CONFIG_TXT="${BOOT_DIR}/config.txt"
touch "${CONFIG_TXT}"

# Append a single idempotent oxigotchi block to config.txt.
if ! grep -q "# --- oxigotchi ---" "${CONFIG_TXT}"; then
	cat >> "${CONFIG_TXT}" << 'EOF'

# --- oxigotchi ---
dtparam=spi=on
dtparam=i2c_arm=on
# Free the PL011 UART for the Bluetooth chip (BT PAN tether)
dtoverlay=disable-bt
gpu_mem=64
# --- end oxigotchi ---
EOF
fi

# Load i2c-dev at boot for PiSugar battery monitoring.
echo "i2c-dev" >> "${ROOTFS_DIR}/etc/modules"

# Reduce SD wear: disable swap file service (best-effort — may be absent).
on_chroot << 'EOF'
systemctl disable dphys-swapfile.service 2>/dev/null || true
EOF

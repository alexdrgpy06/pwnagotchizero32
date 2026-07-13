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
dtparam=i2c1=on
enable_uart=1
# USB gadget (usb0) fallback networking
dtoverlay=dwc2
gpu_mem=16
# Bluetooth is left ENABLED — BT PAN tethering depends on it.
[pi0]
dtoverlay=spi0-2cs
[all]
# --- end oxigotchi ---
EOF
fi

# Load the dwc2 USB controller so a gadget can bind. We do NOT load g_ether:
# on Windows g_ether (CDC-ECM) gets mis-bound as a serial COM port. Instead a
# boot service (usb-gadget.service) builds an RNDIS gadget via configfs, which
# Windows recognises as a real network adapter.
CMDLINE="${BOOT_DIR}/cmdline.txt"
if [ -f "${CMDLINE}" ]; then
	sed -i 's/[[:space:]]*$//' "${CMDLINE}"
	# Drop any prior g_ether request, then ensure dwc2 is module-loaded.
	sed -i 's/ *modules-load=dwc2,g_ether//g' "${CMDLINE}"
	if ! grep -q "modules-load=dwc2" "${CMDLINE}"; then
		sed -i 's/$/ modules-load=dwc2/' "${CMDLINE}"
	fi
fi

# Load i2c-dev at boot for PiSugar battery monitoring.
echo "i2c-dev" >> "${ROOTFS_DIR}/etc/modules"

# Reduce SD wear: disable swap file service (best-effort — may be absent).
on_chroot << 'EOF'
systemctl disable dphys-swapfile.service 2>/dev/null || true
EOF

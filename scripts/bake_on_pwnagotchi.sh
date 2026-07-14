#!/bin/bash -e
# bake_on_pwnagotchi.sh — install the oxigotchi Rust daemon onto a proven,
# already-working pwnagotchi (Bullseye) base image, instead of building an OS
# from scratch. The base image already has a working kernel, nexmon firmware
# pairing, e-ink SPI setup, and USB SSH access on both the original Pi Zero W
# and the Pi Zero 2 W — verified on real hardware, so this script leaves
# config.txt and nexmon firmware alone. It DOES install the full set of
# supporting units the from-scratch stage3/ build ships (USB gadget, zram
# wear-leveling, NetworkManager watchdog, Bluetooth PAN tether, safe-shutdown
# hook) — an earlier version of this script only installed the daemon itself,
# leaving the device without RNDIS networking (g_ether gets mis-bound as a
# serial COM port on Windows) or any of the rest.
#
# Usage: sudo bash bake_on_pwnagotchi.sh <base-image.img> <oxigotchi-binary> <output.img> [angryoxide-binary]

set -euo pipefail

BASE_IMG="$1"
DAEMON_BIN="$2"
OUT_IMG="$3"
ANGRYOXIDE_BIN="${4:-}"
OVERLAY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/stage3/05-install-oxigotchi/files"

if [ "$(id -u)" -ne 0 ]; then
    echo "Run with sudo (needs losetup/mount/chroot)." >&2
    exit 1
fi

WORK="$(mktemp -d)"
BOOT_MNT="${WORK}/boot"
ROOT_MNT="${WORK}/root"
mkdir -p "${BOOT_MNT}" "${ROOT_MNT}"

# Modify BASE_IMG in place (renamed to OUT_IMG) rather than copying first —
# a full copy would double disk usage for no benefit on a space-constrained
# CI runner. The caller's copy of BASE_IMG is consumed by this move.
mv "${BASE_IMG}" "${OUT_IMG}"

cleanup() {
    set +e
    for d in dev/pts dev proc sys; do
        umount -lf "${ROOT_MNT}/${d}" 2>/dev/null
    done
    umount -lf "${BOOT_MNT}" 2>/dev/null
    umount -lf "${ROOT_MNT}" 2>/dev/null
    [ -n "${LOOPDEV:-}" ] && losetup -d "${LOOPDEV}" 2>/dev/null
    rm -rf "${WORK}"
}
trap cleanup EXIT

LOOPDEV="$(losetup -f -P --show "${OUT_IMG}")"
echo "loop device: ${LOOPDEV}"
partprobe "${LOOPDEV}" 2>/dev/null || true
# Partition device nodes can take a moment to appear after losetup -P.
for _ in $(seq 1 15); do
    [ -e "${LOOPDEV}p1" ] && [ -e "${LOOPDEV}p2" ] && break
    sleep 1
done
test -e "${LOOPDEV}p1" && test -e "${LOOPDEV}p2"

# Pwnagotchi/Raspberry Pi OS images: partition 1 = boot (FAT32), partition 2 = root (ext4).
mount "${LOOPDEV}p1" "${BOOT_MNT}"
mount "${LOOPDEV}p2" "${ROOT_MNT}"

# Cross-arch chroot support (armhf image on an x86_64 runner).
if [ ! -e "${ROOT_MNT}/usr/bin/qemu-arm-static" ]; then
    install -m 755 /usr/bin/qemu-arm-static "${ROOT_MNT}/usr/bin/qemu-arm-static"
fi
mount --bind /dev "${ROOT_MNT}/dev"
mount --bind /dev/pts "${ROOT_MNT}/dev/pts"
mount --bind /proc "${ROOT_MNT}/proc"
mount --bind /sys "${ROOT_MNT}/sys"

echo "== Installing the oxigotchi daemon =="
install -d -m 755 "${ROOT_MNT}/etc/oxigotchi"
install -m 755 "${DAEMON_BIN}" "${ROOT_MNT}/usr/local/bin/oxigotchi"

if [ -n "${ANGRYOXIDE_BIN}" ] && [ -f "${ANGRYOXIDE_BIN}" ]; then
    echo "== Installing AngryOxide (attack/capture engine) =="
    install -m 755 "${ANGRYOXIDE_BIN}" "${ROOT_MNT}/usr/local/bin/angryoxide"
fi

# The daemon binary's own helper scripts. We deliberately do NOT copy
# config.txt changes or nexmon firmware from our own overlay — those already
# exist, correctly, in this base image. (The USB gadget is the one exception;
# see below.)
install -m 755 "${OVERLAY_DIR}/usr/bin/monstart" "${ROOT_MNT}/usr/bin/monstart" 2>/dev/null || true
install -m 755 "${OVERLAY_DIR}/usr/bin/monstop" "${ROOT_MNT}/usr/bin/monstop" 2>/dev/null || true

echo "== Installing our RNDIS USB gadget (base image's g_ether gets mis-bound as a serial COM port on Windows) =="
install -m 755 "${OVERLAY_DIR}/usr/local/bin/usb-gadget.sh" "${ROOT_MNT}/usr/local/bin/usb-gadget.sh"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/usb-gadget.service" "${ROOT_MNT}/etc/systemd/system/usb-gadget.service"
install -d -m 755 "${ROOT_MNT}/etc/NetworkManager/conf.d"
install -m 644 "${OVERLAY_DIR}/etc/NetworkManager/conf.d/99-unmanage-usb0.conf" "${ROOT_MNT}/etc/NetworkManager/conf.d/99-unmanage-usb0.conf"

# The base image loads g_ether via cmdline.txt modules-load= at early boot,
# which grabs the dwc2 UDC before usb-gadget.service (a configfs gadget) ever
# gets a chance to bind it — only one gadget can own the UDC at a time. Strip
# g_ether so our RNDIS-with-MS-OS-descriptors gadget wins the race instead;
# same fix already applied to the from-scratch build in
# stage3/00-pre-pwn/00-run.sh, just never ported to this hybrid bake path.
if [ -f "${BOOT_MNT}/cmdline.txt" ]; then
    sed -i 's/[[:space:]]*$//' "${BOOT_MNT}/cmdline.txt"
    sed -i 's/ *modules-load=dwc2,g_ether//g; s/ *modules-load=dwc2,g_serial//g; s/ *modules-load=dwc2,g_mass_storage//g' "${BOOT_MNT}/cmdline.txt"
    if ! grep -q "modules-load=dwc2" "${BOOT_MNT}/cmdline.txt"; then
        sed -i 's/$/ modules-load=dwc2/' "${BOOT_MNT}/cmdline.txt"
    fi
fi

# Confirmed on real hardware via /sys/kernel/debug/gpio: config.txt's
# dtoverlay=spi1-3cs claims GPIO16/17/18 as SPI1 chip-select lines at the
# pinctrl level — a permanent kernel-level claim, not a transient one. GPIO17
# is the e-ink panel's RST line, so the display driver's GPIO export
# (/sys/class/gpio/export) failed with "Device or resource busy" every time,
# regardless of retries — this isn't a race, the pin is just unavailable
# while spi1-3cs is loaded. Nothing in this project uses SPI1 (only SPI0, for
# the display), so disable it. This is the one exception to "don't touch
# config.txt" beyond the cmdline.txt USB gadget fix above — same reasoning:
# the base image's own settings actively conflict with our hardware wiring.
if [ -f "${BOOT_MNT}/config.txt" ]; then
    sed -i 's/^dtoverlay=spi1-3cs/#dtoverlay=spi1-3cs/' "${BOOT_MNT}/config.txt"
fi

echo "== Installing zram wear-leveling (log + data mounts) =="
install -m 755 "${OVERLAY_DIR}/usr/local/bin/zram-setup.sh" "${ROOT_MNT}/usr/local/bin/zram-setup.sh"
install -m 755 "${OVERLAY_DIR}/usr/local/bin/zram-teardown.sh" "${ROOT_MNT}/usr/local/bin/zram-teardown.sh"
install -m 755 "${OVERLAY_DIR}/usr/local/bin/rsync-zram.sh" "${ROOT_MNT}/usr/local/bin/rsync-zram.sh"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/zram-log.service" "${ROOT_MNT}/etc/systemd/system/zram-log.service"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/zram-data.service" "${ROOT_MNT}/etc/systemd/system/zram-data.service"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/rsync-zram.service" "${ROOT_MNT}/etc/systemd/system/rsync-zram.service"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/rsync-zram.timer" "${ROOT_MNT}/etc/systemd/system/rsync-zram.timer"
install -d -m 755 "${ROOT_MNT}/var/lib/pwnagotchi/log" "${ROOT_MNT}/var/lib/pwnagotchi/data"

install -d -m 755 "${ROOT_MNT}/etc/logrotate.d"
cat > "${ROOT_MNT}/etc/logrotate.d/oxigotchi" << 'EOF'
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

echo "== Installing NetworkManager watchdog (BCM43436 UART occasionally wedges NM during monitor-mode transitions) =="
install -m 755 "${OVERLAY_DIR}/usr/local/bin/nm-watchdog.sh" "${ROOT_MNT}/usr/local/bin/nm-watchdog.sh"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/nm-watchdog.service" "${ROOT_MNT}/etc/systemd/system/nm-watchdog.service"

echo "== Installing Bluetooth PAN tether support (bt-agent auto-pair + bt-pan@ template) =="
install -m 755 "${OVERLAY_DIR}/usr/local/bin/bt-pan-connect" "${ROOT_MNT}/usr/local/bin/bt-pan-connect"
install -m 755 "${OVERLAY_DIR}/usr/local/bin/bt-pan-disconnect" "${ROOT_MNT}/usr/local/bin/bt-pan-disconnect"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/bt-agent.service" "${ROOT_MNT}/etc/systemd/system/bt-agent.service"
install -m 644 "${OVERLAY_DIR}/etc/systemd/system/bt-pan@.service" "${ROOT_MNT}/etc/systemd/system/bt-pan@.service"

echo "== Installing safe-shutdown hook (syncs zram to disk on power loss) =="
install -d -m 755 "${ROOT_MNT}/lib/systemd/system-shutdown"
install -m 755 "${OVERLAY_DIR}/lib/systemd/system-shutdown/safe-shutdown.sh" "${ROOT_MNT}/lib/systemd/system-shutdown/safe-shutdown.sh"

# Preserve pwnagotchi's own config for reference, then install ours.
if [ -f "${ROOT_MNT}/etc/pwnagotchi/config.toml" ] && [ ! -f "${ROOT_MNT}/etc/pwnagotchi/config.toml.pwnagotchi-orig" ]; then
    cp "${ROOT_MNT}/etc/pwnagotchi/config.toml" "${ROOT_MNT}/etc/pwnagotchi/config.toml.pwnagotchi-orig"
fi
install -d -m 755 "${ROOT_MNT}/etc/pwnagotchi/conf.d" "${ROOT_MNT}/etc/pwnagotchi/handshakes" \
    "${ROOT_MNT}/etc/pwnagotchi/log" "${ROOT_MNT}/etc/pwnagotchi/backups" "${ROOT_MNT}/etc/pwnagotchi/sessions"
install -m 644 "${OVERLAY_DIR}/etc/pwnagotchi/config.toml" "${ROOT_MNT}/etc/pwnagotchi/config.toml"

# Inject secrets at bake time only — never as literals in the tracked overlay
# config, which lives in a public repo. Set WPA_SEC_API_KEY in the environment
# (from a GitHub Actions secret) to have it written into the baked image.
# Scoped to the [main.plugins.wpa-sec] section specifically: other plugins
# (ohcapi, wigle) also have an empty api_key field in this file.
if [ -n "${WPA_SEC_API_KEY:-}" ]; then
    awk -v key="${WPA_SEC_API_KEY}" '
        /^\[main\.plugins\.wpa-sec\]/ { in_section=1 }
        /^\[/ && !/^\[main\.plugins\.wpa-sec\]/ { in_section=0 }
        in_section && /^api_key = ""$/ { print "api_key = \"" key "\""; next }
        { print }
    ' "${ROOT_MNT}/etc/pwnagotchi/config.toml" > "${ROOT_MNT}/etc/pwnagotchi/config.toml.tmp"
    mv "${ROOT_MNT}/etc/pwnagotchi/config.toml.tmp" "${ROOT_MNT}/etc/pwnagotchi/config.toml"
fi

cat > "${ROOT_MNT}/etc/systemd/system/oxigotchi.service" << 'UNIT'
[Unit]
Description=oxigotchi daemon (Rust pwnagotchi)

[Service]
Type=simple
Environment=PWNAGOTCHI_CONFIG=/etc/pwnagotchi/config.toml
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1
ExecStart=/usr/local/bin/oxigotchi
Restart=on-failure
RestartSec=5
Nice=-5

[Install]
WantedBy=multi-user.target
UNIT

echo "== Disabling stock pwnagotchi/bettercap/pwngrid-peer so oxigotchi owns the radio+display =="
# Mask (not just disable) directly via /dev/null symlinks — this is
# unambiguous and doesn't depend on systemctl's offline [Install]-section
# symlink handling working correctly under a qemu chroot.
#
# pwngrid-peer.service IS masked, unlike an earlier version of this script:
# upstream (evilsocket/jayofelony) hardcodes `-iface mon0` in its ExecStart,
# but oxigotchi's monitor interface is wlan0mon, so pwngrid-peer just
# restart-loops forever trying to bind an interface that will never exist.
# Checked the actual unit dependency graph (After=/Wants=, not just guessing):
# nothing in the base image Requires= or BindsTo= any of these three units,
# so masking all of them is safe for boot — an earlier boot-time timeout that
# was blamed on disabling pwngrid-peer was something else.
for unit in pwnagotchi.service bettercap.service pwngrid-peer.service; do
    ln -sf /dev/null "${ROOT_MNT}/etc/systemd/system/${unit}"
    # Remove any stale enablement symlinks pointing at the real unit file.
    find "${ROOT_MNT}/etc/systemd/system" -type l -lname "*/${unit}" -not -path "*/etc/systemd/system/${unit}" -delete 2>/dev/null || true
done

chroot "${ROOT_MNT}" /usr/bin/qemu-arm-static /bin/bash -c '
set +e
systemctl enable oxigotchi.service
systemctl enable usb-gadget.service
systemctl enable zram-log.service
systemctl enable zram-data.service
systemctl enable rsync-zram.timer
systemctl enable nm-watchdog.service
# bt-agent needs bluez-tools (not part of core bluez); the base pwnagotchi
# image may not have it. Only enable if the binary is actually present, so a
# missing package cannot turn into the same kind of restart-loop churn we
# just fixed for pwngrid-peer above. bt-pan@.service is a per-device
# template started at runtime by the bt-tether plugin, not enabled directly
# — same as the from-scratch build.
if command -v bt-agent >/dev/null 2>&1; then
    systemctl enable bt-agent.service
else
    echo "bt-agent binary not present on base image; leaving bt-agent.service disabled"
fi
'

sync

echo "== Bake complete: ${OUT_IMG} =="

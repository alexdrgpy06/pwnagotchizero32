#!/bin/bash -e
# bake_on_pwnagotchi.sh — install the oxigotchi Rust daemon onto a proven,
# already-working pwnagotchi (Bullseye) base image, instead of building an OS
# from scratch. The base image already has a working kernel, nexmon firmware
# pairing, e-ink SPI setup, and USB-gadget/SSH access on both the original Pi
# Zero W and the Pi Zero 2 W — verified on real hardware. This script touches
# NOTHING but the daemon layer, deliberately, so none of that proven state can
# regress.
#
# Usage: sudo bash bake_on_pwnagotchi.sh <base-image.img> <oxigotchi-binary> <output.img>

set -euo pipefail

BASE_IMG="$1"
DAEMON_BIN="$2"
OUT_IMG="$3"
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

# Only bring in what our daemon actually needs beyond what pwnagotchi already
# ships: the daemon binary's own helper scripts (monstart/monstop) and our
# systemd unit. We deliberately do NOT copy config.txt/cmdline.txt changes,
# NetworkManager profiles, or nexmon firmware from our own overlay — those
# already exist, correctly, in this base image.
install -m 755 "${OVERLAY_DIR}/usr/bin/monstart" "${ROOT_MNT}/usr/bin/monstart" 2>/dev/null || true
install -m 755 "${OVERLAY_DIR}/usr/bin/monstop" "${ROOT_MNT}/usr/bin/monstop" 2>/dev/null || true

# Preserve pwnagotchi's own config for reference, then install ours.
if [ -f "${ROOT_MNT}/etc/pwnagotchi/config.toml" ] && [ ! -f "${ROOT_MNT}/etc/pwnagotchi/config.toml.pwnagotchi-orig" ]; then
    cp "${ROOT_MNT}/etc/pwnagotchi/config.toml" "${ROOT_MNT}/etc/pwnagotchi/config.toml.pwnagotchi-orig"
fi
install -d -m 755 "${ROOT_MNT}/etc/pwnagotchi/conf.d" "${ROOT_MNT}/etc/pwnagotchi/handshakes" \
    "${ROOT_MNT}/etc/pwnagotchi/log" "${ROOT_MNT}/etc/pwnagotchi/backups" "${ROOT_MNT}/etc/pwnagotchi/sessions"
install -m 644 "${OVERLAY_DIR}/etc/pwnagotchi/config.toml" "${ROOT_MNT}/etc/pwnagotchi/config.toml"

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

echo "== Disabling stock pwnagotchi/bettercap so oxigotchi owns the radio+display =="
# Mask (not just disable) pwnagotchi + bettercap directly via /dev/null
# symlinks — this is unambiguous and doesn't depend on systemctl's offline
# [Install]-section symlink handling working correctly under a qemu chroot.
# Deliberately NOT touching pwngrid-peer.service: it only maintains an
# identity keypair, is harmless to leave running, and disabling it in an
# earlier attempt correlated with a boot-time timeout — something else on
# this image likely waits on it reaching "active".
for unit in pwnagotchi.service bettercap.service; do
    ln -sf /dev/null "${ROOT_MNT}/etc/systemd/system/${unit}"
    # Remove any stale enablement symlinks pointing at the real unit file.
    find "${ROOT_MNT}/etc/systemd/system" -type l -lname "*/${unit}" -not -path "*/etc/systemd/system/${unit}" -delete 2>/dev/null || true
done

chroot "${ROOT_MNT}" /usr/bin/qemu-arm-static /bin/bash -c '
set +e
systemctl enable oxigotchi.service
'

sync

echo "== Bake complete: ${OUT_IMG} =="

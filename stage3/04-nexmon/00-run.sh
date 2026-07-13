#!/bin/bash -e
# 04-nexmon/00-run.sh - Build nexmon monitor-mode + injection firmware.
#
# Firmware-only install: we patch the brcmfmac firmware blobs (which is what
# enables frame injection / deauth) and keep the stock brcmfmac driver, which
# already supports monitor mode via `iw` on kernel 6.x. This deliberately
# avoids building/replacing the kernel module, so there is no kernel-version /
# vermagic matching to get wrong.
#
# Covers every Pi Zero 2 W wireless-chip revision (43436 / 43436s / 43430 /
# 43430b0) plus the original Pi Zero W (43430), using the DrSchottky/nexmon
# fork and toolchain (same as jayofelony/pwnagotchi).

on_chroot << 'CHROOT'
set -e
export DEBIAN_FRONTEND=noninteractive

apt-get update
# Full nexmon build dependency set (per the nexmon README + jayofelony), added
# in one shot to avoid discovering missing tools one CI cycle at a time.
apt-get install -y --no-install-recommends \
    git build-essential gcc-arm-none-eabi \
    gawk xxd qpdf bc \
    autoconf automake libtool texinfo bison flex libfl-dev pkg-config \
    libgmp3-dev libmpfr-dev libmpc-dev libisl-dev zlib1g-dev libssl-dev

# The nexmon b43 assembler links the lex library via the legacy "-ll" flag,
# but modern flex only ships libfl (no libl). Provide a libl.a -> libfl.a
# compatibility symlink so the link succeeds (arch-independent lookup).
LIBFL="$(find /usr/lib -name 'libfl.a' 2>/dev/null | head -1)"
if [ -n "$LIBFL" ]; then
    ln -sf "$LIBFL" "$(dirname "$LIBFL")/libl.a"
    echo "nexmon: linked $(dirname "$LIBFL")/libl.a -> $LIBFL"
else
    echo "nexmon: WARNING libfl.a not found; b43 assembler link may fail" >&2
fi

FWDIR=/usr/lib/firmware/brcm
SRC=/usr/local/src/nexmon
rm -rf "$SRC"
git clone --depth 1 https://github.com/DrSchottky/nexmon.git "$SRC"

cd "$SRC"
# Build the nexmon utilities / flashpatch toolchain (uses gcc-arm-none-eabi).
source ./setup_env.sh
make

install -d -m 755 "$FWDIR"

build_patch() {
    local chip="$1" ver="$2" out="$3"
    echo "nexmon: building firmware patch ${chip}/${ver}"
    ( source "${SRC}/setup_env.sh" && cd "${SRC}/patches/${chip}/${ver}/nexmon" && make )
    local built="${SRC}/patches/${chip}/${ver}/nexmon/${out}"
    if [ ! -f "$built" ]; then
        echo "nexmon: ERROR expected firmware ${out} not produced for ${chip}" >&2
        exit 1
    fi
    install -D -m 644 "$built" "${FWDIR}/${out}"
    echo "nexmon: installed ${FWDIR}/${out}"
}

# DIAGNOSTIC BUILD: the Pi Zero 2 W (bcm43436b0 chip) fails to boot on our
# images while it boots fine with jayofelony/pwnagotchi's image on the same
# SD card/power. Nexmon firmware is version-locked to a specific kernel, and
# our pi-gen build tracks the bookworm branch HEAD rather than a pinned kernel
# — a mismatched firmware/kernel pairing on this chip is a known way to crash
# the SDIO/WiFi subsystem hard enough to reset the board. Skipping the
# bcm43436b0 patch here (stock apt firmware-brcm80211 blob stays in place) to
# isolate whether this is the boot blocker. bcm43430a1 (OG Pi Zero W, which
# demonstrably boots) is unaffected and still gets patched.
# TODO: once confirmed, pin the pi-gen kernel version and re-enable this.
# build_patch bcm43436b0 9_88_4_65 brcmfmac43436-sdio.bin
# cp -f "${FWDIR}/brcmfmac43436-sdio.bin" "${FWDIR}/brcmfmac43436s-sdio.bin"

# Pi Zero W and older Pi Zero 2 W revisions.
build_patch bcm43430a1 7_45_41_46 brcmfmac43430-sdio.bin
cp -f "${FWDIR}/brcmfmac43430-sdio.bin" "${FWDIR}/brcmfmac43430b0-sdio.bin"

# Trim build artefacts and the largest build-only package to keep the image
# small; ignore errors so cleanup can't fail the build.
rm -rf "$SRC"
apt-get purge -y gcc-arm-none-eabi 2>/dev/null || true
apt-get autoremove -y 2>/dev/null || true
apt-get clean
rm -rf /var/lib/apt/lists/*

echo "nexmon: firmware install complete"
CHROOT

# Monitor-mode helper scripts (used by the daemon's mon_start/mon_stop hooks).
install -m 755 /dev/stdin "${ROOTFS_DIR}/usr/bin/monstart" << 'EOF'
#!/bin/bash
# Put the Wi-Fi radio into monitor mode as wlan0mon.
set -e
ip link set wlan0 down
iw dev wlan0 set type monitor
ip link set wlan0 name wlan0mon 2>/dev/null || true
ip link set wlan0mon up
EOF

install -m 755 /dev/stdin "${ROOTFS_DIR}/usr/bin/monstop" << 'EOF'
#!/bin/bash
# Return the radio to managed mode as wlan0.
set -e
ip link set wlan0mon down 2>/dev/null || true
iw dev wlan0mon set type managed 2>/dev/null || true
ip link set wlan0mon name wlan0 2>/dev/null || true
ip link set wlan0 up
EOF

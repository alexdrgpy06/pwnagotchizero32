#!/bin/bash -e
# 04-nexmon/00-run.sh - Install monitor-mode helper scripts.
#
# NOTE: The nexmon firmware build is intentionally deferred. Compiling nexmon
# (which bootstraps a gcc-4.9 cross toolchain) inside the qemu-emulated ARM
# chroot takes hours and is highly fragile, and it must match the exact kernel
# shipped in the image. Doing it here made the whole image build unbuildable.
#
# Without patched firmware the BCM43430/43436 radios still enter monitor mode
# for scanning via `iw`, but frame injection (deauth) needs nexmon. Track that
# as a follow-up; the image otherwise boots and runs the daemon + display.

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

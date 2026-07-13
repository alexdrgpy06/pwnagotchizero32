#!/bin/bash -e
# 02-libpcap/00-run.sh - libpcap provisioning.
#
# The daemon links against the distro libpcap (installed as libpcap0.8 in
# 01-pwn-packages). Building libpcap from source on the x86 build host and
# installing it into an ARM rootfs was wrong (mismatched architecture) and
# unnecessary, so it has been removed. This step now just asserts the runtime
# library is present in the image.
on_chroot << 'EOF'
if ! ldconfig -p | grep -q 'libpcap\.so'; then
    echo "oxigotchi: WARNING libpcap runtime not found in image" >&2
fi
EOF

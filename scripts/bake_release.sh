#!/bin/bash -e
# bake_release.sh - Build and package pwnagotchi-zero SD card image
# Usage: ./bake_release.sh --arch 32bit|64bit --release v1.0.0

set -euo pipefail

ARCH="32bit"
RELEASE="dev"
WORK_DIR="${WORK_DIR:-/workspace/work}"
DEPLOY_DIR="${DEPLOY_DIR:-/workspace/output}"
PI_GEN_DIR="${PI_GEN_DIR:-/pi-gen}"

usage() {
    echo "Usage: $0 --arch 32bit|64bit --release VERSION"
    echo "  --arch    Target architecture: 32bit (armhf) or 64bit (aarch64 kernel)"
    echo "  --release Release version tag (e.g., v1.0.0)"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --arch) ARCH="$2"; shift 2 ;;
        --release) RELEASE="$2"; shift 2 ;;
        *) usage ;;
    esac
done

echo "=== Pwnagotchi Zero Release Builder ==="
echo "=== net-monitor-zero Release Builder ==="
echo "Architecture: $ARCH"
echo "Release: $RELEASE"

# Determine config file
if [[ "$ARCH" == "32bit" ]]; then
    CONFIG_FILE="config/config-32bit"
    IMG_NAME="net-monitor-zero-32bit"
elif [[ "$ARCH" == "64bit" ]]; then
    CONFIG_FILE="config/config-64bit"
    IMG_NAME="net-monitor-zero-64bit"
else
    echo "Invalid arch: $ARCH"
    usage
fi

# Build Rust binary first
echo "=== Building Rust binary ==="
./scripts/build_oxigotchi.sh --arch "$ARCH"

# Copy binary to stage3
cp rust/target/*/release/oxigotchi stage3/05-install-oxigotchi/files/usr/local/bin/oxigotchi
chmod +x stage3/05-install-oxigotchi/files/usr/local/bin/oxigotchi

# Clone pi-gen if needed
if [[ ! -d "$PI_GEN_DIR" ]]; then
    echo "=== Cloning pi-gen ==="
    git clone --depth 1 https://github.com/RPi-Distro/pi-gen.git "$PI_GEN_DIR"
fi

# Copy stage3 to pi-gen
echo "=== Preparing pi-gen stages ==="
rsync -a --delete stage3/ "$PI_GEN_DIR/stage3/"

# Copy config
cp "$CONFIG_FILE" "$PI_GEN_DIR/config"

# Build image
echo "=== Building image (this takes 30-60 minutes) ==="
cd "$PI_GEN_DIR"
./build.sh -c config

# Move and compress output
echo "=== Compressing image ==="
OUTPUT_IMG=$(ls -t deploy/*.img | head -1)
if [[ -f "$OUTPUT_IMG" ]]; then
    FINAL_NAME="${IMG_NAME}-${RELEASE}.img"
    mv "$OUTPUT_IMG" "$DEPLOY_DIR/$FINAL_NAME"
    cd "$DEPLOY_DIR"
    xz -T0 "$FINAL_NAME"
    echo "=== Done: $DEPLOY_DIR/${FINAL_NAME}.xz ==="
    
    # Generate SHA256
    sha256sum "${FINAL_NAME}.xz" > "${FINAL_NAME}.xz.sha256"
    echo "SHA256: $(cat ${FINAL_NAME}.xz.sha256)"
else
    echo "ERROR: No image found in deploy/"
    exit 1
fi
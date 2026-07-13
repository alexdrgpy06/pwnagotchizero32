#!/bin/bash -e
# build_oxigotchi.sh - Cross-compile oxigotchi for ARM

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RUST_DIR="$PROJECT_ROOT/rust"

ARCH="32bit"
RELEASE=true

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --debug)
            RELEASE=false
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

cd "$RUST_DIR"

# Install targets
rustup target add arm-unknown-linux-gnueabihf
rustup target add aarch64-unknown-linux-gnu

export CC_arm_unknown_linux_gnueabihf="arm-linux-gnueabihf-gcc"
export CC_aarch64_unknown_linux_gnu="aarch64-linux-gnu-gcc"

# Install cross-compilation toolchain
apt-get update && apt-get install -y \
    gcc-arm-linux-gnueabihf \
    gcc-aarch64-linux-gnu \
    libc6-dev-armhf-cross \
    libc6-dev-arm64-cross

if [[ "$ARCH" == "32bit" ]]; then
    TARGET="arm-unknown-linux-gnueabihf"
    LINKER="arm-linux-gnueabihf-gcc"
    CARGO_TARGET="arm-unknown-linux-gnueabihf"
else
    TARGET="aarch64-unknown-linux-gnu"
    LINKER="aarch64-linux-gnu-gcc"
    CARGO_TARGET="aarch64-unknown-linux-gnu"
fi

# Set up cargo config for cross-compilation
mkdir -p .cargo
cat > .cargo/config.toml << EOF
[target.$CARGO_TARGET]
linker = "$LINKER"
rustflags = ["-C", "link-arg=-s"]
EOF

# Build
if [[ "$RELEASE" == "true" ]]; then
    cargo build --release --target "$TARGET"
else
    cargo build --target "$TARGET"
fi

# Copy binary
if [[ "$RELEASE" == "true" ]]; then
    BINARY="target/$TARGET/release/oxigotchi"
else
    BINARY="target/$TARGET/debug/oxigotchi"
fi

if [[ -f "$BINARY" ]]; then
    echo "Binary built: $BINARY"
    echo "Size: $(du -h "$BINARY" | cut -f1)"
else
    echo "Build failed: binary not found"
    exit 1
fi
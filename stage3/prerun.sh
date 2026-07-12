#!/bin/bash -e
# prerun.sh - runs before stage3 build

# Install build dependencies
apt-get update
apt-get install -y \
    git \
    curl \
    wget \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    libclang-dev \
    clang \
    llvm \
    libudev-dev \
    libsqlite3-dev \
    libpcap-dev \
    libbluetooth-dev \
    libdbus-1-dev \
    libi2c-dev \
    python3 \
    python3-pip \
    python3-venv \
    lua5.4 \
    liblua5.4-dev \
    cargo \
    rustup

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup target add armv7-unknown-linux-gnueabihf
rustup target add aarch64-unknown-linux-gnu

# Install cross-compilation toolchain
apt-get install -y gcc-arm-linux-gnueabihf gcc-aarch64-linux-gnu

# Install pi-gen dependencies
apt-get install -y \
    qemu-user-static \
    binfmt-support \
    debootstrap \
    kpartx \
    dosfstools \
    xz-utils \
    zip

echo "Build environment ready"
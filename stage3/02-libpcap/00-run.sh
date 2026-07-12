#!/bin/bash -e
# 02-libpcap/00-run.sh - Build and install libpcap with optimizations

cd /tmp
git clone --depth 1 --branch libpcap-1.10.4 https://github.com/the-tcpdump-group/libpcap.git
cd libpcap
./configure --prefix=/usr --enable-shared --disable-yydebug --disable-universal
make -j$(nproc)
make install
ldconfig
cd /
rm -rf /tmp/libpcap
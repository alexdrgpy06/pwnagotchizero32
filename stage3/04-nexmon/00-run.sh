#!/bin/bash -e
# 04-nexmon/00-run.sh - Install nexmon firmware for monitor mode

cd /tmp
git clone --depth 1 https://github.com/seemoo-lab/nexmon.git
cd nexmon

# Build for Pi Zero W (BCM43430) and Pi Zero 2W (BCM43436)
# We need the firmware patches for monitor mode
source setup_env.sh
make -C buildtools/isl-0.10
make -C buildtools/mpc-1.0.3
make -C buildtools/mpfr-3.1.4
make -C buildtools/gmp-6.1.2
make -C buildtools/cloog-0.18.4
make -C buildtools/gcc-4.9.4

# Build firmware for BCM43436 (Pi Zero 2W)
cd firmware/brcmfmac43436-sdio
make clean
make -j$(nproc)
cp brcmfmac43436-sdio.bin /lib/firmware/brcm/brcmfmac43436-sdio.bin

# Build firmware for BCM43430 (Pi Zero W)
cd ../brcmfmac43430-sdio
make clean
make -j$(nproc)
cp brcmfmac43430-sdio.bin /lib/firmware/brcm/brcmfmac43430-sdio.bin

# Also copy NVRAM configs
cp ../brcmfmac43436-sdio.txt /lib/firmware/brcm/brcmfmac43436-sdio.txt
cp ../brcmfmac43430-sdio.txt /lib/firmware/brcm/brcmfmac43430-sdio.txt

# Create monitor mode enable script
cat > /usr/bin/monstart << 'EOF'
#!/bin/bash
# Start monitor mode on wlan0
ip link set wlan0 down
iw dev wlan0 set type monitor
ip link set wlan0 name wlan0mon
ip link set wlan0mon up
# Enable channel hopping
iw dev wlan0mon set freq 2412
EOF
chmod +x /usr/bin/monstart

cat > /usr/bin/monstop << 'EOF'
#!/bin/bash
# Stop monitor mode
ip link set wlan0mon down
iw dev wlan0mon set type managed
ip link set wlan0mon name wlan0
ip link set wlan0 up
EOF
chmod +x /usr/bin/monstop

cd /
rm -rf /tmp/nexmon
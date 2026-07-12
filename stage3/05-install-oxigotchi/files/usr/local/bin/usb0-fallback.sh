#!/bin/bash
# usb0-fallback.sh - USB gadget fallback for when BT tether fails

set -euo pipefail

USB_IFACE="usb0"
USB_IP="192.168.42.1"
USB_NETMASK="255.255.255.0"
DHCP_RANGE_START="192.168.42.10"
DHCP_RANGE_END="192.168.42.50"

# Check if dwc2 gadget module is loaded
if ! lsmod | grep -q dwc2; then
    modprobe dwc2
fi

if ! lsmod | grep -q libcomposite; then
    modprobe libcomposite
fi

# Create USB gadget
GADGET_DIR="/sys/kernel/config/usb_gadget/pwnagotchi"
mkdir -p "$GADGET_DIR"

# Set vendor/product IDs (Raspberry Pi Foundation)
echo 0x1d6b > "$GADGET_DIR/idVendor"  # Linux Foundation
echo 0x0104 > "$GADGET_DIR/idProduct" # Multifunction Composite Gadget
echo 0x0100 > "$GADGET_DIR/bcdDevice"
echo 0x0200 > "$GADGET_DIR/bcdUSB"

# Strings
mkdir -p "$GADGET_DIR/strings/0x409"
echo "deadbeef00115599" > "$GADGET_DIR/strings/0x409/serialnumber"
echo "Raspberry Pi" > "$GADGET_DIR/strings/0x409/manufacturer"
echo "Pwnagotchi Zero" > "$GADGET_DIR/strings/0x409/product"

# Config
mkdir -p "$GADGET_DIR/configs/c.1/strings/0x409"
echo "USB OTG" > "$GADGET_DIR/configs/c.1/strings/0x409/configuration"
echo 250 > "$GADGET_DIR/configs/c.1/MaxPower"

# Function: ECM (Ethernet)
mkdir -p "$GADGET_DIR/functions/ecm.usb0"
# Use a fixed MAC
echo "02:00:00:00:00:01" > "$GADGET_DIR/functions/ecm.usb0/host_addr"
echo "02:00:00:00:00:02" > "$GADGET_DIR/functions/ecm.usb0/dev_addr"

# Link function to config
ln -sf "$GADGET_DIR/functions/ecm.usb0" "$GADGET_DIR/configs/c.1/"

# Enable gadget
UDC=$(ls /sys/class/udc/ | head -1)
if [[ -n "$UDC" ]]; then
    echo "$UDC" > "$GADGET_DIR/UDC"
fi

# Configure interface
sleep 2
if ip link show "$USB_IFACE" &>/dev/null; then
    ip addr add "$USB_IP/24" dev "$USB_IFACE" 2>/dev/null || true
    ip link set "$USB_IFACE" up
    
    # Start dnsmasq for DHCP
    cat > /etc/dnsmasq.d/pwnagotchi-usb0.conf << EOF
interface=$USB_IFACE
dhcp-range=$DHCP_RANGE_START,$DHCP_RANGE_END,255.255.255.0,1h
dhcp-option=3,$USB_IP
dhcp-option=6,$USB_IP
server=8.8.8.8
server=1.1.1.1
log-queries
log-dhcp
EOF
    
    systemctl restart dnsmasq || true
    
    echo "USB gadget enabled on $USB_IFACE ($USB_IP)"
else
    echo "USB interface $USB_IFACE not found"
fi
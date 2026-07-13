#!/bin/bash
# usb-gadget.sh - Build a CDC-NCM USB-ethernet gadget via configfs, so the Pi
# appears as a network adapter over the USB data port.
#
# NCM is natively supported by Windows 10 (1809+) / 11, macOS and Linux with no
# driver install. We use it because g_ether/CDC-ECM shows up as a serial COM
# port on Windows, and modern Windows blocks automatic RNDIS driver install.

set -e

GADGET=/sys/kernel/config/usb_gadget/oxigotchi

modprobe libcomposite || true

# Already set up (service re-run) — nothing to do.
[ -d "${GADGET}" ] && exit 0

# Wait for the dwc2 UDC to appear (module may load slightly after us).
UDC=""
for _ in $(seq 1 30); do
    UDC=$(ls /sys/class/udc 2>/dev/null | head -1 || true)
    [ -n "${UDC}" ] && break
    sleep 1
done
if [ -z "${UDC}" ]; then
    echo "usb-gadget: no UDC found; is dtoverlay=dwc2 set?" >&2
    exit 0
fi

mkdir -p "${GADGET}"
cd "${GADGET}"

echo 0x1d6b > idVendor            # Linux Foundation
echo 0x0104 > idProduct           # Multifunction Composite Gadget
echo 0x0100 > bcdDevice
echo 0x0200 > bcdUSB

# Communications device class with IAD (NCM presents a control + data iface).
echo 0xEF > bDeviceClass
echo 0x02 > bDeviceSubClass
echo 0x01 > bDeviceProtocol

mkdir -p strings/0x409
echo "0123456789abcdef" > strings/0x409/serialnumber
echo "oxigotchi"        > strings/0x409/manufacturer
echo "oxigotchi USB"    > strings/0x409/product

mkdir -p configs/c.1/strings/0x409
echo "CDC-NCM" > configs/c.1/strings/0x409/configuration
echo 250       > configs/c.1/MaxPower

mkdir -p functions/ncm.usb0
# Fixed locally-administered MACs so the interface name/IP stay stable.
echo "42:61:64:55:53:42" > functions/ncm.usb0/host_addr
echo "42:61:64:55:53:43" > functions/ncm.usb0/dev_addr

ln -s functions/ncm.usb0 configs/c.1/

udevadm settle -t 5 2>/dev/null || true
echo "${UDC}" > UDC

echo "usb-gadget: CDC-NCM gadget bound to ${UDC}"
exit 0

#!/bin/bash
# usb-gadget.sh - Build a Windows-friendly RNDIS USB-ethernet gadget via
# configfs, so the Pi appears as a proper network adapter (not a COM port)
# when plugged into a host's USB data port. RNDIS + Microsoft OS descriptors
# let Windows 10/11 auto-install the driver; macOS/Linux also accept RNDIS.

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

# Composite class + IAD — required for Windows to load the RNDIS driver.
echo 0xEF > bDeviceClass
echo 0x02 > bDeviceSubClass
echo 0x01 > bDeviceProtocol

mkdir -p strings/0x409
echo "0123456789abcdef" > strings/0x409/serialnumber
echo "oxigotchi"        > strings/0x409/manufacturer
echo "oxigotchi USB"    > strings/0x409/product

mkdir -p configs/c.1/strings/0x409
echo "RNDIS" > configs/c.1/strings/0x409/configuration
echo 250     > configs/c.1/MaxPower

# Microsoft OS descriptors -> Windows silently installs the RNDIS driver.
echo 1       > os_desc/use
echo 0xcd    > os_desc/b_vendor_code
echo MSFT100 > os_desc/qw_sign

mkdir -p functions/rndis.usb0
# Fixed locally-administered MACs so the interface name/IP stay stable.
echo "42:61:64:55:53:42" > functions/rndis.usb0/host_addr
echo "42:61:64:55:53:43" > functions/rndis.usb0/dev_addr
echo RNDIS   > functions/rndis.usb0/os_desc/interface.rndis/compatible_id
echo 5162001 > functions/rndis.usb0/os_desc/interface.rndis/sub_compatible_id

ln -s functions/rndis.usb0 configs/c.1/
ln -s configs/c.1 os_desc/

udevadm settle -t 5 2>/dev/null || true
echo "${UDC}" > UDC

echo "usb-gadget: RNDIS gadget bound to ${UDC}"
exit 0

#!/bin/bash
# usb-gadget.sh - Build an RNDIS USB-ethernet gadget AND bring usb0 up with a
# static IP + its own DHCP server, fully self-contained (no NetworkManager
# dependency, which was leaving usb0 down / without an address).
#
# RNDIS + Microsoft OS descriptors so Windows binds it as a network adapter.
# The Pi is 10.0.0.2; the host gets 10.0.0.10-30 via dnsmasq, so `ssh
# pi@10.0.0.2` works with no manual IP setup.

set -e

GADGET=/sys/kernel/config/usb_gadget/oxigotchi

modprobe libcomposite || true

if [ ! -d "${GADGET}" ]; then
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

    echo 0x1d6b > idVendor
    echo 0x0104 > idProduct
    echo 0x0100 > bcdDevice
    echo 0x0200 > bcdUSB
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

    # Microsoft OS descriptors so Windows installs the RNDIS network driver.
    echo 1       > os_desc/use
    echo 0xcd    > os_desc/b_vendor_code
    echo MSFT100 > os_desc/qw_sign

    mkdir -p functions/rndis.usb0
    echo "42:61:64:55:53:42" > functions/rndis.usb0/host_addr
    echo "42:61:64:55:53:43" > functions/rndis.usb0/dev_addr
    echo RNDIS   > functions/rndis.usb0/os_desc/interface.rndis/compatible_id
    echo 5162001 > functions/rndis.usb0/os_desc/interface.rndis/sub_compatible_id

    ln -s functions/rndis.usb0 configs/c.1/
    ln -s configs/c.1 os_desc/

    udevadm settle -t 5 2>/dev/null || true
    echo "${UDC}" > UDC
fi

# Bring usb0 up with static IPs once the interface exists.
for _ in $(seq 1 15); do
    [ -e /sys/class/net/usb0 ] && break
    sleep 1
done
if [ -e /sys/class/net/usb0 ]; then
    ip link set usb0 up || true
    ip addr flush dev usb0 2>/dev/null || true
    # 10.0.0.2 is our own scheme (DHCP served below). 192.168.137.2 is a
    # second static address on the SAME subnet Windows Internet Connection
    # Sharing defaults to (host 192.168.137.1) — if Windows auto-activates ICS
    # on this adapter and takes over addressing, the Pi is still reachable at
    # a fixed IP in that subnet without the user having to configure anything.
    ip addr add 10.0.0.2/24 dev usb0 || true
    ip addr add 192.168.137.2/24 dev usb0 || true

    # Best-effort default route via the ICS gateway, at a deliberately high
    # (low-priority) metric so it only kicks in when nothing better already
    # provides a default route (BT tether, a wlan0 client connection, etc.)
    # — `add`, never `replace`, so this can't hijack routing away from a
    # working connection. Confirmed on real hardware: with ICS active on the
    # host, this is the difference between NTP never syncing at all (no
    # route out of usb0 whatsoever, clock stuck on whatever fake-hwclock
    # last saved — every log timestamp reading "2023" instead of the real
    # date) and it syncing within seconds. Silent no-op when ICS isn't
    # enabled — 192.168.137.1 just won't answer ARP and the route sits unused.
    ip route add default via 192.168.137.1 dev usb0 metric 400 2>/dev/null || true

    # Hand the connected host an address on our subnet so `ssh pi@10.0.0.2`
    # works with no manual IP setup, when ICS hasn't already claimed the link.
    if command -v dnsmasq >/dev/null 2>&1; then
        pkill -f "dnsmasq.*usb0" 2>/dev/null || true
        dnsmasq --interface=usb0 --bind-interfaces --except-interface=lo \
            --dhcp-range=10.0.0.10,10.0.0.30,255.255.255.0,1h \
            --dhcp-option=3 --dhcp-option=6 \
            --no-resolv --no-hosts --leasefile-ro \
            --pid-file=/run/usb0-dnsmasq.pid 2>/dev/null || true
    fi
    echo "usb-gadget: usb0 up at 10.0.0.2 + 192.168.137.2 (dhcp 10.0.0.10-30)"
else
    echo "usb-gadget: usb0 interface never appeared" >&2
fi
exit 0

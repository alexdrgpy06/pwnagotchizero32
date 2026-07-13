#!/bin/bash
# deploy-oxigotchi.sh — install the oxigotchi daemon on top of a working
# pwnagotchi (Bullseye) image. Run over SSH on the Pi:
#
#   curl -L -o oxigotchi https://github.com/alexdrgpy06/pwnagotchizero32/releases/latest/download/oxigotchi
#   curl -L -o deploy-oxigotchi.sh https://github.com/alexdrgpy06/pwnagotchizero32/releases/latest/download/deploy-oxigotchi.sh
#   sudo bash deploy-oxigotchi.sh
#
# The base pwnagotchi image already provides a booting OS, working e-ink SPI,
# WiFi firmware, and USB/SSH access — this just puts our Rust daemon in charge.
set -e

BIN_SRC="${1:-./oxigotchi}"

if [ "$(id -u)" -ne 0 ]; then
    echo "Run with sudo." >&2
    exit 1
fi
if [ ! -f "${BIN_SRC}" ]; then
    echo "oxigotchi binary not found at ${BIN_SRC}" >&2
    exit 1
fi

echo "== Stopping the stock pwnagotchi daemon =="
systemctl stop pwnagotchi 2>/dev/null || true
systemctl disable pwnagotchi 2>/dev/null || true
# bettercap grabs the radio; keep it off so our daemon owns wlan0.
systemctl stop bettercap 2>/dev/null || true
systemctl disable bettercap 2>/dev/null || true

echo "== Installing the oxigotchi binary =="
install -m 755 "${BIN_SRC}" /usr/local/bin/oxigotchi

echo "== Installing config (separate from pwnagotchi's) =="
install -d -m 755 /etc/oxigotchi
if [ ! -f /etc/oxigotchi/config.toml ] && [ -f ./config.toml ]; then
    install -m 644 ./config.toml /etc/oxigotchi/config.toml
fi
# Runtime dirs the daemon expects.
install -d -m 755 /etc/pwnagotchi/handshakes /etc/pwnagotchi/log \
    /etc/pwnagotchi/conf.d /etc/pwnagotchi/sessions /etc/pwnagotchi/backups

echo "== Installing systemd service =="
cat > /etc/systemd/system/oxigotchi.service << 'UNIT'
[Unit]
Description=oxigotchi daemon (Rust pwnagotchi)
After=network.target bluetooth.target

[Service]
Type=simple
Environment=PWNAGOTCHI_CONFIG=/etc/oxigotchi/config.toml
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1
ExecStart=/usr/local/bin/oxigotchi
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable oxigotchi.service
systemctl restart oxigotchi.service

echo
echo "== Done. Check it: =="
echo "  systemctl status oxigotchi --no-pager"
echo "  journalctl -u oxigotchi -b --no-pager | tail -40"
echo
echo "To go back to stock pwnagotchi:"
echo "  sudo systemctl disable --now oxigotchi && sudo systemctl enable --now pwnagotchi"

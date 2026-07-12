# Pwnagotchi Zero

Modern pwnagotchi for Raspberry Pi Zero 2W / Zero W (32-bit) with Waveshare V4 e-ink, PiSugar UPS, Bluetooth PAN tethering, and local hashcat sync.

## Features

- **Rust core** — Single binary (~1.2MB), no Python runtime, fast and memory-efficient
- **32-bit ARM support** — Runs on Pi Zero W (armv6) and Pi Zero 2W (armv7/aarch64 kernel)
- **Waveshare 2.13" V4 e-ink** — 250×122, partial refresh, kaomoji + PNG faces
- **PiSugar 3/2/S UPS** — Battery %, button actions, watchdog, wake timer via I2C
- **Bluetooth PAN tether** — Auto-pair phone, internet for wpa-sec/pwncrack upload
- **SD card longevity** — zram for logs/data, rsync sync every 60s + on shutdown
- **Handshake capture** — PMKID, full/half WPA handshakes in pcapng
- **Cloud cracking** — wpa-sec.stanev.org, pwncrack.org upload + result download
- **Local hashcat sync** — rsync captures to local cracking box, pull potfile
- **Web dashboard** — htmx + Alpine.js, real-time WS updates, optional auth
- **Lua plugin API** — Drop-in compatible with pwnagotchi plugins
- **Config migration** — Import legacy pwnagotchi config.toml

## Hardware

| Component | Tested Models |
|-----------|---------------|
| Pi | Zero W, Zero 2 W |
| Display | Waveshare 2.13" V4 (SSD1680, SPI) |
| UPS | PiSugar 3, PiSugar 2, PiSugar S |
| SD Card | 8GB+ Class 10 / A1/A2 |

## Quick Start

### Flash Pre-built Image

```bash
# Download latest release
wget https://github.com/pwnagotchi-zero/pwnagotchi-zero/releases/latest/download/pwnagotchi-zero-32bit-v1.0.0.img.xz

# Flash to SD card (replace /dev/sdX)
xzcat pwnagotchi-zero-32bit-v1.0.0.img.xz | sudo dd of=/dev/sdX bs=4M status=progress conv=fsync
```

### First Boot

1. Insert SD card, attach display + PiSugar
2. Power on — boot face appears in ~15s
3. Connect via SSH: `ssh pi@pwnagotchi.local` (default password: `raspberry`)
4. Configure: `sudo nano /etc/pwnagotchi/config.toml`
5. Pair phone: `bluetoothctl` → `scan on` → `pair <MAC>` → `trust <MAC>`
6. Enable BT tether in config: `bt_tether_enabled = true`
7. Reboot: `sudo reboot`

### Web Dashboard

Open `http://pwnagotchi.local:8080` — shows epoch, mood, handshakes, battery, handshake list with download.

## Configuration

Main config: `/etc/pwnagotchi/config.toml` (pwnagotchi-compatible TOML)

Drop-ins: `/etc/pwnagotchi/conf.d/*.toml` (merged alphabetically)

Key sections:
```toml
[main]
name = "my-pwnagotchi"
iface = "wlan0mon"
whitelist = ["HOME_WIFI", "aa:bb:cc:dd:ee:ff"]

[main.plugins.bt-tether]
enabled = true
auto_reconnect = true

[main.plugins.wpa-sec]
enabled = true
api_key = "YOUR_KEY"

[main.plugins.hashcat-sync]
enabled = true
remote_host = "hashcat.local"
remote_user = "hashcat"
remote_path = "/home/hashcat/captures"

[oxigotchi]
bt_tether_enabled = true
pisugar_i2c_addr = 0x24
epoch_duration = 30
attack_rate_limit = 1
display_partial_refresh = true
```

## Plugins

Place `.lua` files in `/usr/local/share/pwnagotchi/custom-plugins/` or `/etc/pwnagotchi/custom-plugins/`

Built-in plugins:
- `bt-tether.lua` — Bluetooth PAN status on display
- `wpa-sec.lua` — Upload to wpa-sec, download cracked
- `pwncrack.lua` — Upload to pwncrack.org
- `hashcat-sync.lua` — rsync to local hashcat box
- `auto-backup.lua` — Hourly config/handshake backup
- `memtemp.lua` — RAM/CPU temp on display

## Building from Source

### Prerequisites

- Linux x86_64 host (Debian/Ubuntu recommended)
- Docker (for pi-gen build)
- Rust 1.75+ (for cross-compilation)

### Build

```bash
# Clone
git clone https://github.com/pwnagotchi-zero/pwnagotchi-zero
cd pwnagotchi-zero

# Build Rust binary for 32-bit ARM
./scripts/build_oxigotchi.sh --arch 32bit

# Build full SD image (requires pi-gen, ~45 min)
./scripts/bake_release.sh --arch 32bit --release v1.0.0
```

Output: `output/pwnagotchi-zero-32bit-v1.0.0.img.xz`

### Cross-compile only

```bash
# 32-bit (Pi Zero W / Zero 2W 32-bit userland)
./scripts/build_oxigotchi.sh --arch 32bit

# 64-bit kernel / 32-bit userland (Pi Zero 2W optimized)
./scripts/build_oxigotchi.sh --arch 64bit
```

Binaries: `rust/target/armv7-unknown-linux-gnueabihf/release/oxigotchi` or `aarch64-unknown-linux-gnu/release/oxigotchi`

## Architecture

```
pwnagotchi-zero/
├── rust/                    # Oxigotchi core (Rust)
│   ├── src/
│   │   ├── main.rs          # Daemon entry, epoch loop
│   │   ├── config/          # TOML config with conf.d overlay
│   │   ├── epoch.rs         # Scan→Attack→Capture→Display→Sleep
│   │   ├── display/         # SSD1680 driver, framebuffer, API
│   │   ├── wifi/            # Monitor mode, channel hop, AP tracking
│   │   ├── attacks/         # Rate-limited deauth/assoc (rate=1)
│   │   ├── capture/         # pcapng management, hccapx conversion
│   │   ├── personality/     # Mood, XP, level, 28 kaomoji faces
│   │   ├── bluetooth/       # BlueZ D-Bus PAN, bt-agent
│   │   ├── pisugar/         # I2C battery, button, watchdog
│   │   ├── recovery/        # SDIO reset, GPIO power-cycle, watchdog
│   │   ├── plugins/         # Lua VM (mlua) + native trait
│   │   ├── web/             # axum + htmx dashboard
│   │   └── migration/       # Legacy config import
├── stage3/                  # pi-gen stage3 overlay
│   └── 05-install-oxigotchi/ # Binary, systemd, configs, plugins
├── scripts/                 # Build helpers
└── docker/                  # CI build container
```

## SD Card Longevity

| Mount | Backing | Sync | Writes/day |
|-------|---------|------|------------|
| `/etc/pwnagotchi/log` | zram 50M | 60s | < 1 MB |
| `/var/tmp/pwnagotchi` | zram 10M | 1h | < 0.5 MB |
| Handshakes | SD (ext4) | immediate | ~5 MB |

Noatime, journald volatile, swap disabled.

## WiFi Recovery

BCM43436 firmware hangs → SDIO errors → SD corruption on forced reboot.

**Recovery flow:**
1. Watchdog detects 3+ firmware errors in 60s
2. Soft recovery: `modprobe -r brcmfmac` → GPIO 4 (WL_REG_ON) low 500ms → high → reload
3. After 3 soft recoveries: display "ZOMBIE - UNPLUG USB+BATT 30s", require power cycle
4. PiSugar hardware switch or JST disconnect for true power-off

## License

GPL-3.0-or-later — based on oxigotchi (Rust) and pwnagotchi (Python/plugins)

## Credits

- **oxigotchi** — CoderFX (Rust core, display, PiSugar, BT)
- **pwnagotchi** — jayofelony / evilsocket (plugins, config format)
- **AngryOxide** — Ragnt (802.11 attack engine)
- **nexmon** — SEEMOO Lab (monitor mode firmware)
- **wpa-sec** — Stanev (distributed cracking)
- **PiSugar** — PiSugar team (UPS HAT)

## Support

- GitHub Issues: https://github.com/pwnagotchi-zero/pwnagotchi-zero/issues
- Discord: #pwnagotchi on pwnagotchi Discord
- Reddit: r/pwnagotchi

---

**Authorized use only.** Only test on networks you own or have explicit written permission to audit.
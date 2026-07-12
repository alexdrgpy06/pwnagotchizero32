# Pwnagotchi Zero - Project Structure

## Created Files (66 files)

### Core Spec & Documentation
- `SPEC.md` - Complete technical specification (1200+ lines)
- `README.md` - User documentation with hardware, config, build instructions
- `Makefile` - Build automation targets

### Rust Daemon (oxigotchi-core)
```
rust/
├── Cargo.toml              # Dependencies, release profile (LTO, strip, panic=abort)
├── defaults.toml           # Full default config (pwnagotchi-compatible)
├── src/
│   ├── main.rs             # Daemon entry, signal handling, subsystem init
│   ├── config/mod.rs       # TOML config loader with conf.d overlay
│   ├── epoch.rs            # Epoch state machine (scan→attack→capture→display→sleep)
│   ├── display/
│   │   ├── mod.rs          # Display wrapper
│   │   ├── driver.rs       # SSD1680 SPI driver (Waveshare V4)
│   │   ├── buffer.rs       # 1-bit packed framebuffer (DrawTarget impl)
│   │   └── api.rs          # High-level drawing (faces, text, status)
│   ├── wifi/mod.rs         # Monitor mode, channel hop (1-13), AP/client tracking
│   ├── attacks/mod.rs      # Rate-limited deauth/assoc via aireplay-ng
│   ├── capture/mod.rs      # pcapng management, hccapx conversion, upload queue
│   ├── personality/mod.rs  # Mood, XP, level, 28 kaomoji faces + PNG support
│   ├── bluetooth/mod.rs    # BlueZ D-Bus PAN tether, bt-agent auto-pair
│   ├── pisugar/mod.rs      # I2C battery %, button, watchdog, wake timer
│   ├── recovery/mod.rs     # BCM43436 firmware hang detection + GPIO power-cycle
│   ├── plugins/mod.rs      # Lua VM (mlua) + native Rust plugin trait
│   ├── web/mod.rs          # axum HTTP + WS dashboard (htmx + Alpine.js)
│   ├── migration/mod.rs    # Import legacy pwnagotchi config/handshakes
│   └── qpu/                # Optional QPU-assisted capture (disabled by default)
└── templates/
    └── dashboard.html      # Embedded web UI template
```

### pi-gen Stage3 Overlay (SD Image Build)
```
stage3/
├── prerun.sh                      # Build deps, Rust, cross-compile toolchain
├── 00-pre-pwn/00-run.sh           # Enable SPI/I2C, disable swap, zram setup
├── 01-pwn-packages/00-run.sh      # bettercap, hcxtools, bluez, lua5.4, etc.
├── 02-libpcap/00-run.sh           # Build libpcap from source
├── 03-bettercap-pwngrid/00-run.sh # Disable bettercap service, config
├── 04-nexmon/00-run.sh            # Build nexmon firmware for BCM43436/43430
├── 05-install-oxigotchi/
│   ├── 00-run.sh                  # Install binary, configs, systemd, scripts
│   └── files/                     # Full rootfs overlay:
│       ├── etc/
│       │   ├── pwnagotchi/
│       │   │   ├── config.toml    # Full default config
│       │   │   ├── conf.d/        # Drop-in configs
│       │   │   └── custom-plugins/ # Lua plugins + faces/
│       │   ├── NetworkManager/conf.d/99-bt-pan.conf
│       │   ├── systemd/system/    # 12 services (oxigotchi, bt-agent, bt-pan@, zram-*, safe-shutdown, etc.)
│       │   └── cron.d/rsync-zram
│       ├── lib/systemd/system-shutdown/safe-shutdown.sh
│       └── usr/local/bin/         # 11 helper scripts
├── 06-hcxtools/
├── 07-patches/
├── 08-pwnstore/
├── EXPORT_IMAGE
└── prerun.sh
```

### Build Scripts
- `scripts/build_oxigotchi.sh` - Cross-compile for armv7/aarch64
- `scripts/bake_release.sh` - Full pi-gen image build + xz compression

### Docker
- `docker/Dockerfile.build` - Build environment with all deps

### Lua Plugins (pwnagotchi-compatible)
- `bt-tether.lua` - Bluetooth PAN status on display
- `wpa-sec.lua` - Upload to wpa-sec, download cracked passwords
- `pwncrack.lua` - Upload to pwncrack.org
- `hashcat-sync.lua` - rsync captures to local hashcat, pull potfile
- `auto-backup.lua` - Hourly config/handshake tarballs
- `memtemp.lua` - RAM/CPU temp display

### Systemd Services (12)
| Service | Purpose |
|---------|---------|
| oxigotchi.service | Core daemon (Type=notify, Nice=-5, MemoryMax=200M) |
| bt-agent.service | Bluetooth auto-pair agent (NoInputNoOutput) |
| bt-pan@.service | PAN template per device MAC |
| epd-startup.service | Boot splash on e-ink |
| zram-log.service | 50MB zram for /etc/pwnagotchi/log |
| zram-data.service | 10MB zram for /var/tmp/pwnagotchi |
| safe-shutdown.service | Sync zram→disk on shutdown/reboot/halt |
| nm-watchdog.service | Restart NetworkManager on failure |
| rsync-zram.timer | Sync zram to disk every 60s |

### Config Highlights (/etc/pwnagotchi/config.toml)
- pwnagotchi-compatible TOML structure
- `oxigotchi.*` section for Rust-specific settings
- zram mounts with rsync sync (60s + shutdown)
- wpa-sec, pwncrack, hashcat-sync plugins enabled by default
- PiSugar I2C 0x24, button hold 3s = shutdown
- WiFi recovery GPIO 4 (WL_REG_ON), 3 soft recoveries then hard reset
- Attack rate limit: 1 deauth/epoch (BCM43436 safe)

## Build Instructions

```bash
# Cross-compile Rust binary
make build-rust

# Build 32-bit SD image (Pi Zero W / Zero 2W)
make build-32bit VERSION=v1.0.0

# Build 64-bit kernel image (Pi Zero 2W optimized)
make build-64bit VERSION=v1.0.0

# Or directly:
./scripts/bake_release.sh --arch 32bit --release v1.0.0
```

## Key Design Decisions (Ponytail Full)

1. **Rust over Python** - Single binary, no GIL, predictable memory, ~1.2MB
2. **zram + rsync** - SD writes <10MB/day vs ~100MB+ on stock pwnagotchi
3. **AngryOxide/bettercap disabled on device** - Attacks via rate-limited aireplay-ng
4. **Bluetooth before WiFi monitor** - BCM43436 UART shared; bt-agent + bt-network before monstart
5. **Lua plugin API** - Drop-in compatible with jayofelony/pwnagotchi plugins
6. **Boot splash + safe shutdown** - No corrupted FS on power loss
7. **WiFi firmware recovery** - Prevents "zombie" state requiring 30-60s power-off
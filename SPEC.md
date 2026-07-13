# SPEC: pwnagotchi-zero — Modern 32-bit Pwnagotchi for Pi Zero 2W / Zero W

## 1. Project Overview

**Project name:** `pwnagotchi-zero`  
**Type:** Embedded Linux SD image builder + Rust daemon + Lua plugin system  
**Target hardware:** Raspberry Pi Zero 2 W (aarch64/armv7) and Pi Zero W (armv6, 32-bit)  
**Display:** Waveshare 2.13" V4 e-ink (SPI, 250×122, SSD1680)  
**Power:** PiSugar 3 / PiSugar 2 / PiSugar S / generic UPS HAT (I2C)  
**Bluetooth:** PAN tethering for internet backhaul (phone hotspot)  
**Storage:** SD card optimized for longevity (zram + tmpfs + rsync sync)  
**Core engine:** Rust daemon (oxigotchi-style) replacing Python pwnagotchi core  
**Attack engine:** AngryOxide (Rust, nexmon firmware) via bettercap compatibility layer  
**Plugins:** Lua (pwnagotchi-compatible) + native Rust plugins  
**Web UI:** Embedded HTTP dashboard (ox HTTP server, htmx + Alpine.js, auth optional)  
**Handshake upload:** wpa-sec.stanev.org, pwncrack.org, local hashcat sync via rsync/SSH  
**SD card longevity:** zram log/data mounts, tmpfs, rsync-on-shutdown, noatime, journald offload  

**Goal:** A modern, maintainable, SD-card-friendly pwnagotchi image for 32-bit Pi Zero W / Zero 2 W with Waveshare V4 e-ink, BT PAN tethering, wpa-sec/pwncrack upload, and local hashcat sync — built on modern Rust core (oxigotchi lineage) with pwnagotchi plugin compatibility.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    SD Card Image (pi-gen based)                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Rust Daemon (oxigotchi-core) — single binary, ~1.2MB    │  │
│  │  ├─ main.rs          → daemon entry, epoch loop           │  │
│  │  ├─ config/mod.rs    → TOML config (pwnagotchi-compat)    │  │
│  │  ├─ epoch.rs         → Scan→Attack→Capture→Display→Sleep  │  │
│  │  ├─ display/         → SPI e-ink driver (Waveshare V4)    │  │
│  │  │   ├─ driver.rs   → SSD1680 driver, 250×122, 1-bit      │  │
│  │  │   ├─ buffer.rs   → 1-bit packed framebuffer            │  │
│  │  │   └─ api.rs      → draw_face, draw_text, draw_status   │  │
│  │  ├─ wifi/           → monitor mode, channel hop, AP track │  │
│  │  ├─ attacks/        → rate-limited deauth/assoc (rate=1)  │  │
│  │  ├─ capture/        → pcapng management, upload queue     │  │
│  │  ├─ personality/    → mood, XP, faces (28 kaomoji + PNG)  │  │
│  │  ├─ bluetooth/      → BlueZ D-Bus PAN tether + BT agent   │  │
│  │  ├─ pisugar/        → I2C battery, button, watchdog       │  │
│  │  ├─ recovery/       → SDIO reset, GPIO power-cycle, watchdog│  │
│  │  ├─ plugins/        → Lua VM (mlua) + native Rust plugins │  │
│  │  ├─ web/            → embedded HTTP dashboard (axum)      │  │
│  │  └─ migration/      → import legacy pwnagotchi config     │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  System Layer (stage3 overlay via pi-gen)                 │  │
│  │  ├─ systemd units: oxigotchi.service, bt-agent.service,   │  │
│  │  │  bt-pan@.service, epd-startup.service,                │  │
│  │  │  zram-log.service, zram-data.service,                 │  │
│  │  │  safe-shutdown.service, nm-watchdog.service            │  │
│  │  ├─ NetworkManager: bnep* unmanaged (99-bt-pan.conf)      │  │
│  │  ├─ dhcpcd: BT PAN static lease, usb0 fallback           │  │
│  │  ├─ zram: /etc/pwnagotchi/log (50M), /var/tmp/pwn (10M)  │  │
│  │  ├─ cron: rsync zram→SD every 60s, on shutdown           │  │
│  │  ├─ firmware: nexmon patched brcmfmac43436-sdio.bin       │  │
│  │  └─ config: /etc/pwnagotchi/config.toml (TOML, compat)   │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Requirements

### 3.1 Hardware Support Matrix

| Hardware | Support | Notes |
|----------|---------|-------|
| Pi Zero W (BCM2835, armv6, 32-bit) | **Full** | armv6 build, 512MB RAM |
| Pi Zero 2 W (BCM2710A1, armv7/aarch64, 32-bit userland) | **Full** | armv7 build, 512MB RAM, aarch64 kernel |
| Waveshare 2.13" V4 e-ink (SSD1680, SPI) | **Full** | 250×122, 1-bit, partial refresh |
| PiSugar 3 / 2 / S (I2C 0x24) | **Full** | Battery %, button, watchdog, wake timer |
| Generic UPS HAT (INA219/ADS1115) | **Plugin** | Via Lua plugin |
| Bluetooth PAN (phone tether) | **Full** | BlueZ D-Bus, auto-reconnect, bt-agent |
| WiFi monitor mode (brcmfmac + nexmon) | **Full** | wlan0mon, channel hop 1-13, rate=1 |
| WPA handshake capture (pcapng) | **Full** | PMKID, full/half handshakes |
| wpa-sec upload | **Plugin** | API key config, auto-upload, download results |
| pwncrack.org upload | **Plugin** | API key config |
| Local hashcat sync (rsync/SSH) | **Plugin** | Push .pcapng to remote, pull .potfile |
| Web UI (htmx + Alpine) | **Full** | Port 8080, optional auth |
| SD card longevity (zram + rsync) | **Full** | log/data in RAM, sync 60s + shutdown |

### 3.2 Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| F1 | Build 32-bit armhf SD image for Pi Zero W / Zero 2 W via pi-gen | P0 |
| F2 | Rust daemon single binary (~1.2MB release, LTO, strip, panic=abort) | P0 |
| F3 | Waveshare V4 e-ink driver (SPI, SSD1680, 250×122, partial refresh) | P0 |
| F4 | Epoch loop: scan (ch 1-13 hop) → attack (rate=1) → capture → display → sleep | P0 |
| F5 | Bettercap-compatible attack engine (deauth, assoc) via AngryOxide or native | P0 |
| F6 | pcapng capture management (rotate, dedup, upload queue, auto-backup) | P0 |
| F7 | Bluetooth PAN tethering: BlueZ D-Bus, auto-reconnect, bt-agent (auto-pair) | P0 |
| F8 | PiSugar I2C: battery %, button actions, watchdog, wake timer | P0 |
| F9 | WiFi SDIO recovery: GPIO WL_REG_ON toggle, firmware reload, watchdog | P0 |
| F10 | Lua plugin VM (mlua) compatible with pwnagotchi plugin API | P1 |
| F11 | Native Rust plugin trait for performance-critical plugins | P1 |
| F12 | Embedded web dashboard (axum, htmx, Alpine.js, auth optional) | P1 |
| F13 | wpa-sec plugin: upload handshakes, download cracked passwords | P1 |
| F14 | pwncrack plugin: upload, API key config | P1 |
| F15 | Local hashcat sync plugin: rsync push captures, pull potfile | P1 |
| F16 | SD card longevity: zram for /log + /data, rsync 60s + shutdown | P0 |
| F17 | Config migration from pwnagotchi config.toml | P1 |
| F18 | Safe shutdown on button hold / low battery / SIGTERM | P0 |
| F19 | Boot splash on e-ink (oxigotchi logo) | P2 |
| F20 | OTA update mechanism (github releases, verify sig) | P2 |

### 3.3 Non-Functional Requirements

| ID | Requirement |
|----|-------------|
| NF1 | Boot to operational in < 30s on Pi Zero W (cold) |
| NF2 | Idle power < 180mA (Pi Zero 2W + PiSugar + e-ink idle) |
| NF3 | SD card write < 10 MB/day average (zram + rsync) |
| NF4 | Binary size < 2MB (release, stripped, LTO) |
| NF5 | 1070+ unit tests pass (cargo test) |
| NF6 | Cross-compile from x86_64 Linux (docker or cargo cross) |
| NF7 | GPL-3.0 compatible (oxigotchi + pwnagotchi plugins GPL-3.0) |

---

## 4. Configuration Schema (TOML)

```toml
# /etc/pwnagotchi/config.toml — pwnagotchi-compatible
[main]
name = "pwnagotchi-zero"
lang = "en"
iface = "wlan0mon"
mon_start_cmd = "/usr/bin/monstart"
mon_stop_cmd = "/usr/bin/monstop"
mon_max_blind_epochs = 5
no_restart = false
whitelist = ["HOME_WIFI", "aa:bb:cc:dd:ee:ff"]
confd = "/etc/pwnagotchi/conf.d/"
custom_plugin_repos = [
  "https://github.com/jayofelony/pwnagotchi-torch-plugins/archive/master.zip",
  "https://github.com/wpa-2/Pwnagotchi-Plugins/archive/master.zip",
]
custom_plugins = "/usr/local/share/pwnagotchi/custom-plugins/"

[main.plugins.auto-tune]
enabled = true

[main.plugins.auto_backup]
enabled = true
backup_location = "/etc/pwnagotchi/backups"

[main.plugins.auto-update]
enabled = true
install = true
interval = 1
token = ""

[main.plugins.bt-tether]
enabled = true
auto_reconnect = true
show_on_screen = true
show_mini_status = true
mini_status_position = [110, 0]
show_detailed_status = true
detailed_status_position = [0, 82]

[main.plugins.fix_services]
enabled = true

[main.plugins.cache]
enabled = true

[main.plugins.gps]
enabled = false
speed = 19200
device = "/dev/ttyUSB0"

[main.plugins.grid]
enabled = true
report = true

[main.plugins.logtail]
enabled = false
max_lines = 10000

[main.plugins.memtemp]
enabled = true
scale = "celsius"
orientation = "horizontal"

[main.plugins.pwncrack]
enabled = true
key = ""

[main.plugins.session-stats]
enabled = true
save_directory = "/etc/pwnagotchi/sessions/"

[main.plugins.ups_lite]
enabled = false
shutdown = 2

[main.plugins.webcfg]
enabled = true

[main.plugins.pwnstore_ui]
enabled = true

[main.plugins.wpa-sec]
enabled = true
api_key = ""
api_url = "https://wpa-sec.stanev.org"
download_results = true
show_pwd = false
single_files = false

[main.plugins.hashcat-sync]
enabled = true
remote_host = "hashcat.local"
remote_user = "hashcat"
remote_path = "/home/hashcat/captures"
local_path = "/etc/pwnagotchi/handshakes"
sync_interval = 300
pull_potfile = true
potfile_path = "/home/hashcat/hashcat.potfile"
ssh_key = "/home/pi/.ssh/id_ed25519"

[main.log]
path = "/etc/pwnagotchi/log/pwnagotchi.log"
path_debug = "/etc/pwnagotchi/log/pwnagotchi-debug.log"

[main.log.rotation]
enabled = true
size = "10M"

[personality]
advertise = true
happy = ["(•‿‿•)", "(^‿‿^)", "(^◡◡^)"]
excited = ["(ᵔ◡◡ᵔ)", "(✜‿‿✜)"]
grateful = ["(^‿‿^)"]
motivated = ["(☼‿‿☼)", "(★‿★)", "(•̀ᴗ•́)"]
demotivated = ["(≖_≖)", "(￣ヘ￣)", "(¬_¬)"]
smart = ["(✜‿‿✜)"]
lonely = ["(ب_ب)", "(｡•́︿•̀｡)", "(︶︹︺)"]
sad = ["(╥☁╥ )", "(╥﹏╥)", "(ಥ﹏ಥ)"]
angry = ["(-_-')", "(⇀_⇀)", "(`___´)"]
friend = ["(♥‿‿♥)", "(♡‿‿♡)", "(♥‿♥ )", "(♥ω♥ )"]
broken = ["(☓‿‿☓)"]
debug = ["(#_#)"]
upload = ["(1_0)", "(1_1)", "(0_1)"]
png = true
position_x = 0
position_y = 16

[ui.web]
enabled = true
address = "::"
port = 8080
auth = false
username = "changeme"
password = "changeme"

[ui.display]
enabled = true
rotation = 180
type = "waveshare_v4"

[bettercap]
handshakes = "/etc/pwnagotchi/handshakes"
silence = [
  "ble.device.new", "ble.device.lost", "ble.device.service.discovered",
  "ble.device.characteristic.discovered", "ble.device.disconnected",
  "ble.device.connected", "ble.connection.timeout",
  "wifi.client.new", "wifi.client.lost", "wifi.client.probe",
  "wifi.ap.new", "wifi.ap.lost", "mod.started"
]

[fs.memory]
enabled = true

[fs.memory.mounts.log]
enabled = true
mount = "/etc/pwnagotchi/log/"
size = "50M"
sync = 60
zram = true
rsync = true

[fs.memory.mounts.data]
enabled = true
mount = "/var/tmp/pwnagotchi"
size = "10M"
sync = 3600
zram = true
rsync = true

# oxigotchi-specific extensions
[oxigotchi]
bt_tether_enabled = true
bt_agent_enabled = true
phone_mac = ""  # optional, auto-discover
pisugar_i2c_addr = 0x24
pisugar_button_long_press = 3  # seconds
pisugar_watchdog_interval = 30
wifi_recovery_enabled = true
wifi_recovery_gpio = 4  # WL_REG_ON
epoch_duration = 30  # seconds
attack_rate_limit = 1  # deauths/epoch
display_partial_refresh = true
web_ui_enabled = true
```

---

## 5. Build System (pi-gen based)

### 5.1 Directory Structure

```
pwnagotchi-zero/
├── config/
│   ├── config-32bit              # pi-gen config for 32-bit build
│   ├── config-64bit              # pi-gen config for 64-bit kernel/32-bit userland
│   └── angryoxide-v5.toml        # oxigotchi overlay config
├── stage3/                       # pi-gen stage3 overlay (replaces pwnagotchi stage3)
│   ├── 00-pre-pwn/
│   ├── 01-pwn-packages/
│   ├── 02-libpcap/
│   ├── 03-bettercap-pwngrid/
│   ├── 04-nexmon/
│   ├── 05-install-oxigotchi/     # rust binary + systemd units + configs
│   ├── 06-hcxtools/
│   ├── 07-patches/
│   ├── 08-pwnstore/
│   ├── EXPORT_IMAGE
│   └── prerun.sh
├── scripts/
│   ├── bake_release.sh           # builds and packages .img.xz
│   ├── build_oxigotchi.sh        # cross-compiles rust binary for armv7/aarch64
│   ├── fix_ndev_on_boot.sh
│   ├── safe_shutdown.sh
│   ├── buffer_cleaner.sh
│   ├── pisugar_watchdog.sh
│   ├── usb0_fallback.sh
│   └── bt_keepalive.sh
├── rust/                         # oxigotchi-core source (submodule or subtree)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/...                   # see architecture diagram
├── plugin/                       # Lua plugins (pwnagotchi-compat)
│   ├── bt-tether.lua
│   ├── wpa-sec.lua
│   ├── pwncrack.lua
│   ├── hashcat-sync.lua
│   ├── auto-backup.lua
│   ├── memtemp.lua
│   └── faces/                    # PNG faces for bull mode
├── config/
│   ├── 99-bt-pan.conf            # NetworkManager unmanaged bnep*
│   ├── pisugar-config.json
│   └── conf.d/                   # drop-in configs
├── Dockerfile.build              # cross-compile container
├── Makefile
├── SPEC.md                       # this file
└── README.md
```

### 5.2 Build Commands

```bash
# Build 32-bit image (Pi Zero W / Zero 2 W 32-bit userland)
./scripts/bake_release.sh --arch 32bit --release v1.0.0

# Build 64-bit kernel / 32-bit userland image (Pi Zero 2 W optimized)
./scripts/bake_release.sh --arch 64bit --release v1.0.0

# Cross-compile Rust binary only
./scripts/build_oxigotchi.sh --target arm-unknown-linux-gnueabihf
./scripts/build_oxigotchi.sh --target aarch64-unknown-linux-gnu
```

---

## 6. Rust Daemon (oxigotchi-core) — Module Spec

### 6.1 Cargo.toml (key deps)

```toml
[package]
name = "oxigotchi"
version = "1.0.0"
edition = "2021"
description = "Modern pwnagotchi core in Rust for Pi Zero 2W/Zero W"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"

[dependencies]
# Config
config = { version = "0.14", features = ["toml"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# Hardware
linux-embedded-hal = "0.3"
embedded-hal = "1.0"
spidev = "0.4"
i2cdev = "0.5"
gpio-cdev = "0.5"

# Bluetooth
zbus = { version = "4.0", features = ["xml", "blocking"] }
zbus_names = "4.0"
bluez-async = "0.3"

# Web
axum = { version = "0.7", features = ["json", "ws"] }
tower-http = { version = "0.5", features = ["cors", "trace", "services"] }
tera = "1.19"
minijinja = "2.0"

# Lua plugins
mlua = { version = "0.10", features = ["lua54", "vendored", "async", "serde"] }

# Crypto / hashcat
blake3 = "1.5"
hmac = "0.12"

# Logging
tracing = { version = "0.1", features = ["std"] }
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
color-eyre = "0.6"

# Utils
anyhow = "1.0"
tokio = { version = "1.38", features = ["full", "rt-multi-thread"] }
bytes = "1.5"
chrono = { version = "0.4", features = ["serde"] }
dirs = "5.0"
notify = "6.1"
```

### 6.2 Module Responsibilities

| Module | File | Responsibility |
|--------|------|----------------|
| `main.rs` | `src/main.rs` | Daemon entry, signal handling, subsystem init, epoch loop |
| `config/mod.rs` | `src/config/mod.rs` | TOML load/merge (defaults + /etc/pwnagotchi/conf.d/*), validation |
| `epoch.rs` | `src/epoch.rs` | Epoch state machine: Scan→Attack→Capture→Display→Sleep |
| `display/` | `src/display/` | SPI e-ink driver, framebuffer, drawing API |
| `wifi/` | `src/wifi/mod.rs` | Monitor mode, channel hop, AP tracking, whitelist |
| `attacks/` | `src/attacks/mod.rs` | Rate-limited deauth/assoc (AngryOxide or bettercap shim) |
| `capture/` | `src/capture/mod.rs` | pcapng management, dedup, upload queue, rotation |
| `personality/` | `src/personality/mod.rs` | Mood, XP, level, faces (kaomoji + PNG), mood deltas |
| `bluetooth/` | `src/bluetooth/mod.rs` | BlueZ D-Bus: PAN connect/disconnect, Agent1, scan |
| `pisugar/` | `src/pisugar/mod.rs` | I2C battery %, button debounce, watchdog, wake timer |
| `recovery/` | `src/recovery/mod.rs` | SDIO reset (GPIO), firmware reload, health watchdog |
| `plugins/` | `src/plugins/mod.rs` | Lua VM (mlua), plugin registry, native plugin trait |
| `web/` | `src/web/mod.rs` | axum HTTP server, embedded templates, WS for live updates |
| `migration/` | `src/migration/mod.rs` | Import pwnagotchi config.toml + handshakes |
| `qpu/` | `src/qpu/` | (Optional) QPU-assisted capture/classifier — disabled by default |

---

## 7. Systemd Units (stage3/05-install-oxigotchi)

```ini
# oxigotchi.service
[Unit]
Description=Pwnagotchi Zero Core Daemon
After=network-online.target bluetooth.target
Wants=network-online.target
StartLimitIntervalSec=60
StartLimitBurst=3

[Service]
Type=notify
ExecStart=/usr/local/bin/oxigotchi
Restart=on-failure
RestartSec=5
Nice=-5
LimitNOFILE=65536
MemoryMax=200M
CPUQuota=80%
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
```

```ini
# bt-agent.service
[Unit]
Description=Bluetooth Auto-Pair Agent
After=bluetooth.service
Requires=bluetooth.service

[Service]
Type=dbus
BusName=org.pwnagotchi.BTAgent
ExecStart=/usr/local/bin/oxigotchi-bt-agent
Restart=on-failure

[Install]
WantedBy=bluetooth.target
```

```ini
# bt-pan@.service (template, instantiated per device MAC)
[Unit]
Description=Bluetooth PAN Tether for %i
After=network-online.target bluetooth.service bt-agent.service
Wants=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/bt-pan-connect %i
ExecStop=/usr/local/bin/bt-pan-disconnect %i

[Install]
WantedBy=multi-user.target
```

```ini
# zram-log.service
[Unit]
Description=Zram log mount
Before=oxigotchi.service
DefaultDependencies=no

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/zram-setup log 50M /etc/pwnagotchi/log
ExecStop=/usr/local/bin/zram-teardown log

[Install]
WantedBy=local-fs.target
```

```ini
# safe-shutdown.service
[Unit]
Description=Safe shutdown handler (button + low battery)
Before=shutdown.target reboot.target halt.target
DefaultDependencies=no

[Service]
Type=oneshot
ExecStart=/usr/local/bin/safe-shutdown.sh
TimeoutSec=60
RemainAfterExit=yes

[Install]
WantedBy=shutdown.target reboot.target halt.target
```

---

## 8. SD Card Longevity Strategy

| Mount | Backing | Sync Interval | Rationale |
|-------|---------|---------------|-----------|
| `/etc/pwnagotchi/log` | zram (50M) | 60s + shutdown | pwnagotchi logs, rotation |
| `/var/tmp/pwnagotchi` | zram (10M) | 3600s + shutdown | capture queue, temp files |
| `/etc/pwnagotchi/handshakes` | SD (ext4) | immediate | handshakes are valuable, sync on write |
| `/etc/pwnagotchi/backups` | SD (ext4) | on demand | config backups |

**Implementation:**
- `zram-setup <name> <size> <mountpoint>` creates zram device, formats ext4, mounts with `noatime,nodiratime,discard`
- `rsync -a --delete /mnt/zram-log/ /etc/pwnagotchi/log/` every 60s via cron
- `safe-shutdown.sh` runs rsync on SIGTERM/SIGINT/button hold/low battery
- `journald.conf`: `Storage=volatile`, `RuntimeMaxUse=20M`, `ForwardToSyslog=no`

---

## 9. Bluetooth PAN Tethering Flow

```
Boot → bt-agent.service (auto-pair) → scan for known phone MAC
      → if found: bt-pan-connect <MAC> → bnep0 up → dhcpcd
      → NetworkManager ignores bnep* (99-bt-pan.conf)
      → Internet via phone → wpa-sec/pwncrack upload works
      → On disconnect: bt-pan-disconnect → rescan every 30s
```

**Key files:**
- `bluetooth/mod.rs`: BlueZ D-Bus wrapper, `Adapter1`, `Device1`, `Network1`
- `bt-agent`: implements `org.bluez.Agent1` (auto-accept, display passkey)
- `bt-pan-connect.sh`: `nmcli con up bt-pan-<MAC>` or `bt-network -c <MAC> nap`

---

## 10. Plugins — Lua API (pwnagotchi-compat)

```lua
-- plugin structure (same as pwnagotchi)
local plugin = {
  name = "wpa-sec",
  version = "1.0.0",
  author = "pwnagotchi-zero",
  description = "Upload handshakes to wpa-sec.stanev.org",
  config = {
    enabled = true,
    api_key = "",
    api_url = "https://wpa-sec.stanev.org",
    download_results = true,
  }
}

function plugin:on_loaded()
  -- called once at startup
end

function plugin:on_unload()
  -- cleanup
end

function plugin:on_internet_available()
  -- upload pending handshakes
end

function plugin:on_handshake_captured(path, ap, client)
  -- queue for upload
end

function plugin:on_ui_update(ui)
  -- draw on e-ink via ui:draw_text(x, y, text), ui:draw_face(face_name)
end

function plugin:on_epoch(epoch, status)
  -- per-epoch callback
end
```

**Native Rust plugin trait** (for performance-critical):

```rust
#[async_trait]
pub trait NativePlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn on_loaded(&mut self, ctx: &PluginContext) -> Result<()>;
    async fn on_internet_available(&mut self, ctx: &PluginContext) -> Result<()>;
    async fn on_handshake(&mut self, ctx: &PluginContext, path: &Path, ap: &AccessPoint, client: &Client) -> Result<()>;
    async fn on_epoch(&mut self, ctx: &PluginContext, epoch: u64, status: &EpochStatus) -> Result<()>;
}
```

---

## 11. Web Dashboard (Embedded)

**Stack:** axum + minijinja (templates) + htmx + Alpine.js (embedded in binary via `include_str!`)

**Routes:**
- `GET /` — dashboard (status, faces, stats, handshakes list)
- `GET /api/status` — JSON: epoch, mood, battery, uptime, handshakes, peers
- `GET /api/handshakes` — list with download links
- `GET /api/config` — current config (redacted secrets)
- `POST /api/config` — update config (requires auth if enabled)
- `WS /ws` — live updates (epoch tick, face change, handshake capture)
- `POST /api/shutdown` — safe shutdown
- `POST /api/reboot` — reboot

**Auth:** Optional HTTP Basic (config `ui.web.auth`, `username`, `password`)

---

## 12. WiFi Recovery (Critical for SD Card Longevity)

**Problem:** BCM43436B0 firmware hangs → SDIO errors → SD card corruption on forced reboot.

**Solution (oxigotchi recovery module):**
1. Watchdog monitors `dmesg` for `brcmfmac: brcmf_sdio_bus_rxctl: resumed on timeout`
2. On 3+ errors in 60s: trigger soft recovery
3. Soft recovery: `ip link set wlan0 down` → `modprobe -r brcmfmac` → GPIO 4 (WL_REG_ON) low 500ms → high → `modprobe brcmfmac` → wait for `wlan0`
4. Hard recovery (after 3 soft failures): trigger safe-shutdown, require power-cycle (PiSugar battery disconnect)
5. Display "ZOMBIE - UNPLUG USB+BATT 30s" on e-ink (sticky status)

---

## 13. Acceptance Criteria (Definition of Done)

| # | Criterion | Verification |
|---|-----------|--------------|
| 1 | `./scripts/bake_release.sh --arch 32bit` produces bootable `.img.xz` | Flash to SD, boot Pi Zero W, SSH works |
| 2 | `oxigotchi` binary < 2MB, runs on armv7 + aarch64 | `file`, `cargo build --release --target=...`, `scp` + run |
| 3 | Waveshare V4 e-ink shows boot face → epoch faces | Visual check on hardware |
| 4 | WiFi monitor mode + channel hop 1-13 works | `iw wlan0mon info`, `tcpdump -i wlan0mon` |
| 5 | Handshake captured → pcapng in `/etc/pwnagotchi/handshakes` | `hcxpcapngtool -o test.hash test.pcapng` |
| 6 | wpa-sec upload works (with API key) | Check wpa-sec.stanev.org dashboard |
| 7 | pwncrack upload works | Check pwncrack.org dashboard |
| 8 | Local hashcat sync pushes captures, pulls potfile | `rsync -avz /etc/pwnagotchi/handshakes/ hashcat@host:/path/` |
| 9 | BT PAN tether connects to phone, provides internet | `ping 8.8.8.8` via bnep0 |
| 10 | PiSugar battery % shown on display, button works | Press button → face changes |
| 11 | Safe shutdown on button hold (3s) / low battery (10%) | Pull power → no FS corruption on next boot |
| 12 | SD card writes < 10MB/day average (idle) | `iostat -d /dev/mmcblk0` over 24h |
| 13 | Web UI accessible at `http://pwnagotchi.local:8080` | Browser test |
| 14 | Config migration from pwnagotchi config.toml works | Copy old config, boot, verify settings |
| 15 | All 1070+ cargo tests pass | `cargo test --all-targets` |

---

## 14. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| 1 | Repo init, pi-gen config, stage3 skeleton | Buildable empty image |
| 2 | Cross-compile Rust daemon (minimal: config + epoch loop + display) | Binary runs on Pi, shows face |
| 3 | WiFi monitor + channel hop + capture (pcapng) | Handshakes captured |
| 4 | AngryOxide attack integration (rate-limited) | Deauth/assoc working |
| 5 | Bluetooth PAN + bt-agent | Internet via phone |
| 6 | PiSugar I2C + safe shutdown | Battery %, button, watchdog |
| 7 | WiFi recovery (SDIO reset + watchdog) | No SD corruption on WiFi hang |
| 8 | Lua plugin VM + pwnagotchi compat plugins | bt-tether, wpa-sec, pwncrack, hashcat-sync |
| 9 | Web dashboard (axum + htmx) | Browser UI works |
| 10 | zram + rsync SD longevity | Write volume < 10MB/day |
| 11 | Config migration + polish | Import old config, docs |
| 12 | Release build + test matrix (Pi Zero W, Zero 2W) | `.img.xz` releases |

---

## 15. Repository Layout (Final)

```
pwnagotchi-zero/
├── .github/workflows/build.yml      # CI: cross-compile, test, build image
├── config/
│   ├── config-32bit
│   ├── config-64bit
│   ├── angryoxide-v5.toml
│   ├── 99-bt-pan.conf
│   ├── pisugar-config.json
│   └── conf.d/
│       ├── 01-wpa-sec.toml
│       ├── 02-hashcat-sync.toml
│       └── 03-local.toml.example
├── stage3/
│   ├── 00-pre-pwn/
│   ├── 01-pwn-packages/
│   ├── 02-libpcap/
│   ├── 03-bettercap-pwngrid/
│   ├── 04-nexmon/
│   ├── 05-install-oxigotchi/
│   │   ├── files/
│   │   │   ├── usr/local/bin/oxigotchi
│   │   │   ├── usr/local/bin/bt-pan-connect
│   │   │   ├── usr/local/bin/bt-pan-disconnect
│   │   │   ├── usr/local/bin/zram-setup
│   │   │   ├── usr/local/bin/zram-teardown
│   │   │   ├── usr/local/bin/safe-shutdown.sh
│   │   │   ├── usr/local/bin/buffer-cleaner.sh
│   │   │   ├── usr/local/bin/fix_ndev_on_boot.sh
│   │   │   ├── usr/local/bin/pisugar-watchdog.sh
│   │   │   ├── usr/local/bin/usb0-fallback.sh
│   │   │   ├── usr/local/bin/bt-keepalive.sh
│   │   │   ├── etc/systemd/system/oxigotchi.service
│   │   │   ├── etc/systemd/system/bt-agent.service
│   │   │   ├── etc/systemd/system/bt-pan@.service
│   │   │   ├── etc/systemd/system/zram-log.service
│   │   │   ├── etc/systemd/system/zram-data.service
│   │   │   ├── etc/systemd/system/safe-shutdown.service
│   │   │   ├── etc/systemd/system/nm-watchdog.service
│   │   │   ├── etc/systemd/system/epd-startup.service
│   │   │   ├── etc/NetworkManager/conf.d/99-bt-pan.conf
│   │   │   ├── etc/pwnagotchi/config.toml
│   │   │   ├── etc/pwnagotchi/conf.d/
│   │   │   ├── etc/pwnagotchi/custom-plugins/
│   │   │   │   ├── faces/
│   │   │   │   ├── bt-tether.lua
│   │   │   │   ├── wpa-sec.lua
│   │   │   │   ├── pwncrack.lua
│   │   │   │   ├── hashcat-sync.lua
│   │   │   │   ├── auto-backup.lua
│   │   │   │   └── memtemp.lua
│   │   │   └── lib/systemd/system-shutdown/safe-shutdown.sh
│   │   └── 00-run.sh
│   ├── 06-hcxtools/
│   ├── 07-patches/
│   ├── 08-pwnstore/
│   ├── EXPORT_IMAGE
│   └── prerun.sh
├── scripts/
│   ├── bake_release.sh
│   ├── build_oxigotchi.sh
│   ├── fix_ndev_on_boot.sh
│   ├── safe_shutdown.sh
│   ├── buffer_cleaner.sh
│   ├── pisugar_watchdog.sh
│   ├── usb0_fallback.sh
│   └── bt_keepalive.sh
├── rust/                          # git submodule → oxigotchi
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── main.rs
│   │   ├── config/mod.rs
│   │   ├── epoch.rs
│   │   ├── display/{mod.rs,driver.rs,buffer.rs,api.rs}
│   │   ├── wifi/mod.rs
│   │   ├── attacks/mod.rs
│   │   ├── capture/mod.rs
│   │   ├── personality/mod.rs
│   │   ├── bluetooth/{mod.rs,dbus.rs,attacks/}
│   │   ├── pisugar/mod.rs
│   │   ├── recovery/mod.rs
│   │   ├── plugins/{mod.rs,lua.rs,native.rs}
│   │   ├── web/mod.rs
│   │   ├── migration/mod.rs
│   │   └── qpu/{mod.rs,capture.rs,classifier.rs,engine.rs,mailbox.rs,rf.rs,ringbuf.rs}
│   ├── templates/
│   │   ├── dashboard.html.j2
│   │   └── partials/
│   ├── static/
│   │   ├── htmx.min.js
│   │   ├── alpine.min.js
│   │   └── style.css
│   └── tests/
├── plugin/                        # Lua plugins (synced to stage3)
│   ├── bt-tether.lua
│   ├── wpa-sec.lua
│   ├── pwncrack.lua
│   ├── hashcat-sync.lua
│   ├── auto-backup.lua
│   ├── memtemp.lua
│   └── faces/
├── docker/
│   ├── Dockerfile.build
│   └── Dockerfile.pi-gen
├── Makefile
├── SPEC.md
└── README.md
```

---

## 16. Key Implementation Notes

1. **32-bit build:** Use `pi-gen` with `config-32bit` (armhf). For Zero 2W, kernel is aarch64 but userland stays armhf — `config-64bit` handles this.

2. **Cross-compile Rust:** Use `cross` or Docker with `gcc-arm-linux-gnueabihf` / `gcc-aarch64-linux-gnu`. Set `CARGO_TARGET_*_LINKER`.

3. **AngryOxide integration:** Either (a) bundle `angryoxide` binary and shell out, or (b) embed as library. Option (a) simpler, matches oxigotchi config.

4. **Bettercap not on Pi:** oxigotchi disables bettercap/pwngrid on device (saves 50MB RAM). Attacks run via AngryOxide native.

5. **Display driver:** Waveshare V4 = SSD1680, 250×122, SPI mode 0, 1-bit. Partial refresh for face updates, full refresh every 10 epochs.

6. **Config migration:** On first boot, if `/etc/pwnagotchi/config.toml` missing but old `/etc/pwnagotchi/config.yml` exists, run migration tool.

7. **OTA updates:** GitHub releases with `.sig` (minisign). Daemon checks weekly, verifies, downloads, atomic replace, systemctl restart.

---

## 17. References

- oxigotchi: https://github.com/CoderFX/oxigotchi (Rust core, Waveshare V4, PiSugar, BT PAN)
- jayofelony/pwnagotchi: https://github.com/jayofelony/pwnagotchi (modern Python fork, 32-bit config, plugins)
- AngryOxide: https://github.com/Ragnt/AngryOxide (Rust 802.11 attack engine)
- nexmon: https://nexmon.org (firmware patching for monitor mode)
- wpa-sec: https://wpa-sec.stanev.org (distributed cracking)
- pwncrack: https://pwncrack.org (handshake cracking API)
- PiSugar: https://pisugar.com (UPS HAT for Pi Zero)
- Waveshare 2.13" V4: https://www.waveshare.com/2.13inch-e-paper-hat.htm

---

**Ponytail note:** This spec is the single source of truth. Implementation follows the ladder: stdlib → native platform → existing deps → minimal new code. Every module has a test. Binary size < 2MB. SD writes < 10MB/day. No bettercap on Pi. All plugins Lua-compat + native trait. Ship the image, iterate.
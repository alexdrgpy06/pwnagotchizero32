//! Configuration management for Pwnagotchi Zero

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use config::{Config as ConfigLib, Environment, File};
use serde::{Deserialize, Serialize};
use tokio::fs;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub main: MainConfig,

    #[serde(default)]
    pub personality: PersonalityConfig,

    #[serde(default)]
    pub ui: UiConfig,

    #[serde(default)]
    pub bettercap: BettercapConfig,

    #[serde(default)]
    pub fs: FsConfig,

    #[serde(default)]
    pub oxigotchi: OxigotchiConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            main: MainConfig::default(),
            personality: PersonalityConfig::default(),
            ui: UiConfig::default(),
            bettercap: BettercapConfig::default(),
            fs: FsConfig::default(),
            oxigotchi: OxigotchiConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from file with defaults and conf.d overlay
    pub async fn load(path: &str) -> Result<Self> {
        let config_path = Path::new(path);

        // Start with defaults
        let mut builder = ConfigLib::builder()
            .add_source(File::from_str(
                include_str!("../../defaults.toml"),
                config::FileFormat::Toml,
            ))
            .add_source(Environment::with_prefix("PWNAGOTCHI").separator("__"));

        // Load main config file if exists
        if config_path.exists() {
            builder = builder.add_source(File::from(config_path).required(false));
        }

        // Load conf.d/*.toml files
        let conf_dir = config_path
            .parent()
            .unwrap_or(Path::new("/etc/pwnagotchi"))
            .join("conf.d");

        if conf_dir.exists() {
            let mut entries = Vec::new();
            let mut dir = fs::read_dir(&conf_dir).await?;
            while let Some(entry) = dir.next_entry().await? {
                if entry.path().extension().map_or(false, |ext| ext == "toml") {
                    entries.push(entry);
                }
            }
            entries.sort_by_key(|e| e.file_name());

            for entry in entries {
                builder = builder.add_source(File::from(entry.path()).required(false));
            }
        }

        let config = builder.build().context("Failed to build configuration")?;

        let mut cfg: Config = config
            .try_deserialize()
            .context("Failed to deserialize configuration")?;

        // Validate and fix up config
        cfg.validate_and_fix().await?;

        Ok(cfg)
    }

    async fn validate_and_fix(&mut self) -> Result<()> {
        // Ensure required directories exist
        let dirs = vec![
            self.main.handshakes_dir(),
            self.main.log_dir(),
            self.main.backup_dir(),
            self.main.sessions_dir(),
        ];

        for dir in &dirs {
            fs::create_dir_all(dir).await?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainConfig {
    #[serde(default = "default_name")]
    pub name: String,

    #[serde(default = "default_lang")]
    pub lang: String,

    #[serde(default = "default_iface")]
    pub iface: String,

    #[serde(default = "default_mon_start_cmd")]
    pub mon_start_cmd: String,

    #[serde(default = "default_mon_stop_cmd")]
    pub mon_stop_cmd: String,

    #[serde(default = "default_max_blind_epochs")]
    pub mon_max_blind_epochs: u32,

    #[serde(default)]
    pub no_restart: bool,

    #[serde(default)]
    pub whitelist: Vec<String>,

    #[serde(default = "default_confd")]
    pub confd: String,

    #[serde(default)]
    pub custom_plugin_repos: Vec<String>,

    #[serde(default = "default_custom_plugins")]
    pub custom_plugins: String,

    #[serde(default)]
    pub plugins: HashMap<String, PluginConfig>,

    #[serde(default)]
    pub log: LogConfig,
}

fn default_name() -> String {
    "pwnagotchi-zero".to_string()
}
fn default_lang() -> String {
    "en".to_string()
}
fn default_iface() -> String {
    // The physical interface, before monitor mode. WifiManager derives the
    // monitor interface by appending "mon" (see wifi::WifiManager::new) —
    // this must NOT already be "wlan0mon", or the very first "bring the
    // interface down" command targets a device that doesn't exist yet and
    // start_monitor_mode() fails immediately, every epoch, forever.
    "wlan0".to_string()
}
fn default_mon_start_cmd() -> String {
    "/usr/bin/monstart".to_string()
}
fn default_mon_stop_cmd() -> String {
    "/usr/bin/monstop".to_string()
}
fn default_max_blind_epochs() -> u32 {
    5
}
fn default_confd() -> String {
    "/etc/pwnagotchi/conf.d/".to_string()
}
fn default_custom_plugins() -> String {
    "/usr/local/share/pwnagotchi/custom-plugins/".to_string()
}

impl Default for MainConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            lang: default_lang(),
            iface: default_iface(),
            mon_start_cmd: default_mon_start_cmd(),
            mon_stop_cmd: default_mon_stop_cmd(),
            mon_max_blind_epochs: default_max_blind_epochs(),
            no_restart: false,
            whitelist: vec![],
            confd: default_confd(),
            custom_plugin_repos: vec![],
            custom_plugins: default_custom_plugins(),
            plugins: HashMap::new(),
            log: LogConfig::default(),
        }
    }
}

impl MainConfig {
    pub fn handshakes_dir(&self) -> PathBuf {
        PathBuf::from("/etc/pwnagotchi/handshakes")
    }

    pub fn log_dir(&self) -> PathBuf {
        PathBuf::from("/etc/pwnagotchi/log")
    }

    pub fn backup_dir(&self) -> PathBuf {
        PathBuf::from("/etc/pwnagotchi/backups")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        PathBuf::from("/etc/pwnagotchi/sessions")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(flatten)]
    pub options: HashMap<String, serde_json::Value>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_path")]
    pub path: String,

    #[serde(default = "default_log_debug_path")]
    pub path_debug: String,

    #[serde(default)]
    pub rotation: LogRotationConfig,
}

fn default_log_path() -> String {
    "/etc/pwnagotchi/log/pwnagotchi.log".to_string()
}
fn default_log_debug_path() -> String {
    "/etc/pwnagotchi/log/pwnagotchi-debug.log".to_string()
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            path: default_log_path(),
            path_debug: default_log_debug_path(),
            rotation: LogRotationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_log_size")]
    pub size: String,
}

fn default_log_size() -> String {
    "10M".to_string()
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            size: default_log_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    #[serde(default = "default_true")]
    pub advertise: bool,

    #[serde(default = "default_happy")]
    pub happy: Vec<String>,

    #[serde(default = "default_excited")]
    pub excited: Vec<String>,

    #[serde(default = "default_grateful")]
    pub grateful: Vec<String>,

    #[serde(default = "default_motivated")]
    pub motivated: Vec<String>,

    #[serde(default = "default_demotivated")]
    pub demotivated: Vec<String>,

    #[serde(default = "default_smart")]
    pub smart: Vec<String>,

    #[serde(default = "default_lonely")]
    pub lonely: Vec<String>,

    #[serde(default = "default_sad")]
    pub sad: Vec<String>,

    #[serde(default = "default_angry")]
    pub angry: Vec<String>,

    #[serde(default = "default_friend")]
    pub friend: Vec<String>,

    #[serde(default = "default_broken")]
    pub broken: Vec<String>,

    #[serde(default = "default_debug")]
    pub debug: Vec<String>,

    #[serde(default = "default_upload")]
    pub upload: Vec<String>,

    #[serde(default)]
    pub png: bool,

    #[serde(default)]
    pub position_x: i32,

    #[serde(default)]
    pub position_y: i32,

    #[serde(default = "default_frame_padding")]
    pub frame_padding: bool,

    #[serde(default = "default_frame_padding_min_bytes")]
    pub frame_padding_min_bytes: usize,

    #[serde(default)]
    pub deauth: bool,

    #[serde(default)]
    pub associate: bool,
}

fn default_happy() -> Vec<String> {
    vec![
        "(•‿‿•)".to_string(),
        "(^‿‿^)".to_string(),
        "(^◡◡^)".to_string(),
    ]
}
fn default_excited() -> Vec<String> {
    vec!["(ᵔ◡◡ᵔ)".to_string(), "(✜‿‿✜)".to_string()]
}
fn default_grateful() -> Vec<String> {
    vec!["(^‿‿^)".to_string()]
}
fn default_motivated() -> Vec<String> {
    vec![
        "(☼‿‿☼)".to_string(),
        "(★‿★)".to_string(),
        "(•̀ᴗ•́)".to_string(),
    ]
}
fn default_demotivated() -> Vec<String> {
    vec![
        "(≖_≖)".to_string(),
        "(￣ヘ￣)".to_string(),
        "(¬_¬)".to_string(),
    ]
}
fn default_smart() -> Vec<String> {
    vec!["(✜‿‿✜)".to_string()]
}
fn default_lonely() -> Vec<String> {
    vec![
        "(ب_ب)".to_string(),
        "(｡•́︿•̀｡)".to_string(),
        "(︶︹︺)".to_string(),
    ]
}
fn default_sad() -> Vec<String> {
    vec![
        "(╥☁╥ )".to_string(),
        "(╥﹏╥)".to_string(),
        "(ಥ﹏ಥ)".to_string(),
    ]
}
fn default_angry() -> Vec<String> {
    vec![
        "(-_-')".to_string(),
        "(⇀_⇀)".to_string(),
        "(`___´)".to_string(),
    ]
}
fn default_friend() -> Vec<String> {
    vec![
        "(♥‿‿♥)".to_string(),
        "(♡‿‿♡)".to_string(),
        "(♥‿♥ )".to_string(),
        "(♥ω♥ )".to_string(),
    ]
}
fn default_broken() -> Vec<String> {
    vec!["(☓‿‿☓)".to_string()]
}
fn default_debug() -> Vec<String> {
    vec!["(#_#)".to_string()]
}
fn default_upload() -> Vec<String> {
    vec![
        "(1_0)".to_string(),
        "(1_1)".to_string(),
        "(0_1)".to_string(),
    ]
}
fn default_frame_padding() -> bool {
    true
}
fn default_frame_padding_min_bytes() -> usize {
    650
}

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            advertise: true,
            happy: default_happy(),
            excited: default_excited(),
            grateful: default_grateful(),
            motivated: default_motivated(),
            demotivated: default_demotivated(),
            smart: default_smart(),
            lonely: default_lonely(),
            sad: default_sad(),
            angry: default_angry(),
            friend: default_friend(),
            broken: default_broken(),
            debug: default_debug(),
            upload: default_upload(),
            png: true,
            position_x: 0,
            position_y: 16,
            frame_padding: true,
            frame_padding_min_bytes: 650,
            deauth: false,
            associate: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub web: WebUiConfig,

    #[serde(default)]
    pub display: DisplayUiConfig,

    #[serde(default)]
    pub faces: FacesConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            web: WebUiConfig::default(),
            display: DisplayUiConfig::default(),
            faces: FacesConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_web_address")]
    pub address: String,

    #[serde(default)]
    pub auth: bool,

    #[serde(default = "default_web_user")]
    pub username: String,

    #[serde(default = "default_web_pass")]
    pub password: String,

    #[serde(default)]
    pub origin: String,

    #[serde(default = "default_web_port")]
    pub port: u16,

    #[serde(default)]
    pub on_frame: String,

    #[serde(default)]
    pub theme: WebThemeConfig,
}

fn default_web_address() -> String {
    // Bind all IPv4 interfaces. Note: "::" would format to the invalid
    // ":::8080" (IPv6 needs brackets) and fail to bind. 0.0.0.0 is reachable
    // over the usb0 gadget network at 10.0.0.2:8080.
    "0.0.0.0".to_string()
}
fn default_web_user() -> String {
    "changeme".to_string()
}
fn default_web_pass() -> String {
    "changeme".to_string()
}
fn default_web_port() -> u16 {
    8080
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            address: default_web_address(),
            auth: false,
            username: default_web_user(),
            password: default_web_pass(),
            origin: String::new(),
            port: default_web_port(),
            on_frame: String::new(),
            theme: WebThemeConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebThemeConfig {
    #[serde(default = "default_accent_r")]
    pub accent_r: u8,

    #[serde(default = "default_accent_g")]
    pub accent_g: u8,

    #[serde(default = "default_accent_b")]
    pub accent_b: u8,
}

fn default_accent_r() -> u8 {
    76
}
fn default_accent_g() -> u8 {
    175
}
fn default_accent_b() -> u8 {
    80
}

impl Default for WebThemeConfig {
    fn default() -> Self {
        Self {
            accent_r: default_accent_r(),
            accent_g: default_accent_g(),
            accent_b: default_accent_b(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayUiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_rotation")]
    pub rotation: u16,

    #[serde(default = "default_display_type")]
    pub display_type: String,
}

fn default_rotation() -> u16 {
    180
}
fn default_display_type() -> String {
    "waveshare_v4".to_string()
}

impl Default for DisplayUiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rotation: default_rotation(),
            display_type: default_display_type(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacesConfig {
    #[serde(default)]
    pub png: bool,

    #[serde(default)]
    pub position_x: i32,

    #[serde(default)]
    pub position_y: i32,

    // Face image paths (populated at runtime)
    #[serde(skip)]
    pub face_paths: HashMap<String, String>,
}

impl Default for FacesConfig {
    fn default() -> Self {
        Self {
            png: true,
            position_x: 0,
            position_y: 16,
            face_paths: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettercapConfig {
    #[serde(default = "default_handshakes_path")]
    pub handshakes: String,

    #[serde(default = "default_silence")]
    pub silence: Vec<String>,
}

fn default_handshakes_path() -> String {
    "/etc/pwnagotchi/handshakes".to_string()
}
fn default_silence() -> Vec<String> {
    vec![
        "ble.device.new".to_string(),
        "ble.device.lost".to_string(),
        "ble.device.service.discovered".to_string(),
        "ble.device.characteristic.discovered".to_string(),
        "ble.device.disconnected".to_string(),
        "ble.device.connected".to_string(),
        "ble.connection.timeout".to_string(),
        "wifi.client.new".to_string(),
        "wifi.client.lost".to_string(),
        "wifi.client.probe".to_string(),
        "wifi.ap.new".to_string(),
        "wifi.ap.lost".to_string(),
        "mod.started".to_string(),
    ]
}

impl Default for BettercapConfig {
    fn default() -> Self {
        Self {
            handshakes: default_handshakes_path(),
            silence: default_silence(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub mounts: HashMap<String, FsMountConfig>,
}

impl Default for FsConfig {
    fn default() -> Self {
        let mut mounts = HashMap::new();
        mounts.insert(
            "log".to_string(),
            FsMountConfig {
                enabled: true,
                mount: "/etc/pwnagotchi/log/".to_string(),
                size: "50M".to_string(),
                sync: 60,
                zram: true,
                rsync: true,
            },
        );
        mounts.insert(
            "data".to_string(),
            FsMountConfig {
                enabled: true,
                mount: "/var/tmp/pwnagotchi".to_string(),
                size: "10M".to_string(),
                sync: 3600,
                zram: true,
                rsync: true,
            },
        );
        Self {
            enabled: true,
            mounts,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsMountConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    pub mount: String,

    pub size: String,

    #[serde(default = "default_sync")]
    pub sync: u32,

    #[serde(default = "default_true")]
    pub zram: bool,

    #[serde(default = "default_true")]
    pub rsync: bool,
}

fn default_sync() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OxigotchiConfig {
    #[serde(default = "default_true")]
    pub bt_tether_enabled: bool,

    #[serde(default = "default_true")]
    pub bt_agent_enabled: bool,

    #[serde(default)]
    pub phone_mac: String,

    #[serde(default = "default_pisugar_addr")]
    pub pisugar_i2c_addr: u16,

    #[serde(default = "default_pisugar_button_long")]
    pub pisugar_button_long_press: u32,

    #[serde(default = "default_pisugar_watchdog")]
    pub pisugar_watchdog_interval: u32,

    #[serde(default = "default_true")]
    pub wifi_recovery_enabled: bool,

    #[serde(default = "default_wifi_recovery_gpio")]
    pub wifi_recovery_gpio: u32,

    #[serde(default = "default_epoch_duration")]
    pub epoch_duration: u32,

    #[serde(default = "default_attack_rate")]
    pub attack_rate_limit: u32,

    #[serde(default = "default_true")]
    pub display_partial_refresh: bool,

    /// Force a full (de-ghosting) refresh every N epochs. 0 disables it.
    #[serde(default = "default_full_refresh_interval")]
    pub display_full_refresh_interval: u64,

    #[serde(default = "default_true")]
    pub web_ui_enabled: bool,

    #[serde(default = "default_dc_pin")]
    pub display_dc_pin: u32,

    #[serde(default = "default_rst_pin")]
    pub display_rst_pin: u32,

    #[serde(default = "default_busy_pin")]
    pub display_busy_pin: u32,
}

fn default_pisugar_addr() -> u16 {
    0x24
}
fn default_pisugar_button_long() -> u32 {
    3
}
fn default_pisugar_watchdog() -> u32 {
    30
}
fn default_wifi_recovery_gpio() -> u32 {
    4
}
fn default_epoch_duration() -> u32 {
    30
}
fn default_attack_rate() -> u32 {
    1
}
fn default_full_refresh_interval() -> u64 {
    10
}
fn default_dc_pin() -> u32 {
    25
}
fn default_rst_pin() -> u32 {
    17
}
fn default_busy_pin() -> u32 {
    24
}

impl Default for OxigotchiConfig {
    fn default() -> Self {
        Self {
            bt_tether_enabled: true,
            bt_agent_enabled: true,
            phone_mac: String::new(),
            pisugar_i2c_addr: default_pisugar_addr(),
            pisugar_button_long_press: default_pisugar_button_long(),
            pisugar_watchdog_interval: default_pisugar_watchdog(),
            wifi_recovery_enabled: true,
            wifi_recovery_gpio: default_wifi_recovery_gpio(),
            epoch_duration: default_epoch_duration(),
            attack_rate_limit: default_attack_rate(),
            display_partial_refresh: true,
            display_full_refresh_interval: default_full_refresh_interval(),
            web_ui_enabled: true,
            display_dc_pin: default_dc_pin(),
            display_rst_pin: default_rst_pin(),
            display_busy_pin: default_busy_pin(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.main.name, "pwnagotchi-zero");
        assert_eq!(config.main.iface, "wlan0");
        assert!(config.oxigotchi.bt_tether_enabled);
    }
}

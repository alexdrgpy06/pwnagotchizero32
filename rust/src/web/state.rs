//! Shared, live status snapshot exposed to the web dashboard.
//!
//! The epoch loop owns all the runtime data (mood, battery, captures, ...).
//! Rather than couple the web server to every subsystem, the loop writes a
//! plain snapshot into an `Arc<RwLock<StatusSnapshot>>` once per epoch and the
//! HTTP/WebSocket handlers simply read the latest copy.

use std::sync::{Arc, RwLock};

use serde::Serialize;

/// Cheaply-cloneable handle to the live status shared by the epoch loop and the
/// web server.
pub type SharedStatus = Arc<RwLock<StatusSnapshot>>;

/// Full point-in-time view of the daemon, serialized verbatim to the dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct StatusSnapshot {
    pub name: String,
    pub epoch: u64,
    pub mood: String,
    pub face: String,
    pub handshakes: u64,
    pub aps_found: usize,
    pub channel: u8,
    pub blind_epochs: u32,
    pub level: u32,
    pub xp: u64,
    pub battery: u8,
    pub charging: bool,
    pub bluetooth: bool,
    pub bt_device: Option<String>,
    pub cpu_temp: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub uptime: u64,
}

impl StatusSnapshot {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            epoch: 0,
            mood: "happy".to_string(),
            face: "(•‿‿•)".to_string(),
            handshakes: 0,
            aps_found: 0,
            channel: 0,
            blind_epochs: 0,
            level: 1,
            xp: 0,
            battery: 0,
            charging: false,
            bluetooth: false,
            bt_device: None,
            cpu_temp: 0.0,
            ram_used: 0,
            ram_total: 0,
            uptime: 0,
        }
    }
}

/// Build a fresh shared status handle seeded with the unit name.
pub fn new_shared(name: impl Into<String>) -> SharedStatus {
    Arc::new(RwLock::new(StatusSnapshot::new(name)))
}

/// Read `(cpu_temp_c, ram_used_mb, ram_total_mb)` from procfs/sysfs.
///
/// Returns zeros for any value that cannot be read (e.g. when running off the
/// target hardware), so the caller never has to branch on the platform.
pub fn read_system_metrics() -> (f32, u64, u64) {
    let cpu_temp = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|milli| milli / 1000.0)
        .unwrap_or(0.0);

    let (ram_total, ram_used) = read_meminfo().unwrap_or((0, 0));
    (cpu_temp, ram_used, ram_total)
}

/// Parse `/proc/meminfo` and return `(total_mb, used_mb)`.
fn read_meminfo() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kb = None;
    let mut available_kb = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok());
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kb = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok());
        }
        if total_kb.is_some() && available_kb.is_some() {
            break;
        }
    }
    let total = total_kb?;
    let available = available_kb.unwrap_or(total);
    Some((total / 1024, total.saturating_sub(available) / 1024))
}

//! WiFi recovery for BCM43436 firmware hangs

use anyhow::Result;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::config::Config;

pub struct RecoveryManager {
    config: Arc<Config>,
    error_count: u32,
    last_error: Option<Instant>,
    soft_recovery_count: u32,
}

impl RecoveryManager {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            error_count: 0,
            last_error: None,
            soft_recovery_count: 0,
        })
    }

    pub async fn check(&mut self) -> Result<()> {
        // Check dmesg for firmware errors
        let output = Command::new("dmesg")
            .args(["-T", "--since", "60 seconds ago"])
            .output()?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{} {}", stdout, stderr);

        // Look for BCM43436 firmware errors
        let error_patterns = [
            "brcmf_sdio_bus_rxctl: resumed on timeout",
            "brcmfmac: brcmf_sdio_checkdied",
            "brcmfmac: brcmf_sdio_bus_rxctl: resumed on timeout",
            "brcmfmac: brcmf_sdio_hostmail: mailbox indicates firmware halted",
            "mmc1: error",
        ];

        let mut found_error = false;
        for pattern in &error_patterns {
            if combined.contains(pattern) {
                found_error = true;
                break;
            }
        }

        if found_error {
            self.handle_error().await?;
        }

        Ok(())
    }

    async fn handle_error(&mut self) -> Result<()> {
        self.error_count += 1;
        self.last_error = Some(Instant::now());

        // Soft recovery: reset WiFi via GPIO
        if self.soft_recovery_count < 3 {
            self.soft_recovery().await?;
            self.soft_recovery_count += 1;
        } else {
            // Hard recovery: request full power cycle
            self.hard_recovery().await?;
        }

        Ok(())
    }

    async fn soft_recovery(&self) -> Result<()> {
        // Bring down interface
        let _ = Command::new("ip")
            .args(["link", "set", "wlan0", "down"])
            .output();

        // Remove module
        let _ = Command::new("modprobe").args(["-r", "brcmfmac"]).output();

        // Toggle WL_REG_ON GPIO (typically GPIO 4 on Pi Zero W)
        let gpio = self.config.oxigotchi.wifi_recovery_gpio;

        // Export GPIO if needed
        let gpio_path = format!("/sys/class/gpio/gpio{}", gpio);
        if !std::path::Path::new(&gpio_path).exists() {
            let _ = std::fs::write("/sys/class/gpio/export", gpio.to_string());
        }
        let _ = std::fs::write(format!("/sys/class/gpio/gpio{}/direction", gpio), "out");

        // Pulse low
        let _ = std::fs::write(format!("/sys/class/gpio/gpio{}/value", gpio), "0");
        sleep(Duration::from_millis(500)).await;

        // Pulse high
        let _ = std::fs::write(format!("/sys/class/gpio/gpio{}/value", gpio), "1");
        sleep(Duration::from_millis(100)).await;

        // Reload module
        let _ = Command::new("modprobe").args(["brcmfmac"]).output();

        // Wait for interface
        for _ in 0..20 {
            sleep(Duration::from_millis(500)).await;
            let output = Command::new("ip").args(["link", "show", "wlan0"]).output();
            if output.is_ok() && output.unwrap().status.success() {
                break;
            }
        }

        Ok(())
    }

    async fn hard_recovery(&self) -> Result<()> {
        // Signal that hardware power cycle is needed
        // This would typically trigger the display to show "ZOMBIE" face
        // and the safe-shutdown script would handle the actual power cycle
        eprintln!("HARD RECOVERY NEEDED: WiFi firmware dead, requires power cycle");

        // Write recovery state for display
        let _ = std::fs::write("/tmp/pwnagotchi-recovery", "zombie");

        Ok(())
    }

    pub fn is_recovering(&self) -> bool {
        self.soft_recovery_count > 0
    }

    pub fn error_count(&self) -> u32 {
        self.error_count
    }
}

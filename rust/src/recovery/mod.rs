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
        if !self.config.oxigotchi.wifi_recovery_enabled {
            return Ok(());
        }

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

        // Soft recovery: iw-only interface restart, no state that can't be
        // undone in software. If that doesn't clear the fault after a few
        // tries, stop trying and surface it instead of escalating.
        if self.soft_recovery_count < 3 {
            self.soft_recovery().await?;
            self.soft_recovery_count += 1;
        } else {
            self.hard_recovery().await?;
        }

        Ok(())
    }

    /// Restart monitor mode without touching the radio's power state. This is
    /// deliberately narrow: no modprobe -r, no GPIO toggling of WL_REG_ON, no
    /// MMC/SDIO unbind. Those actions can leave the SDIO bus in a state that
    /// only a physical power cycle clears — exactly the "wlan0mon MAC all
    /// zeros / RSSI -100" zombie state this recovery exists to avoid.
    async fn soft_recovery(&self) -> Result<()> {
        let _ = Command::new("ip")
            .args(["link", "set", "wlan0mon", "down"])
            .output();
        sleep(Duration::from_millis(500)).await;
        let _ = Command::new("iw")
            .args(["dev", "wlan0mon", "set", "type", "monitor"])
            .output();
        let _ = Command::new("ip")
            .args(["link", "set", "wlan0mon", "up"])
            .output();

        for _ in 0..10 {
            sleep(Duration::from_millis(500)).await;
            let output = Command::new("ip").args(["link", "show", "wlan0mon"]).output();
            if output.is_ok() && output.unwrap().status.success() {
                break;
            }
        }

        Ok(())
    }

    /// No automatic power-cycle attempt: GPIO-toggling WL_REG_ON or
    /// unbinding the SDIO controller from software has, in practice (see
    /// CoderFX/oxigotchi's v3.3.5 fix), left the chip in a state that needs a
    /// physical USB+battery power cycle to clear — worse than doing nothing.
    /// Surface the failure and let the operator power-cycle instead.
    async fn hard_recovery(&self) -> Result<()> {
        eprintln!("HARD RECOVERY NEEDED: WiFi firmware dead, requires power cycle");
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

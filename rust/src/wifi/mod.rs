//! WiFi management for monitor mode and channel hopping

use anyhow::Result;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::config::Config;

/// WiFi manager for monitor mode operations
pub struct WifiManager {
    config: Arc<Config>,
    interface: String,
    monitor_interface: String,
    current_channel: u8,
    ap_list: Vec<AccessPoint>,
    client_list: Vec<Client>,
}

#[derive(Debug, Clone)]
pub struct AccessPoint {
    pub bssid: String,
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
    pub encryption: String,
    pub vendor: String,
}

#[derive(Debug, Clone)]
pub struct Client {
    pub mac: String,
    pub ap_bssid: String,
    pub rssi: i8,
}

impl WifiManager {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        let interface = config.main.iface.clone();
        let monitor_interface = format!("{}mon", interface.trim_end_matches("mon"));
        
        Ok(Self {
            config: config.clone(),
            interface: interface.clone(),
            monitor_interface,
            current_channel: 1,
            ap_list: Vec::new(),
            client_list: Vec::new(),
        })
    }
    
    pub async fn start_monitor_mode(&mut self) -> Result<()> {
        // Stop any existing monitor mode
        self.stop_monitor_mode().await.ok();
        
        // Bring interface down
        self.run_cmd("ip", &["link", "set", &self.interface, "down"]).await?;
        
        // Set monitor mode
        self.run_cmd("iw", &["dev", &self.interface, "set", "type", "monitor"]).await?;
        
        // Bring up
        self.run_cmd("ip", &["link", "set", &self.monitor_interface, "up"]).await?;
        
        // Set initial channel
        self.set_channel(1).await?;
        
        Ok(())
    }
    
    pub async fn stop_monitor_mode(&mut self) -> Result<()> {
        self.run_cmd("ip", &["link", "set", &self.monitor_interface, "down"]).await.ok();
        self.run_cmd("iw", &["dev", &self.monitor_interface, "set", "type", "managed"]).await.ok();
        self.run_cmd("ip", &["link", "set", &self.interface, "up"]).await.ok();
        Ok(())
    }
    
    pub async fn set_channel(&mut self, channel: u8) -> Result<()> {
        if channel < 1 || channel > 13 {
            anyhow::bail!("Invalid channel: {}", channel);
        }
        
        self.run_cmd("iw", &["dev", &self.monitor_interface, "set", "freq", &format!("{}", 2407u16 + channel as u16 * 5)]).await?;
        self.current_channel = channel;
        Ok(())
    }
    
    pub async fn hop_channel(&mut self) -> Result<()> {
        let channels = [1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9, 5, 10];
        let next = channels.iter().find(|&&c| c != self.current_channel).copied().unwrap_or(1);
        self.set_channel(next).await
    }
    
    pub async fn scan(&mut self, duration: Duration) -> Result<Vec<AccessPoint>> {
        // Use iw to scan
        let output = self.run_cmd_capture("iw", &["dev", &self.monitor_interface, "scan"]).await?;
        
        // Parse scan results (simplified)
        self.ap_list = self.parse_scan_results(&output);
        Ok(self.ap_list.clone())
    }
    
    fn parse_scan_results(&self, output: &str) -> Vec<AccessPoint> {
        let mut aps = Vec::new();
        let mut current_ap: Option<AccessPoint> = None;
        
        for line in output.lines() {
            let line = line.trim();
            
            if line.starts_with("BSS ") {
                if let Some(ap) = current_ap.take() {
                    aps.push(ap);
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    current_ap = Some(AccessPoint {
                        bssid: parts[1].to_string(),
                        ssid: String::new(),
                        channel: 1,
                        rssi: -100,
                        encryption: String::new(),
                        vendor: String::new(),
                    });
                }
            } else if let Some(ref mut ap) = current_ap {
                if line.starts_with("SSID: ") {
                    ap.ssid = line[6..].to_string();
                } else if line.starts_with("freq: ") {
                    let freq: u32 = line[6..].parse().unwrap_or(2412);
                    ap.channel = Self::freq_to_channel(freq);
                } else if line.starts_with("signal: ") {
                    let rssi_str = line[8..].split(' ').next().unwrap_or("-100");
                    ap.rssi = rssi_str.parse().unwrap_or(-100);
                } else if line.contains("WPA") || line.contains("WEP") || line.contains("RSN") {
                    ap.encryption = line.trim().to_string();
                }
            }
        }
        
        if let Some(ap) = current_ap {
            aps.push(ap);
        }
        
        aps
    }
    
    fn freq_to_channel(freq: u32) -> u8 {
        if freq == 2484 { return 14; }
        if freq >= 2412 && freq <= 2472 {
            return ((freq - 2412) / 5 + 1) as u8;
        }
        1
    }
    
    pub fn current_channel(&self) -> u8 {
        self.current_channel
    }
    
    pub fn get_aps(&self) -> &[AccessPoint] {
        &self.ap_list
    }
    
    pub fn get_clients(&self) -> &[Client] {
        &self.client_list
    }
    
    pub fn is_whitelisted(&self, ssid: &str, bssid: &str) -> bool {
        for entry in &self.config.main.whitelist {
            if entry == ssid || entry == bssid {
                return true;
            }
            // Check MAC prefix
            if entry.len() >= 8 && bssid.starts_with(&entry[..8]) {
                return true;
            }
        }
        false
    }
    
    async fn run_cmd(&self, cmd: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(cmd).args(args).status()?;
        if !status.success() {
            anyhow::bail!("Command failed: {} {}", cmd, args.join(" "));
        }
        Ok(())
    }
    
    async fn run_cmd_capture(&self, cmd: &str, args: &[&str]) -> Result<String> {
        let output = Command::new(cmd).args(args).output()?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
//! Bluetooth PAN tethering

use anyhow::Result;
use std::process::Command;
use std::sync::Arc;

use crate::config::Config;

pub struct BluetoothManager {
    config: Arc<Config>,
    connected: bool,
    current_device: Option<String>,
}

impl BluetoothManager {
    pub async fn new(config: &Arc<Config>) -> Result<Self> {
        // Start bluetooth service
        Command::new("systemctl").args(["start", "bluetooth"]).output().ok();
        
        Ok(Self {
            config: config.clone(),
            connected: false,
            current_device: None,
        })
    }
    
    pub async fn connect_pan(&mut self, mac: &str) -> Result<bool> {
        if mac.is_empty() {
            return Ok(false);
        }
        
        // Pair and trust
        let _ = Command::new("bluetoothctl")
            .args(["pair", mac])
            .output();
        let _ = Command::new("bluetoothctl")
            .args(["trust", mac])
            .output();
        
        // Connect PAN
        let output = Command::new("bt-network")
            .args(["-c", mac, "nap"])
            .output()?;
        
        if output.status.success() {
            self.connected = true;
            self.current_device = Some(mac.to_string());
            
            // Request DHCP on bnep0
            let _ = Command::new("dhcpcd")
                .args(["bnep0"])
                .output();
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    pub async fn disconnect_pan(&mut self) -> Result<()> {
        if let Some(mac) = &self.current_device {
            let _ = Command::new("bt-network")
                .args(["-d", mac])
                .output();
            
            let _ = Command::new("dhcpcd")
                .args(["-k", "bnep0"])
                .output();
            
            self.connected = false;
            self.current_device = None;
        }
        Ok(())
    }
    
    pub fn is_connected(&self) -> bool {
        self.connected
    }
    
    pub fn current_device(&self) -> Option<&str> {
        self.current_device.as_deref()
    }
}
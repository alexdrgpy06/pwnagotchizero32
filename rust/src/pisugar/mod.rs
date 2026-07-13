//! PiSugar UPS battery management

use anyhow::Result;
use std::time::Duration;

use crate::config::Config;

/// PiSugar register map
mod reg {
    pub const BATTERY_LEVEL: u8 = 0x0A;
    pub const CHARGING_STATUS: u8 = 0x0B;
    pub const BUTTON_STATUS: u8 = 0x0C;
    pub const FIRMWARE_VERSION: u8 = 0x00;
    pub const POWER_OFF: u8 = 0x10;
    pub const WAKEUP_TIMER: u8 = 0x11;
    pub const WATCHDOG: u8 = 0x12;
}

/// PiSugar manager
pub struct PiSugar {
    battery_percent: u8,
    charging: bool,
    button_pressed: bool,
    /// True only once a battery register has been read successfully, i.e. a
    /// PiSugar is actually present. Guards against acting on placeholder values.
    present: bool,
    #[cfg(target_os = "linux")]
    device: Option<i2cdev::linux::LinuxI2CDevice>,
    long_press_secs: u32,
}

impl PiSugar {
    #[cfg(target_os = "linux")]
    pub async fn new(config: &std::sync::Arc<Config>) -> Result<Self> {
        let addr = config.oxigotchi.pisugar_i2c_addr as u16;
        let device = i2cdev::linux::LinuxI2CDevice::new("/dev/i2c-1", addr).ok();
        let long_press_secs = config.oxigotchi.pisugar_button_long_press;

        let mut slf = Self {
            battery_percent: 100,
            charging: false,
            button_pressed: false,
            present: false,
            device,
            long_press_secs,
        };
        slf.read_all().await.ok();
        Ok(slf)
    }

    #[cfg(not(target_os = "linux"))]
    pub async fn new(config: &std::sync::Arc<Config>) -> Result<Self> {
        Ok(Self {
            battery_percent: 100,
            charging: false,
            button_pressed: false,
            present: false,
            long_press_secs: config.oxigotchi.pisugar_button_long_press,
        })
    }

    #[cfg(target_os = "linux")]
    async fn read_all(&mut self) -> Result<()> {
        match self.read_reg(reg::BATTERY_LEVEL).await {
            Ok(level) => {
                // A PiSugar reports 0..=100. Values outside that mean garbage on
                // the bus (no PiSugar) — don't treat those as a real reading.
                if level <= 100 {
                    self.battery_percent = level;
                    self.present = true;
                }
            }
            // No device / read error: keep the safe default and stay "absent"
            // so the low-battery auto-shutdown never triggers on a phantom 0%.
            Err(_) => {
                self.battery_percent = 100;
            }
        }
        let charging_raw = self.read_reg(reg::CHARGING_STATUS).await.unwrap_or(0);
        self.charging = charging_raw & 0x01 != 0;
        let btn_raw = self.read_reg(reg::BUTTON_STATUS).await.unwrap_or(0);
        self.button_pressed = btn_raw & 0x01 != 0;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    async fn read_all(&mut self) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn read_reg(&mut self, reg: u8) -> Result<u8> {
        use i2cdev::core::I2CDevice;
        if let Some(dev) = &mut self.device {
            let val = dev.smbus_read_byte_data(reg)?;
            Ok(val)
        } else {
            // No I2C device: this is an error, NOT a valid 0 reading. Returning
            // Ok(0) here made battery_percent fall to 0 and tripped the
            // low-battery shutdown when no PiSugar was present.
            anyhow::bail!("no pisugar i2c device")
        }
    }

    #[cfg(target_os = "linux")]
    async fn write_reg(&mut self, reg: u8, value: u8) -> Result<()> {
        use i2cdev::core::I2CDevice;
        if let Some(dev) = &mut self.device {
            dev.smbus_write_byte_data(reg, value)?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    async fn read_reg(&mut self, _reg: u8) -> Result<u8> {
        Ok(0)
    }

    #[cfg(not(target_os = "linux"))]
    async fn write_reg(&mut self, _reg: u8, _value: u8) -> Result<()> {
        Ok(())
    }

    pub async fn update(&mut self) -> Result<()> {
        self.read_all().await
    }

    pub fn battery_percent(&self) -> u8 {
        self.battery_percent
    }

    /// Whether a PiSugar was actually detected. Used to gate battery-based
    /// actions so a missing UPS can never trigger a shutdown.
    pub fn is_present(&self) -> bool {
        self.present
    }

    pub fn is_charging(&self) -> bool {
        self.charging
    }

    pub fn is_button_pressed(&self) -> bool {
        self.button_pressed
    }

    pub async fn check_button_hold(&self) -> Result<bool> {
        if !self.button_pressed {
            return Ok(false);
        }
        let hold_time = self.long_press_secs as u64;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(hold_time) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(true)
    }

    pub async fn set_watchdog(&mut self, seconds: u16) -> Result<()> {
        self.write_reg(reg::WATCHDOG, (seconds & 0xFF) as u8)
            .await?;
        self.write_reg(reg::WATCHDOG + 1, ((seconds >> 8) & 0xFF) as u8)
            .await?;
        Ok(())
    }

    pub async fn power_off(&mut self) -> Result<()> {
        self.write_reg(reg::POWER_OFF, 0x01).await
    }

    pub async fn set_wakeup_timer(&mut self, seconds: u32) -> Result<()> {
        let bytes = seconds.to_le_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            self.write_reg(reg::WAKEUP_TIMER + i as u8, b).await?;
        }
        Ok(())
    }
}

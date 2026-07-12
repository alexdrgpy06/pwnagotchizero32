//! E-ink driver for Waveshare 2.13" V4 (SSD1680)

use anyhow::Result;

use crate::config::Config;
use crate::display::buffer::FrameBuffer;

/// SSD1680 commands
mod cmd {
    pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;
    pub const GATE_DRIVING_VOLTAGE: u8 = 0x03;
    pub const SOURCE_DRIVING_VOLTAGE: u8 = 0x04;
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    pub const SW_RESET: u8 = 0x12;
    pub const TEMPERATURE_SENSOR: u8 = 0x1A;
    pub const MASTER_ACTIVATION: u8 = 0x20;
    pub const DISPLAY_UPDATE_CONTROL_1: u8 = 0x21;
    pub const DISPLAY_UPDATE_CONTROL_2: u8 = 0x22;
    pub const WRITE_RAM: u8 = 0x24;
    pub const WRITE_RAM_RED: u8 = 0x26;
    pub const BORDER_WAVEFORM: u8 = 0x3C;
    pub const SET_RAM_X_ADDRESS: u8 = 0x44;
    pub const SET_RAM_Y_ADDRESS: u8 = 0x45;
    pub const SET_RAM_X_COUNTER: u8 = 0x4E;
    pub const SET_RAM_Y_COUNTER: u8 = 0x4F;
}

/// E-ink display driver (SSD1680 over SPI + GPIO)
pub struct EpdDriver {
    #[cfg(target_os = "linux")]
    spi: spidev::Spidev,
    dc_pin: u32,
    rst_pin: u32,
    busy_pin: u32,
    width: u32,
    height: u32,
}

impl EpdDriver {
    #[cfg(target_os = "linux")]
    pub fn new(config: &Config) -> Result<Self> {
        use spidev::{Spidev, SpidevOptions, SpiModeFlags};
        use std::fs;

        let mut spi = Spidev::open("/dev/spidev0.0")?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(4_000_000)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build();
        spi.configure(&options)?;

        let dc_pin = config.oxigotchi.display_dc_pin;
        let rst_pin = config.oxigotchi.display_rst_pin;
        let busy_pin = config.oxigotchi.display_busy_pin;

        Self::setup_gpio(dc_pin, rst_pin, busy_pin)?;

        Ok(Self {
            spi,
            dc_pin,
            rst_pin,
            busy_pin,
            width: 250,
            height: 122,
        })
    }

    /// Non-Linux stub: SPI sysfs absent. Only safe for type-checking or unit tests
    /// that never touch hardware; **never** call method bodies on this instance.
    #[cfg(not(target_os = "linux"))]
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            dc_pin: config.oxigotchi.display_dc_pin,
            rst_pin: config.oxigotchi.display_rst_pin,
            busy_pin: config.oxigotchi.display_busy_pin,
            width: 250,
            height: 122,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn mock() -> Self {
        Self {
            dc_pin: 25,
            rst_pin: 17,
            busy_pin: 24,
            width: 250,
            height: 122,
        }
    }

    #[cfg(target_os = "linux")]
    fn setup_gpio(dc: u32, rst: u32, busy: u32) -> Result<()> {
        use std::fs;
        for &pin in &[dc, rst, busy] {
            let gpio_dir = format!("/sys/class/gpio/gpio{}", pin);
            if !std::path::Path::new(&gpio_dir).exists() {
                fs::write("/sys/class/gpio/export", pin.to_string())?;
            }
        }
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", dc), "out");
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", rst), "out");
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", busy), "in");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn gpio_write(&self, pin: u32, value: u8) -> Result<()> {
        std::fs::write(format!("/sys/class/gpio/gpio{}/value", pin), value.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn gpio_read(&self, pin: u32) -> Result<u8> {
        let val = std::fs::read_to_string(format!("/sys/class/gpio/gpio{}/value", pin))?;
        Ok(val.trim().parse().unwrap_or(0))
    }

    #[cfg(target_os = "linux")]
    fn send_command(&mut self, cmd: u8) -> Result<()> {
        use std::io::Write;
        self.gpio_write(self.dc_pin, 0)?;
        self.spi.write(&[cmd])?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn send_data(&mut self, data: &[u8]) -> Result<()> {
        use std::io::Write;
        self.gpio_write(self.dc_pin, 1)?;
        self.spi.write(data)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn send_data_byte(&mut self, data: u8) -> Result<()> {
        use std::io::Write;
        self.gpio_write(self.dc_pin, 1)?;
        self.spi.write(&[data])?;
        Ok(())
    }

    /// Hardware + software reset, then initial register configuration.
    #[cfg(target_os = "linux")]
    pub fn init(&mut self) -> Result<()> {
        use std::{fs, thread, time::Duration};

        self.gpio_write(self.rst_pin, 0)?;
        thread::sleep(Duration::from_millis(10));
        self.gpio_write(self.rst_pin, 1)?;
        thread::sleep(Duration::from_millis(10));

        self.send_command(cmd::SW_RESET)?;
        thread::sleep(Duration::from_millis(10));

        self.send_command(cmd::DRIVER_OUTPUT_CONTROL)?;
        self.send_data_byte((self.height - 1) as u8)?;
        self.send_data_byte(((self.height - 1) >> 8) as u8)?;
        self.send_data_byte(0x00)?;

        self.send_command(cmd::GATE_DRIVING_VOLTAGE)?;
        self.send_data_byte(0x00)?;

        self.send_command(cmd::SOURCE_DRIVING_VOLTAGE)?;
        self.send_data_byte(0x0F)?;
        self.send_data_byte(0x00)?;

        self.send_command(cmd::DATA_ENTRY_MODE)?;
        self.send_data_byte(0x03)?;

        self.send_command(cmd::SET_RAM_X_ADDRESS)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x18)?;

        self.send_command(cmd::SET_RAM_Y_ADDRESS)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte((self.height - 1) as u8)?;
        self.send_data_byte(((self.height - 1) >> 8) as u8)?;

        self.send_command(cmd::BORDER_WAVEFORM)?;
        self.send_data_byte(0x03)?;

        self.send_command(cmd::TEMPERATURE_SENSOR)?;
        self.send_data_byte(0x80)?;

        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data_byte(0xB1)?;
        self.send_command(cmd::MASTER_ACTIVATION)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn init(&mut self) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn clear(&mut self) -> Result<()> {
        let buffer_size = (self.width * self.height / 8) as usize;
        let empty_data = vec![0xFF; buffer_size];
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(&empty_data)?;
        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data_byte(0xC7)?;
        self.send_command(cmd::MASTER_ACTIVATION)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn clear(&mut self) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn full_update(&mut self, buffer: &FrameBuffer) -> Result<()> {
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(buffer.as_bytes())?;
        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data_byte(0xC7)?;
        self.send_command(cmd::MASTER_ACTIVATION)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn full_update(&mut self, _buffer: &FrameBuffer) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn partial_update(&mut self, buffer: &FrameBuffer) -> Result<()> {
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(buffer.as_bytes())?;
        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data_byte(0x0C)?;
        self.send_command(cmd::MASTER_ACTIVATION)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn partial_update(&mut self, _buffer: &FrameBuffer) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn sleep(&mut self) -> Result<()> {
        self.send_command(cmd::DEEP_SLEEP_MODE)?;
        self.send_data_byte(0x01)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn sleep(&mut self) -> Result<()> {
        Ok(())
    }
}

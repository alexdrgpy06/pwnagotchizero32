//! E-ink driver for Waveshare 2.13" V4 (SSD1680)

use anyhow::Result;

use crate::config::Config;
use crate::display::buffer::FrameBuffer;

/// SSD1680 commands
mod cmd {
    pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    pub const SW_RESET: u8 = 0x12;
    pub const TEMP_CONTROL: u8 = 0x18;
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

/// Sequence byte for command 0x22 (DISPLAY_UPDATE_CONTROL_2).
mod update_mode {
    /// Load temperature + LUT and run a full refresh (screen flashes clean).
    pub const FULL: u8 = 0xF7;
    /// Reuse the loaded LUT for a partial refresh (no flashing).
    pub const PARTIAL: u8 = 0x0C;
}

/// E-ink display driver (SSD1680 over SPI + GPIO)
pub struct EpdDriver {
    // `None` when the SPI device couldn't be opened/configured at startup
    // (wrong bus, dtoverlay not applied yet, permissions). The daemon must
    // keep running headless rather than dying entirely over one bad panel —
    // every send_* method below becomes a no-op in that case.
    #[cfg(target_os = "linux")]
    spi: Option<spidev::Spidev>,
    dc_pin: u32,
    rst_pin: u32,
    busy_pin: u32,
    width: u32,
    height: u32,
}

impl EpdDriver {
    #[cfg(target_os = "linux")]
    pub fn new(config: &Config) -> Result<Self> {
        use spidev::{SpiModeFlags, Spidev, SpidevOptions};

        let spi = match Spidev::open("/dev/spidev0.0") {
            Ok(mut spi) => {
                let options = SpidevOptions::new()
                    .bits_per_word(8)
                    .max_speed_hz(4_000_000)
                    .mode(SpiModeFlags::SPI_MODE_0)
                    .build();
                match spi.configure(&options) {
                    Ok(()) => Some(spi),
                    Err(e) => {
                        tracing::warn!("e-ink SPI configure failed, running headless: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("e-ink SPI open failed, running headless: {e}");
                None
            }
        };

        let dc_pin = config.oxigotchi.display_dc_pin;
        let rst_pin = config.oxigotchi.display_rst_pin;
        let busy_pin = config.oxigotchi.display_busy_pin;

        if spi.is_some() {
            Self::setup_gpio(dc_pin, rst_pin, busy_pin)?;
        }

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
                let _ = fs::write("/sys/class/gpio/export", pin.to_string());
            }
        }
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", dc), "out");
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", rst), "out");
        let _ = fs::write(format!("/sys/class/gpio/gpio{}/direction", busy), "in");
        Ok(())
    }

    // Associated functions (no `self`) so send_command/send_data can call
    // them while holding a `&mut` borrow of `self.spi` at the same time.
    #[cfg(target_os = "linux")]
    fn gpio_write(pin: u32, value: u8) -> Result<()> {
        std::fs::write(
            format!("/sys/class/gpio/gpio{}/value", pin),
            value.to_string(),
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn gpio_read(pin: u32) -> Result<u8> {
        let val = std::fs::read_to_string(format!("/sys/class/gpio/gpio{}/value", pin))?;
        Ok(val.trim().parse().unwrap_or(0))
    }

    /// Block until the panel deasserts BUSY (active-high on the SSD1680).
    ///
    /// Every RAM write and display activation raises BUSY; issuing the next
    /// command before it clears corrupts the frame and makes the panel flash.
    /// A stuck panel must not take the daemon down, so a timeout is logged and
    /// treated as non-fatal rather than propagated.
    #[cfg(target_os = "linux")]
    fn wait_until_idle(&self) {
        use std::{
            thread,
            time::{Duration, Instant},
        };
        let start = Instant::now();
        loop {
            match Self::gpio_read(self.busy_pin) {
                Ok(0) => return,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("e-ink BUSY read failed: {e}");
                    return;
                }
            }
            if start.elapsed() > Duration::from_secs(10) {
                tracing::warn!("e-ink BUSY timeout after 10s; continuing");
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Hardware reset pulse (RST is active-low).
    #[cfg(target_os = "linux")]
    fn hw_reset(&self) -> Result<()> {
        use std::{thread, time::Duration};
        Self::gpio_write(self.rst_pin, 1)?;
        thread::sleep(Duration::from_millis(20));
        Self::gpio_write(self.rst_pin, 0)?;
        thread::sleep(Duration::from_millis(5));
        Self::gpio_write(self.rst_pin, 1)?;
        thread::sleep(Duration::from_millis(20));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn send_command(&mut self, command: u8) -> Result<()> {
        use std::io::Write;
        let dc_pin = self.dc_pin;
        let Some(spi) = self.spi.as_mut() else {
            return Ok(());
        };
        Self::gpio_write(dc_pin, 0)?;
        spi.write_all(&[command])?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn send_data(&mut self, data: &[u8]) -> Result<()> {
        use std::io::Write;
        let dc_pin = self.dc_pin;
        let Some(spi) = self.spi.as_mut() else {
            return Ok(());
        };
        Self::gpio_write(dc_pin, 1)?;
        // spidev caps a single transfer; chunk to stay within the driver limit.
        for chunk in data.chunks(4096) {
            spi.write_all(chunk)?;
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn send_data_byte(&mut self, data: u8) -> Result<()> {
        use std::io::Write;
        let dc_pin = self.dc_pin;
        let Some(spi) = self.spi.as_mut() else {
            return Ok(());
        };
        Self::gpio_write(dc_pin, 1)?;
        spi.write_all(&[data])?;
        Ok(())
    }

    /// Position the RAM address counters at the top-left of the frame.
    #[cfg(target_os = "linux")]
    fn set_cursor(&mut self) -> Result<()> {
        self.send_command(cmd::SET_RAM_X_COUNTER)?;
        self.send_data_byte(0x00)?;
        self.send_command(cmd::SET_RAM_Y_COUNTER)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x00)?;
        Ok(())
    }

    /// Reset, software-reset, and configure the driver for the 122×250 panel.
    #[cfg(target_os = "linux")]
    pub fn init(&mut self) -> Result<()> {
        self.hw_reset()?;
        self.wait_until_idle();

        self.send_command(cmd::SW_RESET)?;
        self.wait_until_idle();

        // Gate = 250 lines (0..249).
        self.send_command(cmd::DRIVER_OUTPUT_CONTROL)?;
        self.send_data_byte(0xF9)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x00)?;

        // X increment, Y increment.
        self.send_command(cmd::DATA_ENTRY_MODE)?;
        self.send_data_byte(0x03)?;

        // RAM X window: bytes 0..15 (128 source bits, covers the 122 used).
        self.send_command(cmd::SET_RAM_X_ADDRESS)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x0F)?;

        // RAM Y window: gates 0..249.
        self.send_command(cmd::SET_RAM_Y_ADDRESS)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0xF9)?;
        self.send_data_byte(0x00)?;

        self.send_command(cmd::BORDER_WAVEFORM)?;
        self.send_data_byte(0x05)?;

        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_1)?;
        self.send_data_byte(0x00)?;
        self.send_data_byte(0x80)?;

        // Use the built-in temperature sensor to pick the waveform.
        self.send_command(cmd::TEMP_CONTROL)?;
        self.send_data_byte(0x80)?;

        self.set_cursor()?;
        self.wait_until_idle();
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn init(&mut self) -> Result<()> {
        Ok(())
    }

    /// Blank the panel white with a full refresh.
    #[cfg(target_os = "linux")]
    pub fn clear(&mut self) -> Result<()> {
        use crate::display::PANEL_RAM_SIZE;
        let blank = vec![0xFFu8; PANEL_RAM_SIZE];

        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(&blank)?;
        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM_RED)?;
        self.send_data(&blank)?;

        self.refresh(update_mode::FULL)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn clear(&mut self) -> Result<()> {
        Ok(())
    }

    /// Full refresh: flashes the panel clean, eliminating any ghosting.
    #[cfg(target_os = "linux")]
    pub fn full_update(&mut self, buffer: &FrameBuffer) -> Result<()> {
        let ram = buffer.to_panel_ram();

        // Seed both the working (0x24) and reference (0x26) RAM banks so the
        // next partial refresh has a correct baseline to diff against.
        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(&ram)?;
        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM_RED)?;
        self.send_data(&ram)?;

        self.refresh(update_mode::FULL)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn full_update(&mut self, _buffer: &FrameBuffer) -> Result<()> {
        Ok(())
    }

    /// Partial refresh: updates only changed pixels without flashing.
    #[cfg(target_os = "linux")]
    pub fn partial_update(&mut self, buffer: &FrameBuffer) -> Result<()> {
        let ram = buffer.to_panel_ram();

        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM)?;
        self.send_data(&ram)?;

        self.refresh(update_mode::PARTIAL)?;

        // Promote the new frame into the reference bank so the following
        // partial refresh diffs against what is actually on screen.
        self.set_cursor()?;
        self.send_command(cmd::WRITE_RAM_RED)?;
        self.send_data(&ram)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn partial_update(&mut self, _buffer: &FrameBuffer) -> Result<()> {
        Ok(())
    }

    /// Trigger a display update with the given 0x22 sequence and wait for it.
    #[cfg(target_os = "linux")]
    fn refresh(&mut self, mode: u8) -> Result<()> {
        self.send_command(cmd::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data_byte(mode)?;
        self.send_command(cmd::MASTER_ACTIVATION)?;
        self.wait_until_idle();
        Ok(())
    }

    /// Enter deep sleep. `init()` must be called again before the next update.
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

//! Display API for drawing faces, text, and status

use anyhow::Result;
use embedded_graphics::{
    mono_font::ascii::FONT_6X10,
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
    Drawable,
};

use crate::config::Config;
use crate::display::buffer::FrameBuffer;
use crate::display::driver::EpdDriver;

/// High-level display: owns the driver, a 1-bit framebuffer, and drawing helpers.
pub struct Display {
    driver: EpdDriver,
    buffer: FrameBuffer,
    current_face: String,
    epoch_count: u64,
}

impl Display {
    pub fn new(config: &Config) -> Result<Self> {
        let driver = EpdDriver::new(config)?;
        let buffer = FrameBuffer::new();
        Ok(Self {
            driver,
            buffer,
            current_face: "happy".to_string(),
            epoch_count: 0,
        })
    }

    /// Constructor that bypasses real hardware (for non-Linux / CI).
    #[cfg(not(target_os = "linux"))]
    pub fn mock() -> Self {
        Self {
            driver: EpdDriver::mock(),
            buffer: FrameBuffer::new(),
            current_face: "happy".to_string(),
            epoch_count: 0,
        }
    }

    pub fn init(&mut self) -> Result<()> {
        self.driver.init()?;
        self.driver.clear()?;
        self.show_boot_face()?;
        Ok(())
    }

    pub fn show_boot_face(&mut self) -> Result<()> {
        self.clear()?;
        self.draw_face("awake");
        self.driver.full_update(&self.buffer)?;
        Ok(())
    }

    pub fn show_shutdown(&mut self) -> Result<()> {
        self.clear()?;
        self.draw_face("broken");
        self.draw_text_centered("SHUTDOWN", 50);
        self.driver.full_update(&self.buffer)?;
        Ok(())
    }

    pub fn show_zombie(&mut self) -> Result<()> {
        self.clear()?;
        self.draw_face("broken");
        self.draw_text_centered("ZOMBIE", 40);
        self.draw_text_centered("UNPLUG USB+BATT", 54);
        self.draw_text_centered("WAIT 30-60s", 68);
        self.driver.full_update(&self.buffer)?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.buffer.clear();
        Ok(())
    }

    /// Draw one of the named kaomoji faces at the personality position.
    pub fn draw_face(&mut self, face_name: &str) {
        self.current_face = face_name.to_string();
        let face = match face_name {
            "awake" => "(•‿‿•)",
            "sleep" => "(─‿─)",
            "happy" => "(^‿‿^)",
            "excited" => "(ᵔ◡◡ᵔ)",
            "bored" => "(・_・)",
            "intense" => "(•̀ᴗ•́)و",
            "cool" => "(⌐■_■)",
            "sad" => "(╥﹏╥)",
            "angry" => "(╬ Ò﹏Ó)",
            "broken" => "(☓‿‿☓)",
            "upload" => "(1_0)",
            "motivated" => "(★‿★)",
            "demotivated" => "(≖_≖)",
            "smart" => "(✜‿‿✜)",
            "lonely" => "(ب_ب)",
            "grateful" => "(^‿‿^)",
            "friend" => "(♥‿‿♥)",
            "debug" => "(#_#)",
            _ => "(•‿‿•)",
        };
        let _ = self.draw_text_centered(face, 16);
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str) -> Result<()> {
        let style = embedded_graphics::mono_font::MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::with_baseline(text, Point::new(x, y), style, Baseline::Top)
            .draw(&mut self.buffer)?;
        Ok(())
    }

    pub fn draw_text_centered(&mut self, text: &str, y: i32) -> Result<()> {
        let style = embedded_graphics::mono_font::MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let bounds = Text::with_baseline(text, Point::zero(), style, Baseline::Top).bounding_box();
        let x = (self.buffer.width() as i32 - bounds.size.width as i32) / 2;
        Text::with_baseline(text, Point::new(x, y), style, Baseline::Top)
            .draw(&mut self.buffer)?;
        Ok(())
    }

    pub fn draw_status_line(&mut self, y: i32, status: &str) -> Result<()> {
        self.draw_text(0, y, status)
    }

    pub fn draw_battery(&mut self, x: i32, y: i32, percent: u8) -> Result<()> {
        self.draw_text(x, y, &format!("[{:>3}%]", percent))?;
        let bar_width = 20;
        let filled = (bar_width * percent as i32 / 100).max(1);
        let mut bar = String::new();
        for i in 0..bar_width {
            bar.push(if i < filled { '█' } else { '░' });
        }
        self.draw_text(x, y + 10, &bar)
    }

    /// Push the current framebuffer to the screen (partial or full).
    pub fn update(&mut self, partial: bool) -> Result<()> {
        if partial {
            self.driver.partial_update(&self.buffer)?;
        } else {
            self.driver.full_update(&self.buffer)?;
        }
        Ok(())
    }

    pub fn sleep(&mut self) -> Result<()> {
        self.driver.sleep()
    }

    pub fn epoch_count(&self) -> u64 {
        self.epoch_count
    }

    pub fn tick_epoch(&mut self) -> u64 {
        self.epoch_count += 1;
        self.epoch_count
    }
}

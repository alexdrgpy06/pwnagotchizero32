//! Display API for drawing faces, text, and status

use anyhow::Result;
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoFont,
    },
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

    /// Draw the face for the given mood. Faces are intentionally ASCII-only so
    /// they render on the bitmap `FONT_10X20`; non-ASCII kaomoji show as blank
    /// boxes on the e-ink panel's fonts.
    pub fn draw_face(&mut self, face_name: &str) {
        self.current_face = face_name.to_string();
        let face = Self::ascii_face(face_name);
        let _ = self.draw_face_str(face);
    }

    /// Map a mood name to a renderable ASCII face.
    fn ascii_face(face_name: &str) -> &'static str {
        match face_name {
            "awake" => "(o__o)",
            "sleep" => "(-__-)",
            "happy" => "(^__^)",
            "excited" => "(O__O)",
            "bored" => "(-__-)",
            "intense" => "(>__<)",
            "cool" => "(-.-)",
            "sad" => "(T__T)",
            "angry" => "(x__x)",
            "broken" => "(X__X)",
            "upload" => "(1__0)",
            "motivated" => "(*__*)",
            "demotivated" => "(z__z)",
            "smart" => "(+__+)",
            "lonely" => "(u__u)",
            "grateful" => "(^__^)",
            "friend" => "(<3_3)",
            "debug" => "(#__#)",
            _ => "(o__o)",
        }
    }

    /// Draw an arbitrary face string centered near the top in the large font.
    pub fn draw_face_str(&mut self, face: &str) -> Result<()> {
        self.draw_text_centered_font(face, 20, &FONT_10X20)
    }

    pub fn draw_text(&mut self, x: i32, y: i32, text: &str) -> Result<()> {
        let style = embedded_graphics::mono_font::MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::with_baseline(text, Point::new(x, y), style, Baseline::Top).draw(&mut self.buffer)?;
        Ok(())
    }

    pub fn draw_text_centered(&mut self, text: &str, y: i32) -> Result<()> {
        self.draw_text_centered_font(text, y, &FONT_6X10)
    }

    fn draw_text_centered_font(&mut self, text: &str, y: i32, font: &MonoFont) -> Result<()> {
        let style = embedded_graphics::mono_font::MonoTextStyle::new(font, BinaryColor::On);
        let bounds = Text::with_baseline(text, Point::zero(), style, Baseline::Top).bounding_box();
        let x = (self.buffer.width() as i32 - bounds.size.width as i32) / 2;
        Text::with_baseline(text, Point::new(x.max(0), y), style, Baseline::Top)
            .draw(&mut self.buffer)?;
        Ok(())
    }

    pub fn draw_status_line(&mut self, y: i32, status: &str) -> Result<()> {
        self.draw_text(0, y, status)
    }

    pub fn draw_battery(&mut self, x: i32, y: i32, percent: u8) -> Result<()> {
        self.draw_text(x, y, &format!("[{:>3}%]", percent))?;
        let bar_width = 20;
        let filled = (bar_width * percent as i32 / 100).clamp(0, bar_width);
        let mut bar = String::with_capacity(bar_width as usize + 2);
        bar.push('[');
        for i in 0..bar_width {
            bar.push(if i < filled { '#' } else { '-' });
        }
        bar.push(']');
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

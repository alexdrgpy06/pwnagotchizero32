//! Display API for drawing faces, text, and status

use anyhow::Result;
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoFont,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
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
    // Real pwnagotchi renders with PIL + DejaVuSansMono (a TTF, full
    // Unicode) — embedded-graphics's bundled fonts are ASCII-only bitmap
    // fonts, so kaomoji like "(•‿‿•)" (● and ‿ aren't ASCII) rendered as
    // blank boxes. `None` if the system TTF isn't installed (e.g. a
    // non-Linux dev machine); draw_ttf_text() falls back to the bitmap
    // font in that case rather than erroring.
    font_regular: Option<ab_glyph::FontVec>,
    font_bold: Option<ab_glyph::FontVec>,
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
            // fonts-dejavu-core's standard Debian install path — the same
            // package pwnagotchi's own Python UI depends on, so it's already
            // on the base image.
            font_regular: Self::load_font(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ]),
            font_bold: Self::load_font(&[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf",
            ]),
        })
    }

    fn load_font(paths: &[&str]) -> Option<ab_glyph::FontVec> {
        for path in paths {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(font) = ab_glyph::FontVec::try_from_vec(bytes) {
                    return Some(font);
                }
            }
        }
        None
    }

    /// Constructor that bypasses real hardware (for non-Linux / CI).
    #[cfg(not(target_os = "linux"))]
    pub fn mock() -> Self {
        Self {
            driver: EpdDriver::mock(),
            buffer: FrameBuffer::new(),
            current_face: "happy".to_string(),
            epoch_count: 0,
            font_regular: None,
            font_bold: None,
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

    /// Render text with the real DejaVuSansMono TTF at (x, y) top-left,
    /// falling back to the bitmap ASCII font if the system TTF isn't
    /// available. `size` is the pixel font size, matching pwnagotchi's own
    /// fonts.py sizing (10 for labels/name/mode, 35 for the face).
    fn draw_ttf_text(&mut self, text: &str, x: i32, y: i32, size: f32, bold: bool) {
        use ab_glyph::{point, Font, ScaleFont};

        let font = if bold {
            self.font_bold.as_ref()
        } else {
            self.font_regular.as_ref()
        };
        let Some(font) = font else {
            let _ = self.draw_text(x, y, text);
            return;
        };
        let scaled = font.as_scaled(size);
        let mut cursor_x = x as f32;
        let baseline_y = y as f32 + scaled.ascent();
        let buf = &mut self.buffer;

        for ch in text.chars() {
            let glyph_id = scaled.glyph_id(ch);
            let advance = scaled.h_advance(glyph_id);
            let glyph = glyph_id.with_scale_and_position(size, point(cursor_x, baseline_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage > 0.5 {
                        let px = bounds.min.x as i32 + gx as i32;
                        let py = bounds.min.y as i32 + gy as i32;
                        if px >= 0 && py >= 0 {
                            buf.set_pixel(px as u32, py as u32, BinaryColor::On);
                        }
                    }
                });
            }
            cursor_x += advance;
        }
    }

    /// Draw text right-aligned so it ends at `x_right`.
    fn draw_text_right(&mut self, x_right: i32, y: i32, text: &str) -> Result<()> {
        let style = embedded_graphics::mono_font::MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let bounds = Text::with_baseline(text, Point::zero(), style, Baseline::Top).bounding_box();
        let x = (x_right - bounds.size.width as i32).max(0);
        Text::with_baseline(text, Point::new(x, y), style, Baseline::Top).draw(&mut self.buffer)?;
        Ok(())
    }

    /// Full-width horizontal divider, one pixel tall.
    fn draw_hline(&mut self, y: i32) -> Result<()> {
        let w = self.buffer.width() as i32;
        Line::new(Point::new(0, y), Point::new(w - 1, y))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut self.buffer)?;
        Ok(())
    }

    /// The exact pwnagotchi layout for this hardware — coordinates taken
    /// directly from jayofelony/pwnagotchi's own
    /// pwnagotchi/ui/hw/waveshare2in13_V4.py `layout()` (the base image's
    /// own driver for this exact panel), not approximated from photos:
    /// channel (0,0), aps (28,0), uptime (185,0), line1 y=14, name (5,20),
    /// status/phrase (125,20), face (0,40) at font size 35, line2 y=108,
    /// shakes/PWND (0,109), mode (225,109). `face` is the real Unicode
    /// kaomoji string (e.g. "(•‿‿•)") from Personality::get_face() — TTF
    /// rendering (draw_ttf_text, DejaVuSansMono, the same font pwnagotchi's
    /// own PIL-based UI uses) is what makes that renderable at all; the
    /// embedded-graphics bitmap font this used to go through is ASCII-only
    /// and drew those as blank boxes.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_pwnagotchi_frame(
        &mut self,
        channel: u8,
        aps_found: usize,
        bt_connected: bool,
        uptime: &str,
        name: &str,
        phrase: &str,
        face: &str,
        handshakes: u64,
        level: u32,
        mode: &str,
        cpu_temp: f32,
        ram_used: u64,
        ram_total: u64,
    ) -> Result<()> {
        self.draw_ttf_text(&format!("CH:{channel}"), 0, 0, 10.0, true);
        self.draw_ttf_text(&format!("APS:{aps_found}"), 28, 0, 10.0, true);
        self.draw_ttf_text(&format!("UP:{uptime}"), 185, 0, 10.0, true);
        self.draw_hline(14)?;

        self.draw_ttf_text(&format!("{name}>"), 5, 20, 10.0, true);
        self.draw_ttf_text(phrase, 125, 20, 10.0, false);

        self.draw_ttf_text(face, 0, 40, 35.0, true);

        // Not part of the original layout (that's a plugin's job on real
        // pwnagotchi, e.g. memtemp.lua) but there's real unused space
        // between the face and the footer divider, so it's free real
        // estate rather than a layout compromise.
        let ram_pct = if ram_total > 0 {
            ram_used * 100 / ram_total
        } else {
            0
        };
        self.draw_ttf_text(
            &format!(
                "BT:{} mem:{ram_pct}% temp:{:.0}C",
                if bt_connected { "on" } else { "off" },
                cpu_temp
            ),
            0,
            92,
            9.0,
            false,
        );

        self.draw_hline(108)?;
        self.draw_ttf_text(&format!("PWND:{handshakes} (Lv{level})"), 0, 109, 10.0, true);
        self.draw_ttf_text(mode, 225, 109, 10.0, true);
        Ok(())
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

    /// The current framebuffer's raw packed 1bpp bytes, for mirroring on the
    /// web dashboard — exactly what was last sent (or is about to be sent) to
    /// the physical panel.
    pub fn framebuffer_bytes(&self) -> &[u8] {
        self.buffer.as_bytes()
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

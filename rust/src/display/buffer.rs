//! Frame buffer for 1-bit e-ink display

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::BinaryColor,
    primitives::Rectangle,
    Pixel,
};

/// 1-bit packed framebuffer for 250x122 display
pub struct FrameBuffer {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        let width = 250;
        let height = 122;
        let bytes_per_row = (width + 7) / 8;
        let data = vec![0xFF; (bytes_per_row * height) as usize];

        Self {
            width,
            height,
            data,
        }
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Convert the logical 250×122 landscape framebuffer into the SSD1680's
    /// native 122(source)×250(gate) RAM layout (16 bytes per gate row).
    ///
    /// The panel can only address 250 lines along its gate axis and ≤176 along
    /// its source axis, so the long (250 px) edge of the image must map to the
    /// gate direction. We rotate 90° and flip the long axis to match the
    /// Waveshare landscape orientation (origin top-left when the panel is held
    /// with the ribbon cable at the bottom). Bit value 0 = black, matching the
    /// controller's convention, so an "on" (black) pixel clears its bit.
    pub(super) fn to_panel_ram(&self) -> Vec<u8> {
        use crate::display::{PANEL_GATE, PANEL_RAM_SIZE, PANEL_STRIDE};

        let mut ram = vec![0xFFu8; PANEL_RAM_SIZE];
        for x in 0..self.width {
            for y in 0..self.height {
                if self.get_pixel(x, y) == BinaryColor::On {
                    let src = y as usize; // source column 0..121
                    let gate = (self.width - 1 - x) as usize; // gate row, long axis flipped
                    debug_assert!((gate as u32) < PANEL_GATE);
                    let idx = gate * PANEL_STRIDE + src / 8;
                    ram[idx] &= !(0x80u8 >> (src % 8));
                }
            }
        }
        ram
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn clear(&mut self) {
        self.data.fill(0xFF);
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: BinaryColor) {
        if x >= self.width || y >= self.height {
            return;
        }

        let byte_index = (y * ((self.width + 7) / 8) + x / 8) as usize;
        let bit_index = 7 - (x % 8);

        match color {
            BinaryColor::On => self.data[byte_index] &= !(1 << bit_index),
            BinaryColor::Off => self.data[byte_index] |= 1 << bit_index,
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> BinaryColor {
        if x >= self.width || y >= self.height {
            return BinaryColor::Off;
        }

        let byte_index = (y * ((self.width + 7) / 8) + x / 8) as usize;
        let bit_index = 7 - (x % 8);

        if self.data[byte_index] & (1 << bit_index) == 0 {
            BinaryColor::On
        } else {
            BinaryColor::Off
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for FrameBuffer {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0 && coord.y >= 0 {
                self.set_pixel(coord.x as u32, coord.y as u32, color);
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        // area.bottom_right() is the *inclusive* corner (top_left + size -
        // (1,1)), not an exclusive bound — using it directly as one made
        // every fill under-draw by a row and a column, and made any area
        // exactly 1px tall or wide (e.g. the horizontal scanlines circles
        // fill with) draw nothing at all, since `y..y` is empty. Compute the
        // exclusive bound from top_left + size instead.
        let mut colors = colors.into_iter();
        let x_end = area.top_left.x + area.size.width as i32;
        let y_end = area.top_left.y + area.size.height as i32;
        for y in area.top_left.y..y_end {
            for x in area.top_left.x..x_end {
                if let Some(color) = colors.next() {
                    if x >= 0 && y >= 0 {
                        self.set_pixel(x as u32, y as u32, color);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{PANEL_GATE, PANEL_RAM_SIZE, PANEL_STRIDE};

    #[test]
    fn panel_ram_is_correct_size() {
        let fb = FrameBuffer::new();
        assert_eq!(fb.to_panel_ram().len(), PANEL_RAM_SIZE);
        assert_eq!(PANEL_RAM_SIZE, PANEL_STRIDE * PANEL_GATE as usize);
    }

    #[test]
    fn blank_buffer_is_all_white() {
        let fb = FrameBuffer::new();
        assert!(fb.to_panel_ram().iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn logical_origin_maps_to_flipped_gate() {
        // Logical (0,0) is the top-left of the landscape image. The long axis
        // is flipped into the gate direction, so it lands on gate row 249,
        // source column 0 → the most-significant bit of that row's first byte.
        let mut fb = FrameBuffer::new();
        fb.set_pixel(0, 0, BinaryColor::On);
        let ram = fb.to_panel_ram();

        let idx = 249 * PANEL_STRIDE; // gate 249, source byte 0
        assert_eq!(ram[idx], 0xFF & !0x80, "black pixel must clear MSB");
        // Every other byte stays white.
        let set_bytes = ram.iter().filter(|&&b| b != 0xFF).count();
        assert_eq!(set_bytes, 1);
    }

    #[test]
    fn ascii_text_renders_visible_pixels() {
        // Guards against regressing to non-ASCII glyphs, which the panel's
        // bitmap fonts render as blank boxes. ASCII faces/status must produce
        // real black pixels the panel can show.
        use embedded_graphics::{
            mono_font::{
                ascii::{FONT_10X20, FONT_6X10},
                MonoTextStyle,
            },
            prelude::*,
            text::{Baseline, Text},
        };
        let mut fb = FrameBuffer::new();
        let big = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);
        let small = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        Text::with_baseline("(^__^)", Point::new(90, 20), big, Baseline::Top)
            .draw(&mut fb)
            .unwrap();
        Text::with_baseline(
            "Epoch:3 HS:1 BT:on Bat:87%",
            Point::new(0, 100),
            small,
            Baseline::Top,
        )
        .draw(&mut fb)
        .unwrap();
        let black = (0..fb.width)
            .flat_map(|x| (0..fb.height).map(move |y| (x, y)))
            .filter(|&(x, y)| fb.get_pixel(x, y) == BinaryColor::On)
            .count();
        assert!(
            black > 100,
            "expected legible text, got {black} black pixels"
        );
    }

    #[test]
    fn opposite_corner_maps_to_gate_zero() {
        // Logical (249,121): far corner → gate 0, source column 121 (bit 1 of
        // byte 15 within the row).
        let mut fb = FrameBuffer::new();
        fb.set_pixel(249, 121, BinaryColor::On);
        let ram = fb.to_panel_ram();

        let idx = 121 / 8; // byte 15 of gate row 0
        assert_eq!(ram[idx], 0xFF & !(0x80 >> (121 % 8)));
    }
}

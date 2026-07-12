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
        
        Self { width, height, data }
    }
    
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.data
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
        let mut colors = colors.into_iter();
        for y in area.top_left.y..area.bottom_right().unwrap().y {
            for x in area.top_left.x..area.bottom_right().unwrap().x {
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
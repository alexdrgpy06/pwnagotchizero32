//! Display module for Waveshare V4 e-ink (SSD1680)

pub mod driver;
pub mod buffer;
pub mod api;

pub use api::Display;
pub use buffer::FrameBuffer;
pub use driver::EpdDriver;

/// Display dimensions for Waveshare 2.13" V4
pub const DISPLAY_WIDTH: u32 = 250;
pub const DISPLAY_HEIGHT: u32 = 122;

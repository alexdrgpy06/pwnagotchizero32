//! Display module for Waveshare V4 e-ink (SSD1680)

pub mod api;
pub mod buffer;
pub mod driver;

pub use api::Display;
pub use buffer::FrameBuffer;
pub use driver::EpdDriver;

/// Logical (landscape) framebuffer dimensions the UI draws into.
pub const DISPLAY_WIDTH: u32 = 250;
pub const DISPLAY_HEIGHT: u32 = 122;

/// Physical SSD1680 RAM layout for the Waveshare 2.13" V4 panel.
///
/// The panel is natively portrait: 122 source lines (X) by 250 gate lines (Y).
/// Source lines are packed 8-per-byte and padded up to a 16-byte (128-bit) row,
/// so a full frame in panel RAM is `PANEL_STRIDE * PANEL_GATE` bytes.
pub const PANEL_SOURCE: u32 = 122;
pub const PANEL_GATE: u32 = 250;
pub const PANEL_STRIDE: usize = 16;
pub const PANEL_RAM_SIZE: usize = PANEL_STRIDE * PANEL_GATE as usize;

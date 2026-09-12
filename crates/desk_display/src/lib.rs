#![no_std]
use purec::fb::{self, DisplayInfo};
static mut CACHED: Option<DisplayInfo> = None;
fn cached() -> Option<DisplayInfo> {
    unsafe {
        if (*core::ptr::addr_of!(CACHED)).is_none() {
            *core::ptr::addr_of_mut!(CACHED) = fb::display_info();
        }
        *core::ptr::addr_of!(CACHED)
    }
}
pub fn is_available() -> bool {
    cached().is_some_and(|info| info.available)
}
pub fn width() -> u32 {
    cached().map(|info| info.width).unwrap_or(0)
}
pub fn height() -> u32 {
    cached().map(|info| info.height).unwrap_or(0)
}
pub fn draw_text(x: u32, y: u32, text: &str, fg: u32, bg: u32) {
    fb::draw_text(x, y, text, fg, bg);
}
pub fn draw_text_sized(x: u32, y: u32, text: &str, fg: u32, bg: u32, size: u32) {
    fb::draw_text_sized(x, y, text, fg, bg, size);
}
pub fn draw_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    fb::draw_rect(x, y, w, h, color);
}
pub fn clear(color: u32) {
    purec::display_clear(color);
}
pub fn begin_update() {
    fb::display_begin_update();
}
pub fn end_update() {
    fb::display_end_update();
}

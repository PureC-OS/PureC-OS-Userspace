#![no_std]
use core::ffi::c_char;

type RectFn = extern "C" fn(u32, u32, u32, u32, u32);
type TextFn = extern "C" fn(u32, u32, *const c_char, u32, u32);

unsafe extern "C" {
    fn panel_set_draw_backend(rect: RectFn, text: TextFn);
    fn audio_panel_init();
    fn audio_panel_draw(screen_width: u32);
    fn audio_panel_draw_badge(screen_width: u32);
    fn audio_panel_draw_popup(screen_width: u32);
    fn audio_panel_popup_rect(screen_width: u32, x: *mut u32, y: *mut u32, w: *mut u32, h: *mut u32);
    fn audio_panel_set_popup(visible: bool);
    fn audio_panel_handle_mouse(
        x: i32,
        y: i32,
        buttons: u8,
        pressed: bool,
        released: bool,
        screen_width: u32,
        redraw_required: *mut bool,
    ) -> bool;
    fn audio_panel_handle_special_key(key: u8) -> bool;
    fn audio_panel_is_popup_visible() -> bool;
}

extern "C" fn backend_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    desk_display::draw_rect(x, y, w, h, color);
}

extern "C" fn backend_text(x: u32, y: u32, text: *const c_char, fg: u32, bg: u32) {
    if text.is_null() {
        return;
    }
    let mut len = 0usize;
    while len < 256 && unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    let bytes = unsafe { core::slice::from_raw_parts(text as *const u8, len) };
    if let Ok(s) = core::str::from_utf8(bytes) {
        desk_display::draw_text(x, y, s, fg, bg);
    }
}

pub fn init() {
    unsafe {
        panel_set_draw_backend(backend_rect, backend_text);
        audio_panel_init();
    }
}

pub fn draw(screen_width: u32) {
    unsafe { audio_panel_draw(screen_width) }
}

pub fn handle_mouse(
    x: i32,
    y: i32,
    buttons: u8,
    pressed: bool,
    released: bool,
    screen_width: u32,
    redraw_required: &mut bool,
) -> bool {
    unsafe {
        audio_panel_handle_mouse(
            x,
            y,
            buttons,
            pressed,
            released,
            screen_width,
            redraw_required,
        )
    }
}

pub fn handle_special_key(key: u8) -> bool {
    unsafe { audio_panel_handle_special_key(key) }
}

pub fn is_popup_visible() -> bool {
    unsafe { audio_panel_is_popup_visible() }
}

pub fn draw_badge(screen_width: u32) {
    unsafe { audio_panel_draw_badge(screen_width) }
}

pub fn draw_popup(screen_width: u32) {
    unsafe { audio_panel_draw_popup(screen_width) }
}

pub fn popup_rect(screen_width: u32) -> (u32, u32, u32, u32) {
    let (mut x, mut y, mut w, mut h) = (0u32, 0u32, 0u32, 0u32);
    unsafe { audio_panel_popup_rect(screen_width, &mut x, &mut y, &mut w, &mut h) };
    (x, y, w, h)
}

pub fn set_popup(visible: bool) {
    unsafe { audio_panel_set_popup(visible) }
}

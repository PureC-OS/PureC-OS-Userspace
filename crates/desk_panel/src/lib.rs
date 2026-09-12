#![no_std]

unsafe extern "C" {
    fn audio_panel_init();
    fn audio_panel_draw(screen_width: u32);
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

pub fn init() {
    unsafe { audio_panel_init() }
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

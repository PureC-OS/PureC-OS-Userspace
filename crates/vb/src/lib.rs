#![no_std]
use core::ffi::{c_char, c_int, c_void};

pub const ROLE_NORMAL: c_int = 0;
pub const ROLE_BACKGROUND: c_int = 1;
pub const ROLE_PANEL: c_int = 2;
pub const ROLE_POPUP: c_int = 3;

pub const EVENT_KEY: c_int = 1;
pub const EVENT_POINTER: c_int = 2;
pub const EVENT_ENTER: c_int = 3;
pub const EVENT_LEAVE: c_int = 4;
pub const EVENT_FOCUS: c_int = 5;
pub const EVENT_CLOSE: c_int = 6;
pub const EVENT_DISCONNECT: c_int = 7;
pub const EVENT_WORKAREA: c_int = 8;
pub const EVENT_TASK: c_int = 9;
pub const EVENT_TASK_GONE: c_int = 10;
pub const EVENT_POPUP_DONE: c_int = 11;
pub const EVENT_CONFIGURE: c_int = 12;

pub const BUTTON_LEFT: u32 = 1;
pub const TASK_FOCUSED: u32 = 1;
pub const TASK_MINIMIZED: u32 = 2;
pub const MOD_SHIFT: u8 = 0x11;
pub const MOD_CTRL: u8 = 0x22;
pub const MOD_ALT: u8 = 0x44;

pub const KEY_F1: u8 = 1;
pub const KEY_F2: u8 = 2;
pub const KEY_F3: u8 = 3;
pub const KEY_HOME: u8 = 4;
pub const KEY_END: u8 = 5;
pub const KEY_PAGE_UP: u8 = 6;
pub const KEY_PAGE_DOWN: u8 = 7;
pub const KEY_LEFT: u8 = 8;
pub const KEY_RIGHT: u8 = 9;
pub const KEY_UP: u8 = 10;
pub const KEY_DOWN: u8 = 11;
pub const KEY_DELETE: u8 = 12;

#[repr(C)]
pub struct Window {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    pub kind: c_int,
    pub window: *mut Window,
    pub x: c_int,
    pub y: c_int,
    pub buttons: u32,
    pub changed: u32,
    pub wheel: c_int,
    pub key: c_int,
    pub pressed: bool,
    pub repeat: bool,
    pub mods: u8,
    pub focused: bool,
    pub task_id: u32,
    pub task_flags: u32,
    pub title: [u8; 64],
}

impl Event {
    pub const fn empty() -> Self {
        Event {
            kind: 0,
            window: core::ptr::null_mut(),
            x: 0,
            y: 0,
            buttons: 0,
            changed: 0,
            wheel: 0,
            key: 0,
            pressed: false,
            repeat: false,
            mods: 0,
            focused: false,
            task_id: 0,
            task_flags: 0,
            title: [0; 64],
        }
    }

    pub fn title_str(&self) -> &str {
        let n = self.title.iter().position(|&b| b == 0).unwrap_or(self.title.len());
        core::str::from_utf8(&self.title[..n]).unwrap_or("")
    }

    pub fn left_pressed(&self) -> bool {
        self.changed & self.buttons & BUTTON_LEFT != 0
    }

    pub fn left_released(&self) -> bool {
        self.changed & BUTTON_LEFT != 0 && self.buttons & BUTTON_LEFT == 0
    }
}

unsafe extern "C" {
    fn vb_connect() -> c_int;
    fn vb_disconnect();
    fn vb_screen_size(width: *mut c_int, height: *mut c_int);
    fn vb_work_area(x: *mut c_int, y: *mut c_int, width: *mut c_int, height: *mut c_int);
    fn vb_window_create_role(x: c_int, y: c_int, width: c_int, height: c_int, title: *const c_char,
                             role: c_int) -> *mut Window;
    fn vb_window_destroy(window: *mut Window);
    fn vb_window_pixels(window: *mut Window) -> *mut u32;
    fn vb_window_damage(window: *mut Window, x: c_int, y: c_int, width: c_int, height: c_int);
    fn vb_window_title(window: *mut Window, title: *const c_char);
    fn vb_window_move(window: *mut Window);
    fn vb_window_raise(window: *mut Window);
    fn vb_window_id(window: *const Window) -> u32;
    fn vb_tasks_subscribe();
    fn vb_task_activate(id: u32);
    fn vb_task_minimize(id: u32);
    fn vb_next_event(event: *mut Event, timeout_ms: c_int) -> c_int;
}

fn cstr(text: &str, buf: &mut [u8; 64]) -> *const c_char {
    let n = text.len().min(63);
    buf[..n].copy_from_slice(&text.as_bytes()[..n]);
    buf[n] = 0;
    buf.as_ptr() as *const c_char
}

pub fn connect() -> bool {
    unsafe { vb_connect() == 0 }
}

pub fn disconnect() {
    unsafe { vb_disconnect() }
}

pub fn screen_size() -> (u32, u32) {
    let (mut w, mut h) = (0, 0);
    unsafe { vb_screen_size(&mut w, &mut h) };
    (w.max(0) as u32, h.max(0) as u32)
}

pub fn work_area() -> (i32, i32, i32, i32) {
    let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
    unsafe { vb_work_area(&mut x, &mut y, &mut w, &mut h) };
    (x, y, w, h)
}

pub fn create(x: i32, y: i32, width: u32, height: u32, title: &str, role: c_int) -> *mut Window {
    let mut buf = [0u8; 64];
    unsafe { vb_window_create_role(x, y, width as c_int, height as c_int, cstr(title, &mut buf), role) }
}

pub fn destroy(window: *mut Window) {
    if !window.is_null() {
        unsafe { vb_window_destroy(window) }
    }
}

pub fn pixels(window: *mut Window) -> *mut u32 {
    unsafe { vb_window_pixels(window) }
}

pub fn damage(window: *mut Window, x: u32, y: u32, width: u32, height: u32) {
    unsafe { vb_window_damage(window, x as c_int, y as c_int, width as c_int, height as c_int) }
}

pub fn set_title(window: *mut Window, title: &str) {
    let mut buf = [0u8; 64];
    unsafe { vb_window_title(window, cstr(title, &mut buf)) }
}

pub fn start_move(window: *mut Window) {
    unsafe { vb_window_move(window) }
}

pub fn raise(window: *mut Window) {
    unsafe { vb_window_raise(window) }
}

pub fn id(window: *mut Window) -> u32 {
    unsafe { vb_window_id(window) }
}

pub fn tasks_subscribe() {
    unsafe { vb_tasks_subscribe() }
}

pub fn task_activate(id: u32) {
    unsafe { vb_task_activate(id) }
}

pub fn task_minimize(id: u32) {
    unsafe { vb_task_minimize(id) }
}

pub fn next_event(event: &mut Event, timeout_ms: i32) -> i32 {
    unsafe { vb_next_event(event, timeout_ms) }
}

/// HID usage + modifiers -> (ASCII, special key code). One of them is 0.
pub fn translate_key(key: i32, mods: u8) -> (u8, u8) {
    const LOWER: &[u8; 53] = b"abcdefghijklmnopqrstuvwxyz1234567890\n\x1b\x08\t -=[]\\#;'`,./";
    const UPPER: &[u8; 53] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ!@#$%^&*()\n\x1b\x08\t _+{}|~:\"~<>?";
    match key {
        0x04..=0x38 => {
            let table = if mods & MOD_SHIFT != 0 { UPPER } else { LOWER };
            (table[(key - 0x04) as usize], 0)
        }
        0x3A => (0, KEY_F1),
        0x3B => (0, KEY_F2),
        0x3C => (0, KEY_F3),
        0x4A => (0, KEY_HOME),
        0x4B => (0, KEY_PAGE_UP),
        0x4C => (0, KEY_DELETE),
        0x4D => (0, KEY_END),
        0x4E => (0, KEY_PAGE_DOWN),
        0x4F => (0, KEY_RIGHT),
        0x50 => (0, KEY_LEFT),
        0x51 => (0, KEY_DOWN),
        0x52 => (0, KEY_UP),
        0x54 => (b'/', 0),
        0x55 => (b'*', 0),
        0x56 => (b'-', 0),
        0x57 => (b'+', 0),
        0x58 => (b'\n', 0),
        0x59..=0x61 => (b'1' + (key - 0x59) as u8, 0),
        0x62 => (b'0', 0),
        0x63 => (b'.', 0),
        _ => (0, 0),
    }
}

#[allow(dead_code)]
fn _unused(_: *mut c_void) {}

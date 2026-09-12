#![no_std]
use desk_datetime as dt;
use desk_display as disp;
const INPUT_CAP: usize = 20;
struct State {
    editing: bool,
    input: [u8; INPUT_CAP],
    len: usize,
    message: &'static str,
}
static mut STATE: State = State {
    editing: false,
    input: [0; INPUT_CAP],
    len: 0,
    message: "",
};
fn state() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}
fn draw_text(wx: u32, wy: u32, ox: u32, oy: u32, text: &str, color: u32, size: u32) {
    disp::draw_text_sized(wx + ox, wy + oy, text, color, 0x1E1E2E, size);
}
pub fn open() {
    let s = state();
    s.editing = false;
    s.message = "";
    s.len = 0;
}
pub fn draw(window_x: u32, window_y: u32) {
    let mut text = [0u8; 20];
    dt::format(&mut text);
    let shown = core::str::from_utf8(&text[..19]).unwrap_or("?");
    draw_text(window_x, window_y, 28, 70, shown, 0xCDD6F4, 18);
    draw_text(
        window_x,
        window_y,
        22,
        130,
        "Press E to set YYYY-MM-DD HH:MM:SS",
        0x9399B2,
        8,
    );
    let s = state();
    if s.editing {
        disp::draw_rect(window_x + 15, window_y + 204, 330, 28, 0x313244);
        let input = core::str::from_utf8(&s.input[..s.len]).unwrap_or("?");
        draw_text(window_x, window_y, 22, 213, input, 0xCDD6F4, 8);
    }
    draw_text(window_x, window_y, 18, 244, s.message, 0xA6E3A1, 8);
}
pub fn handle_key(key: u8) -> bool {
    let s = state();
    if !s.editing {
        if key != b'e' && key != b'E' {
            return false;
        }
        s.editing = true;
        s.message = "Enter full date and time";
        s.len = 0;
        return true;
    }
    if key == 27 {
        s.editing = false;
        return true;
    }
    if (key == 8 || key == 127) && s.len > 0 {
        s.len -= 1;
        return true;
    }
    if key == b'\n' || key == b'\r' {
        if let Some(parsed) = dt::parse(&s.input[..s.len], false) {
            dt::set(&parsed);
            s.message = "Saved to /purec/datetime.cfg";
            s.editing = false;
        } else {
            s.message = "Invalid date/time";
        }
        return true;
    }
    if (b' '..=b'~').contains(&key) && s.len + 1 < INPUT_CAP {
        s.input[s.len] = key;
        s.len += 1;
    }
    true
}

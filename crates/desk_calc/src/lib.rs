#![no_std]
use desk_display as disp;
const INPUT_CAP: usize = 40;
struct State {
    input: [u8; INPUT_CAP],
    len: usize,
    left: i64,
    op: u8,
    has_left: bool,
    message: &'static str,
}
static mut STATE: State = State {
    input: [0; INPUT_CAP],
    len: 0,
    left: 0,
    op: 0,
    has_left: false,
    message: "",
};
fn state() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}
fn clear_input(s: &mut State) {
    s.len = 0;
}
fn current_text(s: &State) -> &str {
    if s.len == 0 {
        "0"
    } else {
        core::str::from_utf8(&s.input[..s.len]).unwrap_or("?")
    }
}
fn append_input(s: &mut State, ch: u8) -> bool {
    if s.len + 1 >= INPUT_CAP {
        return false;
    }
    s.input[s.len] = ch;
    s.len += 1;
    true
}
fn parse_input(s: &State) -> i64 {
    let mut result: i64 = 0;
    for &ch in &s.input[..s.len] {
        if ch.is_ascii_digit() {
            result = result * 10 + (ch - b'0') as i64;
        }
    }
    result
}
fn format_result(s: &mut State, result: i64) {
    let mut reversed = [0u8; 24];
    let mut length = 0usize;
    let negative = result < 0;
    let mut magnitude: u64 = if negative {
        (result.wrapping_add(1).wrapping_neg() as u64).wrapping_add(1)
    } else {
        result as u64
    };
    loop {
        reversed[length] = b'0' + (magnitude % 10) as u8;
        length += 1;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    clear_input(s);
    if negative {
        append_input(s, b'-');
    }
    while length > 0 {
        length -= 1;
        append_input(s, reversed[length]);
    }
}
fn select_operator(s: &mut State, op: u8) {
    s.left = parse_input(s);
    s.op = op;
    s.has_left = true;
    clear_input(s);
}
fn evaluate(s: &mut State) {
    let right = parse_input(s);
    let mut result: i64 = 0;
    let mut valid = true;
    match s.op {
        b'+' => result = s.left.wrapping_add(right),
        b'-' => result = s.left.wrapping_sub(right),
        b'*' => result = s.left.wrapping_mul(right),
        b'/' => {
            if right == 0 {
                valid = false;
            } else {
                result = s.left / right;
            }
        }
        _ => valid = false,
    }
    format_result(s, result);
    s.message = if valid { "Result" } else { "Division by zero" };
    s.has_left = false;
}
pub fn open() {
    let s = state();
    clear_input(s);
    s.has_left = false;
    s.message = "";
}
pub fn draw(window_x: u32, window_y: u32) {
    let s = state();
    disp::draw_text_sized(
        window_x + 20,
        window_y + 52,
        current_text(s),
        0xCDD6F4,
        0x1E1E2E,
        16,
    );
    disp::draw_text_sized(
        window_x + 20,
        window_y + 105,
        "Keyboard: 0-9  + - * /  Enter  C",
        0x9399B2,
        0x1E1E2E,
        8,
    );
    disp::draw_text_sized(window_x + 18, window_y + 244, s.message, 0xA6E3A1, 0x1E1E2E, 8);
}
pub fn handle_key(key: u8) -> bool {
    let s = state();
    let is_operator = matches!(key, b'+' | b'-' | b'*' | b'/');
    if key == b'c' || key == b'C' {
        open();
        return true;
    }
    if key.is_ascii_digit() {
        append_input(s, key);
        return true;
    }
    if is_operator && s.len > 0 {
        select_operator(s, key);
        return true;
    }
    if matches!(key, b'\n' | b'\r' | b'=') && s.has_left && s.len > 0 {
        evaluate(s);
        return true;
    }
    true
}

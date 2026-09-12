#![no_std]
use desk_datetime as dt;
use desk_display as disp;
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const INPUT_CAP: usize = 11;
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
fn weekday(mut year: u16, mut month: u8, day: u8) -> u8 {
    if month < 3 {
        month += 12;
        year = year.wrapping_sub(1);
    }
    let yic = (year % 100) as u32;
    let century = (year / 100) as u32;
    ((day as u32 + (13 * (month as u32 + 1)) / 5 + yic + yic / 4 + century / 4
        + 5 * century
        + 6)
        % 7) as u8
}
fn draw_grid(window_x: u32, window_y: u32) {
    let datetime = dt::get();
    let first_weekday = weekday(datetime.year, datetime.month, 1);
    let first_column = if first_weekday == 0 { 6 } else { first_weekday - 1 };
    let maximum_day = dt::days_in_month(datetime.year, datetime.month);
    let mut day: u8 = 1;
    draw_text(
        window_x,
        window_y,
        42,
        108,
        "Mo Tu We Th Fr Sa Su",
        0x9399B2,
        8,
    );
    let mut row: u8 = 0;
    while row < 6 && day <= maximum_day {
        let mut line = [0u8; 24];
        let mut pos = 0usize;
        let mut column: u8 = 0;
        while column < 7 {
            if (row == 0 && column < first_column) || day > maximum_day {
                line[pos] = b' ';
                pos += 1;
                line[pos] = b' ';
                pos += 1;
            } else {
                line[pos] = if day >= 10 { b'0' + day / 10 } else { b' ' };
                pos += 1;
                line[pos] = b'0' + day % 10;
                pos += 1;
                day += 1;
            }
            if column < 6 {
                line[pos] = b' ';
                pos += 1;
            }
            column += 1;
        }
        let shown = core::str::from_utf8(&line[..pos]).unwrap_or("?");
        draw_text(window_x, window_y, 42, 124 + row as u32 * 15, shown, 0xCDD6F4, 8);
        row += 1;
    }
}
pub fn open() {
    let s = state();
    s.editing = false;
    s.message = "";
    s.len = 0;
}
pub fn draw(window_x: u32, window_y: u32) {
    let datetime = dt::get();
    let mut text = [0u8; 20];
    dt::format(&mut text);
    let date = core::str::from_utf8(&text[..10]).unwrap_or("?");
    draw_text(window_x, window_y, 92, 42, date, 0xCDD6F4, 14);
    draw_text(
        window_x,
        window_y,
        120,
        76,
        MONTH_NAMES[(datetime.month - 1) as usize],
        0xF9E2AF,
        12,
    );
    draw_grid(window_x, window_y);
    draw_text(
        window_x,
        window_y,
        22,
        222,
        "Press E to set YYYY-MM-DD",
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
        s.message = "Enter date";
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
        match dt::parse(&s.input[..s.len], true) {
            None => {
                s.message = "Invalid date";
                return true;
            }
            Some(mut parsed) => {
                let current = dt::get();
                parsed.hour = current.hour;
                parsed.minute = current.minute;
                parsed.second = current.second;
                dt::set(&parsed);
                s.message = "Saved to /purec/datetime.cfg";
                s.editing = false;
                return true;
            }
        }
    }
    if (b' '..=b'~').contains(&key) && s.len + 1 < INPUT_CAP {
        s.input[s.len] = key;
        s.len += 1;
    }
    true
}

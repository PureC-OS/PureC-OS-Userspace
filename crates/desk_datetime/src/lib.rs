#![no_std]
use purec::fb::{uptime_ms, wall_clock};
use purec::{dir_create, file_close, file_open, file_read, file_write};
const DATETIME_PATH: &str = "/purec/datetime.cfg";
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Datetime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
static mut CURRENT: Datetime = Datetime {
    year: 2026,
    month: 8,
    day: 28,
    hour: 12,
    minute: 0,
    second: 0,
};
static mut LAST_UPTIME_SECOND: u64 = 0;
static mut OFFSET_SECONDS: i64 = 0;

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn from_epoch(seconds: i64) -> Datetime {
    let days = seconds.div_euclid(86400);
    let rem = seconds.rem_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Datetime {
        year: year as u16,
        month: month as u8,
        day: day as u8,
        hour: (rem / 3600) as u8,
        minute: (rem / 60 % 60) as u8,
        second: (rem % 60) as u8,
    }
}

fn to_epoch(dt: &Datetime) -> i64 {
    days_from_civil(dt.year as i64, dt.month as i64, dt.day as i64) * 86400
        + dt.hour as i64 * 3600
        + dt.minute as i64 * 60
        + dt.second as i64
}

fn kernel_local() -> Option<i64> {
    wall_clock().map(|(unix, tz)| unix + tz as i64 * 60)
}
fn current_mut() -> &'static mut Datetime {
    unsafe { &mut *core::ptr::addr_of_mut!(CURRENT) }
}
fn current() -> &'static Datetime {
    unsafe { &*core::ptr::addr_of!(CURRENT) }
}
pub fn days_in_month(year: u16, month: u8) -> u8 {
    const DAYS: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if month < 1 || month > 12 {
        return 0;
    }
    if month == 2 && is_leap_year(year) {
        return 29;
    }
    DAYS[(month - 1) as usize]
}
fn is_leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
fn advance_one_second() {
    let dt = current_mut();
    dt.second += 1;
    if dt.second < 60 {
        return;
    }
    dt.second = 0;
    dt.minute += 1;
    if dt.minute < 60 {
        return;
    }
    dt.minute = 0;
    dt.hour += 1;
    if dt.hour < 24 {
        return;
    }
    dt.hour = 0;
    dt.day += 1;
    if dt.day <= days_in_month(dt.year, dt.month) {
        return;
    }
    dt.day = 1;
    dt.month += 1;
    if dt.month <= 12 {
        return;
    }
    dt.month = 1;
    dt.year += 1;
}
fn append_unsigned(out: &mut [u8], mut pos: usize, mut number: u32, min_digits: u8) -> usize {
    let mut reversed = [0u8; 12];
    let mut len = 0usize;
    loop {
        reversed[len] = b'0' + (number % 10) as u8;
        len += 1;
        number /= 10;
        if number == 0 {
            break;
        }
    }
    while len < min_digits as usize {
        reversed[len] = b'0';
        len += 1;
    }
    while len > 0 {
        len -= 1;
        out[pos] = reversed[len];
        pos += 1;
    }
    pos
}
pub fn format(out: &mut [u8; 20]) {
    let dt = current();
    let mut pos = 0;
    pos = append_unsigned(out, pos, dt.year as u32, 4);
    out[pos] = b'-';
    pos += 1;
    pos = append_unsigned(out, pos, dt.month as u32, 2);
    out[pos] = b'-';
    pos += 1;
    pos = append_unsigned(out, pos, dt.day as u32, 2);
    out[pos] = b' ';
    pos += 1;
    pos = append_unsigned(out, pos, dt.hour as u32, 2);
    out[pos] = b':';
    pos += 1;
    pos = append_unsigned(out, pos, dt.minute as u32, 2);
    out[pos] = b':';
    pos += 1;
    pos = append_unsigned(out, pos, dt.second as u32, 2);
    out[pos] = 0;
}
fn parse_number(text: &[u8], start: usize, count: usize) -> Option<u32> {
    let mut value: u32 = 0;
    for i in 0..count {
        let ch = *text.get(start + i)?;
        if !ch.is_ascii_digit() {
            return None;
        }
        value = value * 10 + (ch - b'0') as u32;
    }
    Some(value)
}
pub fn parse(text: &[u8], date_only: bool) -> Option<Datetime> {
    let expected = if date_only { 10 } else { 19 };
    if text.len() != expected || text[4] != b'-' || text[7] != b'-' {
        return None;
    }
    let year = parse_number(text, 0, 4)?;
    let month = parse_number(text, 5, 2)?;
    let day = parse_number(text, 8, 2)?;
    let (hour, minute, second) = if !date_only {
        if text[10] != b' ' || text[13] != b':' || text[16] != b':' {
            return None;
        }
        (
            parse_number(text, 11, 2)?,
            parse_number(text, 14, 2)?,
            parse_number(text, 17, 2)?,
        )
    } else {
        (0, 0, 0)
    };
    if year < 1980
        || year > 9999
        || !(1..=12).contains(&month)
        || day < 1
        || day > days_in_month(year as u16, month as u8) as u32
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    Some(Datetime {
        year: year as u16,
        month: month as u8,
        day: day as u8,
        hour: hour as u8,
        minute: minute as u8,
        second: second as u8,
    })
}
pub fn save() {
    let mut text = [0u8; 20];
    format(&mut text);
    dir_create("/purec");
    file_write(DATETIME_PATH, &text[..19]);
}
pub fn init() {
    if let Some(now) = kernel_local() {
        *current_mut() = from_epoch(now + unsafe { *core::ptr::addr_of!(OFFSET_SECONDS) });
        return;
    }
    let fd = file_open(DATETIME_PATH);
    if fd >= 0 {
        let mut text = [0u8; 20];
        let count = file_read(fd, &mut text[..19]);
        file_close(fd);
        if count == 19 {
            if let Some(loaded) = parse(&text[..19], false) {
                *current_mut() = loaded;
            }
        }
    }
    unsafe {
        *core::ptr::addr_of_mut!(LAST_UPTIME_SECOND) = uptime_ms() / 1000;
    }
}
pub fn update() -> bool {
    if let Some(now) = kernel_local() {
        let next = from_epoch(now + unsafe { *core::ptr::addr_of!(OFFSET_SECONDS) });
        let old = *current();
        *current_mut() = next;
        return to_epoch(&old) != to_epoch(&next);
    }
    let uptime_second = uptime_ms() / 1000;
    let mut changed = false;
    unsafe {
        let last = &mut *core::ptr::addr_of_mut!(LAST_UPTIME_SECOND);
        while *last < uptime_second {
            advance_one_second();
            *last += 1;
            changed = true;
        }
    }
    changed
}
pub fn get() -> Datetime {
    *current()
}
pub fn set(dt: &Datetime) -> bool {
    if let Some(now) = kernel_local() {
        unsafe {
            *core::ptr::addr_of_mut!(OFFSET_SECONDS) = to_epoch(dt) - now;
        }
    }
    *current_mut() = *dt;
    unsafe {
        *core::ptr::addr_of_mut!(LAST_UPTIME_SECOND) = uptime_ms() / 1000;
    }
    save();
    true
}

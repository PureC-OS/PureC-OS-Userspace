use crate::entries;
use desk_display as disp;
use desk_personal as personal;
use purec::fb::{
    battery, uptime_ms, Battery, wm_activate, wm_command, wm_tasks, TaskInfo, CMD_MINIMIZE, TASK_FOCUSED, TASK_MINIMIZED,
};

pub const HEIGHT: u32 = 30;
const START_W: u32 = 86;
const TASK_W: u32 = 128;
const TASK_GAP: u32 = 4;
const TRAY_W: u32 = 84;
const BATTERY_W: u32 = 64;
const BATTERY_POLL_MS: u64 = 5000;
const MENU_W: u32 = 248;
const MENU_ROW_H: u32 = 26;
const MENU_MAX_ROWS: u32 = 14;
const FOOTER_ROWS: u32 = 2;
const MAX_TASKS: usize = 16;

pub enum Action {
    None,
    Launch(usize),
    Reboot,
    Shutdown,
}

static mut TASKS: [TaskInfo; MAX_TASKS] = [TaskInfo::empty(); MAX_TASKS];
static mut TASK_COUNT: usize = 0;
static mut SIGNATURE: u64 = 0;
static mut BATTERY: Option<Battery> = None;
static mut BATTERY_AT: u64 = 0;
static mut BATTERY_POLLED: bool = false;

fn battery_state() -> Option<Battery> {
    unsafe { *core::ptr::addr_of!(BATTERY) }
}

fn poll_battery() {
    let now = uptime_ms();
    unsafe {
        let polled = *core::ptr::addr_of!(BATTERY_POLLED);
        let last = *core::ptr::addr_of!(BATTERY_AT);
        if polled && now.wrapping_sub(last) < BATTERY_POLL_MS {
            return;
        }
        *core::ptr::addr_of_mut!(BATTERY_POLLED) = true;
        *core::ptr::addr_of_mut!(BATTERY_AT) = now;
        *core::ptr::addr_of_mut!(BATTERY) = battery();
    }
}

fn battery_text(state: Battery, buffer: &mut [u8; 8]) -> &str {
    let mut n = 0;
    let p = state.percent.min(100);
    if p >= 100 {
        buffer[n] = b'1';
        buffer[n + 1] = b'0';
        buffer[n + 2] = b'0';
        n += 3;
    } else {
        if p >= 10 {
            buffer[n] = b'0' + (p / 10) as u8;
            n += 1;
        }
        buffer[n] = b'0' + (p % 10) as u8;
        n += 1;
    }
    buffer[n] = b'%';
    n += 1;
    if state.charging {
        buffer[n] = b'+';
        n += 1;
    }
    core::str::from_utf8(&buffer[..n]).unwrap_or("")
}

fn draw_battery(x: u32, y: u32, state: Battery) {
    let th = personal::current_colors();
    let bg = th.border;
    disp::draw_rect(x, y + 4, BATTERY_W, HEIGHT - 8, bg);
    let bx = x + 6;
    let by = y + 10;
    let bw = 18;
    let bh = 10;
    disp::draw_rect(bx, by, bw, 1, th.text);
    disp::draw_rect(bx, by + bh - 1, bw, 1, th.text);
    disp::draw_rect(bx, by, 1, bh, th.text);
    disp::draw_rect(bx + bw - 1, by, 1, bh, th.text);
    disp::draw_rect(bx + bw, by + 3, 2, bh - 6, th.text);
    let fill_color = if state.charging {
        0x40C060
    } else if state.percent <= 15 {
        0xE04040
    } else {
        th.accent
    };
    let inner = bw - 4;
    let fill = (inner * state.percent.min(100) + 50) / 100;
    if fill > 0 {
        disp::draw_rect(bx + 2, by + 2, fill, bh - 4, fill_color);
    }
    let mut buffer = [0u8; 8];
    let text = battery_text(state, &mut buffer);
    disp::draw_text(bx + bw + 6, y + 11, text, th.text, bg);
}

fn tasks() -> &'static [TaskInfo] {
    unsafe { &(&*core::ptr::addr_of!(TASKS))[..*core::ptr::addr_of!(TASK_COUNT)] }
}

fn point_in(x: i32, y: i32, left: u32, top: u32, w: u32, h: u32) -> bool {
    x >= left as i32 && y >= top as i32 && x < (left + w) as i32 && y < (top + h) as i32
}

fn signature(list: &[TaskInfo], second: u8) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325 ^ second as u64;
    for task in list {
        hash = (hash ^ task.pid as u64).wrapping_mul(0x100_0000_01B3);
        hash = (hash ^ task.flags as u64).wrapping_mul(0x100_0000_01B3);
        for &byte in task.name.iter().take_while(|&&b| b != 0) {
            hash = (hash ^ byte as u64).wrapping_mul(0x100_0000_01B3);
        }
    }
    hash
}

fn clock_text(buffer: &mut [u8; 20]) -> &str {
    desk_datetime::format(buffer);
    core::str::from_utf8(&buffer[11..19]).unwrap_or("")
}

pub fn poll() -> bool {
    let table = unsafe { &mut *core::ptr::addr_of_mut!(TASKS) };
    let count = wm_tasks(table);
    unsafe {
        *core::ptr::addr_of_mut!(TASK_COUNT) = count;
    }
    let mut buffer = [0u8; 20];
    desk_datetime::format(&mut buffer);
    let second = buffer[18];
    poll_battery();
    let mut current = signature(&table[..count], second);
    if let Some(state) = battery_state() {
        current ^= ((state.percent as u64) << 40) ^ ((state.charging as u64) << 48);
    }
    let previous = unsafe { *core::ptr::addr_of!(SIGNATURE) };
    unsafe {
        *core::ptr::addr_of_mut!(SIGNATURE) = current;
    }
    current != previous
}

fn truncate<'a>(text: &'a str, max_chars: usize) -> &'a str {
    if text.len() <= max_chars {
        text
    } else {
        &text[..max_chars]
    }
}

pub fn draw(screen_w: u32, screen_h: u32, menu_open: bool) {
    let th = personal::current_colors();
    let y = screen_h.saturating_sub(HEIGHT);
    disp::draw_rect(0, y, screen_w, HEIGHT, th.titlebar);
    disp::draw_rect(0, y, screen_w, 1, th.border);
    let start_bg = if menu_open { th.accent } else { th.border };
    let start_fg = if menu_open { th.window } else { th.text };
    disp::draw_rect(4, y + 4, START_W, HEIGHT - 8, start_bg);
    disp::draw_text(14, y + 11, "Start", start_fg, start_bg);

    let mut x = 4 + START_W + 8;
    let tray_x = screen_w.saturating_sub(TRAY_W + 4);
    let battery_now = battery_state();
    let tasks_end = if battery_now.is_some() {
        tray_x.saturating_sub(BATTERY_W + 4)
    } else {
        tray_x
    };
    for task in tasks() {
        if x + TASK_W > tasks_end {
            break;
        }
        let focused = task.flags & TASK_FOCUSED != 0;
        let minimized = task.flags & TASK_MINIMIZED != 0;
        let bg = if focused { th.accent } else { th.border };
        let fg = if focused {
            th.window
        } else if minimized {
            th.muted_text
        } else {
            th.text
        };
        disp::draw_rect(x, y + 4, TASK_W, HEIGHT - 8, bg);
        let name = truncate(task.name_str(), 14);
        disp::draw_text(x + 8, y + 11, name, fg, bg);
        x += TASK_W + TASK_GAP;
    }

    let mut buffer = [0u8; 20];
    let clock = clock_text(&mut buffer);
    if let Some(state) = battery_now {
        draw_battery(tasks_end, y, state);
    }
    disp::draw_rect(tray_x, y + 4, TRAY_W, HEIGHT - 8, th.border);
    disp::draw_text(tray_x + 10, y + 11, clock, th.text, th.border);
}

fn visible_entries() -> u32 {
    let mut visible = 0;
    for index in 0..entries::count() {
        if let Some(entry) = entries::get(index) {
            if !entry.hidden {
                visible += 1;
            }
        }
    }
    visible
}

pub fn menu_rect(screen_h: u32) -> (u32, u32, u32, u32) {
    let rows = (visible_entries() + FOOTER_ROWS).clamp(FOOTER_ROWS, MENU_MAX_ROWS);
    let height = rows * MENU_ROW_H + 10;
    let y = screen_h.saturating_sub(HEIGHT + height);
    (0, y, MENU_W, height)
}

pub fn draw_menu(screen_h: u32) {
    let th = personal::current_colors();
    let (mx, my, mw, mh) = menu_rect(screen_h);
    disp::draw_rect(mx, my, mw, mh, th.border);
    disp::draw_rect(mx + 2, my + 2, mw - 4, mh - 4, th.window);
    let rows = (mh - 10) / MENU_ROW_H;
    let app_rows = rows.saturating_sub(FOOTER_ROWS);
    let mut row = 0;
    for index in 0..entries::count() {
        let Some(entry) = entries::get(index) else {
            continue;
        };
        if entry.hidden {
            continue;
        }
        if row >= app_rows {
            break;
        }
        let top = my + 5 + row * MENU_ROW_H;
        disp::draw_rect(mx + 8, top + 3, 22, 20, entry.color);
        disp::draw_text(mx + 11, top + 9, entry.icon_text(), 0x1E1E2E, entry.color);
        disp::draw_text(mx + 40, top + 9, entry.name(), th.text, th.window);
        row += 1;
    }
    let footer = my + 5 + app_rows * MENU_ROW_H;
    disp::draw_rect(mx + 8, footer, mw - 16, 1, th.border);
    disp::draw_text(mx + 14, footer + 9, "Restart", th.text, th.window);
    disp::draw_text(mx + 14, footer + MENU_ROW_H + 9, "Power off", th.danger, th.window);
}

pub fn menu_click(x: i32, y: i32, screen_h: u32) -> Option<Action> {
    let (mx, my, mw, mh) = menu_rect(screen_h);
    if !point_in(x, y, mx, my, mw, mh) {
        return None;
    }
    let relative = y - my as i32 - 5;
    if relative < 0 {
        return Some(Action::None);
    }
    let row = relative as u32 / MENU_ROW_H;
    let rows = (mh - 10) / MENU_ROW_H;
    let app_rows = rows.saturating_sub(FOOTER_ROWS);
    if row < app_rows {
        let mut seen = 0;
        for index in 0..entries::count() {
            let Some(entry) = entries::get(index) else {
                continue;
            };
            if entry.hidden {
                continue;
            }
            if seen == row {
                return Some(Action::Launch(index));
            }
            seen += 1;
        }
        return Some(Action::None);
    }
    if row == app_rows {
        Some(Action::Reboot)
    } else if row == app_rows + 1 {
        Some(Action::Shutdown)
    } else {
        Some(Action::None)
    }
}

pub enum BarClick {
    Start,
    Handled,
    Outside,
}

pub fn bar_click(x: i32, y: i32, screen_w: u32, screen_h: u32) -> BarClick {
    let top = screen_h.saturating_sub(HEIGHT);
    if !point_in(x, y, 0, top, screen_w, HEIGHT) {
        return BarClick::Outside;
    }
    if point_in(x, y, 4, top + 4, START_W, HEIGHT - 8) {
        return BarClick::Start;
    }
    let tray_x = screen_w.saturating_sub(TRAY_W + 4);
    let tasks_end = if battery_state().is_some() {
        tray_x.saturating_sub(BATTERY_W + 4)
    } else {
        tray_x
    };
    let mut bx = 4 + START_W + 8;
    for task in tasks() {
        if bx + TASK_W > tasks_end {
            break;
        }
        if point_in(x, y, bx, top + 4, TASK_W, HEIGHT - 8) {
            let focused = task.flags & TASK_FOCUSED != 0;
            let minimized = task.flags & TASK_MINIMIZED != 0;
            if focused && !minimized {
                wm_command(task.pid, CMD_MINIMIZE);
            } else {
                wm_activate(task.pid);
            }
            return BarClick::Handled;
        }
        bx += TASK_W + TASK_GAP;
    }
    BarClick::Handled
}

use crate::{entries, icons};
use desk_panel as panel;
use purec::{file_close, file_open};
use desk_display as disp;
use desk_personal as personal;
use purec::fb::{battery, uptime_ms, Battery};
use vb::{TASK_FOCUSED, TASK_MINIMIZED};

#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub id: u32,
    pub flags: u32,
    pub name: [u8; 64],
}

impl TaskInfo {
    const fn empty() -> Self {
        TaskInfo { id: 0, flags: 0, name: [0; 64] }
    }

    fn name_str(&self) -> &str {
        let n = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..n]).unwrap_or("")
    }
}

pub const HEIGHT: u32 = 38;
const START_W: u32 = 100;
const TASK_W: u32 = 128;
const TASK_GAP: u32 = 4;
const TRAY_W: u32 = 84;
const BATTERY_W: u32 = 64;
const BATTERY_POLL_MS: u64 = 5000;
const MENU_W: u32 = 288;
const MENU_ROW_H: u32 = 36;
const MENU_MAX_ROWS: u32 = 12;
const MENU_HEADER_H: u32 = 40;
const MENU_FOOTER_H: u32 = 50;
const MENU_PAD: u32 = 4;
const MENU_ICON: u32 = 28;
const AUDIO_BADGE_H: u32 = 22;
const AUDIO_POPUP_W: u32 = 198;
const LOGIN_PROGRAM: &str = "/usr/bin/qt-login";
const MAX_TASKS: usize = 16;

pub enum Action {
    None,
    Launch(usize),
    Logout,
    Reboot,
    Shutdown,
}

pub fn can_log_out() -> bool {
    let fd = file_open(LOGIN_PROGRAM);
    if fd < 0 {
        return false;
    }
    file_close(fd);
    true
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

pub fn task_update(id: u32, flags: u32, title: &str) {
    let table = unsafe { &mut *core::ptr::addr_of_mut!(TASKS) };
    let count = unsafe { &mut *core::ptr::addr_of_mut!(TASK_COUNT) };
    let mut slot = None;
    for (i, task) in table[..*count].iter().enumerate() {
        if task.id == id {
            slot = Some(i);
        }
    }
    let i = match slot {
        Some(i) => i,
        None if *count < MAX_TASKS => {
            *count += 1;
            *count - 1
        }
        None => return,
    };
    table[i] = TaskInfo::empty();
    table[i].id = id;
    table[i].flags = flags;
    let n = title.len().min(63);
    table[i].name[..n].copy_from_slice(&title.as_bytes()[..n]);
    if flags & TASK_FOCUSED != 0 {
        for (j, task) in table[..*count].iter_mut().enumerate() {
            if j != i {
                task.flags &= !TASK_FOCUSED;
            }
        }
    }
}

pub fn task_gone(id: u32) {
    let table = unsafe { &mut *core::ptr::addr_of_mut!(TASKS) };
    let count = unsafe { &mut *core::ptr::addr_of_mut!(TASK_COUNT) };
    if let Some(i) = table[..*count].iter().position(|t| t.id == id) {
        table.copy_within(i + 1..*count, i);
        *count -= 1;
    }
}

fn signature(list: &[TaskInfo], second: u8) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325 ^ second as u64;
    for task in list {
        hash = (hash ^ task.id as u64).wrapping_mul(0x100_0000_01B3);
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
    let table = unsafe { &*core::ptr::addr_of!(TASKS) };
    let count = unsafe { *core::ptr::addr_of!(TASK_COUNT) };
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

struct Tray {
    clock_x: u32,
    battery_x: Option<u32>,
    volume_x: u32,
    tasks_end: u32,
}

fn tray_layout(screen_w: u32) -> Tray {
    let clock_x = screen_w.saturating_sub(TRAY_W + 4);
    let mut cursor = clock_x;
    let battery_x = if battery_state().is_some() {
        cursor = cursor.saturating_sub(BATTERY_W + 4);
        Some(cursor)
    } else {
        None
    };
    let volume_x = cursor.saturating_sub(panel::badge_width() + 4);
    Tray { clock_x, battery_x, volume_x, tasks_end: volume_x.saturating_sub(4) }
}

fn apply_audio_layout(screen_w: u32, screen_h: u32) {
    let tray = tray_layout(screen_w);
    let bar_y = screen_h.saturating_sub(HEIGHT);
    let badge_y = bar_y + (HEIGHT - AUDIO_BADGE_H) / 2;
    let popup_x = (tray.volume_x + panel::badge_width()).saturating_sub(AUDIO_POPUP_W);
    let popup_y = bar_y.saturating_sub(panel::popup_height() + 4);
    panel::set_layout(tray.volume_x, badge_y, popup_x, popup_y);
}

pub fn draw(screen_w: u32, screen_h: u32, menu_open: bool) {
    let th = personal::current_colors();
    let y = screen_h.saturating_sub(HEIGHT);
    disp::draw_rect(0, y, screen_w, HEIGHT, th.titlebar);
    disp::draw_rect(0, y, screen_w, 1, th.border);
    let start_bg = if menu_open { th.accent } else { th.border };
    let start_fg = if menu_open { th.window } else { th.text };
    disp::draw_rect(4, y + 4, START_W, HEIGHT - 8, start_bg);
    let start_icon = icons::load("desktop-rs");
    if !icons::draw(start_icon, 12, y + 8, 22, start_fg) {
        disp::draw_rect(12, y + 10, 18, 18, start_fg);
    }
    disp::draw_text(42, y + 15, "Start", start_fg, start_bg);

    let tray = tray_layout(screen_w);
    let mut x = 4 + START_W + 8;
    for task in tasks() {
        if x + TASK_W > tray.tasks_end {
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
        disp::draw_text(x + 8, y + 15, name, fg, bg);
        x += TASK_W + TASK_GAP;
    }

    apply_audio_layout(screen_w, screen_h);
    panel::draw_badge(screen_w);
    if let (Some(battery_x), Some(state)) = (tray.battery_x, battery_state()) {
        draw_battery(battery_x, y, state);
    }
    let mut buffer = [0u8; 20];
    let clock = clock_text(&mut buffer);
    disp::draw_rect(tray.clock_x, y + 4, TRAY_W, HEIGHT - 8, th.border);
    disp::draw_text(tray.clock_x + 10, y + 15, clock, th.text, th.border);
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

fn app_rows() -> u32 {
    visible_entries().min(MENU_MAX_ROWS)
}

fn footer_buttons() -> u32 {
    if can_log_out() {
        3
    } else {
        2
    }
}

pub fn menu_rect(screen_h: u32) -> (u32, u32, u32, u32) {
    let height = MENU_PAD * 2 + MENU_HEADER_H + app_rows() * MENU_ROW_H + MENU_FOOTER_H;
    let y = screen_h.saturating_sub(HEIGHT + height);
    (0, y, MENU_W, height)
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    App(usize, u32),
    Footer(u32),
    Dead,
}

static mut HOVER: Option<Hit> = None;

fn hover() -> Option<Hit> {
    unsafe { *core::ptr::addr_of!(HOVER) }
}

fn footer_top(my: u32) -> u32 {
    my + MENU_PAD + MENU_HEADER_H + app_rows() * MENU_ROW_H
}

fn footer_geometry(mx: u32, mw: u32, count: u32, index: u32) -> (u32, u32) {
    let gap = 6;
    let inner = mw - 16;
    let width = (inner - gap * (count - 1)) / count;
    (mx + 8 + index * (width + gap), width)
}

fn hit_test(x: i32, y: i32, screen_h: u32) -> Option<Hit> {
    let (mx, my, mw, mh) = menu_rect(screen_h);
    if !point_in(x, y, mx, my, mw, mh) {
        return None;
    }
    let list_top = my + MENU_PAD + MENU_HEADER_H;
    let rows = app_rows();
    if y >= list_top as i32 && y < (list_top + rows * MENU_ROW_H) as i32 {
        let row = (y - list_top as i32) as u32 / MENU_ROW_H;
        let mut seen = 0;
        for index in 0..entries::count() {
            let Some(entry) = entries::get(index) else {
                continue;
            };
            if entry.hidden {
                continue;
            }
            if seen == row {
                return Some(Hit::App(index, row));
            }
            seen += 1;
        }
        return Some(Hit::Dead);
    }
    let footer = footer_top(my);
    if y >= footer as i32 + 8 && y < (footer + 8 + 32) as i32 {
        let count = footer_buttons();
        for index in 0..count {
            let (bx, bw) = footer_geometry(mx, mw, count, index);
            if point_in(x, y, bx, footer + 8, bw, 32) {
                return Some(Hit::Footer(index));
            }
        }
    }
    Some(Hit::Dead)
}

pub fn menu_hover(x: i32, y: i32, screen_h: u32) -> bool {
    let current = match hit_test(x, y, screen_h) {
        Some(Hit::Dead) | None => None,
        other => other,
    };
    if current == hover() {
        return false;
    }
    unsafe {
        *core::ptr::addr_of_mut!(HOVER) = current;
    }
    true
}

pub fn menu_reset() {
    unsafe {
        *core::ptr::addr_of_mut!(HOVER) = None;
    }
}

pub fn draw_menu(screen_h: u32) {
    let th = personal::current_colors();
    let (mx, my, mw, mh) = menu_rect(screen_h);
    disp::draw_rect(mx, my, mw, mh, th.border);
    disp::draw_rect(mx + 2, my + 2, mw - 4, mh - 4, th.window);
    let title_icon = icons::load("desktop-rs");
    if !icons::draw(title_icon, mx + 12, my + MENU_PAD + 8, 24, th.accent) {
        disp::draw_rect(mx + 12, my + MENU_PAD + 10, 20, 20, th.accent);
    }
    disp::draw_text_sized(mx + 46, my + MENU_PAD + 14, "PureC OS", th.text, th.window, 12);
    disp::draw_rect(mx + 8, my + MENU_PAD + MENU_HEADER_H - 2, mw - 16, 1, th.border);

    let list_top = my + MENU_PAD + MENU_HEADER_H;
    let hovered = hover();
    let mut row = 0;
    for index in 0..entries::count() {
        let Some(entry) = entries::get(index) else {
            continue;
        };
        if entry.hidden {
            continue;
        }
        if row >= app_rows() {
            break;
        }
        let top = list_top + row * MENU_ROW_H;
        let selected = hovered == Some(Hit::App(index, row));
        let bg = if selected { th.border } else { th.window };
        if selected {
            disp::draw_rect(mx + 6, top + 1, mw - 12, MENU_ROW_H - 2, bg);
        }
        if !icons::draw(entry.icon_slot, mx + 12, top + 4, MENU_ICON, th.accent) {
            disp::draw_rect(mx + 12, top + 6, 24, 24, entry.color);
            disp::draw_text(mx + 18, top + 14, entry.icon_text(), 0x1E1E2E, entry.color);
        }
        disp::draw_text(mx + 52, top + 14, entry.name(), th.text, bg);
        row += 1;
    }

    let footer = footer_top(my);
    disp::draw_rect(mx + 8, footer + 2, mw - 16, 1, th.border);
    let count = footer_buttons();
    for index in 0..count {
        let (bx, bw) = footer_geometry(mx, mw, count, index);
        let label = match (count, index) {
            (3, 0) => "Log out",
            (3, 1) | (2, 0) => "Restart",
            _ => "Power off",
        };
        let danger = label == "Power off";
        let selected = hovered == Some(Hit::Footer(index));
        let bg = if selected { if danger { th.danger } else { th.accent } } else { th.border };
        let fg = if selected {
            th.window
        } else if danger {
            th.danger
        } else {
            th.text
        };
        disp::draw_rect(bx, footer + 8, bw, 32, bg);
        let text_w = label.len() as u32 * 8;
        disp::draw_text(bx + bw.saturating_sub(text_w) / 2, footer + 8 + 12, label, fg, bg);
    }
}

pub fn menu_click(x: i32, y: i32, screen_h: u32) -> Option<Action> {
    match hit_test(x, y, screen_h)? {
        Hit::App(index, _) => Some(Action::Launch(index)),
        Hit::Footer(index) => {
            let count = footer_buttons();
            Some(match (count, index) {
                (3, 0) => Action::Logout,
                (3, 1) | (2, 0) => Action::Reboot,
                _ => Action::Shutdown,
            })
        }
        Hit::Dead => Some(Action::None),
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
    let tray = tray_layout(screen_w);
    let mut bx = 4 + START_W + 8;
    for task in tasks() {
        if bx + TASK_W > tray.tasks_end {
            break;
        }
        if point_in(x, y, bx, top + 4, TASK_W, HEIGHT - 8) {
            let focused = task.flags & TASK_FOCUSED != 0;
            let minimized = task.flags & TASK_MINIMIZED != 0;
            if focused && !minimized {
                vb::task_minimize(task.id);
            } else {
                vb::task_activate(task.id);
            }
            return BarClick::Handled;
        }
        bx += TASK_W + TASK_GAP;
    }
    BarClick::Handled
}

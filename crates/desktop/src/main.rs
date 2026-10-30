#![no_std]
#![no_main]

mod entries;
mod taskbar;

use desk_apps as apps;
use desk_display as disp;
use desk_panel as panel;
use desk_personal as personal;
use desk_wallpaper as wallpaper;
use purec::fb::mouse_get;
use purec::fb::{desktop_claim, desktop_wait, uptime_ms, WAIT_KEY, WAIT_MOUSE, WAIT_REDRAW};
use purec::fb::{wm_has_focus, wm_pointer, wm_request_repaint, wm_set_work_area};
use purec::{
    exec, file_close, file_open, reboot, shutdown, try_get_special, try_getchar, wait,
};
use taskbar::{Action, BarClick};

const TOPBAR_HEIGHT: u32 = 28;
const ICON_W: u32 = 58;
const ICON_H: u32 = 72;
const DETACHED_CAP: usize = 8;
const LAUNCH_COOLDOWN_MS: u64 = 500;
const TICK_MS: u64 = 100;
const SLOW_TICK_MS: u64 = 500;
const POWER_MENU_W: u32 = 150;
const POWER_MENU_H: u32 = 62;

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    None,
    Start,
    Power,
    Audio,
}

static mut POPUP: Popup = Popup::None;
static mut SCREEN_W: u32 = 0;
static mut SCREEN_H: u32 = 0;
static mut DETACHED: [i32; DETACHED_CAP] = [0; DETACHED_CAP];
static mut PREV_BUTTONS: u8 = 0;
static mut DRAGGED_ICON: i32 = -1;
static mut DRAG_OFF_X: i32 = 0;
static mut DRAG_OFF_Y: i32 = 0;
static mut ICON_MOVED: bool = false;
static mut LAST_LAUNCH_MS: u64 = 0;
static mut WP_PATH: [u8; 128] = [0; 128];
static mut WP_PATH_LEN: usize = 0;
static mut WP_SYNCED: bool = false;

fn popup() -> Popup {
    unsafe { *core::ptr::addr_of!(POPUP) }
}

fn screen_w() -> u32 {
    unsafe { *core::ptr::addr_of!(SCREEN_W) }
}

fn screen_h() -> u32 {
    unsafe { *core::ptr::addr_of!(SCREEN_H) }
}

fn point_inside(px: i32, py: i32, left: u32, top: u32, w: u32, h: u32) -> bool {
    px >= left as i32 && py >= top as i32 && px < (left + w) as i32 && py < (top + h) as i32
}

fn sync_wallpaper_path() {
    let mut buf = [0u8; 128];
    let n = personal::wallpaper_path_copy(&mut buf);
    unsafe {
        let cur = &*core::ptr::addr_of!(WP_PATH);
        let cur_len = *core::ptr::addr_of!(WP_PATH_LEN);
        if *core::ptr::addr_of!(WP_SYNCED) && cur_len == n && cur[..n] == buf[..n] {
            return;
        }
        let dst = &mut *core::ptr::addr_of_mut!(WP_PATH);
        dst[..n].copy_from_slice(&buf[..n]);
        *core::ptr::addr_of_mut!(WP_PATH_LEN) = n;
        *core::ptr::addr_of_mut!(WP_SYNCED) = true;
    }
    wallpaper::set_path(&buf[..n]);
}

fn draw_app_icon(ix: u32, iy: u32, symbol: &str, label_text: &str, color: u32) {
    let th = personal::current_colors();
    let label = personal::label_size();
    disp::draw_rect(ix, iy, ICON_W, 50, th.titlebar);
    disp::draw_rect(ix + 8, iy + 7, 42, 34, color);
    disp::draw_text_sized(ix + 17, iy + 17, symbol, 0x1E1E2E, color, 12);
    disp::draw_text_sized(ix + 4, iy + 55, label_text, th.text, th.desktop, label);
}

fn draw_icons() {
    for index in 0..entries::count() {
        if let Some(entry) = entries::get(index) {
            if !entry.hidden {
                draw_app_icon(entry.x, entry.y, entry.icon_text(), entry.name(), entry.color);
            }
        }
    }
}

fn draw_power_button() {
    let th = personal::current_colors();
    let x = screen_w().saturating_sub(38);
    disp::draw_rect(x, 3, 30, 22, th.border);
    disp::draw_text(x + 7, 9, "PWR", th.text, th.border);
}

fn power_menu_origin() -> (u32, u32) {
    (screen_w().saturating_sub(POWER_MENU_W + 8), TOPBAR_HEIGHT)
}

fn draw_power_menu() {
    let th = personal::current_colors();
    let (x, y) = power_menu_origin();
    disp::draw_rect(x, y, POWER_MENU_W, POWER_MENU_H, th.border);
    disp::draw_rect(x + 2, y + 2, POWER_MENU_W - 4, 28, th.window);
    disp::draw_rect(x + 2, y + 32, POWER_MENU_W - 4, 28, th.window);
    disp::draw_text(x + 12, y + 11, "Restart", th.text, th.window);
    disp::draw_text(x + 12, y + 41, "Power off", th.danger, th.window);
}

fn draw_desktop() {
    let mut w = disp::width();
    let mut h = disp::height();
    if w == 0 {
        w = 1280;
    }
    if h == 0 {
        h = 800;
    }
    unsafe {
        *core::ptr::addr_of_mut!(SCREEN_W) = w;
        *core::ptr::addr_of_mut!(SCREEN_H) = h;
    }
    let th = personal::current_colors();
    disp::set_font_face(personal::font_face());
    if wallpaper::draw(disp::buffer(), disp::pitch(), w, h) {
        disp::mark_all_dirty();
    } else {
        disp::clear(th.desktop);
    }
    disp::draw_rect(0, 0, w, TOPBAR_HEIGHT, th.titlebar);
    disp::draw_text(12, 8, "PureC OS", th.accent, th.titlebar);
    panel::draw_badge(w);
    draw_icons();
    draw_power_button();
    taskbar::draw(w, h, popup() == Popup::Start);
}

fn draw_popup() {
    match popup() {
        Popup::None => disp::popup_close(),
        Popup::Start => {
            let (x, y, w, h) = taskbar::menu_rect(screen_h());
            if disp::popup_begin(x, y, w, h) {
                taskbar::draw_menu(screen_h());
                disp::popup_end();
            }
        }
        Popup::Power => {
            let (x, y) = power_menu_origin();
            if disp::popup_begin(x, y, POWER_MENU_W, POWER_MENU_H) {
                draw_power_menu();
                disp::popup_end();
            }
        }
        Popup::Audio => {
            let (x, y, w, h) = panel::popup_rect(screen_w());
            if disp::popup_begin(x, y, w, h) {
                panel::draw_popup(screen_w());
                disp::popup_end();
            }
        }
    }
}

fn set_popup(kind: Popup) {
    let previous = popup();
    if previous == kind {
        return;
    }
    unsafe {
        *core::ptr::addr_of_mut!(POPUP) = kind;
    }
    if kind != Popup::Audio {
        panel::set_popup(false);
    }
    draw_popup();
    if previous == Popup::Start || kind == Popup::Start {
        redraw_taskbar();
    }
}

fn redraw_taskbar() {
    disp::begin_update();
    taskbar::draw(screen_w(), screen_h(), popup() == Popup::Start);
    disp::end_update();
}

fn redraw_badge() {
    disp::begin_update();
    panel::draw_badge(screen_w());
    disp::end_update();
}

fn draw_all() {
    disp::begin_update();
    draw_desktop();
    if apps::is_visible() {
        apps::draw();
    }
    disp::end_update();
    draw_popup();
}

fn install_present() -> bool {
    let fd = file_open("/purec/install.cfg");
    if fd < 0 {
        return false;
    }
    file_close(fd);
    true
}

fn run_detached(path: &str) -> i32 {
    unsafe {
        let slots = &mut *core::ptr::addr_of_mut!(DETACHED);
        for slot in slots.iter_mut() {
            if *slot <= 0 {
                let pid = exec(path);
                if pid >= 0 {
                    *slot = pid;
                }
                return pid;
            }
        }
    }
    -1
}

fn reap_detached() {
    let mut finished = false;
    unsafe {
        let slots = &mut *core::ptr::addr_of_mut!(DETACHED);
        for slot in slots.iter_mut() {
            if *slot <= 0 {
                continue;
            }
            let mut status = 0i32;
            if wait(*slot, Some(&mut status), true) > 0 {
                *slot = 0;
                finished = true;
            }
        }
    }
    if finished {
        entries::set_installer_visible(!install_present());
        disp::invalidate();
        draw_all();
        taskbar::poll();
        wm_request_repaint(0);
    }
}

fn launch_entry(index: usize) {
    let now = uptime_ms();
    let last = unsafe { *core::ptr::addr_of!(LAST_LAUNCH_MS) };
    if now.wrapping_sub(last) < LAUNCH_COOLDOWN_MS {
        return;
    }
    unsafe {
        *core::ptr::addr_of_mut!(LAST_LAUNCH_MS) = now;
    }
    let Some(entry) = entries::get(index) else {
        return;
    };
    let exec_path = entry.exec();
    if !exec_path.is_empty() {
        run_detached(exec_path);
        return;
    }
    let app = match entry.builtin() {
        "clock" => apps::APP_CLOCK,
        "calc" => apps::APP_CALCULATOR,
        "calendar" => apps::APP_CALENDAR,
        _ => return,
    };
    apps::open(app, screen_w(), screen_h());
}

fn run_action(action: Action) {
    match action {
        Action::None => {}
        Action::Launch(index) => launch_entry(index),
        Action::Reboot => {
            apps::save_time();
            reboot();
        }
        Action::Shutdown => {
            apps::save_time();
            shutdown();
        }
    }
}

fn handle_popup_click(x: i32, y: i32) -> bool {
    match popup() {
        Popup::Start => {
            if let Some(action) = taskbar::menu_click(x, y, screen_h()) {
                set_popup(Popup::None);
                run_action(action);
                return true;
            }
            false
        }
        Popup::Power => {
            let (px, py) = power_menu_origin();
            if point_inside(x, y, px, py, POWER_MENU_W, POWER_MENU_H) {
                let restart = y < (py + 31) as i32;
                set_popup(Popup::None);
                run_action(if restart {
                    Action::Reboot
                } else {
                    Action::Shutdown
                });
                return true;
            }
            false
        }
        _ => false,
    }
}

fn clamp_icon(x: i32, y: i32) -> (u32, u32) {
    let max_x = screen_w() as i32 - ICON_W as i32;
    let max_y = screen_h() as i32 - ICON_H as i32 - taskbar::HEIGHT as i32;
    let cx = x.clamp(0, max_x.max(0));
    let cy = y.clamp(TOPBAR_HEIGHT as i32, max_y.max(TOPBAR_HEIGHT as i32));
    (cx as u32, cy as u32)
}

fn handle_mouse() {
    let mouse = match mouse_get() {
        Some(m) => m,
        None => return,
    };
    let prev = unsafe { *core::ptr::addr_of!(PREV_BUTTONS) };
    let pressed = (mouse.buttons & 1) != 0 && (prev & 1) == 0;
    let released = (mouse.buttons & 1) == 0 && (prev & 1) != 0;
    unsafe {
        *core::ptr::addr_of_mut!(PREV_BUTTONS) = mouse.buttons;
    }
    let (wm_consumed, _) = wm_pointer(mouse.x, mouse.y, mouse.buttons);
    let mut consumed = false;

    if pressed && handle_popup_click(mouse.x, mouse.y) {
        consumed = true;
    }
    if !consumed && pressed && matches!(popup(), Popup::Start | Popup::Power) {
        set_popup(Popup::None);
    }

    if popup() == Popup::Audio || (!consumed && !wm_consumed) {
        let mut panel_redraw = false;
        let panel_consumed = panel::handle_mouse(
            mouse.x,
            mouse.y,
            mouse.buttons,
            pressed,
            released,
            screen_w(),
            &mut panel_redraw,
        );
        let visible = panel::is_popup_visible();
        if visible && popup() != Popup::Audio {
            set_popup(Popup::Audio);
        } else if !visible && popup() == Popup::Audio {
            set_popup(Popup::None);
        } else if panel_redraw {
            draw_popup();
        }
        if panel_redraw {
            redraw_badge();
        }
        consumed = consumed || panel_consumed;
    }

    if !consumed && !wm_consumed && pressed {
        if point_inside(mouse.x, mouse.y, screen_w().saturating_sub(38), 3, 30, 22) {
            let next = if popup() == Popup::Power {
                Popup::None
            } else {
                Popup::Power
            };
            set_popup(next);
            consumed = true;
        } else {
            match taskbar::bar_click(mouse.x, mouse.y, screen_w(), screen_h()) {
                BarClick::Start => {
                    let next = if popup() == Popup::Start {
                        Popup::None
                    } else {
                        Popup::Start
                    };
                    set_popup(next);
                    consumed = true;
                }
                BarClick::Handled => {
                    taskbar::poll();
                    redraw_taskbar();
                    consumed = true;
                }
                BarClick::Outside => {}
            }
        }
    }

    if !consumed && !wm_consumed && apps::is_visible() {
        let mut app_redraw = false;
        consumed = apps::handle_mouse(
            mouse.x,
            mouse.y,
            mouse.buttons,
            pressed,
            released,
            screen_w(),
            screen_h(),
            &mut app_redraw,
        );
        if app_redraw {
            draw_all();
        }
    }

    if pressed && !consumed && !wm_consumed {
        for index in 0..entries::count() {
            let Some(entry) = entries::get(index) else {
                continue;
            };
            if entry.hidden {
                continue;
            }
            if point_inside(mouse.x, mouse.y, entry.x, entry.y, ICON_W, ICON_H) {
                unsafe {
                    *core::ptr::addr_of_mut!(DRAGGED_ICON) = index as i32;
                    *core::ptr::addr_of_mut!(DRAG_OFF_X) = mouse.x - entry.x as i32;
                    *core::ptr::addr_of_mut!(DRAG_OFF_Y) = mouse.y - entry.y as i32;
                    *core::ptr::addr_of_mut!(ICON_MOVED) = false;
                }
                break;
            }
        }
    }

    let dragged = unsafe { *core::ptr::addr_of!(DRAGGED_ICON) };
    if dragged >= 0 && (mouse.buttons & 1) != 0 {
        let off_x = unsafe { *core::ptr::addr_of!(DRAG_OFF_X) };
        let off_y = unsafe { *core::ptr::addr_of!(DRAG_OFF_Y) };
        let (nx, ny) = clamp_icon(mouse.x - off_x, mouse.y - off_y);
        if let Some(entry) = entries::get(dragged as usize) {
            if nx != entry.x || ny != entry.y {
                entries::set_position(dragged as usize, nx, ny);
                unsafe {
                    *core::ptr::addr_of_mut!(ICON_MOVED) = true;
                }
                draw_all();
            }
        }
    }
    if released && dragged >= 0 {
        let moved = unsafe { *core::ptr::addr_of!(ICON_MOVED) };
        unsafe {
            *core::ptr::addr_of_mut!(DRAGGED_ICON) = -1;
        }
        if !moved {
            launch_entry(dragged as usize);
        }
    }
}

fn handle_keyboard() {
    if wm_has_focus() {
        return;
    }
    loop {
        let c = try_getchar();
        if c < 0 {
            break;
        }
        apps::handle_key(c as u8);
    }
    loop {
        let key = try_get_special();
        if key < 0 {
            break;
        }
        if panel::handle_special_key(key as u8) {
            redraw_badge();
            if popup() == Popup::Audio {
                draw_popup();
            }
        }
    }
}

fn update_work_area() {
    let w = disp::width();
    let h = disp::height();
    let reserved = TOPBAR_HEIGHT + taskbar::HEIGHT;
    wm_set_work_area(0, TOPBAR_HEIGHT, w, h.saturating_sub(reserved));
}

fn reload_mode() {
    disp::refresh_mode();
    update_work_area();
    disp::invalidate();
    draw_all();
    wm_request_repaint(0);
}

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("desktop: Ring-3 desktop starting\n");
    if !disp::init() || disp::width() < 320 || disp::height() < 240 {
        purec::write("desktop: no usable framebuffer\n");
        purec::exit(1);
    }
    if !desktop_claim() {
        purec::write("desktop: session claim refused (boot with desktop=rs)\n");
        purec::exit(1);
    }
    update_work_area();
    apps::init();
    personal::poll();
    sync_wallpaper_path();
    panel::init();
    entries::rescan();
    entries::set_installer_visible(!install_present());
    run_detached("/bin/program/login");
    taskbar::poll();
    draw_all();
    let mut last_tick = uptime_ms();
    let mut last_slow = last_tick;
    loop {
        let events = desktop_wait(50);
        if events & WAIT_REDRAW != 0 {
            reload_mode();
        }
        if events & WAIT_MOUSE != 0 {
            handle_mouse();
        }
        if events & WAIT_KEY != 0 {
            handle_keyboard();
        }
        let now = uptime_ms();
        if now.wrapping_sub(last_tick) >= TICK_MS {
            last_tick = now;
            reap_detached();
            apps::update();
            handle_keyboard();
            if taskbar::poll() {
                redraw_taskbar();
            }
        }
        if now.wrapping_sub(last_slow) >= SLOW_TICK_MS {
            last_slow = now;
            if personal::poll() {
                sync_wallpaper_path();
                draw_all();
            }
        }
        disp::flush();
    }
}

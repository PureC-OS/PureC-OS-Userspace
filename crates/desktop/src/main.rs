#![no_std]
#![no_main]

use desk_apps as apps;
use desk_display as disp;
use desk_panel as panel;
use desk_personal as personal;
use purec::fb::mouse_get;
use purec::fb::{compose_begin, compose_end, klog_set_screen, uptime_ms};
use purec::fb::{desktop_redraw_take, wm_handle_pointer, wm_has_focus, wm_request_repaint};
use purec::{
    exec, file_close, file_open, reboot, shutdown, sleep_ms, try_get_special, try_getchar, wait,
};

const TOPBAR_HEIGHT: u32 = 28;
const ICON_Y: u32 = 48;
const ICON_W: u32 = 58;
const ICON_H: u32 = 72;
const DETACHED_CAP: usize = 8;
const ICON_SLOTS: usize = 12;

struct Icon {
    x: u32,
    y: u32,
}

static mut ICONS: [Icon; ICON_SLOTS] = [
    Icon { x: 348, y: ICON_Y },
    Icon { x: 420, y: ICON_Y },
    Icon { x: 500, y: ICON_Y },
    Icon { x: 40, y: ICON_Y },
    Icon { x: 112, y: ICON_Y },
    Icon { x: 184, y: ICON_Y },
    Icon { x: 256, y: ICON_Y },
    Icon { x: 328, y: ICON_Y },
    Icon { x: 400, y: 130 },
    Icon { x: 472, y: 130 },
    Icon { x: 544, y: 130 },
    Icon { x: 40, y: 210 },
];
static mut SCREEN_W: u32 = 0;
static mut SCREEN_H: u32 = 0;
static mut LAYOUT_READY: bool = false;
static mut INSTALLER_VISIBLE: bool = true;
static mut DETACHED: [i32; DETACHED_CAP] = [0; DETACHED_CAP];
static mut PREV_BUTTONS: u8 = 0;
static mut POWER_MENU: bool = false;
static mut DRAGGED_ICON: i8 = -1;
static mut DRAG_OFF_X: i32 = 0;
static mut DRAG_OFF_Y: i32 = 0;
static mut LAST_DRAW_MS: u64 = 0;
static mut LAST_LAUNCH_MS: u64 = 0;
static mut ICON_MOVED: bool = false;

fn icons() -> &'static mut [Icon; ICON_SLOTS] {
    unsafe { &mut *core::ptr::addr_of_mut!(ICONS) }
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

fn draw_htop_icon() {
    let th = personal::current_colors();
    let (x, y) = (icons()[1].x, icons()[1].y);
    let label = personal::label_size();
    disp::draw_rect(x, y, ICON_W, 50, th.titlebar);
    disp::draw_rect(x + 7, y + 8, 44, 30, th.window);
    disp::draw_text_sized(x + 9, y + 55, "HTOP", th.text, th.desktop, label);
}

fn draw_explorer_icon() {
    let th = personal::current_colors();
    let (x, y) = (icons()[0].x, icons()[0].y);
    let label = personal::label_size();
    disp::draw_rect(x, y, ICON_W, 50, th.titlebar);
    disp::draw_rect(x + 7, y + 13, 44, 27, 0xF9E2AF);
    disp::draw_rect(x + 10, y + 9, 20, 8, 0xF9E2AF);
    disp::draw_rect(x + 10, y + 18, 38, 4, 0xFAB387);
    disp::draw_text_sized(x + 7, y + 55, "Files", th.text, th.desktop, label);
}

fn draw_terminal_icon() {
    let th = personal::current_colors();
    let (x, y) = (icons()[2].x, icons()[2].y);
    let label = personal::label_size();
    disp::draw_rect(x, y, ICON_W, 50, th.titlebar);
    disp::draw_rect(x + 7, y + 8, 44, 30, th.window);
    disp::draw_text(x + 13, y + 18, ">_", 0xA6E3A1, th.window);
    disp::draw_text_sized(x, y + 55, "Terminal", th.text, th.desktop, label);
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
    let icons_visible = unsafe { *core::ptr::addr_of!(INSTALLER_VISIBLE) };
    draw_explorer_icon();
    draw_htop_icon();
    draw_terminal_icon();
    draw_app_icon(icons()[3].x, icons()[3].y, "12", "Clock", 0x89DCEB);
    draw_app_icon(icons()[4].x, icons()[4].y, "+", "Calc", 0xA6E3A1);
    draw_app_icon(icons()[5].x, icons()[5].y, "28", "Calendar", 0xF9E2AF);
    draw_app_icon(icons()[6].x, icons()[6].y, "{}", "Settings", 0x94E2D5);
    if icons_visible {
        draw_app_icon(icons()[7].x, icons()[7].y, "OS", "Install", 0xCBA6F7);
    }
    draw_app_icon(icons()[8].x, icons()[8].y, "HD", "Disks", 0xF9E2AF);
    draw_app_icon(icons()[9].x, icons()[9].y, "[]", "Tetris", 0xF38BA8);
    draw_app_icon(icons()[10].x, icons()[10].y, "LOG", "Logs", 0x89B4FA);
    draw_app_icon(icons()[11].x, icons()[11].y, "HX", "HexEdit", 0xF5C2E7);
}

fn draw_power_button() {
    let th = personal::current_colors();
    let x = screen_w() - 38;
    disp::draw_rect(x, 3, 30, 22, th.border);
    disp::draw_text(x + 7, 9, "PWR", th.text, th.border);
}

fn draw_power_menu() {
    if !unsafe { *core::ptr::addr_of!(POWER_MENU) } {
        return;
    }
    let th = personal::current_colors();
    let x = screen_w() - 158;
    disp::draw_rect(x, 28, 150, 62, th.border);
    disp::draw_rect(x + 2, 30, 146, 28, th.window);
    disp::draw_rect(x + 2, 60, 146, 28, th.window);
    disp::draw_text(x + 12, 39, "Restart", th.text, th.window);
    disp::draw_text(x + 12, 69, "Power off", th.danger, th.window);
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
        if !*core::ptr::addr_of!(LAYOUT_READY) {
            let icons = icons();
            icons[0].x = if w > 700 { 420 } else { w - 212 };
            icons[1].x = icons[0].x + 72;
            icons[2].x = icons[1].x + 72;
            if w <= 700 {
                icons[6].y = 130;
                icons[7].y = 130;
            }
            if w <= 560 {
                icons[3].y = 130;
                icons[4].y = 130;
                icons[5].y = 130;
            }
            *core::ptr::addr_of_mut!(LAYOUT_READY) = true;
        }
    }
    let th = personal::current_colors();
    disp::clear(th.desktop);
    disp::draw_rect(0, 0, w, TOPBAR_HEIGHT, th.titlebar);
    disp::draw_text(12, 8, "PureC OS", th.accent, th.titlebar);
    panel::draw(w);
    draw_icons();
    draw_power_button();
}

fn draw_all() {
    let now = uptime_ms();
    let last = unsafe { *core::ptr::addr_of!(LAST_DRAW_MS) };
    if now - last < 33 {
        return;
    }
    unsafe {
        *core::ptr::addr_of_mut!(LAST_DRAW_MS) = now;
    }
    disp::begin_update();
    compose_begin();
    draw_desktop();
    if apps::is_visible() {
        apps::draw();
    }
    draw_power_menu();
    wm_request_repaint(0);
    compose_end();
    disp::end_update();
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
    unsafe {
        let slots = &mut *core::ptr::addr_of_mut!(DETACHED);
        for slot in slots.iter_mut() {
            if *slot <= 0 {
                continue;
            }
            let mut status = 0i32;
            if wait(*slot, Some(&mut status), true) > 0 {
                *slot = 0;
                *core::ptr::addr_of_mut!(INSTALLER_VISIBLE) = !install_present();
            }
        }
    }
}

fn launch(icon: usize) {
    let now = uptime_ms();
    let last = unsafe { *core::ptr::addr_of!(LAST_LAUNCH_MS) };
    if now - last < 500 {
        return;
    }
    unsafe {
        *core::ptr::addr_of_mut!(LAST_LAUNCH_MS) = now;
    }
    match icon {
        0 => {
            run_detached("/bin/program/files");
        }
        1 => {
            run_detached("/bin/program/monitor");
        }
        2 => {
            run_detached("/bin/program/terminal");
        }
        6 => {
            run_detached("/bin/program/settings");
        }
        7 => {
            run_detached("/bin/installer");
        }
        8 => {
            run_detached("/bin/program/disks");
        }
        9 => {
            run_detached("/bin/program/tetris");
        }
        10 => {
            run_detached("/bin/program/logview");
        }
        11 => {
            run_detached("/bin/program/hexedit");
        }
        _ => {
            apps::open(icon - 3, screen_w(), screen_h());
        }
    }
}

fn handle_mouse() {
    let mouse = match mouse_get() {
        Some(m) => m,
        None => return,
    };
    let prev = unsafe { *core::ptr::addr_of!(PREV_BUTTONS) };
    let pressed = (mouse.buttons & 1) != 0 && (prev & 1) == 0;
    let released = (mouse.buttons & 1) == 0 && (prev & 1) != 0;
    let mut redraw = false;
    let mut consumed = false;

    if pressed && point_inside(mouse.x, mouse.y, screen_w() - 38, 3, 30, 22) {
        unsafe {
            let menu = &mut *core::ptr::addr_of_mut!(POWER_MENU);
            *menu = !*menu;
        }
        consumed = true;
        redraw = true;
    }
    if !consumed {
        let mut panel_redraw = false;
        consumed = panel::handle_mouse(
            mouse.x,
            mouse.y,
            mouse.buttons,
            pressed,
            released,
            screen_w(),
            &mut panel_redraw,
        );
        redraw = redraw || panel_redraw;
    }
    if !consumed && pressed && unsafe { *core::ptr::addr_of!(POWER_MENU) } {
        let menu_x = screen_w() - 158;
        if point_inside(mouse.x, mouse.y, menu_x, 28, 150, 30) {
            apps::save_time();
            reboot();
        } else if point_inside(mouse.x, mouse.y, menu_x, 58, 150, 32) {
            apps::save_time();
            shutdown();
        } else {
            unsafe {
                *core::ptr::addr_of_mut!(POWER_MENU) = false;
            }
            redraw = true;
        }
        consumed = true;
    }
    if !consumed {
        let (wm_consumed, focus_changed) =
            wm_handle_pointer(mouse.x, mouse.y, pressed);
        consumed = wm_consumed;
        if focus_changed {
            draw_all();
        }
    }
    if !consumed && apps::is_visible() {
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
        redraw = redraw || app_redraw;
    }
    let installer_visible = unsafe { *core::ptr::addr_of!(INSTALLER_VISIBLE) };
    if pressed && !consumed {
        let icons = icons();
        for index in 0..ICON_SLOTS {
            if index == 7 && !installer_visible {
                continue;
            }
            if point_inside(mouse.x, mouse.y, icons[index].x, icons[index].y, ICON_W, ICON_H) {
                unsafe {
                    *core::ptr::addr_of_mut!(DRAGGED_ICON) = index as i8;
                    *core::ptr::addr_of_mut!(DRAG_OFF_X) = mouse.x - icons[index].x as i32;
                    *core::ptr::addr_of_mut!(DRAG_OFF_Y) = mouse.y - icons[index].y as i32;
                    *core::ptr::addr_of_mut!(ICON_MOVED) = false;
                }
                break;
            }
        }
    }
    let dragged = unsafe { *core::ptr::addr_of!(DRAGGED_ICON) };
    if dragged >= 0 && (mouse.buttons & 1) != 0 {
        let icons = icons();
        let slot = &mut icons[dragged as usize];
        let off_x = unsafe { *core::ptr::addr_of!(DRAG_OFF_X) };
        let off_y = unsafe { *core::ptr::addr_of!(DRAG_OFF_Y) };
        let mut next_x = mouse.x - off_x;
        let mut next_y = mouse.y - off_y;
        if next_x < 0 {
            next_x = 0;
        }
        if next_x > screen_w() as i32 - ICON_W as i32 {
            next_x = screen_w() as i32 - ICON_W as i32;
        }
        if next_y < TOPBAR_HEIGHT as i32 {
            next_y = TOPBAR_HEIGHT as i32;
        }
        if next_y > screen_h() as i32 - ICON_H as i32 {
            next_y = screen_h() as i32 - ICON_H as i32;
        }
        if next_x as u32 != slot.x || next_y as u32 != slot.y {
            slot.x = next_x as u32;
            slot.y = next_y as u32;
            unsafe {
                *core::ptr::addr_of_mut!(ICON_MOVED) = true;
            }
            draw_all();
        }
    }
    if released && dragged >= 0 {
        let icon = dragged as usize;
        let moved = unsafe { *core::ptr::addr_of!(ICON_MOVED) };
        unsafe {
            *core::ptr::addr_of_mut!(DRAGGED_ICON) = -1;
        }
        if !moved {
            launch(icon);
        }
        redraw = true;
    }
    unsafe {
        *core::ptr::addr_of_mut!(PREV_BUTTONS) = mouse.buttons;
    }
    if redraw {
        draw_all();
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
        panel::handle_special_key(key as u8);
    }
}

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    klog_set_screen(false);
    purec::write("desktop: Ring-3 desktop starting\n");
    if !disp::is_available() || disp::width() < 320 || disp::height() < 240 {
        purec::write("desktop: no usable framebuffer\n");
        purec::exit(1);
    }
    apps::init();
    personal::poll();
    panel::init();
    unsafe {
        *core::ptr::addr_of_mut!(INSTALLER_VISIBLE) = !install_present();
    }
    run_detached("/bin/program/login");
    draw_all();
    loop {
        reap_detached();
        if personal::poll() {
            draw_all();
        }
        if desktop_redraw_take() {
            draw_all();
        }
        handle_mouse();
        handle_keyboard();
        apps::update();
        sleep_ms(1);
    }
}

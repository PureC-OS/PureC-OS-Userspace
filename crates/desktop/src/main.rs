#![no_std]
#![no_main]

mod entries;
mod icons;
mod taskbar;

use desk_datetime as dt;
use desk_display as disp;
use desk_panel as panel;
use desk_personal as personal;
use desk_wallpaper as wallpaper;
use purec::fb::uptime_ms;
use purec::{exec, exec_with_args, file_close, file_open, reboot, shutdown, wait};
use taskbar::{Action, BarClick};

const DESK_APP: &str = "/usr/bin/desk-app";

const DESK_TOP: u32 = 8;
const ICON_W: u32 = 58;
const ICON_H: u32 = 72;
const DETACHED_CAP: usize = 8;
const LAUNCH_COOLDOWN_MS: u64 = 500;
const TICK_MS: u64 = 100;
const SLOW_TICK_MS: u64 = 500;

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    None,
    Start,
    Audio,
}

static mut POPUP: Popup = Popup::None;
static mut SCREEN_W: u32 = 0;
static mut SCREEN_H: u32 = 0;
static mut DETACHED: [i32; DETACHED_CAP] = [0; DETACHED_CAP];
static mut BUTTONS: u8 = 0;
static mut DRAGGED_ICON: i32 = -1;
static mut DRAG_OFF_X: i32 = 0;
static mut DRAG_OFF_Y: i32 = 0;
static mut ICON_MOVED: bool = false;
static mut LAST_LAUNCH_MS: u64 = 0;
static mut WP_PATH: [u8; 128] = [0; 128];
static mut WP_PATH_LEN: usize = 0;
static mut WP_SYNCED: bool = false;
static mut BACKGROUND: *mut vb::Window = core::ptr::null_mut();
static mut BAR: *mut vb::Window = core::ptr::null_mut();

fn setup_windows(w: u32, h: u32) -> bool {
    unsafe {
        for slot in [core::ptr::addr_of_mut!(BACKGROUND), core::ptr::addr_of_mut!(BAR)] {
            if !(*slot).is_null() {
                vb::destroy(*slot);
                *slot = core::ptr::null_mut();
            }
        }
    }
    disp::clear_views();
    if w < 320 || h < 240 || !disp::init_size(w, h) {
        return false;
    }
    let background = vb::create(0, 0, w, h, "Desktop", vb::ROLE_BACKGROUND);
    let bottom = vb::create(0, (h - taskbar::HEIGHT) as i32, w, taskbar::HEIGHT, "Taskbar", vb::ROLE_PANEL);
    if background.is_null() || bottom.is_null() {
        return false;
    }
    disp::add_view(background, 0, 0, w, h);
    disp::add_view(bottom, 0, h - taskbar::HEIGHT, w, taskbar::HEIGHT);
    unsafe {
        *core::ptr::addr_of_mut!(BACKGROUND) = background;
        *core::ptr::addr_of_mut!(BAR) = bottom;
    }
    true
}

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

fn draw_app_icon(ix: u32, iy: u32, symbol: &str, label_text: &str, color: u32, slot: i32) {
    let th = personal::current_colors();
    let label = personal::label_size();
    disp::draw_rect(ix, iy, ICON_W, 50, th.titlebar);
    if !icons::draw(slot, ix + 9, iy + 5, 40, th.accent) {
        disp::draw_rect(ix + 8, iy + 7, 42, 34, color);
        disp::draw_text_sized(ix + 17, iy + 17, symbol, 0x1E1E2E, color, 12);
    }
    disp::draw_text_sized(ix + 4, iy + 55, label_text, th.text, th.desktop, label);
}

fn draw_icons() {
    for index in 0..entries::count() {
        if let Some(entry) = entries::get(index) {
            if !entry.hidden {
                draw_app_icon(entry.x, entry.y, entry.icon_text(), entry.name(), entry.color, entry.icon_slot);
            }
        }
    }
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
    draw_icons();
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
    if kind == Popup::Start {
        taskbar::menu_reset();
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
    redraw_taskbar();
}

fn draw_all() {
    disp::begin_update();
    draw_desktop();
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

fn run_detached(path: &str, args: &str) -> i32 {
    unsafe {
        let slots = &mut *core::ptr::addr_of_mut!(DETACHED);
        for slot in slots.iter_mut() {
            if *slot <= 0 {
                let pid = if args.is_empty() { exec(path) } else { exec_with_args(path, args) };
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
        draw_all();
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
        run_detached(exec_path, "");
        return;
    }
    match entry.builtin() {
        name @ ("clock" | "calc" | "calendar") => {
            run_detached(DESK_APP, name);
        }
        _ => {}
    }
}

fn run_action(action: Action) {
    match action {
        Action::None => {}
        Action::Launch(index) => launch_entry(index),
        Action::Logout => {
            dt::save();
            vb::disconnect();
            purec::exit(0);
        }
        Action::Reboot => {
            dt::save();
            power(true);
        }
        Action::Shutdown => {
            dt::save();
            power(false);
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
        _ => false,
    }
}

fn clamp_icon(x: i32, y: i32) -> (u32, u32) {
    let max_x = screen_w() as i32 - ICON_W as i32;
    let max_y = screen_h() as i32 - ICON_H as i32 - taskbar::HEIGHT as i32;
    let cx = x.clamp(0, max_x.max(0));
    let cy = y.clamp(DESK_TOP as i32, max_y.max(DESK_TOP as i32));
    (cx as u32, cy as u32)
}

struct Pointer {
    x: i32,
    y: i32,
    buttons: u8,
}

fn handle_mouse(mouse: Pointer, pressed: bool, released: bool) {
    let wm_consumed = false;
    let mut consumed = false;

    if popup() == Popup::Start && taskbar::menu_hover(mouse.x, mouse.y, screen_h()) {
        draw_popup();
    }

    if pressed && handle_popup_click(mouse.x, mouse.y) {
        consumed = true;
    }
    if !consumed && pressed && popup() == Popup::Start {
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
        {
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

fn handle_key(event: &vb::Event) {
    if !event.pressed {
        return;
    }
    let (_, special) = vb::translate_key(event.key, event.mods);
    if special != 0 && panel::handle_special_key(special) {
        redraw_badge();
        if popup() == Popup::Audio {
            draw_popup();
        }
    }
}

fn power(restart: bool) -> ! {
    let verb = if restart { "reboot" } else { "poweroff" };
    let pid = exec_with_args("/bin/systemctl", verb);
    if pid >= 0 {
        let mut status = 0;
        wait(pid, Some(&mut status), false);
        purec::sleep_ms(3000);
    }
    if restart {
        reboot();
    }
    shutdown();
}

fn handle_event(event: &vb::Event) {
    match event.kind {
        vb::EVENT_POINTER | vb::EVENT_ENTER => {
            let Some((ox, oy)) = disp::origin_of(event.window) else {
                return;
            };
            let buttons = event.buttons as u8;
            unsafe {
                *core::ptr::addr_of_mut!(BUTTONS) = buttons;
            }
            let pointer = Pointer { x: event.x + ox as i32, y: event.y + oy as i32, buttons };
            handle_mouse(pointer, event.left_pressed(), event.left_released());
        }
        vb::EVENT_KEY => handle_key(event),
        vb::EVENT_TASK => {
            taskbar::task_update(event.task_id, event.task_flags, event.title_str());
            redraw_taskbar();
        }
        vb::EVENT_TASK_GONE => {
            taskbar::task_gone(event.task_id);
            redraw_taskbar();
        }
        vb::EVENT_POPUP_DONE => {
            if popup() == Popup::Audio {
                panel::set_popup(false);
            }
            set_popup(Popup::None);
        }
        vb::EVENT_SCREEN if event.width > 0 && event.height > 0 => {
            unsafe {
                *core::ptr::addr_of_mut!(POPUP) = Popup::None;
            }
            panel::set_popup(false);
            if !setup_windows(event.width as u32, event.height as u32) {
                purec::exit(1);
            }
            draw_all();
        }
        vb::EVENT_DISCONNECT => purec::exit(1),
        _ => {}
    }
}

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("desktop: starting\n");
    let mut tries = 0;
    while !vb::connect() {
        tries += 1;
        if tries > 50 {
            purec::write("desktop: videod is not running\n");
            purec::exit(1);
        }
        purec::sleep_ms(100);
    }
    let (w, h) = vb::screen_size();
    if !setup_windows(w, h) {
        purec::write("desktop: cannot create the windows\n");
        purec::exit(1);
    }
    vb::tasks_subscribe();
    dt::init();
    personal::poll();
    sync_wallpaper_path();
    panel::init();
    entries::rescan();
    entries::set_installer_visible(!install_present());
    taskbar::poll();
    draw_all();
    disp::flush();
    let mut last_tick = uptime_ms();
    let mut last_slow = last_tick;
    let mut event = vb::Event::empty();
    loop {
        if vb::next_event(&mut event, TICK_MS as i32) > 0 {
            handle_event(&event);
            while vb::next_event(&mut event, 0) > 0 {
                handle_event(&event);
            }
        }
        let now = uptime_ms();
        if now.wrapping_sub(last_tick) >= TICK_MS {
            last_tick = now;
            reap_detached();
            dt::update();
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

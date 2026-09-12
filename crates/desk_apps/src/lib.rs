#![no_std]
use desk_calendar as calendar;
use desk_clock as clock;
use desk_calc as calc;
use desk_datetime as dt;
use desk_display as disp;
pub const APP_CLOCK: usize = 0;
pub const APP_CALCULATOR: usize = 1;
pub const APP_CALENDAR: usize = 2;
pub const APP_COUNT: usize = 3;
const WINDOW_WIDTH: u32 = 360;
const WINDOW_HEIGHT: u32 = 270;
const TITLE_BAR_HEIGHT: u32 = 30;
struct AppWindow {
    x: u32,
    y: u32,
    visible: bool,
}
static mut WINDOWS: [AppWindow; APP_COUNT] = [
    AppWindow { x: 160, y: 70, visible: false },
    AppWindow { x: 200, y: 100, visible: false },
    AppWindow { x: 240, y: 130, visible: false },
];
static mut Z_ORDER: [u8; APP_COUNT] = [APP_CLOCK as u8, APP_CALCULATOR as u8, APP_CALENDAR as u8];
static mut FOCUSED: i8 = -1;
static mut DRAGGED: i8 = -1;
static mut DRAG_OFF_X: i32 = 0;
static mut DRAG_OFF_Y: i32 = 0;
static mut LAST_CAL_YEAR: u16 = 0;
static mut LAST_CAL_MONTH: u8 = 0;
static mut LAST_CAL_DAY: u8 = 0;
fn windows() -> &'static mut [AppWindow; APP_COUNT] {
    unsafe { &mut *core::ptr::addr_of_mut!(WINDOWS) }
}
fn z_order() -> &'static mut [u8; APP_COUNT] {
    unsafe { &mut *core::ptr::addr_of_mut!(Z_ORDER) }
}
fn valid_app(app: i32) -> bool {
    app >= 0 && (app as usize) < APP_COUNT
}
fn point_inside(px: i32, py: i32, left: u32, top: u32, w: u32, h: u32) -> bool {
    px >= left as i32 && py >= top as i32 && px < (left + w) as i32 && py < (top + h) as i32
}
fn app_title(app: usize) -> &'static str {
    match app {
        APP_CLOCK => "Clock",
        APP_CALCULATOR => "Calculator",
        APP_CALENDAR => "Calendar",
        _ => "Application",
    }
}
fn draw_window_frame(app: usize) {
    let w = windows();
    let focused = unsafe { *core::ptr::addr_of!(FOCUSED) } as usize;
    let title_color = if focused == app { 0x89B4FA } else { 0x585B70 };
    let (x, y) = (w[app].x, w[app].y);
    disp::draw_rect(x + 5, y + 5, WINDOW_WIDTH, WINDOW_HEIGHT, 0x11111B);
    disp::draw_rect(x, y, WINDOW_WIDTH, WINDOW_HEIGHT, 0x45475A);
    disp::draw_rect(x + 1, y + 1, WINDOW_WIDTH - 2, WINDOW_HEIGHT - 2, 0x1E1E2E);
    disp::draw_rect(x + 1, y + 1, WINDOW_WIDTH - 2, TITLE_BAR_HEIGHT, title_color);
    disp::draw_text(x + 12, y + 10, app_title(app), 0x1E1E2E, title_color);
    disp::draw_rect(x + WINDOW_WIDTH - 27, y + 6, 18, 18, 0xF38BA8);
    disp::draw_text(x + WINDOW_WIDTH - 23, y + 10, "x", 0x1E1E2E, 0xF38BA8);
}
fn draw_app_content(app: usize) {
    let w = windows();
    let (x, y) = (w[app].x, w[app].y);
    match app {
        APP_CLOCK => clock::draw(x, y),
        APP_CALCULATOR => calc::draw(x, y),
        APP_CALENDAR => calendar::draw(x, y),
        _ => {}
    }
}
fn draw_window(app: usize) {
    draw_window_frame(app);
    draw_app_content(app);
}
fn z_index_of(app: usize) -> i8 {
    let z = z_order();
    for (i, &slot) in z.iter().enumerate() {
        if slot as usize == app {
            return i as i8;
        }
    }
    -1
}
fn bring_to_front(app: usize) -> bool {
    let current = z_index_of(app);
    if current < 0 || current as usize == APP_COUNT - 1 {
        return false;
    }
    let z = z_order();
    let mut i = current as usize;
    while i < APP_COUNT - 1 {
        z[i] = z[i + 1];
        i += 1;
    }
    z[APP_COUNT - 1] = app as u8;
    true
}
fn top_window_at(px: i32, py: i32) -> i8 {
    let (z, w) = (z_order(), windows());
    let mut i = APP_COUNT as i8 - 1;
    while i >= 0 {
        let app = z[i as usize] as usize;
        if w[app].visible
            && point_inside(px, py, w[app].x, w[app].y, WINDOW_WIDTH, WINDOW_HEIGHT)
        {
            return app as i8;
        }
        i -= 1;
    }
    -1
}
fn top_visible_window() -> i8 {
    let (z, w) = (z_order(), windows());
    let mut i = APP_COUNT as i8 - 1;
    while i >= 0 {
        let app = z[i as usize] as usize;
        if w[app].visible {
            return app as i8;
        }
        i -= 1;
    }
    -1
}
fn redraw_from_z_index(first: i8) {
    if first < 0 {
        return;
    }
    disp::begin_update();
    let (z, w) = (z_order(), windows());
    let mut i = first;
    while (i as usize) < APP_COUNT {
        let app = z[i as usize] as usize;
        if w[app].visible {
            draw_window(app);
        }
        i += 1;
    }
    disp::end_update();
}
fn open_app_state(app: usize) {
    match app {
        APP_CLOCK => clock::open(),
        APP_CALCULATOR => calc::open(),
        APP_CALENDAR => calendar::open(),
        _ => {}
    }
}
fn remember_calendar_date() {
    let datetime = dt::get();
    unsafe {
        *core::ptr::addr_of_mut!(LAST_CAL_YEAR) = datetime.year;
        *core::ptr::addr_of_mut!(LAST_CAL_MONTH) = datetime.month;
        *core::ptr::addr_of_mut!(LAST_CAL_DAY) = datetime.day;
    }
}
pub fn init() {
    dt::init();
    remember_calendar_date();
}
pub fn save_time() {
    dt::save();
}
pub fn draw() {
    disp::begin_update();
    let (z, w) = (z_order(), windows());
    for i in 0..APP_COUNT {
        let app = z[i] as usize;
        if w[app].visible {
            draw_window(app);
        }
    }
    disp::end_update();
}
pub fn open(app: usize, screen_width: u32, screen_height: u32) {
    if app >= APP_COUNT {
        return;
    }
    let was_visible = {
        let w = windows();
        let was = w[app].visible;
        w[app].visible = true;
        if w[app].x + WINDOW_WIDTH > screen_width {
            w[app].x = 10 + app as u32 * 24;
        }
        if w[app].y + WINDOW_HEIGHT > screen_height {
            w[app].y = 34 + app as u32 * 24;
        }
        was
    };
    unsafe {
        *core::ptr::addr_of_mut!(FOCUSED) = app as i8;
    }
    bring_to_front(app);
    if !was_visible {
        open_app_state(app);
    }
    let idx = z_index_of(app);
    redraw_from_z_index(idx);
}
pub fn is_visible() -> bool {
    top_visible_window() >= 0
}
pub fn contains_point(x: i32, y: i32) -> bool {
    top_window_at(x, y) >= 0
}
#[allow(clippy::too_many_arguments)]
pub fn handle_mouse(
    point_x: i32,
    point_y: i32,
    buttons: u8,
    pressed: bool,
    released: bool,
    screen_width: u32,
    screen_height: u32,
    redraw_required: &mut bool,
) -> bool {
    let target = top_window_at(point_x, point_y);
    let dragged = unsafe { *core::ptr::addr_of!(DRAGGED) };
    let captured = dragged >= 0 || target >= 0;
    *redraw_required = false;
    if pressed && target >= 0 {
        let app = target as usize;
        unsafe {
            *core::ptr::addr_of_mut!(FOCUSED) = target;
        }
        if bring_to_front(app) {
            *redraw_required = true;
        }
        {
            let w = windows();
            if point_inside(
                point_x,
                point_y,
                w[app].x + WINDOW_WIDTH - 27,
                w[app].y + 6,
                18,
                18,
            ) {
                w[app].visible = false;
                unsafe {
                    *core::ptr::addr_of_mut!(FOCUSED) = top_visible_window();
                    *core::ptr::addr_of_mut!(DRAGGED) = -1;
                }
                *redraw_required = true;
                return true;
            }
            if point_inside(
                point_x,
                point_y,
                w[app].x,
                w[app].y,
                WINDOW_WIDTH,
                TITLE_BAR_HEIGHT,
            ) {
                unsafe {
                    *core::ptr::addr_of_mut!(DRAGGED) = target;
                    *core::ptr::addr_of_mut!(DRAG_OFF_X) = point_x - w[app].x as i32;
                    *core::ptr::addr_of_mut!(DRAG_OFF_Y) = point_y - w[app].y as i32;
                }
            }
        }
    }
    let dragged = unsafe { *core::ptr::addr_of!(DRAGGED) };
    if dragged >= 0 && (buttons & 1) != 0 {
        let w = windows();
        let app = dragged as usize;
        let next_x = point_x - unsafe { *core::ptr::addr_of!(DRAG_OFF_X) };
        let next_y = point_y - unsafe { *core::ptr::addr_of!(DRAG_OFF_Y) };
        let maximum_x = if screen_width > WINDOW_WIDTH {
            (screen_width - WINDOW_WIDTH) as i32
        } else {
            0
        };
        let mut maximum_y = if screen_height > WINDOW_HEIGHT {
            (screen_height - WINDOW_HEIGHT) as i32
        } else {
            28
        };
        if maximum_y < 28 {
            maximum_y = 28;
        }
        let mut nx = next_x;
        let mut ny = next_y;
        if nx < 0 {
            nx = 0;
        }
        if ny < 28 {
            ny = 28;
        }
        if nx > maximum_x {
            nx = maximum_x;
        }
        if ny > maximum_y {
            ny = maximum_y;
        }
        if nx as u32 != w[app].x || ny as u32 != w[app].y {
            w[app].x = nx as u32;
            w[app].y = ny as u32;
            *redraw_required = true;
        }
        return true;
    }
    if released && dragged >= 0 {
        unsafe {
            *core::ptr::addr_of_mut!(DRAGGED) = -1;
        }
        return true;
    }
    captured
}
pub fn handle_key(key: u8) -> bool {
    let mut focused = unsafe { *core::ptr::addr_of!(FOCUSED) };
    if !valid_app(focused as i32) || !windows()[focused as usize].visible {
        focused = top_visible_window();
        unsafe {
            *core::ptr::addr_of_mut!(FOCUSED) = focused;
        }
    }
    if !valid_app(focused as i32) {
        return false;
    }
    let app = focused as usize;
    let before = dt::get();
    let handled = match app {
        APP_CLOCK => clock::handle_key(key),
        APP_CALCULATOR => calc::handle_key(key),
        APP_CALENDAR => calendar::handle_key(key),
        _ => false,
    };
    if !handled {
        return false;
    }
    let after = dt::get();
    let date_changed =
        after.year != before.year || after.month != before.month || after.day != before.day;
    let mut first_redraw = z_index_of(app);
    if date_changed && windows()[APP_CALENDAR].visible {
        let calendar_index = z_index_of(APP_CALENDAR);
        if calendar_index < first_redraw {
            first_redraw = calendar_index;
        }
    }
    remember_calendar_date();
    redraw_from_z_index(first_redraw);
    true
}
pub fn update() {
    let time_changed = dt::update();
    if !time_changed {
        return;
    }
    let datetime = dt::get();
    let (last_y, last_m, last_d) = unsafe {
        (
            *core::ptr::addr_of!(LAST_CAL_YEAR),
            *core::ptr::addr_of!(LAST_CAL_MONTH),
            *core::ptr::addr_of!(LAST_CAL_DAY),
        )
    };
    let date_changed =
        datetime.year != last_y || datetime.month != last_m || datetime.day != last_d;
    remember_calendar_date();
    let mut first_redraw = APP_COUNT as i8;
    let clock_index = z_index_of(APP_CLOCK);
    if windows()[APP_CLOCK].visible && clock_index < first_redraw {
        first_redraw = clock_index;
    }
    let calendar_index = z_index_of(APP_CALENDAR);
    if date_changed && windows()[APP_CALENDAR].visible && calendar_index < first_redraw {
        first_redraw = calendar_index;
    }
    if first_redraw < APP_COUNT as i8 {
        redraw_from_z_index(first_redraw);
    }
}

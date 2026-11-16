#![no_std]
#![no_main]

use desk_calc as calc;
use desk_calendar as calendar;
use desk_clock as clock;
use desk_datetime as dt;
use desk_display as disp;
use desk_personal as personal;

const WIDTH: u32 = 360;
const HEIGHT: u32 = 270;
const TITLE_H: u32 = 30;
const CLOSE_X: u32 = WIDTH - 27;
const TICK_MS: i32 = 250;

#[derive(Clone, Copy, PartialEq)]
enum App {
    Clock,
    Calculator,
    Calendar,
}

impl App {
    fn parse(name: &str) -> Option<App> {
        match name {
            "clock" => Some(App::Clock),
            "calc" | "calculator" => Some(App::Calculator),
            "calendar" => Some(App::Calendar),
            _ => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            App::Clock => "Clock",
            App::Calculator => "Calculator",
            App::Calendar => "Calendar",
        }
    }

    fn open(self) {
        match self {
            App::Clock => clock::open(),
            App::Calculator => calc::open(),
            App::Calendar => calendar::open(),
        }
    }

    fn draw(self) {
        match self {
            App::Clock => clock::draw(0, 0),
            App::Calculator => calc::draw(0, 0),
            App::Calendar => calendar::draw(0, 0),
        }
    }

    fn key(self, key: u8) -> bool {
        match self {
            App::Clock => clock::handle_key(key),
            App::Calculator => calc::handle_key(key),
            App::Calendar => calendar::handle_key(key),
        }
    }
}

fn draw(app: App, focused: bool) {
    let th = personal::current_colors();
    disp::set_font_face(personal::font_face());
    let title_color = if focused { 0x89B4FA } else { 0x585B70 };
    disp::begin_update();
    disp::draw_rect(0, 0, WIDTH, HEIGHT, 0x45475A);
    disp::draw_rect(1, 1, WIDTH - 2, HEIGHT - 2, 0x1E1E2E);
    disp::draw_rect(1, 1, WIDTH - 2, TITLE_H, title_color);
    disp::draw_text(12, 10, app.title(), 0x1E1E2E, title_color);
    disp::draw_rect(CLOSE_X, 6, 18, 18, 0xF38BA8);
    disp::draw_text(CLOSE_X + 4, 10, "x", 0x1E1E2E, 0xF38BA8);
    let _ = th;
    app.draw();
    disp::end_update();
}

fn argument(buf: &mut [u8; 128]) -> &str {
    let n = purec::command_line(buf);
    if n <= 0 {
        return "";
    }
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let text = core::str::from_utf8(&buf[..len]).unwrap_or("");
    let text = text.trim();
    match text.rfind(' ') {
        Some(i) => text[i + 1..].trim(),
        None => text,
    }
}

fn quit(window: *mut vb::Window) -> ! {
    dt::save();
    vb::destroy(window);
    vb::disconnect();
    purec::exit(0);
}

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    let mut buf = [0u8; 128];
    let app = match App::parse(argument(&mut buf)) {
        Some(app) => app,
        None => {
            purec::write("usage: desk-app clock|calc|calendar\n");
            purec::exit(2);
        }
    };
    if !vb::connect() {
        purec::write("desk-app: videod is not running\n");
        purec::exit(1);
    }
    let window = vb::create(-1, -1, WIDTH, HEIGHT, app.title(), vb::ROLE_NORMAL);
    if window.is_null() || !disp::init_size(WIDTH, HEIGHT) || !disp::add_view(window, 0, 0, WIDTH, HEIGHT) {
        purec::write("desk-app: cannot create the window\n");
        purec::exit(1);
    }
    personal::poll();
    dt::init();
    app.open();
    let mut focused = true;
    draw(app, focused);
    let mut event = vb::Event::empty();
    loop {
        let r = vb::next_event(&mut event, TICK_MS);
        if r < 0 {
            quit(window);
        }
        if r == 0 {
            if dt::update() && app != App::Calculator {
                draw(app, focused);
            }
            continue;
        }
        match event.kind {
            vb::EVENT_CLOSE => quit(window),
            vb::EVENT_FOCUS => {
                focused = event.focused;
                draw(app, focused);
            }
            vb::EVENT_POINTER if event.left_pressed() => {
                let (x, y) = (event.x, event.y);
                if y >= 6 && y < 24 && x >= CLOSE_X as i32 && x < CLOSE_X as i32 + 18 {
                    quit(window);
                }
                if y >= 0 && y < TITLE_H as i32 {
                    vb::start_move(window);
                }
            }
            vb::EVENT_KEY if event.pressed => {
                let (ch, _) = vb::translate_key(event.key, event.mods);
                if ch != 0 && app.key(ch) {
                    if app == App::Clock {
                        dt::save();
                    }
                    draw(app, focused);
                }
            }
            _ => {}
        }
    }
}

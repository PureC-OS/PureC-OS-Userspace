#![no_std]
#![no_main]
#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("apps-demo: opening desk apps\n");
    desk_apps::init();
    let w = desk_display::width();
    let h = desk_display::height();
    purec::write("apps-demo: screen ");
    purec::write_u64(w as u64);
    purec::write("x");
    purec::write_u64(h as u64);
    purec::write("\n");
    desk_apps::open(desk_apps::APP_CLOCK, w, h);
    desk_apps::open(desk_apps::APP_CALCULATOR, w, h);
    desk_apps::open(desk_apps::APP_CALENDAR, w, h);
    desk_apps::draw();
    purec::write("apps-demo: done\n");
    purec::exit(0);
}

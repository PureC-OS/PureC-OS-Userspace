#![no_std]
#![no_main]
#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("hello PureC OS\n");
    purec::exit(0);
}

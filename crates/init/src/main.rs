//! `init` — Rust-порт `src/programs/init/main.c` (PID 1).
//!
//! Оригинал:
//! ```c
//! void _start(void){
//!     pc_write("init: PID 1 started\n");
//!     for(;;) pc_sleep(1000);
//! }
//! ```

#![no_std]
#![no_main]

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("init: PID 1 started\n");
    loop {
        purec::sleep_ms(1000);
    }
}

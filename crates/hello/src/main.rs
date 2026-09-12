//! `hello` — Rust-порт `src/programs/hello/main.c`.
//!
//! Минимальный пример hosted-программы на новом стеке:
//! точка входа `_start`, вывод через `libpurec.a`, выход через `pc_exit`.
//! Аргументы командной строки пока не разбираем (следующий шаг —
//! Rust-порт `crt0.c`: `pc_get_command_line` + `environ`).

#![no_std]
#![no_main]

#[no_mangle]
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    purec::write("hello PureC OS\n");
    purec::exit(0);
}

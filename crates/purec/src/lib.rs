//! `purec` — безопасные обертки над `libpurec.a` для PureC OS.
//!
//! Связывается с существующей C-библиотекой (`src/libc`, собирается в
//! `bin/lib/libpurec.a` через `x86_64-elf-gcc`). Весь syscall ABI
//! (`int $0x80`, `rax=number, rbx/rcx/rdx=arg1..3`) уже реализован в C,
//! Rust только объявляет FFI и дает удобные `&str`-обертки.
//!
//! Бинарные крейты: `#![no_std] #![no_main]`, точка входа —
//! `#[no_mangle] pub extern "C" fn _start() -> !`, паника — через
//! этот крейт (один `#[panic_handler]` на бинарь).

#![no_std]

pub mod fb;

use core::ffi::{c_char, c_int, c_void};

// ---------------------------------------------------------------------------
// Raw FFI: символы из bin/lib/libpurec.a (см. src/libc/runtime.c, purec.h)
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn pc_syscall(number: u64, arg1: u64, arg2: u64, arg3: u64) -> i64;
    fn pc_strlen(text: *const c_char) -> u32;
    fn pc_write(text: *const c_char);
    fn pc_write_u64(value: u64);
    fn pc_write_i64(value: i64);
    fn pc_sleep(milliseconds: u32);
    fn pc_getpid() -> i32;
    fn pc_exec(path: *const c_char) -> i32;
    fn pc_exec_with_args(path: *const c_char, arguments: *const c_char) -> i32;
    fn pc_wait(pid: i32, status: *mut i32, nohang: bool) -> i32;
    fn pc_try_getchar() -> i32;
    fn pc_try_get_special() -> i32;
    fn pc_get_command_line(buffer: *mut c_char, capacity: u32) -> i32;
    fn pc_get_process_name(buffer: *mut c_char, capacity: u32) -> i32;
    fn pc_getenv(name: *const c_char, buffer: *mut c_char, capacity: u32) -> i32;
    fn pc_setenv(name: *const c_char, value: *const c_char) -> i32;
    fn pc_unsetenv(name: *const c_char) -> i32;
    fn pc_file_open(path: *const c_char) -> i32;
    fn pc_file_read(descriptor: i32, buffer: *mut c_void, capacity: u32) -> i32;
    fn pc_file_close(descriptor: i32) -> i32;
    fn pc_file_write(path: *const c_char, buffer: *const c_void, size: u32) -> i32;
    fn pc_file_create(path: *const c_char) -> i32;
    fn pc_directory_create(path: *const c_char) -> i32;
    fn pc_file_delete(path: *const c_char) -> i32;
    fn pc_console_clear();
    fn pc_console_disable();
    fn pc_display_clear(color: u32);
    fn pc_reboot();
    fn pc_shutdown();
    fn pc_exit(status: i32) -> !;
}

// Номера syscall (дублируют src/kernel/syscall/syscall.h, чтобы можно было
// бить напрямую через `syscall()` без C-обертки).
pub mod sys {
    pub const WRITE: u64 = 1;
    pub const CLEAR: u64 = 2;
    pub const SLEEP: u64 = 3;
    pub const GETPID: u64 = 39;
    pub const EXEC: u64 = 59;
    pub const EXIT: u64 = 60;
    pub const WAIT: u64 = 61;
}

/// Прямой syscall без C-обертки: `int $0x80` живет в `pc_syscall`.
#[inline]
pub unsafe fn syscall(number: u64, arg1: u64, arg2: u64, arg3: u64) -> i64 {
    unsafe { pc_syscall(number, arg1, arg2, arg3) }
}

// ---------------------------------------------------------------------------
// Безопасные обертки
// ---------------------------------------------------------------------------

/// Записать `&str` через `pc_write` (чанкование в NUL-терминированный буфер).
pub fn write(s: &str) {
    const CHUNK: usize = 120;
    let bytes = s.as_bytes();
    let mut buf = [0 as c_char; CHUNK + 1];
    let mut i = 0;
    while i < bytes.len() {
        let mut n = 0;
        while n < CHUNK && i + n < bytes.len() {
            buf[n] = bytes[i + n] as c_char;
            n += 1;
        }
        buf[n] = 0;
        unsafe { pc_write(buf.as_ptr()) };
        i += n;
    }
}

#[inline]
pub fn write_u64(value: u64) {
    unsafe { pc_write_u64(value) }
}

#[inline]
pub fn write_i64(value: i64) {
    unsafe { pc_write_i64(value) }
}

#[inline]
pub fn sleep_ms(ms: u32) {
    unsafe { pc_sleep(ms) }
}

#[inline]
pub fn getpid() -> i32 {
    unsafe { pc_getpid() }
}

#[inline]
pub fn exit(status: i32) -> ! {
    unsafe { pc_exit(status) }
}

#[inline]
pub fn reboot() -> ! {
    unsafe {
        pc_reboot();
        pc_exit(0);
    }
}

#[inline]
pub fn shutdown() -> ! {
    unsafe {
        pc_shutdown();
        pc_exit(0);
    }
}

/// `pc_write` для уже NUL-терминированной C-строки.
#[inline]
pub unsafe fn write_cstr(ptr: *const c_char) {
    unsafe { pc_write(ptr) }
}

pub fn strlen_of(ptr: *const c_char) -> u32 {
    unsafe { pc_strlen(ptr) }
}

/// Выполнить программу по абсолютному пути (`/bin/program/...`).
/// Возвращает pid или отрицательный код ошибки.
pub fn exec(path: &str) -> i32 {
    const MAX: usize = 128;
    let bytes = path.as_bytes();
    if bytes.len() >= MAX {
        return -1;
    }
    let mut buf = [0 as c_char; MAX];
    for (i, &b) in bytes.iter().enumerate() {
        buf[i] = b as c_char;
    }
    buf[bytes.len()] = 0;
    unsafe { pc_exec(buf.as_ptr()) }
}

/// То же + строка аргументов (как `pc_exec_with_args`).
pub fn exec_with_args(path: &str, args: &str) -> i32 {
    const MAX_P: usize = 128;
    const MAX_A: usize = 256;
    let pb = path.as_bytes();
    let ab = args.as_bytes();
    if pb.len() >= MAX_P || ab.len() >= MAX_A {
        return -1;
    }
    let mut pbuf = [0 as c_char; MAX_P];
    let mut abuf = [0 as c_char; MAX_A];
    for (i, &b) in pb.iter().enumerate() {
        pbuf[i] = b as c_char;
    }
    for (i, &b) in ab.iter().enumerate() {
        abuf[i] = b as c_char;
    }
    unsafe { pc_exec_with_args(pbuf.as_ptr(), abuf.as_ptr()) }
}

#[inline]
pub fn wait(pid: i32, status: Option<&mut i32>, nohang: bool) -> i32 {
    let ptr = status
        .map(|s| s as *mut i32)
        .unwrap_or(core::ptr::null_mut());
    unsafe { pc_wait(pid, ptr, nohang) }
}

#[inline]
pub fn try_getchar() -> i32 {
    unsafe { pc_try_getchar() }
}

#[inline]
pub fn try_get_special() -> i32 {
    unsafe { pc_try_get_special() }
}

/// Прочитать командную строку процесса в буфер. Возвращает длину или <0.
pub fn command_line(buf: &mut [u8]) -> i32 {
    if buf.is_empty() {
        return -1;
    }
    unsafe { pc_get_command_line(buf.as_mut_ptr() as *mut c_char, buf.len() as u32) }
}

/// Прочитать имя процесса в буфер. Возвращает 0 при успехе или <0.
pub fn process_name(buf: &mut [u8]) -> i32 {
    if buf.is_empty() {
        return -1;
    }
    unsafe { pc_get_process_name(buf.as_mut_ptr() as *mut c_char, buf.len() as u32) }
}

pub fn getenv(name: &str, buf: &mut [u8]) -> i32 {
    const MAX_N: usize = 40;
    let nb = name.as_bytes();
    if nb.len() >= MAX_N || buf.is_empty() {
        return -1;
    }
    let mut nbuf = [0 as c_char; MAX_N];
    for (i, &b) in nb.iter().enumerate() {
        nbuf[i] = b as c_char;
    }
    unsafe {
        pc_getenv(
            nbuf.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as u32,
        )
    }
}

pub fn setenv(name: &str, value: &str) -> i32 {
    const MAX_N: usize = 40;
    const MAX_V: usize = 132;
    let (nb, vb) = (name.as_bytes(), value.as_bytes());
    if nb.len() >= MAX_N || vb.len() >= MAX_V {
        return -1;
    }
    let mut nbuf = [0 as c_char; MAX_N];
    let mut vbuf = [0 as c_char; MAX_V];
    for (i, &b) in nb.iter().enumerate() {
        nbuf[i] = b as c_char;
    }
    for (i, &b) in vb.iter().enumerate() {
        vbuf[i] = b as c_char;
    }
    unsafe { pc_setenv(nbuf.as_ptr(), vbuf.as_ptr()) }
}

pub fn unsetenv(name: &str) -> i32 {
    const MAX_N: usize = 40;
    let nb = name.as_bytes();
    if nb.len() >= MAX_N {
        return -1;
    }
    let mut nbuf = [0 as c_char; MAX_N];
    for (i, &b) in nb.iter().enumerate() {
        nbuf[i] = b as c_char;
    }
    unsafe { pc_unsetenv(nbuf.as_ptr()) }
}

// --- файлы (тонкие обертки, пути как &str) ---

fn path_buf<const N: usize>(path: &str) -> Option<[c_char; N]> {
    if path.as_bytes().len() >= N {
        return None;
    }
    let mut buf = [0 as c_char; N];
    for (i, &b) in path.as_bytes().iter().enumerate() {
        buf[i] = b as c_char;
    }
    Some(buf)
}

pub fn file_open(path: &str) -> i32 {
    let Some(buf) = path_buf::<128>(path) else {
        return -1;
    };
    unsafe { pc_file_open(buf.as_ptr()) }
}

pub fn file_read(fd: i32, buf: &mut [u8]) -> i32 {
    if buf.is_empty() || buf.len() > u32::MAX as usize {
        return -1;
    }
    unsafe { pc_file_read(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as u32) }
}

pub fn file_close(fd: i32) -> i32 {
    unsafe { pc_file_close(fd) }
}

pub fn file_write(path: &str, data: &[u8]) -> i32 {
    let Some(p) = path_buf::<128>(path) else {
        return -1;
    };
    if data.len() > u32::MAX as usize {
        return -1;
    }
    unsafe { pc_file_write(p.as_ptr(), data.as_ptr() as *const c_void, data.len() as u32) }
}

pub fn file_create(path: &str) -> i32 {
    let Some(p) = path_buf::<128>(path) else {
        return -1;
    };
    unsafe { pc_file_create(p.as_ptr()) }
}

pub fn dir_create(path: &str) -> i32 {
    let Some(p) = path_buf::<128>(path) else {
        return -1;
    };
    unsafe { pc_directory_create(p.as_ptr()) }
}

pub fn file_delete(path: &str) -> i32 {
    let Some(p) = path_buf::<128>(path) else {
        return -1;
    };
    unsafe { pc_file_delete(p.as_ptr()) }
}

pub fn console_clear() {
    unsafe { pc_console_clear() }
}

pub fn console_disable() {
    unsafe { pc_console_disable() }
}

pub fn display_clear(color: u32) {
    unsafe { pc_display_clear(color) }
}

// ---------------------------------------------------------------------------
// Panic handler: один на бинарь, приезжает из этого крейта.
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    write("purec panic");
    if let Some(loc) = info.location() {
        write(" at ");
        write(loc.file());
        write(":");
        write_u64(loc.line() as u64);
    }
    write("\n");
    exit(1);
}

// c_int нужен, чтобы сигнатура extern-блока не ругалась на неиспользуемый импорт
// в конфигурациях без std (ядро линтует иначе).
#[allow(dead_code)]
fn _use_c_int(_: c_int) {}

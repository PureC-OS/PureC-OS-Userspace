unsafe extern "C" {
    fn pc_display_get_info(info: *mut DisplayInfo) -> bool;
    fn pc_mouse_get(state: *mut MouseState) -> bool;
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub size_bytes: u64,
    pub bpp: u8,
    pub available: bool,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub buttons: u8,
    pub has_data: bool,
    pub wheel: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct TimeInfo {
    unix_seconds: i64,
    millis: u32,
    tz_minutes: i32,
    uptime_ms: u64,
    source: u32,
    reserved: u32,
}
pub fn display_info() -> Option<DisplayInfo> {
    let mut info = DisplayInfo {
        width: 0,
        height: 0,
        pitch: 0,
        size_bytes: 0,
        bpp: 0,
        available: false,
    };
    if unsafe { pc_display_get_info(&mut info) } {
        Some(info)
    } else {
        None
    }
}
pub fn mouse_get() -> Option<MouseState> {
    let mut state = MouseState::default();
    if unsafe { pc_mouse_get(&mut state) } {
        Some(state)
    } else {
        None
    }
}

const SYS_TIME_GET: u64 = 319;
const SYS_GUI_WINDOW_REGISTER: u64 = 251;
const SYS_GUI_WINDOW_UPDATE: u64 = 252;
const SYS_GUI_WINDOW_UNREGISTER: u64 = 253;
const SYS_FB_BLIT: u64 = 322;
const SYS_GUI_WINDOW_FLAGS: u64 = 351;
const SYS_DESKTOP_CLAIM: u64 = 346;
const SYS_DESKTOP_BLIT: u64 = 347;
const SYS_DESKTOP_WAIT: u64 = 348;
const SYS_DESKTOP_WM: u64 = 349;

pub const WAIT_MOUSE: u32 = 1;
pub const WAIT_KEY: u32 = 2;
pub const WAIT_REDRAW: u32 = 4;

const WM_POINTER: u64 = 1;
const WM_HAS_FOCUS: u64 = 2;
const WM_REQUEST_REPAINT: u64 = 3;
const WM_FOCUSED_PID: u64 = 4;
const WM_SET_WORK_AREA: u64 = 5;
const WM_TASKS: u64 = 6;
const WM_ACTIVATE: u64 = 7;
const WM_COMMAND: u64 = 8;

pub const CMD_MINIMIZE: u32 = 1;
pub const CMD_RESTORE: u32 = 2;

pub const WINDOW_FLAG_POPUP: u32 = 4;
pub const TASK_MINIMIZED: u32 = 1;
pub const TASK_FOCUSED: u32 = 2;

pub fn uptime_ms() -> u64 {
    let mut info = TimeInfo {
        unix_seconds: 0,
        millis: 0,
        tz_minutes: 0,
        uptime_ms: 0,
        source: 0,
        reserved: 0,
    };
    let result = unsafe { super::syscall(SYS_TIME_GET, &mut info as *mut TimeInfo as u64, 0, 0) };
    if result < 0 {
        0
    } else {
        info.uptime_ms
    }
}

#[repr(C)]
struct BlitRequest {
    pixels: u64,
    stride: u32,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    flags: u32,
}

pub fn desktop_claim() -> bool {
    unsafe { super::syscall(SYS_DESKTOP_CLAIM, 0, 0, 0) == 0 }
}

pub fn desktop_blit(pixels: *const u32, stride: u32, x: u32, y: u32, w: u32, h: u32) -> bool {
    let request = BlitRequest {
        pixels: pixels as u64,
        stride,
        width: w,
        height: h,
        x: x as i32,
        y: y as i32,
        flags: 0,
    };
    unsafe {
        super::syscall(
            SYS_DESKTOP_BLIT,
            &request as *const BlitRequest as u64,
            0,
            0,
        ) == 0
    }
}

pub fn desktop_wait(timeout_ms: u32) -> u32 {
    let result = unsafe { super::syscall(SYS_DESKTOP_WAIT, timeout_ms as u64, 0, 0) };
    if result < 0 {
        0
    } else {
        result as u32
    }
}

pub fn wm_pointer(x: i32, y: i32, buttons: u8) -> (bool, bool) {
    let packed = (x as u32 as u64) | ((y as u32 as u64) << 32);
    let result = unsafe { super::syscall(SYS_DESKTOP_WM, WM_POINTER, packed, buttons as u64) };
    if result < 0 {
        return (false, false);
    }
    ((result & 1) != 0, (result & 2) != 0)
}

pub fn wm_has_focus() -> bool {
    unsafe { super::syscall(SYS_DESKTOP_WM, WM_HAS_FOCUS, 0, 0) > 0 }
}

pub fn wm_request_repaint(excluded_pid: u32) {
    unsafe {
        super::syscall(SYS_DESKTOP_WM, WM_REQUEST_REPAINT, excluded_pid as u64, 0);
    }
}

pub fn wm_set_work_area(x: u32, y: u32, w: u32, h: u32) {
    let origin = (x as u64) | ((y as u64) << 32);
    let size = (w as u64) | ((h as u64) << 32);
    unsafe {
        super::syscall(SYS_DESKTOP_WM, WM_SET_WORK_AREA, origin, size);
    }
}

pub fn wm_focused_pid() -> u32 {
    let result = unsafe { super::syscall(SYS_DESKTOP_WM, WM_FOCUSED_PID, 0, 0) };
    if result < 0 {
        0
    } else {
        result as u32
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub pid: u32,
    pub flags: u32,
    pub name: [u8; 32],
}

impl TaskInfo {
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            flags: 0,
            name: [0; 32],
        }
    }

    pub fn name_str(&self) -> &str {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..len]).unwrap_or("")
    }
}

pub fn wm_tasks(out: &mut [TaskInfo]) -> usize {
    if out.is_empty() {
        return 0;
    }
    let result = unsafe {
        super::syscall(
            SYS_DESKTOP_WM,
            WM_TASKS,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    };
    if result < 0 {
        0
    } else {
        (result as usize).min(out.len())
    }
}

pub fn wm_activate(pid: u32) -> bool {
    unsafe { super::syscall(SYS_DESKTOP_WM, WM_ACTIVATE, pid as u64, 0) == 0 }
}

#[repr(C)]
struct WindowRequest {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

pub fn window_register(x: u32, y: u32, w: u32, h: u32) -> bool {
    let request = WindowRequest {
        x,
        y,
        width: w,
        height: h,
    };
    unsafe {
        super::syscall(
            SYS_GUI_WINDOW_REGISTER,
            &request as *const WindowRequest as u64,
            0,
            0,
        ) == 0
    }
}

pub fn window_update(x: u32, y: u32, w: u32, h: u32) -> bool {
    let request = WindowRequest {
        x,
        y,
        width: w,
        height: h,
    };
    unsafe {
        super::syscall(
            SYS_GUI_WINDOW_UPDATE,
            &request as *const WindowRequest as u64,
            0,
            0,
        ) == 0
    }
}

pub fn window_unregister() {
    unsafe {
        super::syscall(SYS_GUI_WINDOW_UNREGISTER, 0, 0, 0);
    }
}

pub fn window_set_flags(flags: u32) -> bool {
    unsafe { super::syscall(SYS_GUI_WINDOW_FLAGS, flags as u64, 0, 0) == 0 }
}

pub fn window_blit(pixels: *const u32, stride: u32, x: u32, y: u32, w: u32, h: u32) -> bool {
    let request = BlitRequest {
        pixels: pixels as u64,
        stride,
        width: w,
        height: h,
        x: x as i32,
        y: y as i32,
        flags: 0,
    };
    unsafe {
        super::syscall(SYS_FB_BLIT, &request as *const BlitRequest as u64, 0, 0) == 0
    }
}

pub fn wm_command(pid: u32, command: u32) -> bool {
    unsafe { super::syscall(SYS_DESKTOP_WM, WM_COMMAND, pid as u64, command as u64) == 0 }
}

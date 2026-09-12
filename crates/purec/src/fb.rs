use core::ffi::c_char;
unsafe extern "C" {
    fn pc_draw_rect(x: u32, y: u32, w: u32, h: u32, color: u32);
    fn pc_draw_text(x: u32, y: u32, text: *const c_char, fg: u32, bg: u32);
    fn pc_draw_text_sized(
        x: u32,
        y: u32,
        text: *const c_char,
        fg: u32,
        bg: u32,
        size: u32,
    );
    fn pc_display_get_info(info: *mut DisplayInfo) -> bool;
    fn pc_display_begin_update();
    fn pc_display_end_update();
    fn pc_mouse_get(state: *mut MouseState) -> bool;
    fn pc_cpu_info(info: *mut CpuInfo) -> bool;
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
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpuInfo {
    pub name: [c_char; 49],
    pub logical_processors: u32,
    pub usage_percent: u32,
    pub frequency_hz: u64,
    pub uptime_ms: u64,
}
impl CpuInfo {
    const fn zero() -> Self {
        Self {
            name: [0; 49],
            logical_processors: 0,
            usage_percent: 0,
            frequency_hz: 0,
            uptime_ms: 0,
        }
    }
}
fn with_cstr<const N: usize>(s: &str, f: impl FnOnce(*const c_char)) {
    let mut buf = [0 as c_char; N];
    let bytes = s.as_bytes();
    let n = bytes.len().min(N - 1);
    let mut i = 0;
    while i < n {
        buf[i] = bytes[i] as c_char;
        i += 1;
    }
    buf[n] = 0;
    f(buf.as_ptr());
}
#[inline]
pub fn draw_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    unsafe { pc_draw_rect(x, y, w, h, color) }
}
pub fn draw_text(x: u32, y: u32, text: &str, fg: u32, bg: u32) {
    with_cstr::<256>(text, |ptr| unsafe { pc_draw_text(x, y, ptr, fg, bg) });
}
pub fn draw_text_sized(x: u32, y: u32, text: &str, fg: u32, bg: u32, size: u32) {
    with_cstr::<256>(text, |ptr| unsafe {
        pc_draw_text_sized(x, y, ptr, fg, bg, size)
    });
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
#[inline]
pub fn display_begin_update() {
    unsafe { pc_display_begin_update() }
}
#[inline]
pub fn display_end_update() {
    unsafe { pc_display_end_update() }
}
pub fn mouse_get() -> Option<MouseState> {
    let mut state = MouseState::default();
    if unsafe { pc_mouse_get(&mut state) } {
        Some(state)
    } else {
        None
    }
}
pub fn uptime_ms() -> u64 {
    let mut info = CpuInfo::zero();
    if unsafe { pc_cpu_info(&mut info) } {
        info.uptime_ms
    } else {
        0
    }
}

const SYS_WM_HANDLE_POINTER: u64 = 281;
const SYS_WM_HAS_FOCUS: u64 = 282;
const SYS_KLOG_SET_SCREEN: u64 = 283;
const SYS_GOP_BEGIN_COMPOSE: u64 = 284;
const SYS_GOP_END_COMPOSE: u64 = 285;
const SYS_WM_REQUEST_REPAINT: u64 = 286;
const SYS_DESKTOP_REDRAW_TAKE: u64 = 287;

#[repr(C)]
struct WmPointerRequest {
    x: i32,
    y: i32,
    pressed: u32,
}

pub fn wm_handle_pointer(x: i32, y: i32, pressed: bool) -> (bool, bool) {
    let request = WmPointerRequest {
        x,
        y,
        pressed: pressed as u32,
    };
    let result = unsafe {
        super::syscall(
            SYS_WM_HANDLE_POINTER,
            &request as *const WmPointerRequest as u64,
            0,
            0,
        )
    };
    if result < 0 {
        return (false, false);
    }
    ((result & 1) != 0, (result & 2) != 0)
}

pub fn wm_has_focus() -> bool {
    unsafe { super::syscall(SYS_WM_HAS_FOCUS, 0, 0, 0) != 0 }
}

pub fn klog_set_screen(enabled: bool) {
    unsafe {
        super::syscall(SYS_KLOG_SET_SCREEN, enabled as u64, 0, 0);
    }
}

pub fn compose_begin() {
    unsafe {
        super::syscall(SYS_GOP_BEGIN_COMPOSE, 0, 0, 0);
    }
}

pub fn compose_end() {
    unsafe {
        super::syscall(SYS_GOP_END_COMPOSE, 0, 0, 0);
    }
}

pub fn wm_request_repaint(excluded_pid: u32) {
    unsafe {
        super::syscall(SYS_WM_REQUEST_REPAINT, excluded_pid as u64, 0, 0);
    }
}

pub fn desktop_redraw_take() -> bool {
    unsafe { super::syscall(SYS_DESKTOP_REDRAW_TAKE, 0, 0, 0) != 0 }
}

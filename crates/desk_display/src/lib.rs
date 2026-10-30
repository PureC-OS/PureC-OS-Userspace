#![no_std]
mod font;
use purec::fb;

const MIN_SIZE: u32 = 8;
const MAX_SIZE: u32 = 48;

struct Canvas {
    buf: *mut u32,
    shown: *mut u32,
    force: bool,
    capacity: usize,
    width: u32,
    height: u32,
    dirty_x0: u32,
    dirty_y0: u32,
    dirty_x1: u32,
    dirty_y1: u32,
    depth: u32,
    face: u32,
    origin_x: u32,
    origin_y: u32,
}

static mut CANVAS: Canvas = Canvas {
    buf: core::ptr::null_mut(),
    shown: core::ptr::null_mut(),
    force: true,
    capacity: 0,
    width: 0,
    height: 0,
    dirty_x0: 0,
    dirty_y0: 0,
    dirty_x1: 0,
    dirty_y1: 0,
    depth: 0,
    face: 1,
    origin_x: 0,
    origin_y: 0,
};

static mut POPUP: Canvas = Canvas {
    buf: core::ptr::null_mut(),
    shown: core::ptr::null_mut(),
    force: true,
    capacity: 0,
    width: 0,
    height: 0,
    dirty_x0: 0,
    dirty_y0: 0,
    dirty_x1: 0,
    dirty_y1: 0,
    depth: 0,
    face: 1,
    origin_x: 0,
    origin_y: 0,
};
static mut POPUP_ACTIVE: bool = false;
static mut POPUP_OPEN: bool = false;

fn main_canvas() -> &'static mut Canvas {
    unsafe { &mut *core::ptr::addr_of_mut!(CANVAS) }
}

fn canvas() -> &'static mut Canvas {
    unsafe {
        if *core::ptr::addr_of!(POPUP_ACTIVE) {
            &mut *core::ptr::addr_of_mut!(POPUP)
        } else {
            &mut *core::ptr::addr_of_mut!(CANVAS)
        }
    }
}

impl Canvas {
    fn mark(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) {
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        if self.dirty_x0 >= self.dirty_x1 {
            self.dirty_x0 = x0;
            self.dirty_y0 = y0;
            self.dirty_x1 = x1;
            self.dirty_y1 = y1;
            return;
        }
        self.dirty_x0 = self.dirty_x0.min(x0);
        self.dirty_y0 = self.dirty_y0.min(y0);
        self.dirty_x1 = self.dirty_x1.max(x1);
        self.dirty_y1 = self.dirty_y1.max(y1);
    }

    fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        if self.buf.is_null() {
            return;
        }
        let lx = x as i64 - self.origin_x as i64;
        let ly = y as i64 - self.origin_y as i64;
        let x0 = lx.max(0);
        let y0 = ly.max(0);
        let x1 = (lx + w as i64).min(self.width as i64);
        let y1 = (ly + h as i64).min(self.height as i64);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let stride = self.width as usize;
        for row in y0..y1 {
            let start = row as usize * stride + x0 as usize;
            let line = unsafe {
                core::slice::from_raw_parts_mut(self.buf.add(start), (x1 - x0) as usize)
            };
            line.fill(color);
        }
        self.mark(x0 as u32, y0 as u32, x1 as u32, y1 as u32);
    }

    fn glyph_bits(&self, ch: u8, row: u32) -> u8 {
        let ch = (ch & 0x7F) as usize;
        match self.face {
            2 => {
                let bits = font::CLASSIC[ch][(row & 7) as usize];
                bits | (bits >> 1)
            }
            1 => {
                if row >= 7 {
                    0
                } else {
                    font::CLEAN[ch][row as usize] << 2
                }
            }
            _ => font::CLASSIC[ch][(row & 7) as usize],
        }
    }

    fn text(&mut self, x: u32, y: u32, text: &str, fg: u32, bg: u32, size: u32) {
        let size = size.clamp(MIN_SIZE, MAX_SIZE);
        let bytes = text.as_bytes();
        if bytes.is_empty() {
            return;
        }
        self.fill(x, y, bytes.len() as u32 * size, size, bg);
        for (index, &byte) in bytes.iter().enumerate() {
            if byte >= 128 {
                continue;
            }
            let gx = x + index as u32 * size;
            for row in 0..8u32 {
                let bits = self.glyph_bits(byte, row);
                if bits == 0 {
                    continue;
                }
                let y0 = y + row * size / 8;
                let y1 = y + (row + 1) * size / 8;
                if y1 <= y0 {
                    continue;
                }
                let mut run_start = 0u32;
                let mut in_run = false;
                for col in 0..=8u32 {
                    let set = col < 8 && (bits & (1u8 << (7 - col))) != 0;
                    if set && !in_run {
                        run_start = col;
                        in_run = true;
                    }
                    if (!set || col == 8) && in_run {
                        let x0 = gx + run_start * size / 8;
                        let x1 = gx + col * size / 8;
                        if x1 > x0 {
                            self.fill(x0, y0, x1 - x0, y1 - y0, fg);
                        }
                        in_run = false;
                    }
                }
            }
        }
    }
}

fn pages(bytes: usize) -> u64 {
    (bytes.div_ceil(4096) * 4096) as u64
}

pub fn init() -> bool {
    unsafe {
        *core::ptr::addr_of_mut!(POPUP_ACTIVE) = false;
    }
    let info = match fb::display_info() {
        Some(info) if info.available && info.width != 0 && info.height != 0 => info,
        _ => return false,
    };
    let c = main_canvas();
    let need = info.width as usize * info.height as usize;
    if c.buf.is_null() || need > c.capacity {
        let bytes = pages(need * 4);
        let first = purec::heap_grow(bytes);
        let second = purec::heap_grow(bytes);
        match (first, second) {
            (Some(a), Some(b)) => {
                c.buf = a as *mut u32;
                c.shown = b as *mut u32;
                c.capacity = need;
            }
            _ => return false,
        }
    }
    c.force = true;
    c.width = info.width;
    c.height = info.height;
    c.dirty_x0 = 0;
    c.dirty_y0 = 0;
    c.dirty_x1 = 0;
    c.dirty_y1 = 0;
    true
}

pub fn refresh_mode() -> bool {
    let c = main_canvas();
    let (w, h) = (c.width, c.height);
    if !init() {
        return false;
    }
    c.width != w || c.height != h
}

pub fn is_available() -> bool {
    !main_canvas().buf.is_null()
}
pub fn width() -> u32 {
    main_canvas().width
}
pub fn height() -> u32 {
    main_canvas().height
}
pub fn pitch() -> u32 {
    main_canvas().width
}
pub fn buffer() -> *mut u32 {
    main_canvas().buf
}
pub fn set_font_face(face: u32) {
    canvas().face = face;
}
pub fn draw_text(x: u32, y: u32, text: &str, fg: u32, bg: u32) {
    canvas().text(x, y, text, fg, bg, MIN_SIZE);
}
pub fn draw_text_sized(x: u32, y: u32, text: &str, fg: u32, bg: u32, size: u32) {
    canvas().text(x, y, text, fg, bg, size);
}
pub fn draw_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    canvas().fill(x, y, w, h, color);
}
pub fn clear(color: u32) {
    let c = canvas();
    let (ox, oy, w, h) = (c.origin_x, c.origin_y, c.width, c.height);
    c.fill(ox, oy, w, h, color);
}
pub fn mark_dirty(x: u32, y: u32, w: u32, h: u32) {
    let c = main_canvas();
    let x1 = x.saturating_add(w).min(c.width);
    let y1 = y.saturating_add(h).min(c.height);
    c.mark(x, y, x1, y1);
}
pub fn mark_all_dirty() {
    let c = main_canvas();
    let (w, h) = (c.width, c.height);
    c.mark(0, 0, w, h);
}
pub fn begin_update() {
    canvas().depth += 1;
}
pub fn end_update() {
    let c = canvas();
    if c.depth > 0 {
        c.depth -= 1;
    }
    if c.depth == 0 {
        flush();
    }
}
pub fn invalidate() {
    let c = main_canvas();
    c.force = true;
    let (w, h) = (c.width, c.height);
    c.mark(0, 0, w, h);
}

pub fn flush() {
    let c = main_canvas();
    if c.buf.is_null() || c.dirty_x0 >= c.dirty_x1 || c.dirty_y0 >= c.dirty_y1 {
        return;
    }
    let stride = c.width as usize;
    let (mut x0, mut y0, mut x1, mut y1) = (c.dirty_x0, c.dirty_y0, c.dirty_x1, c.dirty_y1);
    c.dirty_x0 = 0;
    c.dirty_y0 = 0;
    c.dirty_x1 = 0;
    c.dirty_y1 = 0;
    if c.force {
        c.force = false;
        x0 = 0;
        y0 = 0;
        x1 = c.width;
        y1 = c.height;
    } else {
        let mut min_x = u32::MAX;
        let mut max_x = 0u32;
        let mut min_y = u32::MAX;
        let mut max_y = 0u32;
        for row in y0..y1 {
            let base = row as usize * stride;
            let new = unsafe {
                core::slice::from_raw_parts(c.buf.add(base + x0 as usize), (x1 - x0) as usize)
            };
            let old = unsafe {
                core::slice::from_raw_parts(c.shown.add(base + x0 as usize), (x1 - x0) as usize)
            };
            if new == old {
                continue;
            }
            let mut left = 0usize;
            while new[left] == old[left] {
                left += 1;
            }
            let mut right = new.len() - 1;
            while new[right] == old[right] {
                right -= 1;
            }
            min_x = min_x.min(x0 + left as u32);
            max_x = max_x.max(x0 + right as u32 + 1);
            min_y = min_y.min(row);
            max_y = row + 1;
        }
        if min_x == u32::MAX {
            return;
        }
        x0 = min_x;
        x1 = max_x;
        y0 = min_y;
        y1 = max_y;
    }
    for row in y0..y1 {
        let base = row as usize * stride + x0 as usize;
        unsafe {
            core::ptr::copy_nonoverlapping(c.buf.add(base), c.shown.add(base), (x1 - x0) as usize);
        }
    }
    let origin = unsafe { c.buf.add(y0 as usize * stride + x0 as usize) };
    fb::desktop_blit(origin, c.width, x0, y0, x1 - x0, y1 - y0);
}

const POPUP_MIN_PIXELS: usize = 512 * 640;
static mut POPUP_RECT: (u32, u32, u32, u32) = (0, 0, 0, 0);

pub fn popup_begin(x: u32, y: u32, w: u32, h: u32) -> bool {
    if w == 0 || h == 0 {
        return false;
    }
    let p = unsafe { &mut *core::ptr::addr_of_mut!(POPUP) };
    let need = w as usize * h as usize;
    if p.buf.is_null() || need > p.capacity {
        let pixels = need.max(POPUP_MIN_PIXELS);
        match purec::heap_grow(pages(pixels * 4)) {
            Some(ptr) => {
                p.buf = ptr as *mut u32;
                p.capacity = pixels;
            }
            None => return false,
        }
    }
    p.width = w;
    p.height = h;
    p.origin_x = x;
    p.origin_y = y;
    p.face = main_canvas().face;
    p.dirty_x0 = 0;
    p.dirty_y0 = 0;
    p.dirty_x1 = 0;
    p.dirty_y1 = 0;
    unsafe {
        *core::ptr::addr_of_mut!(POPUP_RECT) = (x, y, w, h);
        *core::ptr::addr_of_mut!(POPUP_ACTIVE) = true;
    }
    true
}

pub fn popup_end() {
    let (x, y, w, h) = unsafe { *core::ptr::addr_of!(POPUP_RECT) };
    unsafe {
        *core::ptr::addr_of_mut!(POPUP_ACTIVE) = false;
    }
    let p = unsafe { &mut *core::ptr::addr_of_mut!(POPUP) };
    if p.buf.is_null() {
        return;
    }
    let open = unsafe { *core::ptr::addr_of!(POPUP_OPEN) };
    if !open {
        if !fb::window_register(x, y, w, h) {
            return;
        }
        fb::window_set_flags(fb::WINDOW_FLAG_POPUP);
        unsafe {
            *core::ptr::addr_of_mut!(POPUP_OPEN) = true;
        }
    } else {
        fb::window_update(x, y, w, h);
    }
    fb::window_blit(p.buf, w, x, y, w, h);
}

pub fn popup_close() {
    unsafe {
        if *core::ptr::addr_of!(POPUP_OPEN) {
            fb::window_unregister();
            *core::ptr::addr_of_mut!(POPUP_OPEN) = false;
        }
    }
}

pub fn popup_is_open() -> bool {
    unsafe { *core::ptr::addr_of!(POPUP_OPEN) }
}

#![no_std]

use purec::fb::uptime_ms;
use purec::{file_close, file_open, file_read, syscall};

const SYS_SET_FONT_FACE: u64 = 107;
const CONFIG_PATH: &str = "/config/appear.ini";
const POLL_MS: u64 = 500;

#[derive(Clone, Copy)]
pub struct Colors {
    pub desktop: u32,
    pub window: u32,
    pub titlebar: u32,
    pub border: u32,
    pub text: u32,
    pub muted_text: u32,
    pub accent: u32,
    pub danger: u32,
    pub shadow: u32,
}

struct Theme {
    name: &'static str,
    colors: Colors,
}

const THEMES: [Theme; 5] = [
    Theme {
        name: "catppuccin-dark",
        colors: Colors {
            desktop: 0x181825,
            window: 0x1E1E2E,
            titlebar: 0x313244,
            border: 0x45475A,
            text: 0xCDD6F4,
            muted_text: 0x9399B2,
            accent: 0x89B4FA,
            danger: 0xF38BA8,
            shadow: 0x11111B,
        },
    },
    Theme {
        name: "nord",
        colors: Colors {
            desktop: 0x2E3440,
            window: 0x3B4252,
            titlebar: 0x434C5E,
            border: 0x4C566A,
            text: 0xECEFF4,
            muted_text: 0x9AA0B0,
            accent: 0x88C0D0,
            danger: 0xBF616A,
            shadow: 0x1A1C24,
        },
    },
    Theme {
        name: "dracula",
        colors: Colors {
            desktop: 0x282A36,
            window: 0x2E3247,
            titlebar: 0x44475A,
            border: 0x6272A4,
            text: 0xF8F8F2,
            muted_text: 0x9AA0B2,
            accent: 0xBD93F9,
            danger: 0xFF5555,
            shadow: 0x1A1B26,
        },
    },
    Theme {
        name: "light",
        colors: Colors {
            desktop: 0xE6E9EF,
            window: 0xEFF1F5,
            titlebar: 0xDCE0E8,
            border: 0xBCC0CC,
            text: 0x4C4F69,
            muted_text: 0x8C8FA1,
            accent: 0x1E66F5,
            danger: 0xD20F39,
            shadow: 0x9CA0B0,
        },
    },
    Theme {
        name: "tokyo-night",
        colors: Colors {
            desktop: 0x1A1B26,
            window: 0x24283B,
            titlebar: 0x292E42,
            border: 0x565F89,
            text: 0xC0CAF5,
            muted_text: 0x9AA5CE,
            accent: 0x7AA2F7,
            danger: 0xF7768E,
            shadow: 0x101014,
        },
    },
];

const THEME_CAP: usize = 32;
const WALLPAPER_CAP: usize = 128;
const FONT_CAP: usize = 48;

pub struct Personal {
    pub theme: [u8; THEME_CAP],
    pub theme_len: usize,
    pub wallpaper: [u8; WALLPAPER_CAP],
    pub wallpaper_len: usize,
    pub font: [u8; FONT_CAP],
    pub font_len: usize,
    pub font_size: u32,
}

static mut CURRENT: Personal = Personal {
    theme: [0; THEME_CAP],
    theme_len: 0,
    wallpaper: [0; WALLPAPER_CAP],
    wallpaper_len: 0,
    font: [0; FONT_CAP],
    font_len: 0,
    font_size: 8,
};
static mut HAS_CURRENT: bool = false;
static mut HAD_CONFIG: bool = false;
static mut LAST_POLL_MS: u64 = 0;

fn set_str(buf: &mut [u8], len: &mut usize, src: &[u8]) {
    let n = src.len().min(buf.len() - 1);
    buf[..n].copy_from_slice(&src[..n]);
    *len = n;
}

fn theme_str() -> &'static str {
    unsafe {
        let cur = &*core::ptr::addr_of!(CURRENT);
        core::str::from_utf8(&cur.theme[..cur.theme_len]).unwrap_or("catppuccin-dark")
    }
}

pub fn theme_colors() -> Colors {
    let name = theme_str();
    for theme in THEMES {
        if theme.name == name {
            return theme.colors;
        }
    }
    THEMES[0].colors
}

pub fn font_face() -> u32 {
    unsafe {
        let cur = &*core::ptr::addr_of!(CURRENT);
        let font = core::str::from_utf8(&cur.font[..cur.font_len]).unwrap_or("clean");
        match font {
            "classic" => 0,
            "bold" => 2,
            _ => 1,
        }
    }
}

pub fn font_size_clamped(size: u32) -> u32 {
    size.clamp(8, 24)
}

pub fn label_size() -> u32 {
    let size = unsafe { (*core::ptr::addr_of!(CURRENT)).font_size };
    let clamped = font_size_clamped(size);
    clamped.min(12)
}

fn parse_u32(text: &[u8]) -> u32 {
    let mut value: u32 = 0;
    for &ch in text {
        if !ch.is_ascii_digit() {
            break;
        }
        let digit = (ch - b'0') as u32;
        if value > (u32::MAX - digit) / 10 {
            return u32::MAX;
        }
        value = value * 10 + digit;
    }
    value
}

fn load_into(p: &mut Personal) {
    set_str(&mut p.theme, &mut p.theme_len, b"catppuccin-dark");
    p.wallpaper_len = 0;
    set_str(&mut p.font, &mut p.font_len, b"clean");
    p.font_size = 8;

    let fd = file_open(CONFIG_PATH);
    if fd < 0 {
        unsafe {
            *core::ptr::addr_of_mut!(HAD_CONFIG) = false;
        }
        return;
    }
    let mut buffer = [0u8; 512];
    let mut total = 0usize;
    let end = buffer.len() - 1;
    loop {
        if total >= end {
            break;
        }
        let n = file_read(fd, &mut buffer[total..end]);
        if n <= 0 {
            break;
        }
        total += n as usize;
    }
    file_close(fd);
    unsafe {
        *core::ptr::addr_of_mut!(HAD_CONFIG) = true;
    }
    if total == 0 {
        return;
    }
    let mut start = 0usize;
    while start < total {
        let mut end = start;
        while end < total && buffer[end] != b'\n' && buffer[end] != b'\r' {
            end += 1;
        }
        let line = &buffer[start..end];
        if let Some(rest) = line.strip_prefix(b"theme=") {
            set_str(&mut p.theme, &mut p.theme_len, rest);
        } else if let Some(rest) = line.strip_prefix(b"wallpaper=") {
            set_str(&mut p.wallpaper, &mut p.wallpaper_len, rest);
        } else if let Some(rest) = line.strip_prefix(b"font=") {
            set_str(&mut p.font, &mut p.font_len, rest);
        } else if let Some(rest) = line.strip_prefix(b"font_size=") {
            p.font_size = font_size_clamped(parse_u32(rest));
        }
        if end >= total {
            break;
        }
        start = end + 1;
        while start < total && (buffer[start] == b'\n' || buffer[start] == b'\r') {
            start += 1;
        }
    }
    if p.font_size == 0 {
        p.font_size = 8;
    }
}

fn same(a: &Personal, b: &Personal) -> bool {
    a.theme[..a.theme_len] == b.theme[..b.theme_len]
        && a.wallpaper[..a.wallpaper_len] == b.wallpaper[..b.wallpaper_len]
        && a.font[..a.font_len] == b.font[..b.font_len]
        && a.font_size == b.font_size
}

fn apply() {
    unsafe {
        syscall(SYS_SET_FONT_FACE, font_face() as u64, 0, 0);
    }
}

pub fn poll() -> bool {
    let now = uptime_ms();
    unsafe {
        if *core::ptr::addr_of!(HAS_CURRENT) && now - *core::ptr::addr_of!(LAST_POLL_MS) < POLL_MS {
            return false;
        }
        *core::ptr::addr_of_mut!(LAST_POLL_MS) = now;
    }
    let mut next = Personal {
        theme: [0; THEME_CAP],
        theme_len: 0,
        wallpaper: [0; WALLPAPER_CAP],
        wallpaper_len: 0,
        font: [0; FONT_CAP],
        font_len: 0,
        font_size: 8,
    };
    load_into(&mut next);
    unsafe {
        let has = *core::ptr::addr_of!(HAS_CURRENT);
        if has && same(&next, &*core::ptr::addr_of!(CURRENT)) {
            return false;
        }
        let first = !has;
        *core::ptr::addr_of_mut!(CURRENT) = next;
        *core::ptr::addr_of_mut!(HAS_CURRENT) = true;
        apply();
        !first
    }
}

pub fn current_colors() -> Colors {
    unsafe {
        if !*core::ptr::addr_of!(HAS_CURRENT) {
            let mut next = Personal {
                theme: [0; THEME_CAP],
                theme_len: 0,
                wallpaper: [0; WALLPAPER_CAP],
                wallpaper_len: 0,
                font: [0; FONT_CAP],
                font_len: 0,
                font_size: 8,
            };
            load_into(&mut next);
            *core::ptr::addr_of_mut!(CURRENT) = next;
            *core::ptr::addr_of_mut!(HAS_CURRENT) = true;
            apply();
        }
    }
    theme_colors()
}

pub fn font_size() -> u32 {
    unsafe { (*core::ptr::addr_of!(CURRENT)).font_size }
}

pub fn wallpaper_path_copy(buf: &mut [u8]) -> usize {
    unsafe {
        let cur = &*core::ptr::addr_of!(CURRENT);
        let n = cur.wallpaper_len.min(buf.len());
        buf[..n].copy_from_slice(&cur.wallpaper[..n]);
        n
    }
}

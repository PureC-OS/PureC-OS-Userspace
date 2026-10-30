use purec::{file_close, file_open, file_read, syscall};

pub const MAX_ENTRIES: usize = 24;
const SCAN_DIR: &[u8] = b"/usr/share/applications/\0";
const SYS_DIR_LIST_LONG: u64 = 288;
const NAME_CAPACITY: usize = 256;
const DIR_CAPACITY: usize = 64;
const FILE_MAX: usize = 4096;
const GRID_X0: u32 = 40;
const GRID_DX: u32 = 72;
const GRID_Y0: u32 = 48;
const GRID_DY: u32 = 82;
const GRID_PER_ROW: u32 = 6;

#[derive(Clone, Copy)]
pub struct Entry {
    name: [u8; 32],
    name_len: usize,
    exec: [u8; 128],
    exec_len: usize,
    builtin: [u8; 16],
    builtin_len: usize,
    icon_text: [u8; 8],
    icon_text_len: usize,
    pub color: u32,
    pub x: u32,
    pub y: u32,
    pub hidden: bool,
}

impl Entry {
    const fn empty() -> Self {
        Self {
            name: [0; 32],
            name_len: 0,
            exec: [0; 128],
            exec_len: 0,
            builtin: [0; 16],
            builtin_len: 0,
            icon_text: [0; 8],
            icon_text_len: 0,
            color: 0x89B4FA,
            x: 0,
            y: 0,
            hidden: false,
        }
    }

    pub fn name(&self) -> &str {
        as_str(&self.name[..self.name_len])
    }

    pub fn exec(&self) -> &str {
        as_str(&self.exec[..self.exec_len])
    }

    pub fn builtin(&self) -> &str {
        as_str(&self.builtin[..self.builtin_len])
    }

    pub fn icon_text(&self) -> &str {
        as_str(&self.icon_text[..self.icon_text_len])
    }
}

fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("")
}

static mut ENTRIES: [Entry; MAX_ENTRIES] = [Entry::empty(); MAX_ENTRIES];
static mut COUNT: usize = 0;

fn entries() -> &'static mut [Entry; MAX_ENTRIES] {
    unsafe { &mut *core::ptr::addr_of_mut!(ENTRIES) }
}

pub fn count() -> usize {
    unsafe { *core::ptr::addr_of!(COUNT) }
}

pub fn get(index: usize) -> Option<&'static Entry> {
    if index < count() {
        Some(&entries()[index])
    } else {
        None
    }
}

pub fn set_position(index: usize, x: u32, y: u32) {
    if index < count() {
        let entry = &mut entries()[index];
        entry.x = x;
        entry.y = y;
    }
}

pub fn set_installer_visible(visible: bool) {
    for index in 0..count() {
        let entry = &mut entries()[index];
        if entry.exec() == "/bin/installer" {
            entry.hidden = !visible;
        }
    }
}

fn copy_field(dst: &mut [u8], src: &[u8]) -> usize {
    let len = src.len().min(dst.len());
    dst[..len].copy_from_slice(&src[..len]);
    len
}

fn trim(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && matches!(bytes[start], b' ' | b'\t' | b'\r') {
        start += 1;
    }
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\r') {
        end -= 1;
    }
    &bytes[start..end]
}

fn eq_ignore_case(a: &[u8], b: &str) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

fn parse_color(text: &[u8]) -> Option<u32> {
    let text = trim(text);
    let (digits, base) = if text.len() > 2 && text[0] == b'0' && (text[1] | 0x20) == b'x' {
        (&text[2..], 16)
    } else if text.len() > 1 && text[0] == b'#' {
        (&text[1..], 16)
    } else {
        (text, 10)
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for &ch in digits {
        let digit = (ch as char).to_digit(base)?;
        value = value.checked_mul(base)?.checked_add(digit)?;
    }
    Some(value & 0x00FF_FFFF)
}

pub fn parse(data: &[u8]) -> Option<Entry> {
    let mut entry = Entry::empty();
    let mut in_section = false;
    let mut seen_section = false;
    for raw in data.split(|&b| b == b'\n') {
        let line = trim(raw);
        if line.is_empty() || line[0] == b'#' || line[0] == b';' {
            continue;
        }
        if line[0] == b'[' {
            in_section = eq_ignore_case(line, "[Desktop Entry]");
            seen_section |= in_section;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(eq) = line.iter().position(|&b| b == b'=') else {
            continue;
        };
        let key = trim(&line[..eq]);
        let value = trim(&line[eq + 1..]);
        if eq_ignore_case(key, "Name") {
            entry.name_len = copy_field(&mut entry.name, value);
        } else if eq_ignore_case(key, "Exec") {
            let end = value
                .iter()
                .position(|&b| b == b' ' || b == b'\t')
                .unwrap_or(value.len());
            entry.exec_len = copy_field(&mut entry.exec, &value[..end]);
        } else if eq_ignore_case(key, "Builtin") {
            entry.builtin_len = copy_field(&mut entry.builtin, value);
        } else if eq_ignore_case(key, "IconColor") || eq_ignore_case(key, "Color") {
            if let Some(color) = parse_color(value) {
                entry.color = color;
            }
        } else if eq_ignore_case(key, "IconText") || eq_ignore_case(key, "Symbol") {
            entry.icon_text_len = copy_field(&mut entry.icon_text, value);
        }
    }
    if !seen_section || entry.name_len == 0 || (entry.exec_len == 0 && entry.builtin_len == 0) {
        return None;
    }
    if entry.icon_text_len == 0 {
        let take = entry.name_len.min(2);
        let name = entry.name;
        entry.icon_text_len = copy_field(&mut entry.icon_text, &name[..take]);
    }
    Some(entry)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DirEntry {
    name: [u8; NAME_CAPACITY],
    size: u32,
    attributes: u8,
    reserved: [u8; 3],
}

static mut DIR: [DirEntry; DIR_CAPACITY] = [DirEntry {
    name: [0; NAME_CAPACITY],
    size: 0,
    attributes: 0,
    reserved: [0; 3],
}; DIR_CAPACITY];
static mut FILE_BUFFER: [u8; FILE_MAX] = [0; FILE_MAX];

fn name_bytes(entry: &DirEntry) -> &[u8] {
    let len = entry.name.iter().position(|&b| b == 0).unwrap_or(NAME_CAPACITY);
    &entry.name[..len]
}

fn read_file(path: &str) -> Option<usize> {
    let fd = file_open(path);
    if fd < 0 {
        return None;
    }
    let buffer = unsafe { &mut *core::ptr::addr_of_mut!(FILE_BUFFER) };
    let mut total = 0usize;
    while total < buffer.len() {
        let n = file_read(fd, &mut buffer[total..]);
        if n <= 0 {
            break;
        }
        total += n as usize;
    }
    file_close(fd);
    if total == 0 {
        None
    } else {
        Some(total)
    }
}

fn is_desktop_file(name: &[u8]) -> bool {
    name.len() > 8 && eq_ignore_case(&name[name.len() - 8..], ".desktop")
}

pub fn rescan() {
    let list = unsafe { &mut *core::ptr::addr_of_mut!(DIR) };
    let result = unsafe {
        syscall(
            SYS_DIR_LIST_LONG,
            SCAN_DIR.as_ptr() as u64,
            list.as_mut_ptr() as u64,
            DIR_CAPACITY as u64,
        )
    };
    let table = entries();
    *table = [Entry::empty(); MAX_ENTRIES];
    let mut found = 0usize;
    if result > 0 {
        let total = (result as usize).min(DIR_CAPACITY);
        for i in 1..total {
            let mut j = i;
            while j > 0 && name_bytes(&list[j - 1]) > name_bytes(&list[j]) {
                list.swap(j - 1, j);
                j -= 1;
            }
        }
        for item in list.iter().take(total) {
            if found >= MAX_ENTRIES {
                break;
            }
            let name = name_bytes(item);
            if !is_desktop_file(name) {
                continue;
            }
            let mut path = [0u8; 160];
            let dir = &SCAN_DIR[..SCAN_DIR.len() - 1];
            if dir.len() + name.len() >= path.len() {
                continue;
            }
            path[..dir.len()].copy_from_slice(dir);
            path[dir.len()..dir.len() + name.len()].copy_from_slice(name);
            let Ok(path_str) = core::str::from_utf8(&path[..dir.len() + name.len()]) else {
                continue;
            };
            let Some(size) = read_file(path_str) else {
                continue;
            };
            let data = unsafe { &(&*core::ptr::addr_of!(FILE_BUFFER))[..size] };
            if let Some(entry) = parse(data) {
                if entry.exec().ends_with("desktop-rs") {
                    continue;
                }
                table[found] = entry;
                found += 1;
            }
        }
    }
    for index in 0..found {
        table[index].x = GRID_X0 + (index as u32 % GRID_PER_ROW) * GRID_DX;
        table[index].y = GRID_Y0 + (index as u32 / GRID_PER_ROW) * GRID_DY;
    }
    unsafe {
        *core::ptr::addr_of_mut!(COUNT) = found;
    }
}

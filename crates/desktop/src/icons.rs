use desk_display as disp;
use purec::{file_close, file_open, file_read};

pub const MASK_SIZE: u32 = 96;
const MASK_BYTES: usize = (MASK_SIZE * MASK_SIZE) as usize;
const HEADER_BYTES: usize = 8;
const MAX_ICONS: usize = 32;
const NAME_MAX: usize = 24;
const DIR: &[u8] = b"/usr/share/icons/purec/";
const EXT: &[u8] = b".icn";

#[derive(Clone, Copy)]
struct Slot {
    name: [u8; NAME_MAX],
    len: usize,
    loaded: bool,
}

static mut SLOTS: [Slot; MAX_ICONS] = [Slot { name: [0; NAME_MAX], len: 0, loaded: false }; MAX_ICONS];
static mut COUNT: usize = 0;
static mut MASKS: [[u8; MASK_BYTES]; MAX_ICONS] = [[0; MASK_BYTES]; MAX_ICONS];

fn slots() -> &'static mut [Slot; MAX_ICONS] {
    unsafe { &mut *core::ptr::addr_of_mut!(SLOTS) }
}

fn masks() -> &'static mut [[u8; MASK_BYTES]; MAX_ICONS] {
    unsafe { &mut *core::ptr::addr_of_mut!(MASKS) }
}

fn read_exact(fd: i32, buffer: &mut [u8]) -> bool {
    let mut total = 0;
    while total < buffer.len() {
        let n = file_read(fd, &mut buffer[total..]);
        if n <= 0 {
            return false;
        }
        total += n as usize;
    }
    true
}

fn read_icon(name: &[u8], into: &mut [u8; MASK_BYTES]) -> bool {
    let mut path = [0u8; 96];
    let total = DIR.len() + name.len() + EXT.len();
    if total > path.len() {
        return false;
    }
    path[..DIR.len()].copy_from_slice(DIR);
    path[DIR.len()..DIR.len() + name.len()].copy_from_slice(name);
    path[DIR.len() + name.len()..total].copy_from_slice(EXT);
    let Ok(path_str) = core::str::from_utf8(&path[..total]) else {
        return false;
    };
    let fd = file_open(path_str);
    if fd < 0 {
        return false;
    }
    let mut header = [0u8; HEADER_BYTES];
    let mut ok = read_exact(fd, &mut header)
        && &header[..4] == b"PCIC"
        && u16::from_le_bytes([header[4], header[5]]) as u32 == MASK_SIZE
        && u16::from_le_bytes([header[6], header[7]]) as u32 == MASK_SIZE;
    if ok {
        ok = read_exact(fd, into);
    }
    file_close(fd);
    ok
}

pub fn load(name: &str) -> i32 {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > NAME_MAX {
        return -1;
    }
    let count = unsafe { *core::ptr::addr_of!(COUNT) };
    for (index, slot) in slots()[..count].iter().enumerate() {
        if slot.len == bytes.len() && slot.name[..slot.len] == *bytes {
            return if slot.loaded { index as i32 } else { -1 };
        }
    }
    if count >= MAX_ICONS {
        return -1;
    }
    let loaded = read_icon(bytes, &mut masks()[count]);
    let slot = &mut slots()[count];
    slot.name[..bytes.len()].copy_from_slice(bytes);
    slot.len = bytes.len();
    slot.loaded = loaded;
    unsafe {
        *core::ptr::addr_of_mut!(COUNT) = count + 1;
    }
    if loaded {
        count as i32
    } else {
        -1
    }
}

pub fn draw(slot: i32, x: u32, y: u32, size: u32, color: u32) -> bool {
    if slot < 0 || slot as usize >= MAX_ICONS || !slots()[slot as usize].loaded {
        return false;
    }
    disp::draw_mask(x, y, size, &masks()[slot as usize], MASK_SIZE, color);
    true
}

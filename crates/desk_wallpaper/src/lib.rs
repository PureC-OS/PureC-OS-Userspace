#![cfg_attr(not(test), no_std)]

#[cfg(not(test))]
use purec::{file_close, file_open, file_read, heap_grow};
#[cfg(not(test))]
use purec::fb::uptime_ms;

#[cfg(test)]
mod stubs;
#[cfg(test)]
use stubs::{file_close, file_open, file_read, heap_grow, uptime_ms};

const PATH_CAP: usize = 128;
const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIM: u32 = 1920;
const RETRY_MS: u64 = 5000;

static mut PATH: [u8; PATH_CAP] = [0; PATH_CAP];
static mut PATH_LEN: usize = 0;
static mut CACHE_PTR: *mut u32 = core::ptr::null_mut();
static mut CACHE_CAP: usize = 0;
static mut CACHE_W: u32 = 0;
static mut CACHE_H: u32 = 0;
static mut FAILED: bool = false;
static mut LAST_ATTEMPT_MS: u64 = 0;

fn u16le(p: &[u8]) -> u16 {
    p[0] as u16 | ((p[1] as u16) << 8)
}

fn u32le(p: &[u8]) -> u32 {
    p[0] as u32 | ((p[1] as u32) << 8) | ((p[2] as u32) << 16) | ((p[3] as u32) << 24)
}

fn i32le(p: &[u8]) -> i32 {
    u32le(p) as i32
}

fn u32be(p: &[u8]) -> u32 {
    ((p[0] as u32) << 24) | ((p[1] as u32) << 16) | ((p[2] as u32) << 8) | p[3] as u32
}

fn heap_aligned(bytes: usize) -> Option<*mut u8> {
    let pages = bytes.div_ceil(4096);
    heap_grow(pages as u64 * 4096)
}

unsafe fn heap_slice(ptr: *mut u8, len: usize) -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(ptr, len) }
}

fn cache_ensure(bytes: usize) -> Option<*mut u32> {
    unsafe {
        let cur = *core::ptr::addr_of!(CACHE_PTR);
        let cap = *core::ptr::addr_of!(CACHE_CAP);
        if cur.is_null() || cap < bytes {
            let ptr = heap_aligned(bytes)?;
            *core::ptr::addr_of_mut!(CACHE_PTR) = ptr as *mut u32;
            *core::ptr::addr_of_mut!(CACHE_CAP) = bytes;
            return Some(ptr as *mut u32);
        }
        Some(cur)
    }
}

static mut FILE_BUF: [u8; MAX_FILE_BYTES] = [0; MAX_FILE_BYTES];

fn load_file(path: &[u8], out: &mut &mut [u8]) -> Option<usize> {
    let n = path.len().min(127);
    let path_str = core::str::from_utf8(&path[..n]).ok()?;
    let fd = file_open(path_str);
    if fd < 0 {
        return None;
    }
    let buf = unsafe { &mut *core::ptr::addr_of_mut!(FILE_BUF) };
    let mut total = 0usize;
    let mut chunk = [0u8; 32768];
    loop {
        if total >= MAX_FILE_BYTES {
            break;
        }
        let want = (MAX_FILE_BYTES - total).min(chunk.len());
        let got = file_read(fd, &mut chunk[..want]);
        if got <= 0 {
            break;
        }
        buf[total..total + got as usize].copy_from_slice(&chunk[..got as usize]);
        total += got as usize;
    }
    file_close(fd);
    if total == 0 {
        return None;
    }
    *out = &mut buf[..total];
    Some(total)
}

fn decode_bmp(data: &[u8], dst: &mut [u32], dst_w: u32, dst_h: u32) -> bool {
    if data.len() < 54 || dst_w == 0 || dst_h == 0 {
        return false;
    }
    if u16le(data) != 0x4D42 {
        return false;
    }
    let data_offset = u32le(&data[10..]) as usize;
    let width = i32le(&data[18..]);
    let mut height = i32le(&data[22..]);
    let bpp = u16le(&data[28..]);
    let compression = u32le(&data[30..]);
    if width <= 0 || width as u32 > MAX_DIM {
        return false;
    }
    let top_down = height < 0;
    if top_down {
        height = -height;
    }
    if height <= 0 || height as u32 > MAX_DIM {
        return false;
    }
    let src_w = width as u32;
    let src_h = height as u32;
    if bpp != 24 && bpp != 32 && bpp != 8 {
        return false;
    }
    if compression != 0 && compression != 3 {
        return false;
    }
    if data_offset >= data.len() {
        return false;
    }
    let row_stride = if bpp == 24 {
        (src_w * 3 + 3) & !3
    } else if bpp == 32 {
        src_w * 4
    } else {
        (src_w + 3) & !3
    };
    if bpp == 8 {
        if data_offset < 54 || data_offset > data.len() || data_offset - 54 < 4 {
            return false;
        }
    }
    let palette = &data[54.min(data.len())..];
    let mut dy = 0u32;
    while dy < dst_h {
        let src_y = (dy * src_h) / dst_h;
        let file_row = if top_down {
            src_y
        } else {
            src_h - 1 - src_y
        };
        let row_off = data_offset + file_row as usize * row_stride as usize;
        let px_bytes = if bpp == 24 {
            src_w as usize * 3
        } else if bpp == 32 {
            src_w as usize * 4
        } else {
            src_w as usize
        };
        if row_off + px_bytes > data.len() {
            return false;
        }
        let row = &data[row_off..];
        let mut dx = 0u32;
        while dx < dst_w {
            let src_x = (dx * src_w) / dst_w;
            let color = if bpp == 24 {
                ((row[src_x as usize * 3 + 2] as u32) << 16)
                    | ((row[src_x as usize * 3 + 1] as u32) << 8)
                    | row[src_x as usize * 3] as u32
            } else if bpp == 32 {
                ((row[src_x as usize * 4 + 2] as u32) << 16)
                    | ((row[src_x as usize * 4 + 1] as u32) << 8)
                    | row[src_x as usize * 4] as u32
            } else {
                let idx = row[src_x as usize] as usize;
                let pal_off = idx * 4 + 2;
                if 54 + pal_off >= data_offset || 54 + pal_off >= data.len() {
                    return false;
                }
                ((palette[idx * 4 + 2] as u32) << 16)
                    | ((palette[idx * 4 + 1] as u32) << 8)
                    | palette[idx * 4] as u32
            };
            dst[(dy * dst_w + dx) as usize] = color;
            dx += 1;
        }
        dy += 1;
    }
    true
}

fn decode_ppm(data: &[u8], dst: &mut [u32], dst_w: u32, dst_h: u32) -> bool {
    if data.len() < 15 || data[0] != b'P' || data[1] != b'6' {
        return false;
    }
    let mut idx = 2usize;
    let skip_ws = |idx: &mut usize| {
        while *idx < data.len()
            && (data[*idx] == b' '
                || data[*idx] == b'\n'
                || data[*idx] == b'\r'
                || data[*idx] == b'\t')
        {
            *idx += 1;
        }
    };
    skip_ws(&mut idx);
    if idx < data.len() && data[idx] == b'#' {
        while idx < data.len() && data[idx] != b'\n' {
            idx += 1;
        }
        idx += 1;
    }
    let mut w = 0u32;
    while idx < data.len() && data[idx].is_ascii_digit() {
        w = w * 10 + (data[idx] - b'0') as u32;
        idx += 1;
    }
    skip_ws(&mut idx);
    let mut h = 0u32;
    while idx < data.len() && data[idx].is_ascii_digit() {
        h = h * 10 + (data[idx] - b'0') as u32;
        idx += 1;
    }
    skip_ws(&mut idx);
    while idx < data.len() && data[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx < data.len()
        && (data[idx] == b' '
            || data[idx] == b'\n'
            || data[idx] == b'\r'
            || data[idx] == b'\t')
    {
        idx += 1;
    }
    if w == 0 || w > MAX_DIM || h == 0 || h > MAX_DIM {
        return false;
    }
    let mut dy = 0u32;
    while dy < dst_h {
        let src_y = (dy * h) / dst_h;
        let mut dx = 0u32;
        while dx < dst_w {
            let src_x = (dx * w) / dst_w;
            let off = idx + (src_y * w + src_x) as usize * 3;
            if off + 3 > data.len() {
                return false;
            }
            let color = ((data[off] as u32) << 16)
                | ((data[off + 1] as u32) << 8)
                | data[off + 2] as u32;
            dst[(dy * dst_w + dx) as usize] = color;
            dx += 1;
        }
        dy += 1;
    }
    true
}

const HUFF_NODES: usize = 1152;

struct Huff {
    left: [i16; HUFF_NODES],
    right: [i16; HUFF_NODES],
    symbol: [i16; HUFF_NODES],
    root: i16,
    count: usize,
}

static mut HUFF_LIT: Huff = Huff {
    left: [-1; HUFF_NODES],
    right: [-1; HUFF_NODES],
    symbol: [-1; HUFF_NODES],
    root: 0,
    count: 1,
};
static mut HUFF_DIST: Huff = Huff {
    left: [-1; HUFF_NODES],
    right: [-1; HUFF_NODES],
    symbol: [-1; HUFF_NODES],
    root: 0,
    count: 1,
};
static mut HUFF_CL: Huff = Huff {
    left: [-1; HUFF_NODES],
    right: [-1; HUFF_NODES],
    symbol: [-1; HUFF_NODES],
    root: 0,
    count: 1,
};
static mut DYN_LENS: [u8; 320] = [0; 320];
static mut FIXED_LIT: [u8; 288] = [0; 288];
static mut FIXED_DIST: [u8; 32] = [0; 32];
static mut FIXED_READY: bool = false;

fn huff_init(h: &mut Huff) {
    h.left = [-1; HUFF_NODES];
    h.right = [-1; HUFF_NODES];
    h.symbol = [-1; HUFF_NODES];
    h.root = 0;
    h.count = 1;
}

fn huff_build(h: &mut Huff, lengths: &[u8]) -> bool {
    huff_init(h);
    let mut bl_count = [0u16; 16];
    for &len in lengths {
        if len > 15 {
            return false;
        }
        if len != 0 {
            bl_count[len as usize] = bl_count[len as usize].wrapping_add(1);
        }
    }
    let mut next_code = [0u16; 16];
    let mut code = 0u16;
    let mut bits = 1usize;
    while bits < 16 {
        code = code
            .wrapping_add(bl_count[bits - 1])
            .wrapping_shl(1);
        next_code[bits] = code;
        bits += 1;
    }
    for (n, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let c = next_code[len as usize];
        next_code[len as usize] = next_code[len as usize].wrapping_add(1);
        let mut node = h.root as usize;
        let mut b = len as i32 - 1;
        while b >= 0 {
            let bit = (c >> (b as u32)) & 1;
            if b == 0 {
                let edge = if bit != 0 {
                    &mut h.right[node]
                } else {
                    &mut h.left[node]
                };
                if *edge != -1 {
                    return false;
                }
                if h.count >= HUFF_NODES {
                    return false;
                }
                *edge = h.count as i16;
                h.symbol[h.count] = n as i16;
                h.count += 1;
            } else {
                let edge = if bit != 0 {
                    h.right[node]
                } else {
                    h.left[node]
                };
                if edge == -1 {
                    if h.count >= HUFF_NODES {
                        return false;
                    }
                    if bit != 0 {
                        h.right[node] = h.count as i16;
                    } else {
                        h.left[node] = h.count as i16;
                    }
                    h.count += 1;
                    node = h.count - 1;
                } else {
                    node = edge as usize;
                }
            }
            b -= 1;
        }
    }
    true
}

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bit_count: u32,
}

fn br_fill(br: &mut BitReader, need: u32) -> bool {
    while br.bit_count < need {
        if br.byte_pos >= br.data.len() {
            return false;
        }
        br.bit_buf |= (br.data[br.byte_pos] as u32) << br.bit_count;
        br.byte_pos += 1;
        br.bit_count += 8;
    }
    true
}

fn br_bits(br: &mut BitReader, count: u32, out: &mut u32) -> bool {
    if count == 0 {
        *out = 0;
        return true;
    }
    if !br_fill(br, count) {
        return false;
    }
    *out = if count >= 32 {
        br.bit_buf
    } else {
        br.bit_buf & ((1u32 << count) - 1)
    };
    br.bit_buf >>= count;
    br.bit_count -= count;
    true
}

fn huff_decode(br: &mut BitReader, h: &Huff) -> Option<u32> {
    let mut node = h.root as usize;
    loop {
        let mut bit = 0u32;
        if !br_bits(br, 1, &mut bit) {
            return None;
        }
        node = if bit != 0 {
            h.right[node]
        } else {
            h.left[node]
        } as usize;
        if node >= h.count {
            return None;
        }
        if h.symbol[node] >= 0 {
            return Some(h.symbol[node] as u32);
        }
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67,
    83, 99, 115, 131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5,
    5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513,
    769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10,
    11, 11, 12, 12, 13, 13,
];

fn inflate(input: &[u8], out: &mut [u8]) -> Option<usize> {
    if input.len() < 6 {
        return None;
    }
    if (input[0] & 0x0F) != 8 {
        return None;
    }
    if (((input[0] as u32) << 8 | input[1] as u32) % 31) != 0 {
        return None;
    }
    if (input[1] & 0x20) != 0 {
        return None;
    }
    let mut br = BitReader {
        data: &input[2..input.len() - 4],
        byte_pos: 0,
        bit_buf: 0,
        bit_count: 0,
    };
    let mut out_pos = 0usize;
    let mut is_final = false;
    unsafe {
        if !*core::ptr::addr_of!(FIXED_READY) {
            let lit = &mut *core::ptr::addr_of_mut!(FIXED_LIT);
            let dist = &mut *core::ptr::addr_of_mut!(FIXED_DIST);
            let mut i = 0usize;
            while i <= 143 {
                lit[i] = 8;
                i += 1;
            }
            while i <= 255 {
                lit[i] = 9;
                i += 1;
            }
            while i <= 279 {
                lit[i] = 7;
                i += 1;
            }
            while i <= 287 {
                lit[i] = 8;
                i += 1;
            }
            let mut j = 0usize;
            while j < 32 {
                dist[j] = 5;
                j += 1;
            }
            *core::ptr::addr_of_mut!(FIXED_READY) = true;
        }
    }
    while !is_final {
        let mut bfinal = 0u32;
        let mut btype = 0u32;
        if !br_bits(&mut br, 1, &mut bfinal) {
            return None;
        }
        if !br_bits(&mut br, 2, &mut btype) {
            return None;
        }
        is_final = bfinal != 0;
        if btype == 0 {
            br.bit_buf = 0;
            br.bit_count = 0;
            let consumed = br.byte_pos;
            let raw_left = br.data.len() - consumed;
            if raw_left < 4 {
                return None;
            }
            let raw = &br.data[consumed..];
            let len = raw[0] as u32 | ((raw[1] as u32) << 8);
            let nlen = raw[2] as u32 | ((raw[3] as u32) << 8);
            if (len ^ nlen) != 0xFFFF {
                return None;
            }
            if raw_left - 4 < len as usize {
                return None;
            }
            if out_pos + len as usize > out.len() {
                return None;
            }
            out[out_pos..out_pos + len as usize].copy_from_slice(&raw[4..4 + len as usize]);
            out_pos += len as usize;
            br.byte_pos += 4 + len as usize;
            continue;
        }
        let use_fixed = btype == 1;
        if btype != 1 && btype != 2 {
            return None;
        }
        unsafe {
            if use_fixed {
                let lit_src = &*core::ptr::addr_of!(FIXED_LIT);
                let dist_src = &*core::ptr::addr_of!(FIXED_DIST);
                if !huff_build(&mut *core::ptr::addr_of_mut!(HUFF_LIT), lit_src) {
                    return None;
                }
                if !huff_build(&mut *core::ptr::addr_of_mut!(HUFF_DIST), dist_src) {
                    return None;
                }
            } else {
                let mut hlit = 0u32;
                let mut hdist = 0u32;
                let mut hclen = 0u32;
                if !br_bits(&mut br, 5, &mut hlit) {
                    return None;
                }
                if !br_bits(&mut br, 5, &mut hdist) {
                    return None;
                }
                if !br_bits(&mut br, 4, &mut hclen) {
                    return None;
                }
                hlit += 257;
                hdist += 1;
                hclen += 4;
                if hlit > 288 || hdist > 32 {
                    return None;
                }
                const CL_ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14,
                    1, 15,
                ];
                let mut cl_len = [0u8; 19];
                let mut i = 0u32;
                while i < hclen {
                    let mut v = 0u32;
                    if !br_bits(&mut br, 3, &mut v) {
                        return None;
                    }
                    cl_len[CL_ORDER[i as usize]] = v as u8;
                    i += 1;
                }
                if !huff_build(&mut *core::ptr::addr_of_mut!(HUFF_CL), &cl_len) {
                    return None;
                }
                let total = (hlit + hdist) as usize;
                if total > 320 {
                    return None;
                }
                let dyn_lens = &mut *core::ptr::addr_of_mut!(DYN_LENS);
                let mut k = 0usize;
                while k < total {
                    let sym = match huff_decode(&mut br, &*core::ptr::addr_of!(HUFF_CL)) {
                        Some(s) => s,
                        None => return None,
                    };
                    if sym <= 15 {
                        dyn_lens[k] = sym as u8;
                        k += 1;
                    } else if sym == 16 {
                        let mut rep = 0u32;
                        if k == 0 || !br_bits(&mut br, 2, &mut rep) {
                            return None;
                        }
                        rep += 3;
                        if k + rep as usize > total {
                            return None;
                        }
                        let prev = dyn_lens[k - 1];
                        let mut j = 0u32;
                        while j < rep {
                            dyn_lens[k] = prev;
                            k += 1;
                            j += 1;
                        }
                    } else if sym == 17 {
                        let mut rep = 0u32;
                        if !br_bits(&mut br, 3, &mut rep) {
                            return None;
                        }
                        rep += 3;
                        if k + rep as usize > total {
                            return None;
                        }
                        let mut j = 0u32;
                        while j < rep {
                            dyn_lens[k] = 0;
                            k += 1;
                            j += 1;
                        }
                    } else if sym == 18 {
                        let mut rep = 0u32;
                        if !br_bits(&mut br, 7, &mut rep) {
                            return None;
                        }
                        rep += 11;
                        if k + rep as usize > total {
                            return None;
                        }
                        let mut j = 0u32;
                        while j < rep {
                            dyn_lens[k] = 0;
                            k += 1;
                            j += 1;
                        }
                    } else {
                        return None;
                    }
                }
                let (lit_part, dist_part) = dyn_lens.split_at(hlit as usize);
                if !huff_build(
                    &mut *core::ptr::addr_of_mut!(HUFF_LIT),
                    &lit_part[..hlit as usize],
                ) {
                    return None;
                }
                if !huff_build(
                    &mut *core::ptr::addr_of_mut!(HUFF_DIST),
                    &dist_part[..hdist as usize],
                ) {
                    return None;
                }
            }
        }
        loop {
            let sym = unsafe {
                match huff_decode(&mut br, &*core::ptr::addr_of!(HUFF_LIT)) {
                    Some(s) => s,
                    None => return None,
                }
            };
            if sym < 256 {
                if out_pos >= out.len() {
                    return None;
                }
                out[out_pos] = sym as u8;
                out_pos += 1;
            } else if sym == 256 {
                break;
            } else if sym <= 285 {
                let li = (sym - 257) as usize;
                let mut len = LEN_BASE[li] as u32;
                let mut eb = 0u32;
                if !br_bits(&mut br, LEN_EXTRA[li] as u32, &mut eb) {
                    return None;
                }
                len += eb;
                let dsym = unsafe {
                    match huff_decode(&mut br, &*core::ptr::addr_of!(HUFF_DIST)) {
                        Some(s) => s,
                        None => return None,
                    }
                };
                if dsym > 29 {
                    return None;
                }
                let mut dist_val = DIST_BASE[dsym as usize] as u32;
                if !br_bits(&mut br, DIST_EXTRA[dsym as usize] as u32, &mut eb) {
                    return None;
                }
                dist_val += eb;
                if dist_val == 0 || dist_val as usize > out_pos {
                    return None;
                }
                if out_pos + len as usize > out.len() {
                    return None;
                }
                let mut k = 0u32;
                while k < len {
                    out[out_pos] = out[out_pos - dist_val as usize];
                    out_pos += 1;
                    k += 1;
                }
            } else {
                return None;
            }
        }
    }
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    let mut i = 0usize;
    while i < out_pos {
        s1 = (s1 + out[i] as u32) % 65521;
        s2 = (s2 + s1) % 65521;
        i += 1;
    }
    let expect =
        ((input[input.len() - 4] as u32) << 24) | ((input[input.len() - 3] as u32) << 16)
            | ((input[input.len() - 2] as u32) << 8) | input[input.len() - 1] as u32;
    if (s2 << 16) | s1 != expect {
        return None;
    }
    Some(out_pos)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn decode_png(data: &[u8], dst: &mut [u32], dst_w: u32, dst_h: u32) -> bool {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if data.len() < 57 || dst_w == 0 || dst_h == 0 {
        return false;
    }
    if data[..8] != SIG {
        return false;
    }
    let mut pos = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut interlace = 0u8;
    let mut have_ihdr = false;
    let mut idat_total = 0usize;
    while pos + 8 <= data.len() {
        let len = u32be(&data[pos..]) as usize;
        if len.checked_add(12).and_then(|e| pos.checked_add(e)).is_none() {
            return false;
        }
        let end = pos + 8 + len + 4;
        if end > data.len() {
            return false;
        }
        let typ = &data[pos + 4..pos + 8];
        if typ == b"IHDR" {
            if len != 13 || have_ihdr {
                return false;
            }
            let chunk = &data[pos + 8..];
            width = u32be(chunk);
            height = u32be(&chunk[4..]);
            bit_depth = chunk[8];
            color_type = chunk[9];
            if chunk[10] != 0 || chunk[11] != 0 {
                return false;
            }
            interlace = chunk[12];
            have_ihdr = true;
        } else if typ == b"IDAT" {
            if !have_ihdr {
                return false;
            }
            idat_total += len;
        } else if typ == b"IEND" {
            break;
        }
        pos = end;
    }
    if !have_ihdr || idat_total == 0 {
        return false;
    }
    if width == 0 || width > MAX_DIM || height == 0 || height > MAX_DIM {
        return false;
    }
    if bit_depth != 8 || interlace != 0 {
        return false;
    }
    let channels = match color_type {
        0 => 1u32,
        2 => 3,
        6 => 4,
        _ => return false,
    };
    let stride = width as usize * channels as usize + 1;
    let raw_size = stride * height as usize;
    if raw_size == 0 || raw_size > 16 * 1024 * 1024 {
        return false;
    }
    let idat_ptr = match heap_aligned(idat_total) {
        Some(p) => p,
        None => return false,
    };
    let raw_ptr = match heap_aligned(raw_size) {
        Some(p) => p,
        None => return false,
    };
    let idat = unsafe { heap_slice(idat_ptr, idat_total) };
    let raw = unsafe { heap_slice(raw_ptr, raw_size) };
    pos = 8;
    let mut copied = 0usize;
    while pos + 8 <= data.len() && copied < idat_total {
        let len = u32be(&data[pos..]) as usize;
        let typ = &data[pos + 4..pos + 8];
        let chunk = &data[pos + 8..];
        if typ == b"IDAT" {
            if copied + len > idat_total || len > chunk.len() {
                return false;
            }
            idat[copied..copied + len].copy_from_slice(&chunk[..len]);
            copied += len;
        } else if typ == b"IEND" {
            break;
        }
        pos += 8 + len + 4;
    }
    if copied != idat_total {
        return false;
    }
    let inflated = match inflate(idat, raw) {
        Some(n) => n,
        None => return false,
    };
    if inflated != raw_size {
        return false;
    }
    let mut y = 0u32;
    while y < height {
        let row_off = y as usize * stride;
        let filter = raw[row_off];
        if filter > 4 {
            return false;
        }
        let mut i = 1usize;
        while i < stride {
            let a = if i > channels as usize {
                raw[row_off + i - channels as usize]
            } else {
                0
            };
            let b = if y > 0 {
                raw[row_off - stride + i]
            } else {
                0
            };
            let c = if y > 0 && i > channels as usize {
                raw[row_off - stride + i - channels as usize]
            } else {
                0
            };
            let v = raw[row_off + i];
            raw[row_off + i] = match filter {
                0 => v,
                1 => v.wrapping_add(a),
                2 => v.wrapping_add(b),
                3 => v.wrapping_add(((a as u16 + b as u16) / 2) as u8),
                _ => v.wrapping_add(paeth(a, b, c)),
            };
            i += 1;
        }
        y += 1;
    }
    let mut dy = 0u32;
    while dy < dst_h {
        let src_y = (dy * height) / dst_h;
        let row_off = src_y as usize * stride;
        let mut dx = 0u32;
        while dx < dst_w {
            let src_x = (dx * width) / dst_w;
            let px_off = row_off + 1 + src_x as usize * channels as usize;
            let color = if channels == 1 {
                ((raw[px_off] as u32) << 16)
                    | ((raw[px_off] as u32) << 8)
                    | raw[px_off] as u32
            } else {
                ((raw[px_off] as u32) << 16)
                    | ((raw[px_off + 1] as u32) << 8)
                    | raw[px_off + 2] as u32
            };
            dst[(dy * dst_w + dx) as usize] = color;
            dx += 1;
        }
        dy += 1;
    }
    true
}

fn blit_to_fb(fb: *mut u32, pitch: u32, src: &[u32], w: u32, h: u32) {
    let mut y = 0u32;
    while y < h {
        unsafe {
            let dst_row = fb.add(y as usize * pitch as usize);
            let src_row = &src[y as usize * w as usize..];
            core::ptr::copy_nonoverlapping(src_row.as_ptr(), dst_row, w as usize);
        }
        y += 1;
    }
}

fn try_load(scr_w: u32, scr_h: u32) -> bool {
    let (path, path_len) = unsafe {
        (
            &*core::ptr::addr_of!(PATH),
            *core::ptr::addr_of!(PATH_LEN),
        )
    };
    let mut file_data: &mut [u8] = &mut [];
    if load_file(&path[..path_len], &mut file_data).is_none() {
        return false;
    }
    let screen_bytes = scr_w as usize * scr_h as usize * 4;
    if screen_bytes == 0 || screen_bytes > 32 * 1024 * 1024 {
        return false;
    }
    let screen_ptr = match cache_ensure(screen_bytes) {
        Some(p) => p,
        None => return false,
    };
    let dst = unsafe { core::slice::from_raw_parts_mut(screen_ptr, scr_w as usize * scr_h as usize) };
    let mut decoded = decode_bmp(file_data, dst, scr_w, scr_h);
    if !decoded && file_data.len() >= 8 && file_data[0] == b'P' && file_data[1] == b'6' {
        decoded = decode_ppm(file_data, dst, scr_w, scr_h);
    }
    if !decoded {
        decoded = decode_png(file_data, dst, scr_w, scr_h);
    }
    if !decoded {
        return false;
    }
    unsafe {
        *core::ptr::addr_of_mut!(CACHE_W) = scr_w;
        *core::ptr::addr_of_mut!(CACHE_H) = scr_h;
    }
    true
}

pub fn set_path(path: &[u8]) {
    unsafe {
        let dst = &mut *core::ptr::addr_of_mut!(PATH);
        let n = path.len().min(PATH_CAP - 1);
        dst[..n].copy_from_slice(&path[..n]);
        *core::ptr::addr_of_mut!(PATH_LEN) = n;
        *core::ptr::addr_of_mut!(CACHE_W) = 0;
        *core::ptr::addr_of_mut!(CACHE_H) = 0;
        *core::ptr::addr_of_mut!(FAILED) = false;
        *core::ptr::addr_of_mut!(LAST_ATTEMPT_MS) = 0;
    }
}

pub fn has_path() -> bool {
    unsafe { *core::ptr::addr_of!(PATH_LEN) > 0 }
}

pub fn draw(fb: *mut u32, pitch: u32, scr_w: u32, scr_h: u32) -> bool {
    if fb.is_null() || scr_w == 0 || scr_h == 0 {
        return false;
    }
    if unsafe { *core::ptr::addr_of!(PATH_LEN) } == 0 {
        return false;
    }
    let (cached, cw, ch) = unsafe {
        (
            *core::ptr::addr_of!(CACHE_PTR),
            *core::ptr::addr_of!(CACHE_W),
            *core::ptr::addr_of!(CACHE_H),
        )
    };
    if !cached.is_null() && cw == scr_w && ch == scr_h {
        let src =
            unsafe { core::slice::from_raw_parts(cached, scr_w as usize * scr_h as usize) };
        blit_to_fb(fb, pitch, src, scr_w, scr_h);
        return true;
    }
    if unsafe { *core::ptr::addr_of!(FAILED) } {
        if uptime_ms() - unsafe { *core::ptr::addr_of!(LAST_ATTEMPT_MS) } < RETRY_MS {
            return false;
        }
    }
    unsafe {
        *core::ptr::addr_of_mut!(LAST_ATTEMPT_MS) = uptime_ms();
    }
    if try_load(scr_w, scr_h) {
        unsafe {
            *core::ptr::addr_of_mut!(FAILED) = false;
        }
        let cached = unsafe { *core::ptr::addr_of!(CACHE_PTR) };
        let src =
            unsafe { core::slice::from_raw_parts(cached, scr_w as usize * scr_h as usize) };
        blit_to_fb(fb, pitch, src, scr_w, scr_h);
        return true;
    }
    unsafe {
        *core::ptr::addr_of_mut!(FAILED) = true;
        *core::ptr::addr_of_mut!(CACHE_W) = 0;
        *core::ptr::addr_of_mut!(CACHE_H) = 0;
    }
    false
}

#[cfg(test)]
mod tests;

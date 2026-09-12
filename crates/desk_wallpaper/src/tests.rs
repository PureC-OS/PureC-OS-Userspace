use super::*;
use crate::stubs;
use std::collections::HashSet;

const WP01: &[u8] = include_bytes!("../../../../src/wallpapers/wp01.png");
const WP02: &[u8] = include_bytes!("../../../../src/wallpapers/wp02.png");
const WP03: &[u8] = include_bytes!("../../../../src/wallpapers/wp03.png");
const BMW2: &[u8] = include_bytes!("../../../../src/wallpapers/bmw2.png");
const SPACE1: &[u8] = include_bytes!("../../../../src/wallpapers/space1.png");

fn distinct_colors(dst: &[u32]) -> usize {
    let mut set = HashSet::new();
    let mut i = 0usize;
    while i < dst.len() {
        set.insert(dst[i]);
        i += 997;
    }
    set.len()
}

#[test]
fn png_wallpapers_decode() {
    for (name, data) in [
        ("wp01", WP01),
        ("wp02", WP02),
        ("wp03", WP03),
        ("bmw2", BMW2),
        ("space1", SPACE1),
    ] {
        let mut dst = vec![0u32; 1280 * 800];
        assert!(decode_png(data, &mut dst, 1280, 800), "{name} decoded");
        let colors = distinct_colors(&dst);
        assert!(colors > 100, "{name} has only {colors} colors");
    }
}

#[test]
fn draw_end_to_end() {
    stubs::register("/w/test.png", WP01.to_vec());
    set_path(b"/w/test.png");
    let mut fb = vec![0u32; 640 * 480];
    assert!(draw(fb.as_mut_ptr(), 640, 640, 480));
    assert!(fb.iter().any(|&p| p != 0), "framebuffer stayed black");
    assert!(draw(fb.as_mut_ptr(), 640, 640, 480), "cache-hit redraw");
}

#[test]
fn corrupt_rejected() {
    let mut dst = vec![0u32; 320 * 200];
    assert!(!decode_png(&[0u8; 100], &mut dst, 320, 200));
    assert!(!decode_png(&WP01[..1000], &mut dst, 320, 200));
    assert!(!decode_png(b"definitely not a png at all.............", &mut dst, 320, 200));
}

#[test]
fn bmp_smoke() {
    let mut bmp = vec![0u8; 54 + 4 * 4];
    bmp[0] = b'B';
    bmp[1] = b'M';
    bmp[10] = 54;
    bmp[18] = 2;
    bmp[22] = 2;
    bmp[28] = 24;
    bmp[54] = 10;
    bmp[55] = 20;
    bmp[56] = 30;
    let mut dst = vec![0u32; 2 * 2];
    assert!(decode_bmp(&bmp, &mut dst, 2, 2));
    assert_eq!(dst[2], (30u32 << 16) | (20u32 << 8) | 10);
}

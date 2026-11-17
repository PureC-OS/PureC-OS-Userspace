use crate::icons;
use desk_display as disp;
use desk_personal as personal;
use purec::fb::uptime_ms;
use purec::net::{self, WifiNet, WifiStatus, Wired};

pub const SLOT_W: u32 = 40;
pub const POPUP_W: u32 = 300;
const POLL_MS: u64 = 3000;
const POPUP_POLL_MS: u64 = 1000;
const PAD: u32 = 4;
const HEADER_H: u32 = 36;
const STATUS_H: u32 = 56;
const LABEL_H: u32 = 22;
const ROW_H: u32 = 30;
const MAX_ROWS: usize = 8;
const FOOTER_H: u32 = 44;
const BUTTON_H: u32 = 28;
const SCAN_W: u32 = 64;
const DISCONNECT_W: u32 = 96;

#[derive(Clone, Copy, PartialEq)]
pub enum Hit {
    Row(usize),
    Scan,
    Disconnect,
}

pub enum Action {
    None,
    Connect(usize),
    Disconnect,
    Scan,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Wifi(i8),
    Ethernet,
    Connecting,
    Off,
}

struct State {
    wifi: WifiStatus,
    wired: Option<Wired>,
    polled: bool,
    at: u64,
    nets: [WifiNet; MAX_ROWS],
    count: usize,
    message: &'static str,
    hover: Option<Hit>,
}

static mut STATE: State = State {
    wifi: WifiStatus { device: false, connected: false, state: 0, ssid: [0; 33], rssi: 0, ip: 0 },
    wired: None,
    polled: false,
    at: 0,
    nets: [WifiNet::empty(); MAX_ROWS],
    count: 0,
    message: "",
    hover: None,
};

fn state() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}

fn refresh(list: bool) {
    let s = state();
    s.wifi = net::wifi_status();
    s.wired = net::wired();
    if list && s.wifi.device {
        let mut all = [WifiNet::empty(); net::SCAN_MAX];
        let n = net::wifi_list(&mut all);
        let mut order = [0usize; net::SCAN_MAX];
        for (i, slot) in order.iter_mut().enumerate().take(n) {
            *slot = i;
        }
        let order = &mut order[..n];
        order.sort_unstable_by(|&a, &b| all[b].rssi.cmp(&all[a].rssi));
        s.count = 0;
        for &i in order.iter() {
            if s.count >= MAX_ROWS {
                break;
            }
            if all[i].ssid[0] == 0 {
                continue;
            }
            s.nets[s.count] = all[i];
            s.count += 1;
        }
    }
}

pub fn poll(popup_open: bool) -> u64 {
    let now = uptime_ms();
    let s = state();
    let interval = if popup_open { POPUP_POLL_MS } else { POLL_MS };
    if !s.polled || now.wrapping_sub(s.at) >= interval {
        s.polled = true;
        s.at = now;
        refresh(popup_open);
    }
    signature()
}

fn signature() -> u64 {
    let s = state();
    let mut hash: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut mix = |v: u64| hash = (hash ^ v).wrapping_mul(0x100_0000_01B3);
    mix(s.wifi.device as u64 | (s.wifi.connected as u64) << 1 | (s.wifi.state as u64) << 2);
    mix(icon_name().as_ptr() as u64);
    mix(s.wifi.ip as u64);
    if let Some(w) = s.wired {
        mix(w.link as u64 | (w.ip as u64) << 1);
    }
    for b in s.wifi.ssid.iter().take_while(|&&b| b != 0) {
        mix(*b as u64);
    }
    for n in &s.nets[..s.count] {
        mix(n.rssi as u8 as u64 | (n.security as u64) << 8);
        for b in n.ssid.iter().take_while(|&&b| b != 0) {
            mix(*b as u64);
        }
    }
    mix(s.message.len() as u64);
    hash
}

fn kind() -> Kind {
    let s = state();
    if s.wifi.connected {
        return Kind::Wifi(s.wifi.rssi);
    }
    if let Some(w) = s.wired {
        if w.link {
            return Kind::Ethernet;
        }
    }
    if s.wifi.device && s.wifi.state == net::STATE_CONNECTING {
        return Kind::Connecting;
    }
    Kind::Off
}

fn signal_icon(rssi: i8) -> &'static str {
    if rssi >= -55 {
        "wifi-4"
    } else if rssi >= -67 {
        "wifi-3"
    } else if rssi >= -78 {
        "wifi-2"
    } else {
        "wifi-1"
    }
}

fn icon_name() -> &'static str {
    match kind() {
        Kind::Wifi(rssi) => signal_icon(rssi),
        Kind::Ethernet => "ethernet",
        Kind::Connecting => "wifi-none",
        Kind::Off => {
            if state().wifi.device {
                "wifi-off"
            } else {
                "net-off"
            }
        }
    }
}

fn icon(name: &str, x: u32, y: u32, size: u32, color: u32) {
    let slot = icons::load(name);
    if !icons::draw(slot, x, y, size, color) {
        disp::draw_rect(x + size / 4, y + size / 4, size / 2, size / 2, color);
    }
}

pub fn draw_slot(x: u32, bar_y: u32, bar_h: u32, active: bool) {
    let th = personal::current_colors();
    let bg = if active { th.accent } else { th.border };
    let fg = if active { th.window } else { th.text };
    disp::draw_rect(x, bar_y + 4, SLOT_W, bar_h - 8, bg);
    let size = 22;
    icon(icon_name(), x + (SLOT_W - size) / 2, bar_y + (bar_h - size) / 2, size, fg);
}

fn rows() -> u32 {
    let s = state();
    if !s.wifi.device {
        return 1;
    }
    (s.count as u32).max(1)
}

pub fn popup_rect(slot_x: u32, bar_top: u32, screen_w: u32) -> (u32, u32, u32, u32) {
    let h = PAD * 2 + HEADER_H + STATUS_H + LABEL_H + rows() * ROW_H + FOOTER_H;
    let x = (slot_x + SLOT_W).saturating_sub(POPUP_W).min(screen_w.saturating_sub(POPUP_W));
    let y = bar_top.saturating_sub(h + 4);
    (x, y, POPUP_W, h)
}

pub fn open() {
    let s = state();
    s.hover = None;
    s.message = "";
    refresh(true);
    if s.wifi.device && !s.wifi.connected {
        if net::wifi_scan() >= 0 {
            s.message = "Scanning...";
        }
    }
}

fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn list_top(y: u32) -> u32 {
    y + PAD + HEADER_H + STATUS_H + LABEL_H
}

fn footer_top(y: u32) -> u32 {
    list_top(y) + rows() * ROW_H
}

fn buttons(rect: (u32, u32, u32, u32)) -> ((u32, u32), Option<(u32, u32)>) {
    let (x, y, w, _) = rect;
    let top = footer_top(y) + (FOOTER_H - BUTTON_H) / 2;
    let scan_x = x + w - 10 - SCAN_W;
    let disconnect = if state().wifi.connected { Some((scan_x - 6 - DISCONNECT_W, top)) } else { None };
    ((scan_x, top), disconnect)
}

fn inside(px: i32, py: i32, x: u32, y: u32, w: u32, h: u32) -> bool {
    px >= x as i32 && py >= y as i32 && px < (x + w) as i32 && py < (y + h) as i32
}

fn hit_test(px: i32, py: i32, rect: (u32, u32, u32, u32)) -> Option<Hit> {
    let (x, y, w, h) = rect;
    if !inside(px, py, x, y, w, h) {
        return None;
    }
    let s = state();
    let top = list_top(y);
    if s.wifi.device && py >= top as i32 && py < (top + s.count as u32 * ROW_H) as i32 {
        return Some(Hit::Row((py - top as i32) as usize / ROW_H as usize));
    }
    let ((sx, sy), disconnect) = buttons(rect);
    if s.wifi.device && inside(px, py, sx, sy, SCAN_W, BUTTON_H) {
        return Some(Hit::Scan);
    }
    if let Some((dx, dy)) = disconnect {
        if inside(px, py, dx, dy, DISCONNECT_W, BUTTON_H) {
            return Some(Hit::Disconnect);
        }
    }
    None
}

pub fn hover(px: i32, py: i32, rect: (u32, u32, u32, u32)) -> bool {
    let current = hit_test(px, py, rect);
    let s = state();
    if current == s.hover {
        return false;
    }
    s.hover = current;
    true
}

pub fn click(px: i32, py: i32, rect: (u32, u32, u32, u32)) -> Option<Action> {
    let (x, y, w, h) = rect;
    if !inside(px, py, x, y, w, h) {
        return None;
    }
    let s = state();
    Some(match hit_test(px, py, rect) {
        Some(Hit::Row(i)) if i < s.count => {
            let n = s.nets[i];
            if s.wifi.connected && n.ssid == s.wifi.ssid {
                Action::None
            } else if n.security != net::SECURITY_OPEN {
                s.message = "Secured network: use Settings";
                Action::None
            } else {
                Action::Connect(i)
            }
        }
        Some(Hit::Scan) => Action::Scan,
        Some(Hit::Disconnect) => Action::Disconnect,
        _ => Action::None,
    })
}

pub fn connecting_message() {
    state().message = "Connecting...";
}

pub fn run(action: Action) {
    let s = state();
    match action {
        Action::None => return,
        Action::Connect(i) => {
            let ssid = s.nets[i].ssid;
            s.message = if net::wifi_connect(&ssid) < 0 { "Connection failed" } else { "" };
        }
        Action::Disconnect => {
            net::wifi_disconnect();
            s.message = "";
        }
        Action::Scan => {
            s.message = if net::wifi_scan() < 0 { "Cannot scan while connected" } else { "Scanning..." };
        }
    }
    refresh(true);
}

pub fn draw_popup(rect: (u32, u32, u32, u32)) {
    let th = personal::current_colors();
    let s = state();
    let (x, y, w, h) = rect;
    disp::draw_rect(x, y, w, h, th.border);
    disp::draw_rect(x + 1, y + 1, w - 2, h - 2, th.window);
    disp::draw_text_sized(x + 14, y + PAD + 12, "Network", th.text, th.window, 12);
    disp::draw_rect(x + 8, y + PAD + HEADER_H - 2, w - 16, 1, th.border);

    let top = y + PAD + HEADER_H;
    let mut ip = [0u8; 16];
    let (title, detail): (&str, &str) = match kind() {
        Kind::Wifi(_) => (net::ssid_str(&s.wifi.ssid), net::format_ip(s.wifi.ip, &mut ip)),
        Kind::Ethernet => {
            let wired_ip = s.wired.map(|w| w.ip).unwrap_or(0);
            ("Ethernet", if wired_ip != 0 { net::format_ip(wired_ip, &mut ip) } else { "Waiting for address" })
        }
        Kind::Connecting => ("Connecting...", ""),
        Kind::Off => ("Not connected", if s.wifi.device { "Choose a network below" } else { "No network adapter" }),
    };
    icon(icon_name(), x + 14, top + 12, 32, th.accent);
    disp::draw_text(x + 58, top + 16, truncate(title, 28), th.text, th.window);
    let caption = match kind() {
        Kind::Wifi(_) => "Wi-Fi",
        Kind::Ethernet => "Wired",
        _ => "",
    };
    let mut line = [0u8; 48];
    let mut n = 0;
    for part in [caption, if caption.is_empty() || detail.is_empty() { "" } else { "  " }, detail] {
        for &b in part.as_bytes() {
            if n < line.len() {
                line[n] = b;
                n += 1;
            }
        }
    }
    disp::draw_text(x + 58, top + 32, core::str::from_utf8(&line[..n]).unwrap_or(""), th.muted_text, th.window);
    disp::draw_rect(x + 8, top + STATUS_H - 2, w - 16, 1, th.border);
    disp::draw_text(x + 14, top + STATUS_H + 8, "Wi-Fi networks", th.muted_text, th.window);

    let lt = list_top(y);
    if !s.wifi.device {
        disp::draw_text(x + 14, lt + 11, "No Wi-Fi adapter", th.muted_text, th.window);
    } else if s.count == 0 {
        disp::draw_text(x + 14, lt + 11, "No networks found", th.muted_text, th.window);
    }
    for i in 0..s.count {
        let n = s.nets[i];
        let row_y = lt + i as u32 * ROW_H;
        let current = s.wifi.connected && n.ssid == s.wifi.ssid;
        let hovered = s.hover == Some(Hit::Row(i));
        let bg = if hovered { th.border } else { th.window };
        if hovered {
            disp::draw_rect(x + 6, row_y + 1, w - 12, ROW_H - 2, bg);
        }
        let name = if n.security != net::SECURITY_OPEN { "wifi-lock" } else { signal_icon(n.rssi) };
        icon(name, x + 14, row_y + 5, 20, if current { th.accent } else { th.text });
        disp::draw_text(x + 44, row_y + 11, truncate(net::ssid_str(&n.ssid), 24), th.text, bg);
        if current {
            disp::draw_text(x + w - 14 - 72, row_y + 11, "Connected", th.accent, bg);
        }
    }

    let ft = footer_top(y);
    disp::draw_rect(x + 8, ft, w - 16, 1, th.border);
    if !s.message.is_empty() {
        disp::draw_text(x + 14, ft + 18, truncate(s.message, 16), th.muted_text, th.window);
    }
    let ((sx, sy), disconnect) = buttons(rect);
    if s.wifi.device {
        let active = s.hover == Some(Hit::Scan);
        let bg = if active { th.accent } else { th.border };
        let fg = if active { th.window } else { th.text };
        disp::draw_rect(sx, sy, SCAN_W, BUTTON_H, bg);
        icon("refresh", sx + 6, sy + 6, 16, fg);
        disp::draw_text(sx + 26, sy + 10, "Scan", fg, bg);
    }
    if let Some((dx, dy)) = disconnect {
        let active = s.hover == Some(Hit::Disconnect);
        let bg = if active { th.danger } else { th.border };
        let fg = if active { th.window } else { th.danger };
        disp::draw_rect(dx, dy, DISCONNECT_W, BUTTON_H, bg);
        disp::draw_text(dx + (DISCONNECT_W - 80) / 2, dy + 10, "Disconnect", fg, bg);
    }
}

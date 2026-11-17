use core::ffi::c_char;

const SSID_CAP: usize = 33;
const IF_MAX: usize = 8;
pub const SCAN_MAX: usize = 32;
pub const SECURITY_OPEN: u8 = 0;
pub const STATE_CONNECTING: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct NetIfInfo {
    name: [u8; 16],
    mac: [u8; 6],
    mtu: u16,
    link_up: u32,
    has_ip: u32,
    ip_address: u32,
    netmask: u32,
    gateway: u32,
    dns_server: u32,
    dhcp_bound: u32,
    rx_packets: u64,
    tx_packets: u64,
    rx_bytes: u64,
    tx_bytes: u64,
    rx_dropped: u64,
    tx_dropped: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct WifiNetworkInfo {
    ssid: [u8; SSID_CAP],
    bssid: [u8; 6],
    rssi: i8,
    channel: u8,
    security: u8,
    reserved: u8,
}

#[repr(C)]
struct WifiStatusInfo {
    state: u32,
    connected: u32,
    ssid: [u8; SSID_CAP],
    bssid: [u8; 6],
    rssi: i8,
    channel: u8,
    security: u8,
    reserved: u8,
    last_error: i32,
    ip_address: u32,
    mac: [u8; 6],
    interface_name: [u8; 16],
    scan_count: u32,
    last_scan_ms: u64,
    has_device: u32,
}

unsafe extern "C" {
    fn pc_wifi_scan() -> i32;
    fn pc_wifi_list(networks: *mut WifiNetworkInfo, capacity: u32) -> i32;
    fn pc_wifi_connect(ssid: *const c_char, password: *const c_char) -> i32;
    fn pc_wifi_disconnect() -> i32;
    fn pc_wifi_status(status: *mut WifiStatusInfo) -> bool;
    fn pc_net_if_list(infos: *mut NetIfInfo, capacity: u32) -> i32;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WifiStatus {
    pub device: bool,
    pub connected: bool,
    pub state: u32,
    pub ssid: [u8; SSID_CAP],
    pub rssi: i8,
    pub ip: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WifiNet {
    pub ssid: [u8; SSID_CAP],
    pub rssi: i8,
    pub channel: u8,
    pub security: u8,
}

impl WifiNet {
    pub const fn empty() -> Self {
        WifiNet { ssid: [0; SSID_CAP], rssi: 0, channel: 0, security: 0 }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Wired {
    pub link: bool,
    pub ip: u32,
}

pub fn ssid_str(ssid: &[u8; SSID_CAP]) -> &str {
    let n = ssid.iter().position(|&b| b == 0).unwrap_or(SSID_CAP);
    core::str::from_utf8(&ssid[..n]).unwrap_or("")
}

pub fn wifi_status() -> WifiStatus {
    let mut info: WifiStatusInfo = unsafe { core::mem::zeroed() };
    let ok = unsafe { pc_wifi_status(&mut info) };
    if !ok || info.has_device == 0 {
        return WifiStatus { device: false, connected: false, state: 0, ssid: [0; SSID_CAP], rssi: 0, ip: 0 };
    }
    let mut ssid = info.ssid;
    ssid[SSID_CAP - 1] = 0;
    WifiStatus {
        device: true,
        connected: info.connected != 0,
        state: info.state,
        ssid,
        rssi: info.rssi,
        ip: info.ip_address,
    }
}

pub fn wifi_scan() -> i32 {
    unsafe { pc_wifi_scan() }
}

pub fn wifi_list(out: &mut [WifiNet]) -> usize {
    let mut raw: [WifiNetworkInfo; SCAN_MAX] = unsafe { core::mem::zeroed() };
    let n = unsafe { pc_wifi_list(raw.as_mut_ptr(), SCAN_MAX as u32) };
    if n <= 0 {
        return 0;
    }
    let count = (n as usize).min(SCAN_MAX).min(out.len());
    for i in 0..count {
        let mut ssid = raw[i].ssid;
        ssid[SSID_CAP - 1] = 0;
        out[i] = WifiNet { ssid, rssi: raw[i].rssi, channel: raw[i].channel, security: raw[i].security };
    }
    count
}

pub fn wifi_connect(ssid: &[u8; SSID_CAP]) -> i32 {
    let mut name = *ssid;
    name[SSID_CAP - 1] = 0;
    let empty = [0u8; 1];
    unsafe { pc_wifi_connect(name.as_ptr() as *const c_char, empty.as_ptr() as *const c_char) }
}

pub fn wifi_disconnect() -> i32 {
    unsafe { pc_wifi_disconnect() }
}

pub fn wired() -> Option<Wired> {
    let mut list: [NetIfInfo; IF_MAX] = unsafe { core::mem::zeroed() };
    let n = unsafe { pc_net_if_list(list.as_mut_ptr(), IF_MAX as u32) };
    if n <= 0 {
        return None;
    }
    let mut best: Option<Wired> = None;
    for info in &list[..(n as usize).min(IF_MAX)] {
        if info.name.starts_with(b"wl") || &info.name[..3] == b"lo\0" || info.name[0] == 0 {
            continue;
        }
        let candidate = Wired { link: info.link_up != 0, ip: if info.has_ip != 0 { info.ip_address } else { 0 } };
        best = match best {
            Some(prev) if prev.ip != 0 || (prev.link && !candidate.link) => Some(prev),
            _ => Some(candidate),
        };
    }
    best
}

pub fn format_ip(ip: u32, out: &mut [u8; 16]) -> &str {
    let mut n = 0;
    for i in 0..4 {
        let mut octet = (ip >> (i * 8)) & 255;
        let mut digits = [0u8; 3];
        let mut d = 0;
        loop {
            digits[d] = b'0' + (octet % 10) as u8;
            d += 1;
            octet /= 10;
            if octet == 0 {
                break;
            }
        }
        while d > 0 {
            d -= 1;
            out[n] = digits[d];
            n += 1;
        }
        if i < 3 {
            out[n] = b'.';
            n += 1;
        }
    }
    core::str::from_utf8(&out[..n]).unwrap_or("")
}

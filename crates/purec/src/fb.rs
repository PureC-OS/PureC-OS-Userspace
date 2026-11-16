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
const SYS_TIME_GET: u64 = 319;
const SYS_BATTERY_INFO: u64 = 220;

#[repr(C)]
struct BatteryInfo {
    present: u32,
    percent: u32,
    charging: u32,
    remaining_minutes: u32,
    voltage_mv: u32,
    current_ma: u32,
    name: [u8; 32],
    status_text: [u8; 32],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u32,
    pub charging: bool,
    pub remaining_minutes: u32,
}

pub fn battery() -> Option<Battery> {
    let mut info = BatteryInfo {
        present: 0,
        percent: 0,
        charging: 0,
        remaining_minutes: 0,
        voltage_mv: 0,
        current_ma: 0,
        name: [0; 32],
        status_text: [0; 32],
    };
    let result =
        unsafe { super::syscall(SYS_BATTERY_INFO, &mut info as *mut BatteryInfo as u64, 0, 0) };
    if result < 0 || info.present == 0 || info.percent > 100 {
        return None;
    }
    Some(Battery {
        percent: info.percent,
        charging: info.charging != 0,
        remaining_minutes: info.remaining_minutes,
    })
}

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


/// Kernel wall clock: (unix seconds, time zone offset in minutes), None when unset.
pub fn wall_clock() -> Option<(i64, i32)> {
    let mut info = TimeInfo {
        unix_seconds: 0,
        millis: 0,
        tz_minutes: 0,
        uptime_ms: 0,
        source: 0,
        reserved: 0,
    };
    let result = unsafe { super::syscall(SYS_TIME_GET, &mut info as *mut TimeInfo as u64, 0, 0) };
    if result < 0 || info.source == 0 || info.unix_seconds <= 0 {
        None
    } else {
        Some((info.unix_seconds, info.tz_minutes))
    }
}

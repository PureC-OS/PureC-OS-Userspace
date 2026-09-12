use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static FS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
static OPEN: OnceLock<Mutex<HashMap<i32, (String, usize)>>> = OnceLock::new();
static NEXT_FD: Mutex<i32> = Mutex::new(10);

fn fs() -> std::sync::MutexGuard<'static, HashMap<String, Vec<u8>>> {
    FS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap()
}

fn open() -> std::sync::MutexGuard<'static, HashMap<i32, (String, usize)>> {
    OPEN.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap()
}

pub fn register(path: &str, data: Vec<u8>) {
    fs().insert(path.to_string(), data);
}

pub fn file_open(path: &str) -> i32 {
    if !fs().contains_key(path) {
        return -1;
    }
    let mut next = NEXT_FD.lock().unwrap();
    let fd = *next;
    *next += 1;
    open().insert(fd, (path.to_string(), 0));
    fd
}

pub fn file_read(fd: i32, buf: &mut [u8]) -> i32 {
    let (path, pos) = match open().get(&fd).cloned() {
        Some(entry) => entry,
        None => return -1,
    };
    let data = match fs().get(&path).cloned() {
        Some(data) => data,
        None => return -1,
    };
    if pos >= data.len() {
        return 0;
    }
    let n = (data.len() - pos).min(buf.len());
    buf[..n].copy_from_slice(&data[pos..pos + n]);
    open().insert(fd, (path, pos + n));
    n as i32
}

pub fn file_close(fd: i32) -> i32 {
    open().remove(&fd);
    0
}

pub fn heap_grow(bytes: u64) -> Option<*mut u8> {
    let mut v: Vec<u8> = Vec::with_capacity(bytes as usize);
    v.resize(bytes as usize, 0);
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    Some(ptr)
}

pub fn uptime_ms() -> u64 {
    0
}

//! Dead by Daylight mode: files in the settings folder, background update of the killer list,
//! local time. Everything here is best effort: failures never reach the user.

use crate::dbd::{KillerList, State};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const LIST_URL: &str = "https://raw.githubusercontent.com/GrigoryyKach/random-wheel/main/data/dbd_killers.json";

fn dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Random Wheel"))
}

fn state_path() -> Option<PathBuf> {
    dir().map(|d| d.join("dbd.json"))
}

fn cache_path() -> Option<PathBuf> {
    dir().map(|d| d.join("dbd_killers_cache.json"))
}

/// Write through a temporary file so a crash never leaves a half-written file.
fn write_atomic(path: &std::path::Path, data: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub fn load_state() -> State {
    state_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

pub fn save_state(state: &State) {
    if let (Some(p), Ok(json)) = (state_path(), serde_json::to_string_pretty(state)) {
        let _ = write_atomic(&p, &json);
    }
}

/// Embedded list, replaced by the cached one if that is newer.
pub fn load_list() -> KillerList {
    let cached = cache_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| KillerList::parse(&s));
    KillerList::newest(KillerList::embedded(), cached)
}

/// Background fetch of the list from the repository. The result lands in the returned slot;
/// it is cached on disk when it is newer than `current_version`.
pub fn fetch_list(current_version: u32, on_done: impl FnOnce() + Send + 'static) -> Arc<Mutex<Option<KillerList>>> {
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::spawn(move || {
        let res = Command::new("curl.exe")
            .args(["-sfL", "--max-time", "10", "-A", "RandomWheel", LIST_URL])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        if let Ok(o) = res {
            if o.status.success() {
                if let Some(list) = KillerList::parse(&String::from_utf8_lossy(&o.stdout)) {
                    if list.version > current_version {
                        if let (Some(p), Ok(json)) = (cache_path(), serde_json::to_string_pretty(&list)) {
                            let _ = write_atomic(&p, &json);
                        }
                        *out.lock().unwrap() = Some(list);
                        on_done();
                    }
                }
            }
        }
    });
    slot
}

#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    millis: u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(t: *mut SystemTime);
}

/// Local date and time as `YYYY-MM-DD HH:MM`.
pub fn now_string() -> String {
    let mut t = SystemTime::default();
    // SAFETY: GetLocalTime only writes a SYSTEMTIME into the buffer we pass.
    unsafe { GetLocalTime(&mut t) };
    format!("{:04}-{:02}-{:02} {:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute)
}

/// Seconds since the Unix epoch (used for unique ids of custom killers).
pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

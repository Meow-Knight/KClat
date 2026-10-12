//! Small local log (spec §8.4): `%APPDATA%\Klat\klat.log`, capped in size.
//!
//! When the file passes MAX_BYTES it becomes `klat.log.old` (replacing the previous one), so the
//! log never takes more than ~2 × MAX_BYTES. Callers log event names, session ids, project folder names
//! and outcomes only: never prompts, commands, tool input or transcripts (the app never receives them).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_BYTES: u64 = 512 * 1024;

struct Sink {
    path: PathBuf,
    file: Option<File>,
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);

/// Starts logging to `dir\klat.log`. Logging is best effort: failures are ignored.
pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    *SINK.lock().unwrap() = Some(Sink {
        path: dir.join("klat.log"),
        file: None,
    });
}

pub fn info(msg: impl AsRef<str>) {
    write("INFO", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    write("WARN", msg.as_ref());
}

fn write(level: &str, msg: &str) {
    let line = format!("{} {level} {}\n", timestamp(now_secs()), one_line(msg));
    if cfg!(debug_assertions) {
        eprint!("klat: {line}");
    }
    let Ok(mut guard) = SINK.lock() else { return };
    let Some(sink) = guard.as_mut() else { return };
    let _ = append(sink, &line);
}

fn append(sink: &mut Sink, line: &str) -> std::io::Result<()> {
    if fs::metadata(&sink.path).map(|m| m.len()).unwrap_or(0) + line.len() as u64 > MAX_BYTES {
        sink.file = None; // close before renaming
        let mut old = sink.path.as_os_str().to_owned();
        old.push(".old");
        let _ = fs::rename(&sink.path, PathBuf::from(old));
    }
    if sink.file.is_none() {
        sink.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&sink.path)?,
        );
    }
    sink.file.as_mut().unwrap().write_all(line.as_bytes())
}

/// Keeps one entry per line and bounds its length, whatever the caller passes.
fn one_line(msg: &str) -> String {
    let flat: String = msg
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if flat.chars().count() > 300 {
        flat.chars().take(300).collect::<String>() + "…"
    } else {
        flat
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `YYYY-MM-DDTHH:MM:SSZ` (UTC) without a date crate: days-from-civil inverse (Howard Hinnant).
fn timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_iso() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(timestamp(1_791_723_996), "2026-10-11T13:06:36Z");
    }

    #[test]
    fn messages_are_flattened_and_bounded() {
        assert_eq!(one_line("a\nb\tc"), "a b c");
        assert_eq!(one_line(&"x".repeat(1000)).chars().count(), 301);
    }

    #[test]
    fn rotates_past_the_cap_and_never_grows_beyond_two_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = Sink {
            path: dir.path().join("klat.log"),
            file: None,
        };
        let line = format!("{}\n", "y".repeat(1023));
        for _ in 0..(3 * MAX_BYTES / 1024) {
            append(&mut sink, &line).unwrap();
        }
        let main = fs::metadata(&sink.path).unwrap().len();
        let old = fs::metadata(dir.path().join("klat.log.old")).unwrap().len();
        assert!(main <= MAX_BYTES && old <= MAX_BYTES, "{main} {old}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}

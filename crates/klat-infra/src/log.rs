//! Small local log (spec §8.4): `klat.log` in the data folder (see data_dir.rs), capped in size, behind the
//! `log` facade, so any crate logs with `log::info!` / `log::warn!` without depending on the app.
//!
//! When the file passes MAX_BYTES it becomes `klat.log.old` (replacing the previous one), so the
//! log never takes more than ~2 × MAX_BYTES. Callers log event names, session ids, project folder names
//! and outcomes only: never prompts, commands, tool input or transcripts (the app never receives them).
//!
//! Only Klat's own crates are written: once a logger is installed, Tauri, wry and tao send their records to it
//! too, and those are neither bounded by that rule nor useful in this file.

use log::{Level, LevelFilter, Log, Metadata, Record};
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

/// Writes Klat's records at Info and above to `klat.log`.
pub struct FileLogger {
    sink: Mutex<Sink>,
}

impl FileLogger {
    /// Logs to `dir/klat.log`. Logging is best effort: failures are ignored.
    pub fn new(dir: &Path) -> Self {
        let _ = fs::create_dir_all(dir);
        Self {
            sink: Mutex::new(Sink {
                path: dir.join("klat.log"),
                file: None,
            }),
        }
    }
}

/// Installs the file logger for the whole process. Call once, at start-up; a second call changes nothing.
pub fn init(dir: &Path) {
    if log::set_boxed_logger(Box::new(FileLogger::new(dir))).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Records from Klat's crates: the app (`klat`) and `klat_*`.
fn is_ours(target: &str) -> bool {
    let root = target.split("::").next().unwrap_or(target);
    root == "klat" || root.starts_with("klat_")
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info && is_ours(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {} {}\n",
            timestamp(now_secs()),
            record.level(),
            one_line(&record.args().to_string())
        );
        if cfg!(debug_assertions) {
            eprint!("klat: {line}");
        }
        let Ok(mut sink) = self.sink.lock() else {
            return;
        };
        let _ = append(&mut sink, &line);
    }

    fn flush(&self) {}
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

    fn send(logger: &FileLogger, level: Level, target: &str, msg: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{msg}"))
                .build(),
        );
    }

    fn written(dir: &Path) -> String {
        fs::read_to_string(dir.join("klat.log")).unwrap_or_default()
    }

    #[test]
    fn lines_keep_the_format_of_the_old_log() {
        let dir = tempfile::tempdir().unwrap();
        let logger = FileLogger::new(dir.path());
        send(&logger, Level::Info, "klat::state", "ready armed");
        send(
            &logger,
            Level::Warn,
            "klat_infra::event_server",
            "two\nlines",
        );
        let text = written(dir.path());
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text:?}");
        // `YYYY-MM-DDTHH:MM:SSZ LEVEL message`, one line each.
        for (line, rest) in lines.iter().zip([" INFO ready armed", " WARN two lines"]) {
            let (stamp, tail) = line.split_at(20);
            assert!(
                stamp.ends_with('Z') && stamp.as_bytes()[10] == b'T',
                "{line}"
            );
            assert_eq!(tail, rest);
        }
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn records_of_other_crates_and_debug_records_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let logger = FileLogger::new(dir.path());
        for target in [
            "tauri::app",
            "wry::webview",
            "tao",
            "klatter",
            "my_klat::x",
            "",
        ] {
            send(&logger, Level::Warn, target, "not ours");
        }
        send(&logger, Level::Debug, "klat::state", "too chatty");
        send(&logger, Level::Trace, "klat", "too chatty");
        assert_eq!(written(dir.path()), "");
        send(&logger, Level::Error, "klat", "ours");
        assert!(written(dir.path()).ends_with(" ERROR ours\n"));
    }

    #[test]
    fn klat_crates_are_ours() {
        for target in ["klat", "klat::drag", "klat_infra::log", "klat_core"] {
            assert!(is_ours(target), "{target}");
        }
        for target in ["tauri", "tauri_plugin_dialog", "klatter::x", "xklat", ""] {
            assert!(!is_ours(target), "{target}");
        }
    }
}

//! Settings file `%APPDATA%\ClaudeCat\settings.json` (spec §10).
//!
//! Loading never fails: a missing or invalid field falls back to its default, and a file that is not a
//! JSON object is renamed to `settings.json.corrupt.<unix-seconds>` and replaced with defaults.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LANGUAGES: [&str; 2] = ["en", "vi"];
pub const CORNERS: [&str; 1] = ["bottom-right"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub notifications_enabled: bool,
    pub hide_while_sleeping: bool,
    pub sfw_mode: bool,
    pub languages: Vec<String>,
    pub yell_interval_ms: u64,
    pub max_yells_per_wake: u32,
    pub stale_after_ms: u64,
    pub reduce_motion: bool,
    pub start_at_login: bool,
    pub corner: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications_enabled: true,
            hide_while_sleeping: false,
            sfw_mode: false,
            languages: vec!["en".into(), "vi".into()],
            yell_interval_ms: 30_000,
            max_yells_per_wake: 10,
            stale_after_ms: 1_800_000,
            reduce_motion: false,
            start_at_login: false,
            corner: "bottom-right".into(),
        }
    }
}

const MIN_YELL_INTERVAL_MS: u64 = 1_000;
const MIN_STALE_AFTER_MS: u64 = 60_000;

impl Settings {
    /// Builds settings from a JSON object field by field; anything missing, mistyped or out of range keeps its default.
    fn from_object(obj: &Map<String, Value>) -> Self {
        let d = Self::default();
        let mut languages: Vec<String> = field(obj, "languages", d.languages.clone());
        languages.retain(|l| LANGUAGES.contains(&l.as_str()));
        languages.dedup();
        let corner: String = field(obj, "corner", d.corner.clone());
        Self {
            notifications_enabled: field(obj, "notificationsEnabled", d.notifications_enabled),
            hide_while_sleeping: field(obj, "hideWhileSleeping", d.hide_while_sleeping),
            sfw_mode: field(obj, "sfwMode", d.sfw_mode),
            languages: if languages.is_empty() {
                d.languages
            } else {
                languages
            },
            yell_interval_ms: field(obj, "yellIntervalMs", d.yell_interval_ms)
                .max(MIN_YELL_INTERVAL_MS),
            max_yells_per_wake: field(obj, "maxYellsPerWake", d.max_yells_per_wake).max(1),
            stale_after_ms: field(obj, "staleAfterMs", d.stale_after_ms).max(MIN_STALE_AFTER_MS),
            reduce_motion: field(obj, "reduceMotion", d.reduce_motion),
            start_at_login: field(obj, "startAtLogin", d.start_at_login),
            corner: if CORNERS.contains(&corner.as_str()) {
                corner
            } else {
                d.corner
            },
        }
    }
}

fn field<T: DeserializeOwned>(obj: &Map<String, Value>, key: &str, default: T) -> T {
    obj.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoadOutcome {
    Loaded,
    CreatedDefault,
    /// The broken file was moved here.
    RecoveredCorrupt(PathBuf),
}

/// `%APPDATA%\ClaudeCat`, or None when APPDATA is not set.
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("ClaudeCat"))
}

pub fn load(path: &Path) -> (Settings, LoadOutcome) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let s = Settings::default();
            let _ = save(path, &s);
            return (s, LoadOutcome::CreatedDefault);
        }
        Err(_) => return recover(path),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(obj)) => (Settings::from_object(&obj), LoadOutcome::Loaded),
        _ => recover(path),
    }
}

fn recover(path: &Path) -> (Settings, LoadOutcome) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut moved = path.as_os_str().to_owned();
    moved.push(format!(".corrupt.{ts}"));
    let moved = PathBuf::from(moved);
    let _ = fs::rename(path, &moved);
    let s = Settings::default();
    let _ = save(path, &s);
    (s, LoadOutcome::RecoveredCorrupt(moved))
}

/// Atomic replace: write a sibling temp file, then rename over the target.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    fs::write(&tmp, json + "\n")?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ClaudeCat").join("settings.json");
        (dir, path)
    }

    #[test]
    fn missing_file_creates_defaults_on_disk() {
        let (_d, path) = temp();
        let (s, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::CreatedDefault);
        assert_eq!(s, Settings::default());
        let (again, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(again, s);
    }

    #[test]
    fn defaults_match_the_spec() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            v,
            json!({
                "notificationsEnabled": true, "hideWhileSleeping": false, "sfwMode": false,
                "languages": ["en", "vi"], "yellIntervalMs": 30000, "maxYellsPerWake": 10,
                "staleAfterMs": 1800000, "reduceMotion": false, "startAtLogin": false, "corner": "bottom-right"
            })
        );
    }

    #[test]
    fn missing_fields_use_defaults_and_present_ones_are_kept() {
        let (_d, path) = temp();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{ "sfwMode": true, "yellIntervalMs": 5000 }"#).unwrap();
        let (s, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert!(s.sfw_mode);
        assert_eq!(s.yell_interval_ms, 5000);
        assert_eq!(s.max_yells_per_wake, 10);
        assert_eq!(s.languages, vec!["en", "vi"]);
    }

    #[test]
    fn mistyped_and_out_of_range_fields_fall_back_individually() {
        let obj = json!({
            "sfwMode": "yes", "reduceMotion": true, "yellIntervalMs": 10, "maxYellsPerWake": 0,
            "staleAfterMs": -5, "languages": ["vi", "fr", "vi"], "corner": "top-left", "unknown": 1
        });
        let s = Settings::from_object(obj.as_object().unwrap());
        assert!(!s.sfw_mode);
        assert!(s.reduce_motion);
        assert_eq!(s.yell_interval_ms, MIN_YELL_INTERVAL_MS);
        assert_eq!(s.max_yells_per_wake, 1);
        assert_eq!(s.stale_after_ms, 1_800_000);
        assert_eq!(s.languages, vec!["vi"]);
        assert_eq!(s.corner, "bottom-right");
    }

    #[test]
    fn empty_language_list_falls_back_to_default() {
        let s = Settings::from_object(json!({ "languages": [] }).as_object().unwrap());
        assert_eq!(s.languages, vec!["en", "vi"]);
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_replaced() {
        for broken in ["{ not json", "[1, 2]", "", "\"text\""] {
            let (_d, path) = temp();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, broken).unwrap();
            let (s, outcome) = load(&path);
            assert_eq!(s, Settings::default());
            let LoadOutcome::RecoveredCorrupt(moved) = outcome else {
                panic!("expected recovery for {broken:?}");
            };
            assert!(moved.to_string_lossy().contains("settings.json.corrupt."));
            assert_eq!(fs::read_to_string(&moved).unwrap(), broken);
            assert_eq!(load(&path), (Settings::default(), LoadOutcome::Loaded));
        }
    }

    #[test]
    fn save_round_trips_and_leaves_no_temp_file() {
        let (_d, path) = temp();
        let s = Settings {
            sfw_mode: true,
            hide_while_sleeping: true,
            languages: vec!["vi".into()],
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        save(&path, &s).unwrap(); // replacing an existing file works too
        assert_eq!(load(&path).0, s);
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec!["settings.json"]);
    }
}

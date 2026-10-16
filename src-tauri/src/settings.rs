//! Settings file `settings.json` in the data folder (`%APPDATA%\Klat`, `~/Library/Application Support/Klat`) (spec §10, docs/specs/settings-i18n-macos.md §2).
//!
//! Loading never fails: a missing or invalid field falls back to its default, and a file that is not a
//! JSON object is renamed to `settings.json.corrupt.<unix-seconds>` and replaced with defaults.

use klat_core::geometry::WindowPos;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CORNERS: [&str; 1] = ["bottom-right"];

/// Platform Language: the tray, the Settings window and dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    #[default]
    En,
    Vi,
}

/// Klat Language: what the cat says. Auto mixes both languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KlatLanguage {
    En,
    Vi,
    #[default]
    Auto,
}

/// Who sits in the corner (src/characters in the frontend). The overlay draws it; Rust only stores the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Character {
    #[default]
    Cat,
    Snake,
    Dragon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub character: Character,
    pub notifications_enabled: bool,
    pub hide_while_sleeping: bool,
    pub sfw_mode: bool,
    pub ui_language: UiLanguage,
    pub klat_language: KlatLanguage,
    pub yell_interval_ms: u64,
    pub max_yells_per_wake: u32,
    pub stale_after_ms: u64,
    pub reduce_motion: bool,
    /// Size of the cat in percent (the whole overlay: cat, bubble and Ready button).
    pub cat_scale: u32,
    pub start_at_login: bool,
    pub corner: String,
    /// Where you dragged the cat (window top-left, physical px); None = the corner.
    pub position: Option<WindowPos>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            character: Character::Cat,
            notifications_enabled: true,
            hide_while_sleeping: false,
            sfw_mode: false,
            ui_language: UiLanguage::En,
            klat_language: KlatLanguage::Auto,
            yell_interval_ms: 30_000,
            max_yells_per_wake: 10,
            stale_after_ms: 1_800_000,
            reduce_motion: false,
            cat_scale: 100,
            start_at_login: false,
            corner: "bottom-right".into(),
            position: None,
        }
    }
}

const YELL_INTERVAL_MS: (u64, u64) = (5_000, 600_000);
const MAX_YELLS_PER_WAKE: (u32, u32) = (1, 50);
const STALE_AFTER_MS: (u64, u64) = (300_000, 14_400_000);
pub const CAT_SCALE: (u32, u32) = (50, 200);

/// Fields the Settings window may change. `position` belongs to dragging, `corner` has one value, and
/// `startAtLogin` must go through the OS (see `autostart`).
pub const PATCHABLE: [&str; 11] = [
    "character",
    "notificationsEnabled",
    "hideWhileSleeping",
    "sfwMode",
    "uiLanguage",
    "klatLanguage",
    "yellIntervalMs",
    "maxYellsPerWake",
    "staleAfterMs",
    "reduceMotion",
    "catScale",
];

impl Settings {
    /// `catScale` as a factor: the overlay window's size and its webview zoom.
    pub fn zoom(&self) -> f64 {
        f64::from(self.cat_scale) / 100.0
    }

    /// Builds settings from a JSON object field by field; anything missing or mistyped keeps its default, and
    /// numbers outside their range are clamped into it.
    fn from_object(obj: &Map<String, Value>) -> Self {
        let d = Self::default();
        let corner: String = field(obj, "corner", d.corner.clone());
        Self {
            character: field(obj, "character", d.character),
            notifications_enabled: field(obj, "notificationsEnabled", d.notifications_enabled),
            hide_while_sleeping: field(obj, "hideWhileSleeping", d.hide_while_sleeping),
            sfw_mode: field(obj, "sfwMode", d.sfw_mode),
            ui_language: field(obj, "uiLanguage", d.ui_language),
            klat_language: if obj.contains_key("klatLanguage") {
                field(obj, "klatLanguage", d.klat_language)
            } else {
                klat_language_from_legacy(obj.get("languages"))
            },
            yell_interval_ms: field(obj, "yellIntervalMs", d.yell_interval_ms)
                .clamp(YELL_INTERVAL_MS.0, YELL_INTERVAL_MS.1),
            max_yells_per_wake: field(obj, "maxYellsPerWake", d.max_yells_per_wake)
                .clamp(MAX_YELLS_PER_WAKE.0, MAX_YELLS_PER_WAKE.1),
            stale_after_ms: field(obj, "staleAfterMs", d.stale_after_ms)
                .clamp(STALE_AFTER_MS.0, STALE_AFTER_MS.1),
            reduce_motion: field(obj, "reduceMotion", d.reduce_motion),
            cat_scale: field(obj, "catScale", d.cat_scale).clamp(CAT_SCALE.0, CAT_SCALE.1),
            start_at_login: field(obj, "startAtLogin", d.start_at_login),
            corner: if CORNERS.contains(&corner.as_str()) {
                corner
            } else {
                d.corner
            },
            position: field(obj, "position", d.position),
        }
    }
}

/// Files written before Klat Language existed have `languages`: the phrase languages the cat may use.
fn klat_language_from_legacy(languages: Option<&Value>) -> KlatLanguage {
    let has = |lang: &str| {
        languages
            .and_then(Value::as_array)
            .is_some_and(|l| l.iter().any(|v| v.as_str() == Some(lang)))
    };
    match (has("en"), has("vi")) {
        (true, false) => KlatLanguage::En,
        (false, true) => KlatLanguage::Vi,
        _ => KlatLanguage::Auto,
    }
}

/// A Settings window edit: the patch is merged into the current settings and the load-time validation runs
/// again, so a value the UI should never send is dropped or clamped exactly like a hand-edited file.
pub fn apply_patch(current: &Settings, patch: &Map<String, Value>) -> Settings {
    let Ok(Value::Object(mut obj)) = serde_json::to_value(current) else {
        return current.clone();
    };
    for (k, v) in patch {
        if PATCHABLE.contains(&k.as_str()) {
            obj.insert(k.clone(), v.clone());
        }
    }
    Settings::from_object(&obj)
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

/// Moves `%APPDATA%\ClaudeCat` (the folder from before the rename to Klat) to `%APPDATA%\Klat` once,
/// so settings carry over. Windows only: Claude Cat never ran anywhere else. Best effort: if it fails, the app
/// starts with defaults.
pub fn migrate_legacy_dir() {
    if !cfg!(windows) {
        return;
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        migrate_legacy_dir_in(Path::new(&appdata));
    }
}

fn migrate_legacy_dir_in(appdata: &Path) {
    let (old, new) = (appdata.join("ClaudeCat"), appdata.join("Klat"));
    if old.is_dir() && !new.exists() {
        let _ = fs::rename(old, new);
    }
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
        let path = dir.path().join("Klat").join("settings.json");
        (dir, path)
    }

    #[test]
    fn legacy_dir_moves_once_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("ClaudeCat");
        fs::create_dir(&old).unwrap();
        fs::write(old.join("settings.json"), "{}").unwrap();
        migrate_legacy_dir_in(dir.path());
        assert!(!old.exists());
        assert!(dir.path().join("Klat").join("settings.json").exists());

        fs::create_dir(&old).unwrap();
        migrate_legacy_dir_in(dir.path());
        assert!(old.exists()); // Klat already exists: leave both alone
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
                "character": "cat",
                "notificationsEnabled": true, "hideWhileSleeping": false, "sfwMode": false,
                "uiLanguage": "en", "klatLanguage": "auto", "yellIntervalMs": 30000, "maxYellsPerWake": 10,
                "staleAfterMs": 1800000, "reduceMotion": false, "catScale": 100, "startAtLogin": false,
                "corner": "bottom-right",
                "position": null
            })
        );
    }

    #[test]
    fn missing_fields_use_defaults_and_present_ones_are_kept() {
        let (_d, path) = temp();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{ "sfwMode": true, "yellIntervalMs": 6000 }"#).unwrap();
        let (s, outcome) = load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert!(s.sfw_mode);
        assert_eq!(s.yell_interval_ms, 6000);
        assert_eq!(s.max_yells_per_wake, 10);
        assert_eq!(s.ui_language, UiLanguage::En);
        assert_eq!(s.klat_language, KlatLanguage::Auto);
        // Files written before characters existed get the cat.
        assert_eq!(s.character, Character::Cat);
    }

    #[test]
    fn mistyped_and_out_of_range_fields_fall_back_individually() {
        let obj = json!({
            "sfwMode": "yes", "reduceMotion": true, "yellIntervalMs": 10, "maxYellsPerWake": 0,
            "staleAfterMs": -5, "uiLanguage": "fr", "klatLanguage": 3, "corner": "top-left", "unknown": 1,
            "character": "unicorn"
        });
        let s = Settings::from_object(obj.as_object().unwrap());
        assert!(!s.sfw_mode);
        assert!(s.reduce_motion);
        assert_eq!(s.yell_interval_ms, YELL_INTERVAL_MS.0);
        assert_eq!(s.max_yells_per_wake, 1);
        assert_eq!(s.stale_after_ms, 1_800_000);
        assert_eq!(s.ui_language, UiLanguage::En);
        assert_eq!(s.klat_language, KlatLanguage::Auto);
        assert_eq!(s.corner, "bottom-right");
        assert_eq!(s.character, Character::Cat);
    }

    #[test]
    fn numbers_are_clamped_into_their_range() {
        let obj = |y: u64, m: u32, st: u64| json!({ "yellIntervalMs": y, "maxYellsPerWake": m, "staleAfterMs": st });
        let low = Settings::from_object(obj(1_000, 0, 60_000).as_object().unwrap());
        assert_eq!(
            (
                low.yell_interval_ms,
                low.max_yells_per_wake,
                low.stale_after_ms
            ),
            (5_000, 1, 300_000)
        );
        let high = Settings::from_object(obj(u64::MAX, 999, u64::MAX).as_object().unwrap());
        assert_eq!(
            (
                high.yell_interval_ms,
                high.max_yells_per_wake,
                high.stale_after_ms
            ),
            (600_000, 50, 14_400_000)
        );
    }

    #[test]
    fn cat_scale_is_clamped_and_falls_back() {
        let scale = |v: Value| {
            Settings::from_object(json!({ "catScale": v }).as_object().unwrap()).cat_scale
        };
        assert_eq!(scale(json!(150)), 150);
        assert_eq!(scale(json!(10)), 50);
        assert_eq!(scale(json!(1000)), 200);
        assert_eq!(scale(json!("big")), 100);
        assert_eq!(scale(json!(-3)), 100);
        let patched = apply_patch(
            &Settings::default(),
            json!({ "catScale": 125 }).as_object().unwrap(),
        );
        assert_eq!(patched.cat_scale, 125);
        assert_eq!(patched.zoom(), 1.25);
    }

    #[test]
    fn languages_from_older_files_become_klat_language() {
        let klat = |v: Value| Settings::from_object(v.as_object().unwrap()).klat_language;
        assert_eq!(
            klat(json!({ "languages": ["en", "vi"] })),
            KlatLanguage::Auto
        );
        assert_eq!(
            klat(json!({ "languages": ["vi", "en", "fr"] })),
            KlatLanguage::Auto
        );
        assert_eq!(klat(json!({ "languages": ["en"] })), KlatLanguage::En);
        assert_eq!(
            klat(json!({ "languages": ["vi", "fr", "vi"] })),
            KlatLanguage::Vi
        );
        assert_eq!(klat(json!({ "languages": ["fr"] })), KlatLanguage::Auto);
        assert_eq!(klat(json!({ "languages": [] })), KlatLanguage::Auto);
        assert_eq!(klat(json!({ "languages": "vi" })), KlatLanguage::Auto);
        assert_eq!(klat(json!({})), KlatLanguage::Auto);
        // Once klatLanguage exists the old list is ignored, even when klatLanguage is invalid.
        assert_eq!(
            klat(json!({ "languages": ["vi"], "klatLanguage": "en" })),
            KlatLanguage::En
        );
        assert_eq!(
            klat(json!({ "languages": ["vi"], "klatLanguage": "xx" })),
            KlatLanguage::Auto
        );
    }

    #[test]
    fn an_old_file_loads_migrated_and_saves_without_languages() {
        let (_d, path) = temp();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{ "languages": ["vi"], "sfwMode": true }"#).unwrap();
        let (s, _) = load(&path);
        assert_eq!(s.klat_language, KlatLanguage::Vi);
        assert!(s.sfw_mode);
        save(&path, &s).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"languages\""));
        assert!(text.contains(r#""klatLanguage": "vi""#));
    }

    #[test]
    fn patch_changes_only_whitelisted_fields_and_revalidates() {
        let current = Settings {
            position: Some(WindowPos::new(10, 20)),
            ..Settings::default()
        };
        let patch = json!({
            "character": "dragon", "sfwMode": true, "uiLanguage": "vi", "klatLanguage": "en", "yellIntervalMs": 1,
            "position": null, "startAtLogin": true, "corner": "top-left", "bogus": 1
        });
        let next = apply_patch(&current, patch.as_object().unwrap());
        assert_eq!(next.character, Character::Dragon);
        assert!(next.sfw_mode);
        assert_eq!(next.ui_language, UiLanguage::Vi);
        assert_eq!(next.klat_language, KlatLanguage::En);
        assert_eq!(next.yell_interval_ms, 5_000);
        assert_eq!(next.position, current.position);
        assert!(!next.start_at_login);
        assert_eq!(next.corner, "bottom-right");
    }

    #[test]
    fn a_mistyped_patch_value_falls_back_to_its_default() {
        let current = Settings {
            sfw_mode: true,
            max_yells_per_wake: 3,
            ..Settings::default()
        };
        let next = apply_patch(
            &current,
            json!({ "maxYellsPerWake": 7 }).as_object().unwrap(),
        );
        assert!(next.sfw_mode);
        assert_eq!(next.max_yells_per_wake, 7);
        let next = apply_patch(&current, json!({ "sfwMode": "yes" }).as_object().unwrap());
        assert!(!next.sfw_mode);
        assert_eq!(next.max_yells_per_wake, 3);
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
            ui_language: UiLanguage::Vi,
            klat_language: KlatLanguage::Vi,
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

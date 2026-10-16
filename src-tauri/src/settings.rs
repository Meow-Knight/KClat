//! Settings file `settings.json` in the data folder (`%APPDATA%\Klat`, `~/Library/Application Support/Klat`) (spec §10, docs/specs/settings-i18n-macos.md §2).
//!
//! Loading never fails: a missing or invalid field falls back to its default, and a file that is not a
//! JSON object is renamed to `settings.json.corrupt.<unix-seconds>` and replaced with defaults.

// target: klat-infra::settings_repo (load, save, recovery, the legacy folder). The model is klat_core::settings.

use klat_core::settings::Settings;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    use klat_core::settings::{Character, KlatLanguage, UiLanguage};

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

//! Installs the Klat hooks into `~/.claude/settings.json` (spec §8.5). The edits themselves (which entries
//! are ours, what Install and Uninstall change) are klat_core::claude_hooks; this is the file around them.
//!
//! - User-level settings only; project settings are never touched.
//! - Broken JSON (or a non-object) aborts before anything is written.
//! - Before any write: a copy to `settings.json.bak.<unix-seconds>` (the newest three are kept). Writes are atomic, and refused if the
//!   file changed on disk while we were working (Claude Code edits it too).
//! - Runs only on request (Settings > Hooks), never at start-up.

pub use klat_core::claude_hooks::Status;
use klat_core::claude_hooks::{install_into, status, uninstall_from, UnexpectedShape, MARKER};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Why Install or Uninstall did nothing. The text is for the log; the Settings window shows its own,
/// translated, per variant.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// settings.json is not valid JSON; nothing was written.
    #[error("settings.json is not valid JSON ({0}). Nothing was changed.")]
    BrokenJson(String),
    /// settings.json (or its "hooks") is not a JSON object; nothing was written.
    #[error("{0} in settings.json is not a JSON object. Nothing was changed.")]
    UnexpectedShape(&'static str),
    #[error("The hook program was not found at {}.", .0.display())]
    ExeMissing(PathBuf),
    /// Someone else wrote settings.json while we were working; nothing was written.
    #[error("settings.json changed while installing. Nothing was changed; try again.")]
    ChangedConcurrently,
    #[error("Could not update settings.json: {0}")]
    Io(#[from] io::Error),
}

impl From<UnexpectedShape> for InstallError {
    fn from(UnexpectedShape(what): UnexpectedShape) -> Self {
        Self::UnexpectedShape(what)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// Already as requested; the file was not touched.
    NoChange,
    Written {
        backup: Option<PathBuf>,
    },
}

/// `~/.claude/settings.json` (`%USERPROFILE%` on Windows, `$HOME` elsewhere).
pub fn user_settings_path() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|home| PathBuf::from(home).join(".claude").join("settings.json"))
}

/// `klat-hook.exe` on Windows, `klat-hook` elsewhere.
pub fn adapter_file_name() -> String {
    format!("{MARKER}{}", std::env::consts::EXE_SUFFIX)
}

/// The adapter ships next to the app executable (dev: `target/debug`, Windows install dir, or
/// `Klat.app/Contents/MacOS`).
pub fn default_exe_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe.with_file_name(adapter_file_name()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Install,
    Uninstall,
}

/// Reads, edits and writes the settings file. See the module docs for the safety rules.
pub fn apply(path: &Path, exe: &Path, action: Action) -> Result<Report, InstallError> {
    if action == Action::Install && !exe.is_file() {
        return Err(InstallError::ExeMissing(exe.to_path_buf()));
    }
    let original = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let mut settings = match &original {
        None => {
            if action == Action::Uninstall {
                return Ok(Report::NoChange);
            }
            Value::Object(Map::new())
        }
        Some(text) => serde_json::from_str::<Value>(text)
            .map_err(|e| InstallError::BrokenJson(e.to_string()))?,
    };
    if !settings.is_object() {
        return Err(InstallError::UnexpectedShape("The top level"));
    }
    let before = settings.clone();
    let exe_str = exe.to_string_lossy();
    match action {
        Action::Install => install_into(&mut settings, &exe_str)?,
        Action::Uninstall => {
            uninstall_from(&mut settings)?;
        }
    }
    if settings == before {
        return Ok(Report::NoChange);
    }

    let mut text = serde_json::to_string_pretty(&settings)
        .map_err(|e| InstallError::Io(io::Error::other(e)))?;
    if original.as_deref().is_none_or(|t| t.ends_with('\n')) {
        text.push('\n');
    }

    let backup = match &original {
        Some(text) => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let mut name = path.as_os_str().to_owned();
            name.push(format!(".bak.{ts}"));
            let backup = PathBuf::from(name);
            fs::write(&backup, text)?;
            prune_backups(path, KEEP_BACKUPS);
            Some(backup)
        }
        None => {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            None
        }
    };

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".klat.tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, &text)?;
    // Optimistic check: if the file moved under us, keep theirs.
    let now = fs::read_to_string(path).ok();
    if now != original {
        let _ = fs::remove_file(&tmp);
        return Err(InstallError::ChangedConcurrently);
    }
    fs::rename(&tmp, path)?;
    Ok(Report::Written { backup })
}

/// Backups kept next to settings.json. The file can hold secrets (`env`, `apiKeyHelper`), so copies of it should
/// not pile up.
const KEEP_BACKUPS: usize = 3;

/// Deletes our `<settings>.bak.<unix-seconds>` files beyond the newest `keep`. Any other file is left alone.
fn prune_backups(path: &Path, keep: usize) {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    let prefix = format!("{name}.bak.");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut ours: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name();
            let ts = file.to_str()?.strip_prefix(&prefix)?;
            if ts.is_empty() || !ts.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let ts = ts.parse::<u64>().ok()?;
            entry
                .file_type()
                .ok()?
                .is_file()
                .then(|| (ts, entry.path()))
        })
        .collect();
    ours.sort_unstable_by_key(|(ts, _)| std::cmp::Reverse(*ts));
    for (_, old) in ours.into_iter().skip(keep) {
        let _ = fs::remove_file(old);
    }
}

/// Status of the file on disk (read-only); unreadable or broken files count as not installed.
pub fn status_on_disk(path: &Path, exe: &Path) -> Status {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map_or(Status::NotInstalled, |v| status(&v, &exe.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_adapter_name_matches_this_platform() {
        let name = adapter_file_name();
        assert_eq!(name.ends_with(".exe"), cfg!(windows));
        assert!(name.starts_with(MARKER));
    }

    #[test]
    fn error_texts_for_the_log_are_unchanged() {
        let cases = [
            (
                InstallError::BrokenJson("line 1".into()),
                "settings.json is not valid JSON (line 1). Nothing was changed.",
            ),
            (
                InstallError::UnexpectedShape("\"hooks\""),
                "\"hooks\" in settings.json is not a JSON object. Nothing was changed.",
            ),
            (
                InstallError::ExeMissing(PathBuf::from("/x/klat-hook")),
                "The hook program was not found at /x/klat-hook.",
            ),
            (
                InstallError::ChangedConcurrently,
                "settings.json changed while installing. Nothing was changed; try again.",
            ),
            (
                InstallError::Io(io::Error::other("disk full")),
                "Could not update settings.json: disk full",
            ),
        ];
        for (e, text) in cases {
            assert_eq!(e.to_string(), text);
        }
        assert!(matches!(
            InstallError::from(UnexpectedShape("The top level")),
            InstallError::UnexpectedShape("The top level")
        ));
    }

    #[test]
    fn actions_use_the_names_the_settings_window_sends() {
        assert_eq!(
            serde_json::from_value::<Action>(json!("install")).unwrap(),
            Action::Install
        );
        assert_eq!(
            serde_json::from_value::<Action>(json!("uninstall")).unwrap(),
            Action::Uninstall
        );
        assert!(serde_json::from_value::<Action>(json!("repair")).is_err());
    }

    /// A realistic settings file with someone else's hooks and other keys, in a deliberate order.
    fn existing() -> Value {
        serde_json::from_str(
            r#"{
              "permissions": { "allow": ["Bash(ls)"] },
              "model": "opus",
              "hooks": {
                "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "rtk hook claude" } ] } ],
                "Stop": [ { "hooks": [ { "type": "command", "command": "notify-send done", "timeout": 5 } ] } ]
              },
              "theme": "dark"
            }"#,
        )
        .unwrap()
    }

    struct Env {
        _dir: tempfile::TempDir,
        settings: PathBuf,
        exe: PathBuf,
    }

    fn env(initial: Option<&str>) -> Env {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join(".claude").join("settings.json");
        if let Some(text) = initial {
            fs::create_dir_all(settings.parent().unwrap()).unwrap();
            fs::write(&settings, text).unwrap();
        }
        let exe = dir.path().join("Klat").join(adapter_file_name());
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"").unwrap();
        Env {
            _dir: dir,
            settings,
            exe,
        }
    }

    fn backups(e: &Env) -> Vec<PathBuf> {
        fs::read_dir(e.settings.parent().unwrap())
            .unwrap()
            .map(|d| d.unwrap().path())
            .filter(|p| p.to_string_lossy().contains(".bak."))
            .collect()
    }

    #[test]
    fn broken_json_writes_nothing() {
        let broken = "{ \"hooks\": { \"Stop\": [ ";
        let e = env(Some(broken));
        for action in [Action::Install, Action::Uninstall] {
            assert!(matches!(
                apply(&e.settings, &e.exe, action),
                Err(InstallError::BrokenJson(_))
            ));
        }
        assert_eq!(fs::read_to_string(&e.settings).unwrap(), broken);
        assert!(backups(&e).is_empty());
    }

    #[test]
    fn install_backs_up_then_writes_and_uninstall_round_trips() {
        let original = serde_json::to_string_pretty(&existing()).unwrap() + "\n";
        let e = env(Some(&original));
        let Report::Written {
            backup: Some(backup),
        } = apply(&e.settings, &e.exe, Action::Install).unwrap()
        else {
            panic!("expected a write with a backup");
        };
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(status_on_disk(&e.settings, &e.exe), Status::Installed);
        // Second install: nothing to do, no new backup, file untouched.
        let written = fs::read_to_string(&e.settings).unwrap();
        assert_eq!(
            apply(&e.settings, &e.exe, Action::Install).unwrap(),
            Report::NoChange
        );
        assert_eq!(fs::read_to_string(&e.settings).unwrap(), written);
        std::thread::sleep(std::time::Duration::from_millis(1100)); // backup names are per second
        assert!(matches!(
            apply(&e.settings, &e.exe, Action::Uninstall).unwrap(),
            Report::Written { .. }
        ));
        assert_eq!(fs::read_to_string(&e.settings).unwrap(), original);
        assert_eq!(backups(&e).len(), 2);
        assert!(!e.settings.with_extension("json.klat.tmp").exists());
    }

    #[test]
    fn missing_settings_file() {
        let e = env(None);
        assert_eq!(
            apply(&e.settings, &e.exe, Action::Uninstall).unwrap(),
            Report::NoChange
        );
        assert!(!e.settings.exists());
        assert_eq!(
            apply(&e.settings, &e.exe, Action::Install).unwrap(),
            Report::Written { backup: None }
        );
        assert_eq!(status_on_disk(&e.settings, &e.exe), Status::Installed);
    }

    #[test]
    fn missing_exe_is_refused_before_touching_anything() {
        let e = env(Some("{}"));
        fs::remove_file(&e.exe).unwrap();
        assert!(matches!(
            apply(&e.settings, &e.exe, Action::Install),
            Err(InstallError::ExeMissing(_))
        ));
        assert_eq!(fs::read_to_string(&e.settings).unwrap(), "{}");
        assert!(backups(&e).is_empty());
    }

    #[test]
    fn only_the_newest_backups_of_ours_are_kept() {
        let e = env(Some("{}"));
        let dir = e.settings.parent().unwrap();
        for ts in [100, 900, 300, 1000, 200] {
            fs::write(dir.join(format!("settings.json.bak.{ts}")), "x").unwrap();
        }
        let others = [
            "settings.json.bak.mine",
            "settings.json.bak.+5",
            "settings.json.backup",
            "other.json.bak.1",
        ];
        for name in others {
            fs::write(dir.join(name), "x").unwrap();
        }
        prune_backups(&e.settings, KEEP_BACKUPS);
        let mut ours: Vec<String> = backups(&e)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .filter(|n| {
                n.starts_with("settings.json.bak.") && n[18..].bytes().all(|b| b.is_ascii_digit())
            })
            .collect();
        ours.sort();
        assert_eq!(
            ours,
            [
                "settings.json.bak.1000",
                "settings.json.bak.300",
                "settings.json.bak.900"
            ]
        );
        for name in others {
            assert!(dir.join(name).exists(), "{name}");
        }
        assert!(e.settings.exists());
    }

    #[test]
    fn trailing_newline_style_is_kept() {
        let e = env(Some("{\"model\": \"opus\"}"));
        apply(&e.settings, &e.exe, Action::Install).unwrap();
        assert!(!fs::read_to_string(&e.settings).unwrap().ends_with('\n'));
    }
}

/// Dry run against a copy of a real settings file: `KLAT_DRYRUN=<copy> cargo test -p klat-infra -- --ignored dry_run`.
#[cfg(test)]
mod dry_run {
    use super::*;

    #[test]
    #[ignore]
    fn dry_run_round_trip_on_a_copy() {
        let src = PathBuf::from(std::env::var("KLAT_DRYRUN").expect("set KLAT_DRYRUN"));
        let original = fs::read_to_string(&src).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, &original).unwrap();
        let exe = dir.path().join(adapter_file_name());
        fs::write(&exe, b"").unwrap();

        apply(&path, &exe, Action::Install).unwrap();
        let installed = fs::read_to_string(&path).unwrap();
        println!("--- after install ---\n{installed}");
        assert_eq!(status_on_disk(&path, &exe), Status::Installed);
        apply(&path, &exe, Action::Uninstall).unwrap();
        let back = fs::read_to_string(&path).unwrap();
        let same_value = serde_json::from_str::<Value>(&back).unwrap()
            == serde_json::from_str::<Value>(&original).unwrap();
        println!(
            "--- round trip: same JSON value: {same_value}, byte-identical: {} ---",
            back == original
        );
        assert!(same_value);
    }
}

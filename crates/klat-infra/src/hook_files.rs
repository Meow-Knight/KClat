//! Installs the Klat hooks into the agents' hooks files (spec §8.5): Claude Code's `~/.claude/settings.json` and
//! Codex's `~/.codex/hooks.json`. The edits themselves (which entries are ours, what Install and Uninstall
//! change) are klat_core::agent_hooks; this is the file around them.
//!
//! - User-level files only; project files (`.claude/`, `.codex/` in a repo) are never touched.
//! - Broken JSON (or a non-object) aborts before anything is written.
//! - Before any write: a copy to `<file>.bak.<unix-seconds>` (the newest three are kept). Writes are atomic, and
//!   refused if the file changed on disk while we were working (the agents edit their files too).
//! - Runs only on request (Settings > Hooks), never at start-up.

pub use klat_core::agent_hooks::Status;
use klat_core::agent_hooks::{install_into, status, uninstall_from, UnexpectedShape, MARKER};
pub use klat_protocol::Agent;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Why Install or Uninstall did nothing. The text is for the log, next to the file's path; the Settings window
/// shows its own, translated, per variant.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// The file is not valid JSON; nothing was written.
    #[error("The hooks file is not valid JSON ({0}). Nothing was changed.")]
    BrokenJson(String),
    /// The file (or its "hooks") is not a JSON object; nothing was written.
    #[error("{0} in the hooks file is not a JSON object. Nothing was changed.")]
    UnexpectedShape(&'static str),
    #[error("The hook program was not found at {}.", .0.display())]
    ExeMissing(PathBuf),
    /// Someone else wrote the file while we were working; nothing was written.
    #[error("The hooks file changed while installing. Nothing was changed; try again.")]
    ChangedConcurrently,
    #[error("Could not update the hooks file: {0}")]
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

/// The agent's user-level hooks file: `~/.claude/settings.json`, or `hooks.json` in Codex's home
/// (`$CODEX_HOME`, else `~/.codex`). `~` is `%USERPROFILE%` on Windows, `$HOME` elsewhere.
pub fn hooks_file_path(agent: Agent) -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    hooks_file_in(
        agent,
        var("USERPROFILE").or_else(|| var("HOME")),
        var("CODEX_HOME"),
    )
}

fn hooks_file_in(
    agent: Agent,
    home: Option<std::ffi::OsString>,
    codex_home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let home = || home.map(PathBuf::from);
    match agent {
        Agent::ClaudeCode => Some(home()?.join(".claude").join("settings.json")),
        Agent::Codex => Some(
            codex_home
                .map(PathBuf::from)
                .or_else(|| Some(home()?.join(".codex")))?
                .join("hooks.json"),
        ),
    }
}

/// Whether the agent seems to be set up for this user: the folder its hooks file lives in exists. Install
/// works either way (it creates the folder), the Settings window only says so.
pub fn agent_folder_exists(hooks_file: &Path) -> bool {
    hooks_file.parent().is_some_and(Path::is_dir)
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "HooksAction"))]
pub enum Action {
    Install,
    Uninstall,
}

/// Reads, edits and writes the agent's hooks file. See the module docs for the safety rules.
pub fn apply(
    path: &Path,
    agent: Agent,
    exe: &Path,
    action: Action,
) -> Result<Report, InstallError> {
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
        Action::Install => install_into(&mut settings, agent, &exe_str)?,
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

/// Backups kept next to the file. Claude Code's settings.json can hold secrets (`env`, `apiKeyHelper`), so copies
/// of it should not pile up.
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
pub fn status_on_disk(path: &Path, agent: Agent, exe: &Path) -> Status {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map_or(Status::NotInstalled, |v| {
            status(&v, agent, &exe.to_string_lossy())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CLAUDE: Agent = Agent::ClaudeCode;

    #[test]
    fn the_adapter_name_matches_this_platform() {
        let name = adapter_file_name();
        assert_eq!(name.ends_with(".exe"), cfg!(windows));
        assert!(name.starts_with(MARKER));
    }

    #[test]
    fn each_agent_has_its_file() {
        let home = || Some("/home/me".into());
        assert_eq!(
            hooks_file_in(Agent::ClaudeCode, home(), Some("/elsewhere".into())),
            Some(
                PathBuf::from("/home/me")
                    .join(".claude")
                    .join("settings.json")
            )
        );
        assert_eq!(
            hooks_file_in(Agent::Codex, home(), None),
            Some(PathBuf::from("/home/me").join(".codex").join("hooks.json"))
        );
        // CODEX_HOME moves Codex's whole folder, as it does for Codex itself.
        assert_eq!(
            hooks_file_in(Agent::Codex, home(), Some("/srv/codex".into())),
            Some(PathBuf::from("/srv/codex").join("hooks.json"))
        );
        assert_eq!(hooks_file_in(Agent::ClaudeCode, None, None), None);
        assert_eq!(hooks_file_in(Agent::Codex, None, None), None);
    }

    #[test]
    fn error_texts_for_the_log_are_unchanged() {
        let cases = [
            (
                InstallError::BrokenJson("line 1".into()),
                "The hooks file is not valid JSON (line 1). Nothing was changed.",
            ),
            (
                InstallError::UnexpectedShape("\"hooks\""),
                "\"hooks\" in the hooks file is not a JSON object. Nothing was changed.",
            ),
            (
                InstallError::ExeMissing(PathBuf::from("/x/klat-hook")),
                "The hook program was not found at /x/klat-hook.",
            ),
            (
                InstallError::ChangedConcurrently,
                "The hooks file changed while installing. Nothing was changed; try again.",
            ),
            (
                InstallError::Io(io::Error::other("disk full")),
                "Could not update the hooks file: disk full",
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
                apply(&e.settings, CLAUDE, &e.exe, action),
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
        } = apply(&e.settings, CLAUDE, &e.exe, Action::Install).unwrap()
        else {
            panic!("expected a write with a backup");
        };
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(
            status_on_disk(&e.settings, CLAUDE, &e.exe),
            Status::Installed
        );
        // Second install: nothing to do, no new backup, file untouched.
        let written = fs::read_to_string(&e.settings).unwrap();
        assert_eq!(
            apply(&e.settings, CLAUDE, &e.exe, Action::Install).unwrap(),
            Report::NoChange
        );
        assert_eq!(fs::read_to_string(&e.settings).unwrap(), written);
        std::thread::sleep(std::time::Duration::from_millis(1100)); // backup names are per second
        assert!(matches!(
            apply(&e.settings, CLAUDE, &e.exe, Action::Uninstall).unwrap(),
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
            apply(&e.settings, CLAUDE, &e.exe, Action::Uninstall).unwrap(),
            Report::NoChange
        );
        assert!(!e.settings.exists());
        assert_eq!(
            apply(&e.settings, CLAUDE, &e.exe, Action::Install).unwrap(),
            Report::Written { backup: None }
        );
        assert_eq!(
            status_on_disk(&e.settings, CLAUDE, &e.exe),
            Status::Installed
        );
    }

    #[test]
    fn missing_exe_is_refused_before_touching_anything() {
        let e = env(Some("{}"));
        fs::remove_file(&e.exe).unwrap();
        assert!(matches!(
            apply(&e.settings, CLAUDE, &e.exe, Action::Install),
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
    fn codex_hooks_json_round_trips_and_can_start_from_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".codex").join("hooks.json");
        let exe = dir.path().join(adapter_file_name());
        fs::write(&exe, b"").unwrap();
        assert!(!agent_folder_exists(&file));
        assert_eq!(
            apply(&file, Agent::Codex, &exe, Action::Install).unwrap(),
            Report::Written { backup: None }
        );
        assert!(agent_folder_exists(&file));
        assert_eq!(status_on_disk(&file, Agent::Codex, &exe), Status::Installed);
        // Read as Claude Code's file it is not right: the two are never mixed up.
        assert_eq!(status_on_disk(&file, CLAUDE, &exe), Status::NeedsRepair);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("--agent codex Stop"), "{text}");

        let theirs =
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"bd codex-hook Stop"}]}]}}"#;
        fs::write(&file, theirs).unwrap();
        apply(&file, Agent::Codex, &exe, Action::Install).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100)); // backup names are per second
        apply(&file, Agent::Codex, &exe, Action::Uninstall).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(&file).unwrap()).unwrap(),
            serde_json::from_str::<Value>(theirs).unwrap()
        );
    }

    #[test]
    fn trailing_newline_style_is_kept() {
        let e = env(Some("{\"model\": \"opus\"}"));
        apply(&e.settings, CLAUDE, &e.exe, Action::Install).unwrap();
        assert!(!fs::read_to_string(&e.settings).unwrap().ends_with('\n'));
    }
}

/// Dry run against a copy of a real hooks file: `KLAT_DRYRUN=<copy> cargo test -p klat-infra -- --ignored dry_run`
/// (Claude Code's settings.json; add `KLAT_DRYRUN_AGENT=codex` for a hooks.json).
#[cfg(test)]
mod dry_run {
    use super::*;

    #[test]
    #[ignore]
    fn dry_run_round_trip_on_a_copy() {
        let src = PathBuf::from(std::env::var("KLAT_DRYRUN").expect("set KLAT_DRYRUN"));
        let agent = std::env::var("KLAT_DRYRUN_AGENT")
            .ok()
            .map_or(Agent::ClaudeCode, |a| {
                Agent::parse(&a).expect("claude-code or codex")
            });
        let original = fs::read_to_string(&src).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, &original).unwrap();
        let exe = dir.path().join(adapter_file_name());
        fs::write(&exe, b"").unwrap();

        apply(&path, agent, &exe, Action::Install).unwrap();
        let installed = fs::read_to_string(&path).unwrap();
        println!("--- after install ---\n{installed}");
        assert_eq!(status_on_disk(&path, agent, &exe), Status::Installed);
        apply(&path, agent, &exe, Action::Uninstall).unwrap();
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

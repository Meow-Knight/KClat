//! What Install and Uninstall do to an agent's hooks file (spec §8.5), as edits of the parsed JSON: Claude Code's
//! `~/.claude/settings.json` and Codex's `~/.codex/hooks.json`, which share one shape
//! (`{"hooks": {"<Event>": [{"matcher"?, "hooks": [{"type": "command", "command", ...}]}]}}`). Reading, backing
//! up and writing the file is the app's job.
//!
//! - Our entries are recognised by the program their command runs: a file named `klat-hook` (`klat-hook.exe` on
//!   Windows; or `claude-cat-hook`, the name before the rename to Klat), so reinstalling never duplicates them,
//!   repairing moves old entries to the new program, and uninstalling removes only them. Everything else keeps
//!   its content and order.
//! - Claude Code: exec form (`command` + `args`). No shell is involved (Git Bash or PowerShell on Windows, zsh on
//!   macOS), so a path with spaces needs no quoting at all.
//! - Codex has no exec form: `command` is one line for a shell, `%COMSPEC% /C <line>` on Windows and
//!   `$SHELL -lc '<line>'` elsewhere (codex-rs/hooks command_runner, 2026-10). On Windows the program goes in bare:
//!   Codex 0.144 hands the line to cmd.exe with its quotes escaped as `\"`, which cmd.exe cannot read, so a quoted
//!   program fails there (newer versions pass it raw). The app gives a path without spaces (the 8.3 short name,
//!   platform::cmd_safe_path); only a path that still needs quoting is quoted. The arguments are fixed words.

use klat_protocol::{Agent, HookName};
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The events Install registers for the agent, one entry each: `HookName::is_installed` decides (klat-protocol).
fn installed_events(agent: Agent) -> impl Iterator<Item = &'static str> {
    HookName::installed(agent).map(HookName::as_str)
}

fn is_installed_event(agent: Agent, event: &str) -> bool {
    HookName::parse(event).is_some_and(|h| h.is_installed(agent))
}

/// Our entries run a program with this file name (case-insensitive), with or without `.exe`.
pub const MARKER: &str = "klat-hook";
/// The adapter's name when the app was called Claude Cat; such entries are ours too.
const LEGACY_MARKER: &str = "claude-cat-hook";
/// Seconds. The adapter gives up after 300ms on its own; this is Claude Code's hard stop.
pub const TIMEOUT_SECS: u64 = 1;
/// Seconds, for Codex, which also starts a shell first (`cmd.exe` on Windows). Codex caps `SessionEnd` and
/// `Interrupt` at 3.
pub const CODEX_TIMEOUT_SECS: u64 = 2;

/// The file (or its `"hooks"`, or an event's list) is not the JSON shape the agent writes; nothing was changed.
/// Names the part, for the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnexpectedShape(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "HooksStatus"))]
pub enum Status {
    NotInstalled,
    /// Some of our entries are missing, or point at another exe path.
    NeedsRepair,
    Installed,
}

fn our_entry(agent: Agent, exe: &str, event: &str) -> Value {
    match agent {
        Agent::ClaudeCode => {
            json!({ "type": "command", "command": exe, "args": [event], "timeout": TIMEOUT_SECS })
        }
        Agent::Codex => json!({
            "type": "command",
            "command": shell_line(exe, &["--agent", Agent::Codex.as_str(), event]),
            "timeout": CODEX_TIMEOUT_SECS,
        }),
    }
}

/// `exe` as the shell Codex runs it with needs it, then `args` (plain words). A path that starts with `/` is for
/// `sh` (single quotes, which keep everything literal). Any other is a Windows path for `cmd.exe`: bare when it can
/// be (see the module docs), in double quotes when it has spaces or characters cmd.exe treats specially (a Windows
/// path cannot contain `"`).
fn shell_line(exe: &str, args: &[&str]) -> String {
    let program = if exe.starts_with('/') {
        format!("'{}'", exe.replace('\'', r"'\''"))
    } else if exe
        .chars()
        .any(|c| c.is_whitespace() || "&|<>^%!()".contains(c))
    {
        format!("\"{exe}\"")
    } else {
        exe.to_owned()
    };
    std::iter::once(program.as_str())
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ours when the program the command runs is named `klat-hook` or `claude-cat-hook`. The program is the whole
/// command (exec form, where the path may contain spaces) or its first word, quoted (`"` or `'`) or not (the shell
/// form older versions and the Codex entries write). A command that only mentions the name, or runs
/// `my-klat-hook-logger.exe`, is someone else's.
fn is_ours(hook: &Value) -> bool {
    let Some(cmd) = hook.get("command").and_then(Value::as_str) else {
        return false;
    };
    let cmd = cmd.trim();
    let quoted = ['"', '\'']
        .into_iter()
        .filter_map(|q| cmd.strip_prefix(q)?.split_once(q))
        .map(|(program, _)| program);
    let first_word = cmd.split_whitespace().next();
    [Some(cmd), first_word]
        .into_iter()
        .flatten()
        .chain(quoted)
        .any(is_our_program)
}

/// The file name is ours, in any case and with or without `.exe`.
fn is_our_program(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let name = name.to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    stem == MARKER || stem == LEGACY_MARKER
}

fn hooks_map(settings: &Value) -> Result<Option<&Map<String, Value>>, UnexpectedShape> {
    match settings.get("hooks") {
        None => Ok(None),
        Some(Value::Object(m)) => Ok(Some(m)),
        Some(_) => Err(UnexpectedShape("\"hooks\"")),
    }
}

/// Our hook objects under one event, in file order.
fn ours_in<'a>(hooks: &'a Map<String, Value>, event: &str) -> Vec<&'a Value> {
    hooks
        .get(event)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
        .filter(|h| is_ours(h))
        .collect()
}

pub fn status(settings: &Value, agent: Agent, exe: &str) -> Status {
    let Ok(Some(hooks)) = hooks_map(settings) else {
        return Status::NotInstalled;
    };
    let mut any = false;
    let mut all_exact = true;
    for event in installed_events(agent) {
        let ours = ours_in(hooks, event);
        any |= !ours.is_empty();
        all_exact &= ours.len() == 1 && *ours[0] == our_entry(agent, exe, event);
    }
    // Leftovers under events we no longer use also need a repair.
    let stray = hooks
        .keys()
        .filter(|k| !is_installed_event(agent, k))
        .any(|k| !ours_in(hooks, k).is_empty());
    match (any || stray, all_exact && !stray) {
        (false, _) => Status::NotInstalled,
        (true, true) => Status::Installed,
        (true, false) => Status::NeedsRepair,
    }
}

/// Adds or refreshes our entry for every event. An existing entry is updated in place; a missing one is
/// appended as its own matcher group. Extra copies of ours are removed. Nothing else is touched.
pub fn install_into(settings: &mut Value, agent: Agent, exe: &str) -> Result<(), UnexpectedShape> {
    let root = settings
        .as_object_mut()
        .ok_or(UnexpectedShape("The top level"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(UnexpectedShape("\"hooks\""))?;

    // Entries of ours under events we no longer install (from an older version) go away.
    let stale: Vec<String> = hooks
        .keys()
        .filter(|k| !is_installed_event(agent, k))
        .cloned()
        .collect();
    for event in stale {
        remove_ours(hooks, &event);
    }

    for event in installed_events(agent) {
        let wanted = our_entry(agent, exe, event);
        let groups = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(groups) = groups.as_array_mut() else {
            return Err(UnexpectedShape("A hook event list"));
        };
        let mut placed = false;
        groups.retain_mut(|group| {
            let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = list.len();
            list.retain_mut(|h| {
                if !is_ours(h) {
                    return true;
                }
                if placed {
                    return false; // a duplicate of ours
                }
                *h = wanted.clone();
                placed = true;
                true
            });
            // Drop a group only when removing our duplicates is what emptied it.
            !(list.is_empty() && list.len() != before)
        });
        if !placed {
            groups.push(json!({ "hooks": [wanted] }));
        }
    }
    Ok(())
}

/// Removes every entry of ours. Groups and events we emptied are removed; everything else stays as is.
pub fn uninstall_from(settings: &mut Value) -> Result<bool, UnexpectedShape> {
    let Some(root) = settings.as_object_mut() else {
        return Err(UnexpectedShape("The top level"));
    };
    let Some(hooks_value) = root.get_mut("hooks") else {
        return Ok(false);
    };
    let Some(hooks) = hooks_value.as_object_mut() else {
        return Err(UnexpectedShape("\"hooks\""));
    };
    let events: Vec<String> = hooks.keys().cloned().collect();
    let mut changed = false;
    for event in events {
        changed |= remove_ours(hooks, &event);
    }
    if changed && hooks.is_empty() {
        root.remove("hooks");
    }
    Ok(changed)
}

/// Removes our hooks under one event, then any group or event list that this left empty.
fn remove_ours(hooks: &mut Map<String, Value>, event: &str) -> bool {
    let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    groups.retain_mut(|group| {
        let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = list.len();
        list.retain(|h| !is_ours(h));
        if list.len() == before {
            return true;
        }
        changed = true;
        !list.is_empty()
    });
    if changed && groups.is_empty() {
        hooks.shift_remove(event);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_uses_the_names_the_settings_window_expects() {
        let names: Vec<Value> = [Status::Installed, Status::NeedsRepair, Status::NotInstalled]
            .iter()
            .map(|s| serde_json::to_value(s).unwrap())
            .collect();
        assert_eq!(
            names,
            vec![
                json!("installed"),
                json!("needs-repair"),
                json!("not-installed")
            ]
        );
    }

    const EXE: &str = r"C:\Program Files\Klat\klat-hook.exe";
    const CLAUDE: Agent = Agent::ClaudeCode;
    const CODEX: Agent = Agent::Codex;

    #[test]
    fn our_entries_are_recognised_on_every_platform() {
        let cmd = |c: &str| json!({ "type": "command", "command": c });
        assert!(is_ours(&cmd(r"C:\Program Files\Klat\klat-hook.exe")));
        assert!(is_ours(&cmd(
            r"C:\Users\Me\AppData\Local\Klat\KLAT-HOOK.EXE"
        )));
        assert!(is_ours(&cmd(
            "/Applications/Klat.app/Contents/MacOS/klat-hook"
        )));
        assert!(is_ours(&cmd(r"D:\old\claude-cat-hook.exe")));
        // Shell-form entries from older versions: the program, quoted or not, then the event.
        assert!(is_ours(&cmd(r#""D:\x\klat-hook.exe" Stop"#)));
        assert!(is_ours(&cmd(r"C:\a\Klat-Hook.EXE Stop")));
        assert!(is_ours(&cmd("/opt/klat/klat-hook Stop")));
        assert!(!is_ours(&cmd("notify-send done")));
        assert!(!is_ours(&json!({ "type": "command" })));
        // Someone else's program that merely mentions the name is not ours.
        assert!(!is_ours(&cmd(r"C:\tools\my-klat-hook-logger.exe")));
        assert!(!is_ours(&cmd("echo klat-hook")));
        assert!(!is_ours(&cmd(r#""C:\klat-hook.exe\other.exe" Stop"#)));
        assert!(!is_ours(&cmd("/home/me/klat-hook.sh")));
        // The Codex entries, on both shells.
        assert!(is_ours(&cmd(
            r#""C:\Users\A B\AppData\Local\Klat\klat-hook.exe" --agent codex Stop"#
        )));
        assert!(is_ours(&cmd(
            "'/Applications/Klat.app/Contents/MacOS/klat-hook' --agent codex Stop"
        )));
        assert!(!is_ours(&cmd("'/opt/my-klat-hook' Stop")));
        assert!(!is_ours(&cmd("bd codex-hook Stop")));
        assert!(is_ours(&cmd(
            r"C:\Users\me\AppData\Local\Klat\klat-hook.exe --agent codex Stop"
        )));
        assert!(is_ours(&cmd(
            r"C:\PROGRA~1\Klat\klat-hook.exe --agent codex Stop"
        )));
    }

    #[test]
    fn codex_lines_quote_the_program_for_its_shell() {
        // The usual install path has no spaces: bare, so cmd.exe runs it however Codex passes the line.
        assert_eq!(
            shell_line(
                r"C:\Users\me\AppData\Local\Klat\klat-hook.exe",
                &["--agent", "codex", "Stop"]
            ),
            r"C:\Users\me\AppData\Local\Klat\klat-hook.exe --agent codex Stop"
        );
        assert_eq!(
            shell_line(r"C:\PROGRA~1\Klat\klat-hook.exe", &["Stop"]),
            r"C:\PROGRA~1\Klat\klat-hook.exe Stop"
        );
        // No short name to be had: quoted, which newer Codex versions read.
        assert_eq!(
            shell_line(EXE, &["--agent", "codex", "Stop"]),
            r#""C:\Program Files\Klat\klat-hook.exe" --agent codex Stop"#
        );
        assert_eq!(
            shell_line(r"D:\R&D\klat-hook.exe", &[]),
            r#""D:\R&D\klat-hook.exe""#
        );
        assert_eq!(
            shell_line(
                "/Users/me/Apps/Klat.app/Contents/MacOS/klat-hook",
                &["Stop"]
            ),
            "'/Users/me/Apps/Klat.app/Contents/MacOS/klat-hook' Stop"
        );
        // sh: a single quote ends the quoting, so it is written as '\''.
        assert_eq!(
            shell_line("/Users/o'neil/klat-hook", &[]),
            r"'/Users/o'\''neil/klat-hook'"
        );
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

    fn keys(v: &Value) -> Vec<String> {
        v.as_object().unwrap().keys().cloned().collect()
    }

    fn count_ours(v: &Value) -> usize {
        let hooks = v["hooks"].as_object().unwrap();
        hooks.keys().map(|k| ours_in(hooks, k).len()).sum()
    }

    #[test]
    fn install_adds_one_exec_form_entry_per_event() {
        let mut v = existing();
        install_into(&mut v, CLAUDE, EXE).unwrap();
        assert_eq!(count_ours(&v), installed_events(CLAUDE).count());
        let notif = &v["hooks"]["Notification"][0]["hooks"][0];
        assert_eq!(notif["command"], EXE); // spaces in the path need no quoting: there is no shell
        assert_eq!(notif["args"], json!(["Notification"]));
        assert_eq!(notif["timeout"], 1);
        assert_eq!(notif["type"], "command");
        assert!(v["hooks"].get("SubagentStop").is_none());
        assert!(v["hooks"].get("Interrupt").is_none());
        assert_eq!(status(&v, CLAUDE, EXE), Status::Installed);
        // The same file read as Codex's: wrong entries, wrong events.
        assert_eq!(status(&v, CODEX, EXE), Status::NeedsRepair);
    }

    #[test]
    fn codex_gets_one_shell_line_per_event() {
        let mut v = json!({});
        install_into(&mut v, CODEX, EXE).unwrap();
        assert_eq!(count_ours(&v), installed_events(CODEX).count());
        assert_eq!(
            v["hooks"]["Interrupt"][0]["hooks"][0],
            json!({
                "type": "command",
                "command": r#""C:\Program Files\Klat\klat-hook.exe" --agent codex Interrupt"#,
                "timeout": 2
            })
        );
        for missing in [
            "Notification",
            "StopFailure",
            "PermissionDenied",
            "PostToolUseFailure",
        ] {
            assert!(v["hooks"].get(missing).is_none(), "{missing}");
        }
        assert_eq!(status(&v, CODEX, EXE), Status::Installed);
        assert_eq!(
            status(&v, CODEX, r"D:\other\klat-hook.exe"),
            Status::NeedsRepair
        );
        let mut again = v.clone();
        install_into(&mut again, CODEX, EXE).unwrap();
        assert_eq!(again, v);
        assert!(uninstall_from(&mut v).unwrap());
        assert_eq!(v, json!({}));
    }

    #[test]
    fn codex_repair_moves_entries_from_a_claude_style_install() {
        // Someone copied the Claude Code entries into hooks.json: exec form, and events Codex does not have.
        let mut v = json!({});
        install_into(&mut v, CLAUDE, EXE).unwrap();
        install_into(&mut v, CODEX, EXE).unwrap();
        assert_eq!(status(&v, CODEX, EXE), Status::Installed);
        assert_eq!(count_ours(&v), installed_events(CODEX).count());
        assert!(v["hooks"].get("Notification").is_none());
    }

    #[test]
    fn install_keeps_existing_hooks_and_key_order() {
        let mut v = existing();
        install_into(&mut v, CLAUDE, EXE).unwrap();
        assert_eq!(keys(&v), ["permissions", "model", "hooks", "theme"]);
        assert_eq!(v["hooks"]["PreToolUse"], existing()["hooks"]["PreToolUse"]);
        // Their Stop hook stays first; ours is appended as a separate group.
        assert_eq!(v["hooks"]["Stop"][0], existing()["hooks"]["Stop"][0]);
        assert_eq!(v["hooks"]["Stop"][1]["hooks"][0]["args"], json!(["Stop"]));
        let hook_keys = keys(&v["hooks"]);
        assert_eq!(&hook_keys[..2], ["PreToolUse", "Stop"]);
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let mut once = existing();
        install_into(&mut once, CLAUDE, EXE).unwrap();
        let mut twice = once.clone();
        install_into(&mut twice, CLAUDE, EXE).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn repair_updates_paths_in_place_and_drops_duplicates() {
        let mut v = existing();
        // Entries from before the rename to Klat count as ours and get moved to the new program.
        install_into(&mut v, CLAUDE, r"D:\old\target\debug\claude-cat-hook.exe").unwrap();
        assert_eq!(status(&v, CLAUDE, EXE), Status::NeedsRepair);
        // A hand-made duplicate and an old shell-form entry under an event we no longer use.
        v["hooks"]["Stop"][1]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"command","command":"\"D:\\x\\klat-hook.exe\" Stop"}));
        v["hooks"]["SubagentStop"] = json!([{ "hooks": [{"type":"command","command":"\"C:\\a\\Claude-Cat-Hook.EXE\" SubagentStop"}] }]);
        install_into(&mut v, CLAUDE, EXE).unwrap();
        assert_eq!(status(&v, CLAUDE, EXE), Status::Installed);
        assert_eq!(count_ours(&v), installed_events(CLAUDE).count());
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2); // theirs + ours, updated in place
        assert!(v["hooks"].get("SubagentStop").is_none());
    }

    #[test]
    fn uninstall_removes_only_ours_and_restores_the_original() {
        let mut v = existing();
        install_into(&mut v, CLAUDE, EXE).unwrap();
        assert!(uninstall_from(&mut v).unwrap());
        assert_eq!(v, existing());
        assert_eq!(keys(&v), keys(&existing()));
        assert_eq!(keys(&v["hooks"]), keys(&existing()["hooks"]));
        assert!(!uninstall_from(&mut v).unwrap());
    }

    #[test]
    fn uninstall_keeps_their_hooks_sharing_a_group_with_ours() {
        let mut v: Value = serde_json::from_str(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"a"},{"type":"command","command":"C:\\x\\klat-hook.exe","args":["Stop"]},{"type":"command","command":"b"}]}]}}"#,
        )
        .unwrap();
        uninstall_from(&mut v).unwrap();
        assert_eq!(
            v,
            json!({"hooks":{"Stop":[{"hooks":[{"type":"command","command":"a"},{"type":"command","command":"b"}]}]}})
        );
    }

    #[test]
    fn install_then_uninstall_on_an_empty_file_leaves_it_empty() {
        let mut v = json!({});
        install_into(&mut v, CLAUDE, EXE).unwrap();
        uninstall_from(&mut v).unwrap();
        assert_eq!(v, json!({}));
    }

    #[test]
    fn empty_structures_we_did_not_create_are_left_alone() {
        let mut v = json!({"hooks": {"Stop": [], "PreToolUse": [{"hooks": []}]}});
        let before = v.clone();
        assert!(!uninstall_from(&mut v).unwrap());
        assert_eq!(v, before);
    }

    #[test]
    fn non_object_shapes_are_refused() {
        assert!(matches!(
            install_into(&mut json!([1]), CLAUDE, EXE),
            Err(UnexpectedShape(_))
        ));
        assert!(matches!(
            install_into(&mut json!({"hooks": []}), CODEX, EXE),
            Err(UnexpectedShape(_))
        ));
        assert!(matches!(
            uninstall_from(&mut json!({"hooks": "x"})),
            Err(UnexpectedShape(_))
        ));
    }

    /// Golden files (crates/klat-core/testdata/claude_settings): a settings.json with someone else's hooks in
    /// several shapes, our entries from before the rename (shell form, an event we no longer use) and from
    /// other folders, an empty event list, and keys in no particular order. The expected files were written by
    /// this code; any change to them is a change in what Install and Uninstall do to people's files.
    mod golden {
        use super::*;

        const BEFORE: &str = include_str!("../testdata/claude_settings/before.json");
        const INSTALLED: &str = include_str!("../testdata/claude_settings/installed.json");
        const UNINSTALLED: &str = include_str!("../testdata/claude_settings/uninstalled.json");
        const UNINSTALLED_FROM_BEFORE: &str =
            include_str!("../testdata/claude_settings/uninstalled_from_before.json");

        fn text(v: &Value) -> String {
            serde_json::to_string_pretty(v).unwrap() + "\n"
        }
        /// The files may be checked out with CRLF.
        fn lf(s: &str) -> String {
            s.replace("\r\n", "\n")
        }

        #[test]
        fn install_writes_exactly_this() {
            let mut v: Value = serde_json::from_str(BEFORE).unwrap();
            install_into(&mut v, CLAUDE, EXE).unwrap();
            assert_eq!(text(&v), lf(INSTALLED));
            assert_eq!(status(&v, CLAUDE, EXE), Status::Installed);
        }

        #[test]
        fn uninstall_after_install_writes_exactly_this() {
            let mut v: Value = serde_json::from_str(INSTALLED).unwrap();
            assert!(uninstall_from(&mut v).unwrap());
            assert_eq!(text(&v), lf(UNINSTALLED));
            assert_eq!(status(&v, CLAUDE, EXE), Status::NotInstalled);
        }

        #[test]
        fn uninstall_of_the_old_entries_writes_exactly_this() {
            let mut v: Value = serde_json::from_str(BEFORE).unwrap();
            assert_eq!(status(&v, CLAUDE, EXE), Status::NeedsRepair);
            assert!(uninstall_from(&mut v).unwrap());
            assert_eq!(text(&v), lf(UNINSTALLED_FROM_BEFORE));
        }

        /// Codex (crates/klat-core/testdata/codex_hooks): a hooks.json with someone else's hooks (beads, with
        /// matchers and status messages) under events we use and events we don't. Installing and uninstalling
        /// must give the file back as it was.
        const CODEX_BEFORE: &str = include_str!("../testdata/codex_hooks/before.json");
        const CODEX_INSTALLED: &str = include_str!("../testdata/codex_hooks/installed.json");

        #[test]
        fn codex_install_writes_exactly_this_and_uninstall_restores_the_file() {
            let mut v: Value = serde_json::from_str(CODEX_BEFORE).unwrap();
            assert_eq!(status(&v, CODEX, EXE), Status::NotInstalled);
            install_into(&mut v, CODEX, EXE).unwrap();
            assert_eq!(text(&v), lf(CODEX_INSTALLED));
            assert_eq!(status(&v, CODEX, EXE), Status::Installed);
            assert!(uninstall_from(&mut v).unwrap());
            assert_eq!(text(&v), lf(CODEX_BEFORE));
        }
    }
}

//! What Install and Uninstall do to `~/.claude/settings.json` (spec §8.5), as edits of the parsed JSON. Reading,
//! backing up and writing the file is the app's job.
//!
//! - Our entries are recognised by the program their command runs: a file named `klat-hook` (`klat-hook.exe` on
//!   Windows; or `claude-cat-hook`, the name before the rename to Klat), so reinstalling never duplicates them,
//!   repairing moves old entries to the new program, and uninstalling removes only them. Everything else keeps
//!   its content and order.
//! - Exec form (`command` + `args`): no shell is involved (Git Bash or PowerShell on Windows, zsh on macOS),
//!   so a path with spaces needs no quoting at all.

use klat_protocol::HookName;
use serde::Serialize;
use serde_json::{json, Map, Value};

/// The events Install registers, one entry each: `HookName::is_installed` decides (klat-protocol).
fn installed_events() -> impl Iterator<Item = &'static str> {
    HookName::installed().map(HookName::as_str)
}

fn is_installed_event(event: &str) -> bool {
    HookName::parse(event).is_some_and(HookName::is_installed)
}

/// Our entries run a program with this file name (case-insensitive), with or without `.exe`.
pub const MARKER: &str = "klat-hook";
/// The adapter's name when the app was called Claude Cat; such entries are ours too.
const LEGACY_MARKER: &str = "claude-cat-hook";
/// Seconds. The adapter gives up after 300ms on its own; this is Claude Code's hard stop.
pub const TIMEOUT_SECS: u64 = 1;

/// The file (or its `"hooks"`, or an event's list) is not the JSON shape Claude Code writes; nothing was
/// changed. Names the part, for the message.
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

fn our_entry(exe: &str, event: &str) -> Value {
    json!({ "type": "command", "command": exe, "args": [event], "timeout": TIMEOUT_SECS })
}

/// Ours when the program the command runs is named `klat-hook` or `claude-cat-hook`. The program is the whole
/// command (exec form, where the path may contain spaces) or its first word, quoted or not (the shell form older
/// versions wrote). A command that only mentions the name, or runs `my-klat-hook-logger.exe`, is someone else's.
fn is_ours(hook: &Value) -> bool {
    let Some(cmd) = hook.get("command").and_then(Value::as_str) else {
        return false;
    };
    let cmd = cmd.trim();
    let quoted = cmd
        .strip_prefix('"')
        .and_then(|rest| rest.split_once('"'))
        .map(|(program, _)| program);
    let first_word = cmd.split_whitespace().next();
    [Some(cmd), quoted, first_word]
        .into_iter()
        .flatten()
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

pub fn status(settings: &Value, exe: &str) -> Status {
    let Ok(Some(hooks)) = hooks_map(settings) else {
        return Status::NotInstalled;
    };
    let mut any = false;
    let mut all_exact = true;
    for event in installed_events() {
        let ours = ours_in(hooks, event);
        any |= !ours.is_empty();
        all_exact &= ours.len() == 1 && *ours[0] == our_entry(exe, event);
    }
    // Leftovers under events we no longer use also need a repair.
    let stray = hooks
        .keys()
        .filter(|k| !is_installed_event(k))
        .any(|k| !ours_in(hooks, k).is_empty());
    match (any || stray, all_exact && !stray) {
        (false, _) => Status::NotInstalled,
        (true, true) => Status::Installed,
        (true, false) => Status::NeedsRepair,
    }
}

/// Adds or refreshes our entry for every event. An existing entry is updated in place; a missing one is
/// appended as its own matcher group. Extra copies of ours are removed. Nothing else is touched.
pub fn install_into(settings: &mut Value, exe: &str) -> Result<(), UnexpectedShape> {
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
        .filter(|k| !is_installed_event(k))
        .cloned()
        .collect();
    for event in stale {
        remove_ours(hooks, &event);
    }

    for event in installed_events() {
        let wanted = our_entry(exe, event);
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
        install_into(&mut v, EXE).unwrap();
        assert_eq!(count_ours(&v), installed_events().count());
        let notif = &v["hooks"]["Notification"][0]["hooks"][0];
        assert_eq!(notif["command"], EXE); // spaces in the path need no quoting: there is no shell
        assert_eq!(notif["args"], json!(["Notification"]));
        assert_eq!(notif["timeout"], 1);
        assert_eq!(notif["type"], "command");
        assert!(v["hooks"].get("SubagentStop").is_none());
        assert_eq!(status(&v, EXE), Status::Installed);
    }

    #[test]
    fn install_keeps_existing_hooks_and_key_order() {
        let mut v = existing();
        install_into(&mut v, EXE).unwrap();
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
        install_into(&mut once, EXE).unwrap();
        let mut twice = once.clone();
        install_into(&mut twice, EXE).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn repair_updates_paths_in_place_and_drops_duplicates() {
        let mut v = existing();
        // Entries from before the rename to Klat count as ours and get moved to the new program.
        install_into(&mut v, r"D:\old\target\debug\claude-cat-hook.exe").unwrap();
        assert_eq!(status(&v, EXE), Status::NeedsRepair);
        // A hand-made duplicate and an old shell-form entry under an event we no longer use.
        v["hooks"]["Stop"][1]["hooks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"command","command":"\"D:\\x\\klat-hook.exe\" Stop"}));
        v["hooks"]["SubagentStop"] = json!([{ "hooks": [{"type":"command","command":"\"C:\\a\\Claude-Cat-Hook.EXE\" SubagentStop"}] }]);
        install_into(&mut v, EXE).unwrap();
        assert_eq!(status(&v, EXE), Status::Installed);
        assert_eq!(count_ours(&v), installed_events().count());
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2); // theirs + ours, updated in place
        assert!(v["hooks"].get("SubagentStop").is_none());
    }

    #[test]
    fn uninstall_removes_only_ours_and_restores_the_original() {
        let mut v = existing();
        install_into(&mut v, EXE).unwrap();
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
        install_into(&mut v, EXE).unwrap();
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
            install_into(&mut json!([1]), EXE),
            Err(UnexpectedShape(_))
        ));
        assert!(matches!(
            install_into(&mut json!({"hooks": []}), EXE),
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
            install_into(&mut v, EXE).unwrap();
            assert_eq!(text(&v), lf(INSTALLED));
            assert_eq!(status(&v, EXE), Status::Installed);
        }

        #[test]
        fn uninstall_after_install_writes_exactly_this() {
            let mut v: Value = serde_json::from_str(INSTALLED).unwrap();
            assert!(uninstall_from(&mut v).unwrap());
            assert_eq!(text(&v), lf(UNINSTALLED));
            assert_eq!(status(&v, EXE), Status::NotInstalled);
        }

        #[test]
        fn uninstall_of_the_old_entries_writes_exactly_this() {
            let mut v: Value = serde_json::from_str(BEFORE).unwrap();
            assert_eq!(status(&v, EXE), Status::NeedsRepair);
            assert!(uninstall_from(&mut v).unwrap());
            assert_eq!(text(&v), lf(UNINSTALLED_FROM_BEFORE));
        }
    }
}

//! src/bridge/generated.ts: the TypeScript side of every type that crosses IPC, written from the Rust types
//! (docs/ARCHITECTURE.md §7.6). The test fails when the file is out of date, so `pnpm check` catches a Rust
//! change the frontend has not seen; `pnpm gen:types` rewrites it.

use crate::ipc::commands::hooks::{HooksInfo, HooksResult};
use crate::ipc::events::GrabMode;
use crate::platform::Permission;
use crate::ui::overlay::ZoomChange;
use klat_core::cat::{Attention, CatState, SleepCause};
use klat_core::claude_hooks::Status;
use klat_core::geometry::{self, wire, Css, Rect};
use klat_core::session::{SessionView, WakeReason};
use klat_core::settings::{Character, Corner, KlatLanguage, Settings, SettingsPatch, UiLanguage};
use klat_core::time::DurationMs;
use klat_infra::claude_settings::Action;
use std::path::PathBuf;
use ts_rs::{Config, TS};

const HEADER: &str = "\
// Generated from the Rust types by `pnpm gen:types` (src-tauri/src/ipc/bindings.rs). Do not edit:
// change the Rust type, regenerate, and commit both. `pnpm check` fails while this file is out of date.
";

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/bridge/generated.ts")
}

fn declare<T: TS>(cfg: &Config) -> String {
    format!("{}export {}\n", T::docs().unwrap_or_default(), T::decl(cfg))
}

fn generated() -> String {
    let cfg = Config::new();
    let decls = [
        // The cat (cat://state, get_state)
        declare::<CatState>(&cfg),
        declare::<SessionView>(&cfg),
        declare::<Attention>(&cfg),
        declare::<WakeReason>(&cfg),
        declare::<SleepCause>(&cfg),
        // Settings (cat://settings, get_settings, set_settings)
        declare::<Settings>(&cfg),
        declare::<SettingsPatch>(&cfg),
        declare::<Character>(&cfg),
        declare::<UiLanguage>(&cfg),
        declare::<KlatLanguage>(&cfg),
        declare::<Corner>(&cfg),
        declare::<DurationMs>(&cfg),
        // Hooks (hooks_status, hooks_apply)
        declare::<HooksInfo>(&cfg),
        declare::<HooksResult>(&cfg),
        declare::<Status>(&cfg),
        declare::<Action>(&cfg),
        // The overlay window (set_hitboxes, cat://cursor, cat://grab, overlay_preview_zoom, drag_permission)
        declare::<wire::Point>(&cfg),
        declare::<wire::Rect>(&cfg),
        declare::<GrabMode>(&cfg),
        declare::<ZoomChange>(&cfg),
        declare::<Permission>(&cfg),
    ];
    let mut out = HEADER.to_owned();
    for d in decls {
        out.push('\n');
        // ts-rs leaves a space before the line break of a field that has a doc comment.
        for line in d.lines() {
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out
}

#[test]
fn generated_ts_is_up_to_date() {
    let want = generated();
    if std::env::var_os("KLAT_WRITE_BINDINGS").is_some() {
        std::fs::write(path(), &want).unwrap();
        return;
    }
    // Git may check the file out with CRLF.
    let have = std::fs::read_to_string(path())
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        have == want,
        "src/bridge/generated.ts is out of date: run `pnpm gen:types` and commit it"
    );
}

/// The declarations stand for Point and Rect in every unit; they must serialize alike.
#[test]
fn points_and_rects_look_like_their_declarations() {
    let json = |v: serde_json::Value| v.to_string();
    let point = geometry::Point::<Css>::new(1.5, 2.0);
    let wire_point = wire::Point { x: 1.5, y: 2.0 };
    assert_eq!(
        json(serde_json::to_value(point).unwrap()),
        json(serde_json::to_value(wire_point).unwrap())
    );
    let screen = klat_core::geometry::WindowPos::new(3, -4);
    assert_eq!(serde_json::to_string(&screen).unwrap(), r#"{"x":3,"y":-4}"#);
    let rect = Rect::<Css>::new(1.0, 2.0, 3.0, 4.0);
    let wire_rect = wire::Rect {
        x: 1.0,
        y: 2.0,
        w: 3.0,
        h: 4.0,
    };
    assert_eq!(
        json(serde_json::to_value(rect).unwrap()),
        json(serde_json::to_value(wire_rect).unwrap())
    );
}

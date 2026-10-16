//! What the hook adapter (`klat-hook`) and the Klat app agree on (docs/ARCHITECTURE.md §5.1).
//!
//! They are two programs that can be different versions: the app may be updated while
//! `~/.claude/settings.json` still runs an older hook. So everything that crosses between them lives here, once:
//! the body of `POST /event` ([`HookEvent`]), the hook names ([`HookName`]), `runtime.json` ([`RuntimeInfo`])
//! and where it is ([`runtime_path`]).
//!
//! Compatibility: fields are only ever added to [`HookEvent`], as `Option` with `#[serde(default)]`; a field
//! that shipped is never renamed or removed. The app accepts events from an older hook.

mod data_dir;
mod hook;
mod runtime;

pub use data_dir::{data_dir, data_dir_for, runtime_path, Os, CURRENT_OS};
pub use hook::{HookEvent, HookName};
pub use runtime::{is_valid_token, RuntimeInfo, TOKEN_LEN};

/// The one endpoint the app serves.
pub const EVENT_PATH: &str = "/event";
/// Largest event body the app reads. The hook's bodies are a few hundred bytes.
pub const MAX_BODY: usize = 64 * 1024;

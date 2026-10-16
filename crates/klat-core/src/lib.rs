//! Klat's rules (docs/ARCHITECTURE.md §5.2): which Claude Code sessions need you and when the cat is awake
//! (`session`, `cat`), the settings model, the edits Install and Uninstall make to Claude Code's settings, where
//! things are on screen, the texts, and how often to complain about bad requests. Pure: no Tauri, no files or
//! sockets, no clock, no threads, no logging. The app passes the time in and logs or publishes what these
//! functions return, so every rule is tested here in milliseconds (`cargo test -p klat-core`).

pub mod cat;
pub mod claude_hooks;
pub mod geometry;
pub mod i18n;
pub mod ratelimit;
pub mod session;
pub mod settings;
pub mod time;

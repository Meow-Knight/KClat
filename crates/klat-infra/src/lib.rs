//! Klat's I/O (docs/ARCHITECTURE.md §5.3): files, sockets, threads, the clock. No Tauri, so each adapter is
//! tested on its own; the app wires them to its windows and events.

pub mod event_server;
pub mod log;
pub mod runtime_file;

//! What the webviews see: commands they invoke and events they listen to (docs/ARCHITECTURE.md §5.4).

pub mod commands;
pub mod error;
pub mod events;

#[cfg(test)]
mod bindings;

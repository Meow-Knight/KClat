// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap;
mod ipc;
mod platform;
mod services;
mod tasks;
mod ui;

fn main() {
    platform::scrub_env();
    bootstrap::run();
}

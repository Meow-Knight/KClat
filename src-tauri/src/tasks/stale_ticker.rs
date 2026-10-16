//! Clears sessions that have waited longer than `staleAfterMs`.

use crate::services::cat_service;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;

/// How often waiting sessions are checked against `staleAfterMs`.
const STALE_CHECK: Duration = Duration::from_secs(30);

pub fn start(app: AppHandle) {
    thread::Builder::new()
        .name("stale-ticker".into())
        .spawn(move || loop {
            thread::sleep(STALE_CHECK);
            cat_service::expire_stale(&app);
        })
        .expect("failed to start the stale ticker");
}

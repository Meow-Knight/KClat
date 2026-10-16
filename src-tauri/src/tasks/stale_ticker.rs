//! Clears sessions that have waited longer than `staleAfterMs`.

use crate::services::cat_service::CatService;
use klat_core::cat::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// How often waiting sessions are checked against `staleAfterMs`.
const STALE_CHECK: Duration = Duration::from_secs(30);

pub fn start(cat: Arc<CatService>) {
    thread::Builder::new()
        .name("stale-ticker".into())
        .spawn(move || loop {
            thread::sleep(STALE_CHECK);
            let stale_after = cat.settings().stale_after_ms;
            cat.dispatch(Command::ExpireStale { stale_after });
        })
        .expect("failed to start the stale ticker");
}

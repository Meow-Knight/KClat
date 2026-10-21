//! Ends waits that no hook ends: permission prompts you answered (no hook runs when you do) and sessions that
//! have waited longer than `staleAfterMs`.

use crate::services::cat_service::CatService;
use klat_core::cat::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// How often answered prompts are looked for. A prompt counts as answered 8s after it appeared, so this is how
/// late past that the cat may fall asleep.
const TICK: Duration = Duration::from_secs(1);
/// Waiting sessions are checked against `staleAfterMs` every this many ticks (30s).
const STALE_EVERY: u32 = 30;

pub fn start(cat: Arc<CatService>) {
    thread::Builder::new()
        .name("wait-ticker".into())
        .spawn(move || {
            let mut tick = 0;
            loop {
                thread::sleep(TICK);
                cat.dispatch(Command::ClearAnswered);
                tick = (tick + 1) % STALE_EVERY;
                if tick == 0 {
                    let stale_after = cat.settings().stale_after_ms;
                    cat.dispatch(Command::ExpireStale { stale_after });
                }
            }
        })
        .expect("failed to start the wait ticker");
}

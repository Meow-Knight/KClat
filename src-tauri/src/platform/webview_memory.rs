//! WebView2 memory (ClauCat-gwd). Most of Klat's memory is WebView2's own processes; the GPU process alone is
//! ~90 MB of commit, ~80 of it for compositing whether or not the page uses WebGL. Only `--disable-gpu` brings
//! that down, and it costs 5× the CPU while the cat sleeps, so it is not used. What is used:
//!
//! - The network service runs inside the browser process (the page only talks to the app over IPC): one process
//!   and ~10 MB less.
//! - While the cat sleeps, the overlay asks WebView2 for its low memory target, which trims the working sets of
//!   all its processes (Task Manager's "Memory" goes from ~117 MB to ~50 MB). Back to normal when it wakes.
//!
//! Measurements are in docs/development.md (Performance).

use klat_core::cat::CatState;
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::time::Duration;
use tauri::WebviewWindow;
use tokio::sync::watch::Receiver;

/// Browser arguments for every webview: wry's defaults (it drops them when you pass your own) plus the
/// in-process network service. All webviews must use the same arguments, or WebView2 refuses to create the
/// second one in the same data folder.
pub const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --enable-features=NetworkServiceInProcess2";

/// Environment variables WebView2 reads its command line, browser and debugger from. Any program the user runs
/// can set them, and `--remote-debugging-port` would let it drive the app and its commands over DevTools.
/// Release builds clear them before any webview starts; debug builds keep them for driving the app in checks.
pub const WEBVIEW2_ENV: [&str; 3] = [
    "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
    "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
    "WEBVIEW2_PIPE_FOR_SCRIPT_DEBUGGER",
];

/// Call first thing in `main`, before any thread or webview exists.
pub fn scrub_env() {
    if cfg!(all(windows, not(debug_assertions))) {
        for key in WEBVIEW2_ENV {
            std::env::remove_var(key);
        }
    }
}

/// Wait this long after the cat falls asleep before trimming, so the yawn or the knockout (2.1 s) plays out first.
#[cfg(windows)]
const SETTLE: Duration = Duration::from_millis(3000);

/// Follows the cat: normal memory while it is awake, low once it has settled asleep. Starts from the state now
/// (asleep at launch, so the page is trimmed once it has settled).
#[cfg(windows)]
pub fn follow(win: WebviewWindow, mut state: Receiver<CatState>) {
    // Bumped on every awake change, so a trim scheduled before the cat woke up again is dropped.
    let generation = Arc::new(AtomicU64::new(0));
    let mut awake = state.borrow().awake;
    awake_changed(&win, &generation, awake);
    tauri::async_runtime::spawn(async move {
        while state.changed().await.is_ok() {
            let now = state.borrow_and_update().awake;
            if now != awake {
                awake = now;
                awake_changed(&win, &generation, awake);
            }
        }
    });
}

#[cfg(not(windows))]
pub fn follow(_win: WebviewWindow, _state: Receiver<CatState>) {}

/// The cat woke up or fell asleep.
#[cfg(windows)]
fn awake_changed(win: &WebviewWindow, generation: &Arc<AtomicU64>, awake: bool) {
    let current = generation.fetch_add(1, Ordering::SeqCst) + 1;
    if awake {
        set_low(win, false);
        return;
    }
    let (win, generation) = (win.clone(), generation.clone());
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        if generation.load(Ordering::SeqCst) == current {
            set_low(&win, true);
        }
    });
}

#[cfg(windows)]
fn set_low(win: &WebviewWindow, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    let level = if low {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
    } else {
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
    };
    let _ = win.with_webview(move |webview| {
        // Runtimes older than ICoreWebView2_19 (WebView2 114) just keep the normal level.
        let result = unsafe {
            webview
                .controller()
                .CoreWebView2()
                .and_then(|core| core.cast::<ICoreWebView2_19>())
                .and_then(|core| core.SetMemoryUsageTargetLevel(level))
        };
        if let Err(e) = result {
            log::warn!("could not set the webview memory level: {e}");
        }
    });
}

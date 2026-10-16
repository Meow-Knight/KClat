//! WebView2 memory (ClauCat-gwd). Most of Klat's memory is WebView2's own processes; the GPU process alone is
//! ~90 MB of commit, ~80 of it for compositing whether or not the page uses WebGL. Only `--disable-gpu` brings
//! that down, and it costs 5× the CPU while the cat sleeps, so it is not used. What is used:
//!
//! - The network service runs inside the browser process (the page only talks to the app over IPC): one process
//!   and ~10 MB less.
//! - While the cat sleeps, the overlay asks WebView2 for its low memory target, which trims the working sets of
//!   all its processes (Task Manager's "Memory" goes from ~117 MB to ~50 MB). Back to normal when it wakes.
//!
//! Measurements are in README.md (Performance).

#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::time::Duration;
use tauri::AppHandle;

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

/// Bumped on every awake change, so a trim scheduled before the cat woke up again is dropped.
#[cfg(windows)]
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Called when the cat wakes up or falls asleep (and once at startup, asleep).
#[cfg(windows)]
pub fn awake_changed(app: &AppHandle, awake: bool) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if awake {
        set_low(app, false);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        if GENERATION.load(Ordering::SeqCst) == generation {
            set_low(&app, true);
        }
    });
}

#[cfg(not(windows))]
pub fn awake_changed(_app: &AppHandle, _awake: bool) {}

#[cfg(windows)]
fn set_low(app: &AppHandle, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;

    let Some(win) = crate::overlay::get(app) else {
        return;
    };
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
            crate::log::warn(format!("could not set the webview memory level: {e}"));
        }
    });
}

//! Moving the cat: hold Space with the pointer over the cat, press the mouse button and move; releasing Space
//! drops it there, and the spot is saved.
//!
//! The overlay never takes focus (WS_EX_NOACTIVATE), so it never sees the keyboard. A low-level keyboard hook
//! (platform::install_hook) watches Space, and nothing else:
//! - Space pressed while the pointer is over the cat (or its bubble or button), with no modifier held: the press
//!   is kept from the focused app and "grab" starts. A press anywhere else passes through untouched.
//! - Space released: grab ends. If the mouse was never pressed (a plain tap while the pointer happened to rest
//!   on the cat), the tap is replayed to the focused app, so typing is not lost.
//!
//! While grabbing, pressing the primary mouse button starts the drag: the window follows the cursor (~120Hz)
//! until Space is released. The cat is kept inside the work area of the monitor under the cursor.
//! The overlay is told through `cat://grab` ("idle" | "armed" | "dragging"), so it can ignore the click.

use crate::ipc::events::EVENT_GRAB;
use crate::platform::{self, Msg};
use crate::services::cat_service::CatService;
use crate::ui::overlay::{self, OverlayState};
use klat_core::geometry::{clamp_window, Logical, Point, WindowPos};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const DRAG_PERIOD: Duration = Duration::from_millis(8);

/// The window is following the cursor.
static DRAGGING: AtomicBool = AtomicBool::new(false);

pub fn dragging() -> bool {
    DRAGGING.load(Ordering::Relaxed)
}

pub fn start(app: AppHandle, service: Arc<CatService>, overlay_state: Arc<OverlayState>) {
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let worker = app.clone();
    std::thread::Builder::new()
        .name("drag".into())
        .spawn(move || {
            while let Ok(msg) = rx.recv() {
                if let Msg::GrabStarted = msg {
                    run_grab(&worker, &service, &overlay_state, &rx);
                }
            }
        })
        .expect("failed to start the drag worker");
    platform::install_hook(app, tx);
}

/// One Space hold: wait for the mouse button, follow the cursor, save on release.
fn run_grab(
    app: &AppHandle,
    service: &CatService,
    overlay_state: &OverlayState,
    rx: &std::sync::mpsc::Receiver<Msg>,
) {
    let _ = app.emit(EVENT_GRAB, "armed");
    let mut pressed = false;
    // Logical px from the window's top-left to the grab point, so a DPI change mid-drag keeps the cat in hand.
    let mut grip: Option<Point<Logical>> = None;
    let win = overlay::get(app);
    loop {
        match rx.recv_timeout(DRAG_PERIOD) {
            Ok(Msg::GrabEnded) => break,
            Ok(Msg::GrabStarted) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let Some(win) = win.as_ref() else { continue };
        let Some(cursor) = platform::cursor() else {
            continue;
        };
        if grip.is_none() && platform::primary_button_down() {
            pressed = true;
            if let (Ok(origin), Ok(scale)) = (win.outer_position(), win.scale_factor()) {
                let origin = overlay::screen_pos(origin).to_f64();
                grip = Some(cursor.to_f64().to_logical(origin, scale));
                DRAGGING.store(true, Ordering::Relaxed);
                let _ = app.emit(EVENT_GRAB, "dragging");
            }
        }
        if let Some(grip) = grip {
            let scale = win.scale_factor().unwrap_or(1.0);
            let mut pos = grip.window_at(cursor, scale);
            let cat = overlay_state
                .hitboxes()
                .first()
                .map(|r| r.to_physical(Point::new(0.0, 0.0), scale));
            let area = app
                .monitor_from_point(f64::from(cursor.x), f64::from(cursor.y))
                .ok()
                .flatten()
                .map(|m| overlay::work_area(&m));
            if let (Some(cat), Some(area)) = (cat, area) {
                pos = clamp_window(pos, cat, area);
            }
            let _ = win.set_position(tauri::PhysicalPosition::new(pos.x, pos.y));
        }
    }

    let moved = DRAGGING.swap(false, Ordering::Relaxed);
    let _ = app.emit(EVENT_GRAB, "idle");
    if moved {
        if let Some(Ok(p)) = win.as_ref().map(|w| w.outer_position()) {
            log::info!("cat moved to {},{}", p.x, p.y);
            service.update_settings(|s| s.position = Some(WindowPos::new(p.x, p.y)));
        }
    } else if !pressed {
        // A plain tap while the pointer rested on the cat: give it back to the app you were typing in.
        platform::replay_space();
    }
}

//! Cursor poller (spec §9).
//!
//! - Click-through is turned off only while the cursor is over the cat or the bubble (both states).
//! - While the cat is awake it emits `cat://cursor` for head tracking: `{x, y}` in the page's CSS px, or `null`
//!   when the cursor is on another monitor (the cat then looks at its bubble).
//!
//! - It also hands the clickable regions to the Space hook (see tasks::drag_loop).
//!
//! Polls at 30Hz while awake and 5Hz while asleep (hitbox only).

use crate::ipc::events::EVENT_CURSOR;
use crate::platform;
use crate::services::cat_service::AppState;
use crate::tasks::drag_loop;
use crate::ui::overlay;
use klat_core::geometry::{hit_test, to_overlay, LookTracker, OverlayFrame, Point, Rect};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

const AWAKE_PERIOD: Duration = Duration::from_millis(33);
const ASLEEP_PERIOD: Duration = Duration::from_millis(200);

pub fn start(app: AppHandle) {
    thread::Builder::new()
        .name("cursor-poller".into())
        .spawn(move || {
            // The overlay starts click-through (see overlay::create).
            let mut interactive = false;
            let mut tracker = LookTracker::default();
            loop {
                let state = app.state::<AppState>();
                let period = if state.awake.load(Ordering::Relaxed) {
                    AWAKE_PERIOD
                } else {
                    ASLEEP_PERIOD
                };
                thread::sleep(period);
                let awake = state.awake.load(Ordering::Relaxed);

                let Some(win) = overlay::get(&app) else {
                    continue;
                };
                let (Ok(cursor), Ok(origin), Ok(scale)) = (
                    app.cursor_position(),
                    win.inner_position(),
                    win.scale_factor(),
                ) else {
                    continue;
                };
                let monitor = win.current_monitor().ok().flatten().map(|m| {
                    let (p, s) = (m.position(), m.size());
                    Rect::new(p.x as f64, p.y as f64, s.width as f64, s.height as f64)
                });
                let frame = OverlayFrame {
                    origin: Point::new(origin.x as f64, origin.y as f64),
                    scale,
                    monitor,
                };
                let point = to_overlay(Point::new(cursor.x, cursor.y), &frame);

                let inside = {
                    let rects = state.hitboxes();
                    platform::set_zone(frame.origin, scale, &rects);
                    // Never turn click-through on under a cat that is being dragged: the release must land on it.
                    drag_loop::dragging() || hit_test(point, &rects)
                };
                if inside != interactive && win.set_ignore_cursor_events(!inside).is_ok() {
                    interactive = inside;
                }

                if let Some(payload) = tracker.update(awake, point) {
                    // The page works in CSS px.
                    let zoom = overlay::zoom();
                    let _ = app.emit(EVENT_CURSOR, payload.map(|p| p.to_css(zoom)));
                }
            }
        })
        .expect("failed to start the cursor poller");
}

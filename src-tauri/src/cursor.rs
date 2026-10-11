//! Cursor poller (spec §9).
//!
//! - Click-through is turned off only while the cursor is over the cat or the bubble (both states).
//! - While the cat is awake it emits `cat://cursor` for head tracking: logical px relative to the overlay's
//!   top-left corner, or `null` when the cursor is on another monitor (the cat then looks at its bubble).
//!
//! Polls at 30Hz while awake and 5Hz while asleep (hitbox only).

use crate::overlay;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_CURSOR: &str = "cat://cursor";
const AWAKE_PERIOD: Duration = Duration::from_millis(33);
const ASLEEP_PERIOD: Duration = Duration::from_millis(200);

/// A clickable region in logical px relative to the overlay's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn contains(&self, (x, y): Point) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

type Point = (f64, f64);

/// `cat://cursor` payload; the event carries `null` when there is no usable cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CursorPoint {
    pub x: f64,
    pub y: f64,
}

/// Where the overlay is, in physical screen px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub origin: Point,
    pub scale: f64,
    /// The overlay's monitor: (x, y, width, height). None if unknown, which disables the other-monitor check.
    pub monitor: Option<(f64, f64, f64, f64)>,
}

/// Physical cursor → logical px relative to the overlay, or None when the cursor is on another monitor.
pub fn to_overlay(cursor: Point, g: &Geometry) -> Option<Point> {
    if let Some((mx, my, mw, mh)) = g.monitor {
        let on_monitor =
            cursor.0 >= mx && cursor.0 < mx + mw && cursor.1 >= my && cursor.1 < my + mh;
        if !on_monitor {
            return None;
        }
    }
    Some((
        (cursor.0 - g.origin.0) / g.scale,
        (cursor.1 - g.origin.1) / g.scale,
    ))
}

pub fn hit_test(point: Option<Point>, rects: &[Rect]) -> bool {
    point.is_some_and(|p| rects.iter().any(|r| r.contains(p)))
}

/// Decides when to send `cat://cursor`.
///
/// Right after waking it stays quiet until the cursor moves, so the cat first looks at its bubble (spec §6.4);
/// after that it sends every change. Nothing is sent while asleep.
#[derive(Debug, Default)]
pub struct LookTracker {
    awake: bool,
    last: Option<Point>,
}

impl LookTracker {
    /// Returns `Some(payload)` when an event should go out; the payload itself may be `None` (no cursor data).
    pub fn update(&mut self, awake: bool, point: Option<Point>) -> Option<Option<Point>> {
        if !awake {
            self.awake = false;
            return None;
        }
        if !self.awake {
            self.awake = true;
            self.last = point;
            return None;
        }
        if self.last == point {
            return None;
        }
        self.last = point;
        Some(point)
    }
}

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
                    (p.x as f64, p.y as f64, s.width as f64, s.height as f64)
                });
                let geometry = Geometry {
                    origin: (origin.x as f64, origin.y as f64),
                    scale,
                    monitor,
                };
                let point = to_overlay((cursor.x, cursor.y), &geometry);

                let inside = hit_test(point, &state.hitboxes.lock().unwrap());
                if inside != interactive && win.set_ignore_cursor_events(!inside).is_ok() {
                    interactive = inside;
                }

                if let Some(payload) = tracker.update(awake, point) {
                    let _ = app.emit(EVENT_CURSOR, payload.map(|(x, y)| CursorPoint { x, y }));
                }
            }
        })
        .expect("failed to start the cursor poller");
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAT: Rect = Rect {
        x: 150.0,
        y: 60.0,
        w: 170.0,
        h: 190.0,
    };
    const BUBBLE: Rect = Rect {
        x: 8.0,
        y: 14.0,
        w: 200.0,
        h: 60.0,
    };
    /// 2560×1440 primary monitor at 100%, overlay at its bottom-right.
    const PRIMARY: Geometry = Geometry {
        origin: (2184.0, 1096.0),
        scale: 1.0,
        monitor: Some((0.0, 0.0, 2560.0, 1440.0)),
    };

    #[test]
    fn converts_to_overlay_logical_px() {
        assert_eq!(to_overlay((2384.0, 1196.0), &PRIMARY), Some((200.0, 100.0)));
        // Far left of the screen: negative x relative to the overlay, still on the monitor.
        assert_eq!(to_overlay((0.0, 0.0), &PRIMARY), Some((-2184.0, -1096.0)));
    }

    #[test]
    fn scales_physical_px_to_logical() {
        for (scale, expected) in [(1.25, (160.0, 80.0)), (1.5, (133.33, 66.67))] {
            let g = Geometry {
                origin: (1000.0, 500.0),
                scale,
                monitor: Some((0.0, 0.0, 3840.0, 2160.0)),
            };
            let (x, y) = to_overlay((1200.0, 600.0), &g).unwrap();
            assert!(
                (x - expected.0).abs() < 0.01 && (y - expected.1).abs() < 0.01,
                "{scale}: {x},{y}"
            );
        }
    }

    #[test]
    fn another_monitor_means_no_cursor_data() {
        // Second monitor to the left (negative coordinates) and one to the right.
        assert_eq!(to_overlay((-500.0, 300.0), &PRIMARY), None);
        assert_eq!(to_overlay((2560.0, 300.0), &PRIMARY), None);
        let unknown = Geometry {
            monitor: None,
            ..PRIMARY
        };
        assert!(to_overlay((-500.0, 300.0), &unknown).is_some());
    }

    #[test]
    fn hit_test_any_rect_half_open_edges() {
        assert!(hit_test(Some((200.0, 100.0)), &[CAT]));
        assert!(hit_test(Some((10.0, 20.0)), &[CAT, BUBBLE]));
        assert!(hit_test(Some((150.0, 60.0)), &[CAT]));
        assert!(!hit_test(Some((320.0, 60.0)), &[CAT]));
        assert!(!hit_test(Some((20.0, 200.0)), &[CAT, BUBBLE]));
    }

    #[test]
    fn no_rects_or_no_cursor_means_click_through() {
        assert!(!hit_test(Some((5.0, 5.0)), &[]));
        assert!(!hit_test(None, &[CAT, BUBBLE]));
    }

    #[test]
    fn tracker_is_silent_while_asleep() {
        let mut t = LookTracker::default();
        assert_eq!(t.update(false, Some((1.0, 1.0))), None);
        assert_eq!(t.update(false, Some((2.0, 2.0))), None);
    }

    #[test]
    fn tracker_waits_for_movement_after_waking_then_sends_changes() {
        let mut t = LookTracker::default();
        assert_eq!(t.update(true, Some((10.0, 10.0))), None); // just woke
        assert_eq!(t.update(true, Some((10.0, 10.0))), None); // cursor still
        assert_eq!(t.update(true, Some((12.0, 10.0))), Some(Some((12.0, 10.0))));
        assert_eq!(t.update(true, Some((12.0, 10.0))), None);
        assert_eq!(t.update(true, None), Some(None)); // moved to another monitor
        assert_eq!(t.update(true, None), None);
        assert_eq!(t.update(true, Some((0.0, 0.0))), Some(Some((0.0, 0.0))));
    }

    #[test]
    fn tracker_resets_on_each_wake() {
        let mut t = LookTracker::default();
        t.update(true, Some((1.0, 1.0)));
        t.update(true, Some((5.0, 5.0)));
        t.update(false, Some((5.0, 5.0)));
        assert_eq!(t.update(true, Some((9.0, 9.0))), None);
        assert_eq!(t.update(true, Some((9.0, 9.0))), None);
    }
}

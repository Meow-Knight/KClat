//! Where things are on screen: the overlay's clickable regions, the cursor relative to the overlay, keeping
//! the cat on a monitor. Pure arithmetic; the app reads the window and monitor geometry and passes it in.

use serde::{Deserialize, Serialize};

/// A clickable region in logical px relative to the overlay's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// The same region `zoom` times larger: the page reports CSS px, the webview zoom (catScale) enlarges them.
    pub fn scaled(&self, zoom: f64) -> Self {
        Self {
            x: self.x * zoom,
            y: self.y * zoom,
            w: self.w * zoom,
            h: self.h * zoom,
        }
    }

    fn contains(&self, (x, y): Point) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

pub type Point = (f64, f64);

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

/// Window top-left in physical screen px, as saved in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPos {
    pub x: i32,
    pub y: i32,
}

/// (x, y, width, height) in physical px.
pub type PxRect = (i32, i32, i32, i32);

pub fn to_px(origin: (f64, f64), scale: f64, r: &Rect) -> PxRect {
    (
        (origin.0 + r.x * scale).round() as i32,
        (origin.1 + r.y * scale).round() as i32,
        (r.w * scale).round() as i32,
        (r.h * scale).round() as i32,
    )
}

pub fn in_rect((x, y): (i32, i32), (rx, ry, rw, rh): PxRect) -> bool {
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

/// Window position that keeps `cat` (relative to the window's top-left) inside `area`. If the cat is larger
/// than the area, its top-left edge wins.
pub fn clamp_window(pos: (i32, i32), cat: PxRect, area: PxRect) -> (i32, i32) {
    let axis = |p: i32, off: i32, len: i32, start: i32, size: i32| {
        let min = start - off;
        let max = start + size - off - len;
        p.min(max).max(min)
    };
    (
        axis(pos.0, cat.0, cat.2, area.0, area.2),
        axis(pos.1, cat.1, cat.3, area.1, area.3),
    )
}

/// A saved position is used only if the window's centre is still on one of the work areas (a monitor may have
/// been unplugged or rearranged since).
pub fn is_on_screen(pos: WindowPos, size: (u32, u32), areas: &[PxRect]) -> bool {
    let centre = (pos.x + size.0 as i32 / 2, pos.y + size.1 as i32 / 2);
    areas.iter().any(|a| in_rect(centre, *a))
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
    fn page_rects_scale_with_the_cat_size() {
        // A 100×80 CSS px box at (200, 150) on a 150% cat covers 150×120 logical px at (300, 225).
        let page = Rect {
            x: 200.0,
            y: 150.0,
            w: 100.0,
            h: 80.0,
        };
        let logical = page.scaled(1.5);
        assert_eq!(
            logical,
            Rect {
                x: 300.0,
                y: 225.0,
                w: 150.0,
                h: 120.0
            }
        );
        assert!(hit_test(Some((440.0, 340.0)), &[logical]));
        assert!(!hit_test(Some((440.0, 340.0)), &[page]));
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

    /// 2560×1392 work area (48px taskbar). The cat sits at (150, 80)-(320, 300) inside a 360×330 window.
    const AREA: PxRect = (0, 0, 2560, 1392);
    const CAT_PX: PxRect = (150, 80, 170, 220);

    #[test]
    fn a_position_inside_the_area_is_kept() {
        assert_eq!(clamp_window((1000, 500), CAT_PX, AREA), (1000, 500));
    }

    #[test]
    fn the_cat_stays_on_screen_while_the_window_may_hang_off() {
        // Dragged far left and up: the transparent margins may leave the screen, the cat may not.
        assert_eq!(clamp_window((-900, -900), CAT_PX, AREA), (-150, -80));
        // Far right and down, under the taskbar.
        assert_eq!(
            clamp_window((5000, 5000), CAT_PX, AREA),
            (2560 - 320, 1392 - 300)
        );
    }

    #[test]
    fn clamping_respects_a_second_monitor_offset() {
        let left_monitor: PxRect = (-1920, 0, 1920, 1040);
        assert_eq!(
            clamp_window((-3000, 200), CAT_PX, left_monitor),
            (-1920 - 150, 200)
        );
    }

    #[test]
    fn a_cat_larger_than_the_area_keeps_its_top_left_visible() {
        assert_eq!(
            clamp_window((50, 50), CAT_PX, (0, 0, 100, 100)),
            (-150, -80)
        );
    }

    #[test]
    fn saved_positions_must_still_be_on_a_monitor() {
        let size = (360, 330);
        assert!(is_on_screen(WindowPos { x: 100, y: 100 }, size, &[AREA]));
        // Window hanging off the left edge, centre still on screen.
        assert!(is_on_screen(WindowPos { x: -150, y: 100 }, size, &[AREA]));
        // A monitor that was unplugged.
        assert!(!is_on_screen(WindowPos { x: -1500, y: 100 }, size, &[AREA]));
        assert!(!is_on_screen(WindowPos { x: 100, y: 100 }, size, &[]));
    }

    #[test]
    fn zone_converts_logical_rects_to_screen_px() {
        let r = Rect {
            x: 150.0,
            y: 80.0,
            w: 170.0,
            h: 220.0,
        };
        assert_eq!(to_px((1000.0, 500.0), 1.25, &r), (1188, 600, 213, 275));
        assert!(in_rect((1188, 600), to_px((1000.0, 500.0), 1.25, &r)));
        assert!(!in_rect((1401, 600), to_px((1000.0, 500.0), 1.25, &r)));
    }
}

//! Where things are on screen: the overlay's clickable regions, the cursor relative to the overlay, keeping
//! the cat on a monitor. Pure arithmetic; the app reads the window and monitor geometry and passes it in.
//!
//! Three coordinate spaces, kept apart by type (mixing them up was behind several DPI and cat-size bugs):
//! - [`Css`]: the overlay page's px. The page always lays out at the same size; the webview zoom (the cat size
//!   setting) enlarges it.
//! - [`Logical`]: the overlay window's px, from its top-left corner: CSS px × zoom.
//! - [`Physical`]: screen pixels, as the OS reports windows, monitors and the cursor: logical px × the monitor's
//!   scale factor, from the screen's origin.
//!
//! A value changes space only through a named conversion that takes the zoom or scale it needs
//! ([`Rect::to_logical`], [`Point::to_css`], [`Rect::to_physical`], [`Point::to_logical`]...).

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::ops::Add;

/// The overlay page's CSS px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Css {}
/// The overlay window's logical px, relative to its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logical {}
/// Screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Physical {}

/// A point in space `U`. The numbers are `f64`, except for whole screen pixels (window positions): `i32`.
/// On the wire it is `{x, y}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point<U, N = f64> {
    pub x: N,
    pub y: N,
    #[serde(skip)]
    unit: PhantomData<U>,
}

/// A rectangle in space `U`: top-left corner, width and height. On the wire it is `{x, y, w, h}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect<U, N = f64> {
    pub x: N,
    pub y: N,
    pub w: N,
    pub h: N,
    #[serde(skip)]
    unit: PhantomData<U>,
}

/// A window's top-left corner on screen, as saved in settings (`position`).
pub type WindowPos = Point<Physical, i32>;

impl<U, N> Point<U, N> {
    pub const fn new(x: N, y: N) -> Self {
        Self {
            x,
            y,
            unit: PhantomData,
        }
    }
}

impl<U, N> Rect<U, N> {
    pub const fn new(x: N, y: N, w: N, h: N) -> Self {
        Self {
            x,
            y,
            w,
            h,
            unit: PhantomData,
        }
    }
}

impl<U, N: Copy + PartialOrd + Add<Output = N>> Rect<U, N> {
    /// Left and top edges are inside, right and bottom edges outside, so two rects that touch never both
    /// contain a point.
    pub fn contains(&self, p: Point<U, N>) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
}

impl Rect<Css> {
    /// The same region in the window's logical px: the webview zoom (cat size) enlarges the page.
    pub fn to_logical(self, zoom: f64) -> Rect<Logical> {
        Rect::new(self.x * zoom, self.y * zoom, self.w * zoom, self.h * zoom)
    }
}

impl Point<Logical> {
    /// The same point in the page's CSS px.
    pub fn to_css(self, zoom: f64) -> Point<Css> {
        Point::new(self.x / zoom, self.y / zoom)
    }

    /// Where a window must go so that this point of it (a grip, from its top-left) is under `cursor`.
    pub fn window_at(self, cursor: Point<Physical, i32>, scale: f64) -> WindowPos {
        Point::new(
            cursor.x - (self.x * scale).round() as i32,
            cursor.y - (self.y * scale).round() as i32,
        )
    }
}

impl Rect<Logical> {
    /// On screen, in whole pixels, for a window whose top-left is at `origin` on a monitor with this scale
    /// factor. With `origin` at (0, 0): where the region is inside the window, in physical px.
    pub fn to_physical(self, origin: Point<Physical>, scale: f64) -> Rect<Physical, i32> {
        Rect::new(
            (origin.x + self.x * scale).round() as i32,
            (origin.y + self.y * scale).round() as i32,
            (self.w * scale).round() as i32,
            (self.h * scale).round() as i32,
        )
    }
}

impl Point<Physical> {
    /// Relative to a window whose top-left is at `origin`, on a monitor with this scale factor.
    pub fn to_logical(self, origin: Point<Physical>, scale: f64) -> Point<Logical> {
        Point::new((self.x - origin.x) / scale, (self.y - origin.y) / scale)
    }
}

impl Point<Physical, i32> {
    pub fn to_f64(self) -> Point<Physical> {
        Point::new(f64::from(self.x), f64::from(self.y))
    }
}

/// Where the overlay is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayFrame {
    /// The window's top-left.
    pub origin: Point<Physical>,
    /// Its monitor's scale factor.
    pub scale: f64,
    /// Its monitor. None if unknown, which disables the other-monitor check.
    pub monitor: Option<Rect<Physical>>,
}

/// The cursor relative to the overlay, or None when the cursor is on another monitor.
pub fn to_overlay(cursor: Point<Physical>, frame: &OverlayFrame) -> Option<Point<Logical>> {
    if frame.monitor.is_some_and(|m| !m.contains(cursor)) {
        return None;
    }
    Some(cursor.to_logical(frame.origin, frame.scale))
}

pub fn hit_test(point: Option<Point<Logical>>, rects: &[Rect<Logical>]) -> bool {
    point.is_some_and(|p| rects.iter().any(|r| r.contains(p)))
}

/// Decides when to send `cat://cursor`.
///
/// Right after waking it stays quiet until the cursor moves, so the cat first looks at its bubble (spec §6.4);
/// after that it sends every change. Nothing is sent while asleep.
#[derive(Debug, Default)]
pub struct LookTracker {
    awake: bool,
    last: Option<Point<Logical>>,
}

impl LookTracker {
    /// Returns `Some(payload)` when an event should go out; the payload itself may be `None` (no cursor data).
    pub fn update(
        &mut self,
        awake: bool,
        point: Option<Point<Logical>>,
    ) -> Option<Option<Point<Logical>>> {
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

/// Window position that keeps `cat` (relative to the window's top-left) inside `area`. If the cat is larger
/// than the area, its top-left edge wins.
pub fn clamp_window(
    pos: WindowPos,
    cat: Rect<Physical, i32>,
    area: Rect<Physical, i32>,
) -> WindowPos {
    let axis = |p: i32, off: i32, len: i32, start: i32, size: i32| {
        let min = start - off;
        let max = start + size - off - len;
        p.min(max).max(min)
    };
    Point::new(
        axis(pos.x, cat.x, cat.w, area.x, area.w),
        axis(pos.y, cat.y, cat.h, area.y, area.h),
    )
}

/// A saved position is used only if the window's centre is still on one of the work areas (a monitor may have
/// been unplugged or rearranged since). `size` in physical px.
pub fn is_on_screen(pos: WindowPos, size: (u32, u32), areas: &[Rect<Physical, i32>]) -> bool {
    let centre = Point::new(pos.x + size.0 as i32 / 2, pos.y + size.1 as i32 / 2);
    areas.iter().any(|a| a.contains(centre))
}

/// Top-left that keeps a window's bottom-right corner in place when it goes from `old` to `new` size in
/// physical px (a new cat scale, or a DPI change).
pub fn keep_bottom_right(pos: WindowPos, old: (u32, u32), new: (u32, u32)) -> WindowPos {
    Point::new(
        pos.x + old.0 as i32 - new.0 as i32,
        pos.y + old.1 as i32 - new.1 as i32,
    )
}

/// Top-left for a window of `size` (physical px) inside the bottom-right corner of a work area, `margin`
/// logical px from its edges.
pub fn bottom_right(
    area: Rect<Physical, i32>,
    size: (u32, u32),
    margin: f64,
    scale: f64,
) -> WindowPos {
    let margin = (margin * scale).round() as i32;
    Point::new(
        area.x + area.w - size.0 as i32 - margin,
        area.y + area.h - size.1 as i32 - margin,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAT: Rect<Logical> = Rect::new(150.0, 60.0, 170.0, 190.0);
    const BUBBLE: Rect<Logical> = Rect::new(8.0, 14.0, 200.0, 60.0);
    /// 2560×1440 primary monitor at 100%, overlay at its bottom-right.
    const PRIMARY: OverlayFrame = OverlayFrame {
        origin: Point::new(2184.0, 1096.0),
        scale: 1.0,
        monitor: Some(Rect::new(0.0, 0.0, 2560.0, 1440.0)),
    };

    fn at(x: f64, y: f64) -> Option<Point<Logical>> {
        Some(Point::new(x, y))
    }
    fn screen(x: f64, y: f64) -> Point<Physical> {
        Point::new(x, y)
    }

    #[test]
    fn converts_to_overlay_logical_px() {
        assert_eq!(
            to_overlay(screen(2384.0, 1196.0), &PRIMARY),
            at(200.0, 100.0)
        );
        // Far left of the screen: negative x relative to the overlay, still on the monitor.
        assert_eq!(to_overlay(screen(0.0, 0.0), &PRIMARY), at(-2184.0, -1096.0));
    }

    #[test]
    fn scales_physical_px_to_logical() {
        for (scale, expected) in [(1.25, (160.0, 80.0)), (1.5, (133.33, 66.67))] {
            let frame = OverlayFrame {
                origin: Point::new(1000.0, 500.0),
                scale,
                monitor: Some(Rect::new(0.0, 0.0, 3840.0, 2160.0)),
            };
            let p = to_overlay(screen(1200.0, 600.0), &frame).unwrap();
            assert!(
                (p.x - expected.0).abs() < 0.01 && (p.y - expected.1).abs() < 0.01,
                "{scale}: {p:?}"
            );
        }
    }

    #[test]
    fn another_monitor_means_no_cursor_data() {
        // Second monitor to the left (negative coordinates) and one to the right.
        assert_eq!(to_overlay(screen(-500.0, 300.0), &PRIMARY), None);
        assert_eq!(to_overlay(screen(2560.0, 300.0), &PRIMARY), None);
        let unknown = OverlayFrame {
            monitor: None,
            ..PRIMARY
        };
        assert!(to_overlay(screen(-500.0, 300.0), &unknown).is_some());
    }

    #[test]
    fn page_rects_scale_with_the_cat_size() {
        // A 100×80 CSS px box at (200, 150) on a 150% cat covers 150×120 logical px at (300, 225).
        let page: Rect<Css> = Rect::new(200.0, 150.0, 100.0, 80.0);
        let logical = page.to_logical(1.5);
        assert_eq!(logical, Rect::new(300.0, 225.0, 150.0, 120.0));
        assert!(hit_test(at(440.0, 340.0), &[logical]));
        // Before the conversion (the old bug): the same point misses the CSS-sized box.
        assert!(!page.contains(Point::new(440.0, 340.0)));
    }

    #[test]
    fn cursor_points_go_back_to_css_px_for_the_page() {
        assert_eq!(
            Point::<Logical>::new(300.0, 225.0).to_css(1.5),
            Point::new(200.0, 150.0)
        );
        // On the wire it is {x, y}, as `cat://cursor` always was.
        assert_eq!(
            serde_json::to_string(&Point::<Css>::new(1.5, -2.0)).unwrap(),
            r#"{"x":1.5,"y":-2.0}"#
        );
    }

    #[test]
    fn hit_test_any_rect_half_open_edges() {
        assert!(hit_test(at(200.0, 100.0), &[CAT]));
        assert!(hit_test(at(10.0, 20.0), &[CAT, BUBBLE]));
        assert!(hit_test(at(150.0, 60.0), &[CAT]));
        assert!(!hit_test(at(320.0, 60.0), &[CAT]));
        assert!(!hit_test(at(20.0, 200.0), &[CAT, BUBBLE]));
    }

    #[test]
    fn no_rects_or_no_cursor_means_click_through() {
        assert!(!hit_test(at(5.0, 5.0), &[]));
        assert!(!hit_test(None, &[CAT, BUBBLE]));
    }

    #[test]
    fn tracker_is_silent_while_asleep() {
        let mut t = LookTracker::default();
        assert_eq!(t.update(false, at(1.0, 1.0)), None);
        assert_eq!(t.update(false, at(2.0, 2.0)), None);
    }

    #[test]
    fn tracker_waits_for_movement_after_waking_then_sends_changes() {
        let mut t = LookTracker::default();
        assert_eq!(t.update(true, at(10.0, 10.0)), None); // just woke
        assert_eq!(t.update(true, at(10.0, 10.0)), None); // cursor still
        assert_eq!(t.update(true, at(12.0, 10.0)), Some(at(12.0, 10.0)));
        assert_eq!(t.update(true, at(12.0, 10.0)), None);
        assert_eq!(t.update(true, None), Some(None)); // moved to another monitor
        assert_eq!(t.update(true, None), None);
        assert_eq!(t.update(true, at(0.0, 0.0)), Some(at(0.0, 0.0)));
    }

    #[test]
    fn tracker_resets_on_each_wake() {
        let mut t = LookTracker::default();
        t.update(true, at(1.0, 1.0));
        t.update(true, at(5.0, 5.0));
        t.update(false, at(5.0, 5.0));
        assert_eq!(t.update(true, at(9.0, 9.0)), None);
        assert_eq!(t.update(true, at(9.0, 9.0)), None);
    }

    /// 2560×1392 work area (48px taskbar). The cat sits at (150, 80)-(320, 300) inside a 360×330 window.
    const AREA: Rect<Physical, i32> = Rect::new(0, 0, 2560, 1392);
    const CAT_PX: Rect<Physical, i32> = Rect::new(150, 80, 170, 220);

    fn pos(x: i32, y: i32) -> WindowPos {
        Point::new(x, y)
    }

    #[test]
    fn a_position_inside_the_area_is_kept() {
        assert_eq!(clamp_window(pos(1000, 500), CAT_PX, AREA), pos(1000, 500));
    }

    #[test]
    fn the_cat_stays_on_screen_while_the_window_may_hang_off() {
        // Dragged far left and up: the transparent margins may leave the screen, the cat may not.
        assert_eq!(clamp_window(pos(-900, -900), CAT_PX, AREA), pos(-150, -80));
        // Far right and down, under the taskbar.
        assert_eq!(
            clamp_window(pos(5000, 5000), CAT_PX, AREA),
            pos(2560 - 320, 1392 - 300)
        );
    }

    #[test]
    fn clamping_respects_a_second_monitor_offset() {
        let left_monitor = Rect::new(-1920, 0, 1920, 1040);
        assert_eq!(
            clamp_window(pos(-3000, 200), CAT_PX, left_monitor),
            pos(-1920 - 150, 200)
        );
    }

    #[test]
    fn a_cat_larger_than_the_area_keeps_its_top_left_visible() {
        assert_eq!(
            clamp_window(pos(50, 50), CAT_PX, Rect::new(0, 0, 100, 100)),
            pos(-150, -80)
        );
    }

    #[test]
    fn saved_positions_must_still_be_on_a_monitor() {
        let size = (360, 330);
        assert!(is_on_screen(pos(100, 100), size, &[AREA]));
        // Window hanging off the left edge, centre still on screen.
        assert!(is_on_screen(pos(-150, 100), size, &[AREA]));
        // A monitor that was unplugged.
        assert!(!is_on_screen(pos(-1500, 100), size, &[AREA]));
        assert!(!is_on_screen(pos(100, 100), size, &[]));
    }

    #[test]
    fn window_positions_are_saved_as_x_and_y() {
        assert_eq!(
            serde_json::to_string(&pos(-1500, 20)).unwrap(),
            r#"{"x":-1500,"y":20}"#
        );
        let back: WindowPos = serde_json::from_str(r#"{"x":10,"y":20}"#).unwrap();
        assert_eq!(back, pos(10, 20));
    }

    #[test]
    fn page_rects_arrive_as_x_y_w_h() {
        let r: Rect<Css> = serde_json::from_str(r#"{"x":1,"y":2.5,"w":3,"h":4}"#).unwrap();
        assert_eq!(r, Rect::new(1.0, 2.5, 3.0, 4.0));
    }

    #[test]
    fn zone_converts_logical_rects_to_screen_px() {
        let r: Rect<Logical> = Rect::new(150.0, 80.0, 170.0, 220.0);
        let zone = r.to_physical(Point::new(1000.0, 500.0), 1.25);
        assert_eq!(zone, Rect::new(1188, 600, 213, 275));
        assert!(zone.contains(pos(1188, 600)));
        assert!(!zone.contains(pos(1401, 600)));
    }

    #[test]
    fn rescaling_keeps_the_bottom_right_corner() {
        // 360×330 at (1730, 912) grown to 150% (540×495): the corner stays at (2090, 1242).
        assert_eq!(
            keep_bottom_right(pos(1730, 912), (360, 330), (540, 495)),
            pos(1550, 747)
        );
        // And back again.
        assert_eq!(
            keep_bottom_right(pos(1550, 747), (540, 495), (360, 330)),
            pos(1730, 912)
        );
    }

    #[test]
    fn bottom_right_at_100_percent() {
        // 2560×1440 with a 48px taskbar: work area 2560×1392.
        let p = bottom_right(Rect::new(0, 0, 2560, 1392), (360, 280), 16.0, 1.0);
        assert_eq!(p, pos(2184, 1096));
    }

    #[test]
    fn bottom_right_scales_size_and_margin() {
        // 150%: the window is 540×420 physical and the margin 24px.
        let p = bottom_right(Rect::new(0, 0, 2560, 1368), (540, 420), 16.0, 1.5);
        assert_eq!(p, pos(1996, 924));
        // 125%: 450×350, margin 20px.
        let p = bottom_right(Rect::new(0, 0, 2560, 1380), (450, 350), 16.0, 1.25);
        assert_eq!(p, pos(2090, 1010));
    }

    #[test]
    fn bottom_right_respects_a_work_area_offset() {
        // Taskbar on the left or top shifts the work area origin.
        let p = bottom_right(Rect::new(60, 40, 2500, 1400), (360, 280), 16.0, 1.0);
        assert_eq!(p, pos(60 + 2500 - 360 - 16, 40 + 1400 - 280 - 16));
    }

    #[test]
    fn a_grab_that_does_not_move_keeps_the_window_still() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            for (window, cursor) in [((100, 200), (250, 330)), ((-1910, 7), (-1700, 140))] {
                let (window, cursor) = (pos(window.0, window.1), pos(cursor.0, cursor.1));
                let grip = cursor.to_f64().to_logical(window.to_f64(), scale);
                assert_eq!(grip.window_at(cursor, scale), window, "{scale}");
            }
        }
    }

    /// The formulas as they were before the coordinate types (tuples of f64 and i32), to prove the typed
    /// conversions compute exactly the same numbers.
    mod before {
        pub fn scaled((x, y, w, h): (f64, f64, f64, f64), zoom: f64) -> (f64, f64, f64, f64) {
            (x * zoom, y * zoom, w * zoom, h * zoom)
        }
        pub fn to_overlay(
            cursor: (f64, f64),
            origin: (f64, f64),
            scale: f64,
            monitor: (f64, f64, f64, f64),
        ) -> Option<(f64, f64)> {
            let (mx, my, mw, mh) = monitor;
            let on_monitor =
                cursor.0 >= mx && cursor.0 < mx + mw && cursor.1 >= my && cursor.1 < my + mh;
            on_monitor.then(|| ((cursor.0 - origin.0) / scale, (cursor.1 - origin.1) / scale))
        }
        pub fn to_px(
            origin: (f64, f64),
            scale: f64,
            (x, y, w, h): (f64, f64, f64, f64),
        ) -> (i32, i32, i32, i32) {
            (
                (origin.0 + x * scale).round() as i32,
                (origin.1 + y * scale).round() as i32,
                (w * scale).round() as i32,
                (h * scale).round() as i32,
            )
        }
        pub fn in_rect((x, y): (i32, i32), (rx, ry, rw, rh): (i32, i32, i32, i32)) -> bool {
            x >= rx && x < rx + rw && y >= ry && y < ry + rh
        }
        pub fn grip(cursor: (i32, i32), origin: (i32, i32), scale: f64) -> (f64, f64) {
            (
                (cursor.0 - origin.0) as f64 / scale,
                (cursor.1 - origin.1) as f64 / scale,
            )
        }
        pub fn window_at(cursor: (i32, i32), (gx, gy): (f64, f64), scale: f64) -> (i32, i32) {
            (
                cursor.0 - (gx * scale).round() as i32,
                cursor.1 - (gy * scale).round() as i32,
            )
        }
    }

    #[test]
    fn typed_conversions_compute_what_the_old_formulas_did() {
        let zooms = [0.5, 1.0, 2.0];
        let scales = [1.0, 1.25, 1.5, 2.0];
        // Window origins on the primary monitor, a monitor to the left (negative x) and one above.
        let origins = [(0, 0), (2184, 1096), (-1883, 13), (311, -1207)];
        let monitors = [
            (0.0, 0.0, 2560.0, 1440.0),
            (-1920.0, 0.0, 1920.0, 1080.0),
            (0.0, -1440.0, 3840.0, 1440.0),
        ];
        let cursors: [(f64, f64); 5] = [
            (0.0, 0.0),
            (2384.0, 1196.0),
            (-500.5, 300.25),
            (1200.0, -600.75),
            (2559.0, 1439.0),
        ];
        let page_rects = [
            (150.0, 60.0, 170.0, 190.0),
            (8.0, 14.0, 200.0, 60.0),
            (-3.5, 7.25, 10.1, 0.3),
        ];
        let mut checked = 0;
        for &zoom in &zooms {
            for &scale in &scales {
                for &(ox, oy) in &origins {
                    let origin = Point::<Physical, i32>::new(ox, oy);
                    let origin_f = (f64::from(ox), f64::from(oy));
                    for &css in &page_rects {
                        let typed = Rect::<Css>::new(css.0, css.1, css.2, css.3).to_logical(zoom);
                        let old = before::scaled(css, zoom);
                        assert_eq!((typed.x, typed.y, typed.w, typed.h), old);

                        let zone = typed.to_physical(origin.to_f64(), scale);
                        let old_zone = before::to_px(origin_f, scale, old);
                        assert_eq!((zone.x, zone.y, zone.w, zone.h), old_zone);
                        for &(cx, cy) in &cursors {
                            let c = (cx.round() as i32, cy.round() as i32);
                            assert_eq!(
                                zone.contains(Point::new(c.0, c.1)),
                                before::in_rect(c, old_zone)
                            );
                        }
                    }
                    for &(cx, cy) in &cursors {
                        for &m in &monitors {
                            let frame = OverlayFrame {
                                origin: origin.to_f64(),
                                scale,
                                monitor: Some(Rect::new(m.0, m.1, m.2, m.3)),
                            };
                            let typed = to_overlay(Point::new(cx, cy), &frame);
                            let old = before::to_overlay((cx, cy), origin_f, scale, m);
                            assert_eq!(typed.map(|p| (p.x, p.y)), old);
                            if let Some(p) = typed {
                                let css = p.to_css(zoom);
                                assert_eq!(
                                    (css.x, css.y),
                                    (old.unwrap().0 / zoom, old.unwrap().1 / zoom)
                                );
                            }
                        }
                        let cursor =
                            Point::<Physical, i32>::new(cx.round() as i32, cy.round() as i32);
                        let grip = cursor.to_f64().to_logical(origin.to_f64(), scale);
                        let old_grip = before::grip((cursor.x, cursor.y), (ox, oy), scale);
                        assert_eq!((grip.x, grip.y), old_grip);
                        let moved = Point::new(cursor.x + 37, cursor.y - 11);
                        let w = grip.window_at(moved, scale);
                        assert_eq!(
                            (w.x, w.y),
                            before::window_at((moved.x, moved.y), old_grip, scale)
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(
            checked,
            zooms.len() * scales.len() * origins.len() * cursors.len()
        );
    }
}

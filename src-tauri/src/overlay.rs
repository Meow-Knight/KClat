//! Transparent, always-on-top overlay window: at the spot you dragged the cat to, or pinned to the bottom-right
//! of the primary work area.
//!
//! Cat size (`catScale`): the window is WIDTH×HEIGHT times the scale and its webview is zoomed by the same
//! factor, so the page always lays out at WIDTH×HEIGHT CSS px and everything in it grows together. Hitboxes
//! and cursor points are converted between the two (with `zoom()`) in `AppState::hitboxes` and the cursor poller.
//!
//! Dragging the size slider (ClauCat-jds): resizing, moving and zooming the window are separate asynchronous
//! steps, and doing them on every step of a drag made the cat flicker and jump. While the slider is dragged the
//! window is set once to the largest size (`PREVIEW_ZOOM`) and the page scales the cat with a CSS transform
//! (`cat://scale-preview`); on release it is set once to the chosen size. The page hides the cat around both
//! switches (`preview_zoom`).

use crate::drag;
use crate::state::{self, AppState};
use klat_core::geometry::{self, Physical, Point, Rect, WindowPos};
use klat_core::settings::CAT_SCALE;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, PhysicalPosition,
    WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 360.0;
// 330 = the cat's 280px framing plus 50px of headroom above it for the aura spikes.
const HEIGHT: f64 = 330.0;
const MARGIN: f64 = 16.0;

/// Size slider being dragged: the scale to show, as a factor of the page's size at `PREVIEW_ZOOM`; null when
/// the drag ends.
pub const EVENT_SCALE_PREVIEW: &str = "cat://scale-preview";
/// The window's zoom while the size slider is dragged: the largest cat size, so every preview is a shrink.
const PREVIEW_ZOOM: f64 = CAT_SCALE.1 as f64 / 100.0;

/// The webview zoom in effect (f64 bits): the cat scale, or `PREVIEW_ZOOM` during a preview.
static ZOOM: AtomicU64 = AtomicU64::new(0x3FF0_0000_0000_0000); // 1.0
/// The size slider is being dragged (set by `preview`, cleared by `end_preview`).
static PREVIEWING: AtomicBool = AtomicBool::new(false);

/// The webview zoom in effect, for converting between the page's CSS px and the window's logical px.
pub fn zoom() -> f64 {
    f64::from_bits(ZOOM.load(Ordering::Relaxed))
}

pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let zoom = app.state::<AppState>().settings().zoom();
    let size = logical_size(zoom);
    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::default())
        .title("Klat")
        .inner_size(size.width, size.height)
        .resizable(false)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        // Not focused on creation, and WS_EX_NOACTIVATE so clicking the cat never takes focus from the terminal.
        .focused(false)
        .focusable(false)
        .additional_browser_args(crate::webview_memory::BROWSER_ARGS)
        .visible(true);
    // macOS: stay on every desktop (Space), as the overlay does on Windows virtual desktops.
    #[cfg(target_os = "macos")]
    {
        builder = builder.visible_on_all_workspaces(true);
    }

    // Approximate logical position for creation; place() then corrects it in physical px at the real DPI.
    if let Some(pos) = initial_position(app, size) {
        builder = builder.position(pos.x, pos.y);
    }
    let win = builder.build()?;
    win.set_zoom(zoom)?;
    ZOOM.store(zoom.to_bits(), Ordering::Relaxed);
    place(&win, app.state::<AppState>().settings().position)?;
    // Click-through by default; the cursor poller turns it off over the cat and the bubble.
    win.set_ignore_cursor_events(true)?;
    Ok(win)
}

pub fn get(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// A position Tauri reports, as klat-core's type.
pub fn screen_pos(p: PhysicalPosition<i32>) -> WindowPos {
    Point::new(p.x, p.y)
}

/// The part of a monitor windows may use (without the taskbar or Dock).
pub fn work_area(m: &Monitor) -> Rect<Physical, i32> {
    let a = m.work_area();
    Rect::new(
        a.position.x,
        a.position.y,
        a.size.width as i32,
        a.size.height as i32,
    )
}

/// The window's size for a cat scale (`Settings::zoom`).
fn logical_size(zoom: f64) -> LogicalSize<f64> {
    LogicalSize::new(WIDTH * zoom, HEIGHT * zoom)
}

/// The cat scale changed: resize the window and zoom its page to match. The bottom-right corner stays where it
/// is, so the cat (on the right, standing on the bottom) stays put and the window grows up and to the left.
/// Returns the new top-left when the cat had been dragged somewhere, for the caller to save.
pub fn rescale(app: &AppHandle, zoom: f64) -> Option<WindowPos> {
    let pos = resize(app, zoom)?;
    let dragged = app.state::<AppState>().settings().position.is_some();
    dragged.then_some(pos)
}

/// Sets the window's size and zoom for `zoom`, keeping its bottom-right corner. Returns the new top-left.
fn resize(app: &AppHandle, zoom: f64) -> Option<WindowPos> {
    let win = get(app)?;
    let (pos, old, scale) = (
        win.outer_position().ok()?,
        win.outer_size().ok()?,
        win.scale_factor().ok()?,
    );
    let size = logical_size(zoom);
    let new = size.to_physical::<u32>(scale);
    let _ = win.set_zoom(zoom);
    ZOOM.store(zoom.to_bits(), Ordering::Relaxed);
    let _ = win.set_size(size);
    let pos = geometry::keep_bottom_right(
        screen_pos(pos),
        (old.width, old.height),
        (new.width, new.height),
    );
    let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
    Some(pos)
}

/// The size slider moved to `scale` percent: show it in the overlay without touching the window.
pub fn preview(app: &AppHandle, scale: u32) {
    PREVIEWING.store(true, Ordering::SeqCst);
    let factor = f64::from(scale.clamp(CAT_SCALE.0, CAT_SCALE.1)) / 100.0 / PREVIEW_ZOOM;
    let _ = app.emit_to(LABEL, EVENT_SCALE_PREVIEW, Some(factor));
}

/// The drag is over (the value was saved, or the Settings window went away): the overlay hides the cat and asks
/// for its real size through `preview_zoom(false)`. Returns false when no preview was running.
pub fn end_preview(app: &AppHandle) -> bool {
    if !PREVIEWING.swap(false, Ordering::SeqCst) {
        return false;
    }
    let _ = app.emit_to(LABEL, EVENT_SCALE_PREVIEW, None::<f64>);
    true
}

/// What `preview_zoom` did, for the page to know when the switch has landed.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ZoomChange {
    pub from: f64,
    pub to: f64,
    /// The page's size in CSS px, the same at every zoom.
    pub width: f64,
    pub height: f64,
}

/// Called by the overlay page with the cat hidden: `true` sets the window to `PREVIEW_ZOOM` for a preview,
/// `false` back to the cat scale (saving the spot of a dragged cat). The second returns the new settings to
/// save, if any.
pub fn preview_zoom(app: &AppHandle, preview: bool) -> (ZoomChange, Option<WindowPos>) {
    let from = zoom();
    let (to, saved) = if preview {
        resize(app, PREVIEW_ZOOM);
        (PREVIEW_ZOOM, None)
    } else {
        let to = app.state::<AppState>().settings().zoom();
        (to, rescale(app, to))
    };
    let change = ZoomChange {
        from,
        to,
        width: WIDTH,
        height: HEIGHT,
    };
    (change, saved)
}

/// Put the window at the saved spot if it is still on a monitor, otherwise MARGIN logical px inside the
/// bottom-right of the primary monitor's work area.
pub fn place(win: &WebviewWindow, saved: Option<WindowPos>) -> tauri::Result<()> {
    if let Some(pos) = saved {
        let size = win.outer_size()?;
        let areas: Vec<_> = win.available_monitors()?.iter().map(work_area).collect();
        if geometry::is_on_screen(pos, (size.width, size.height), &areas) {
            return win.set_position(PhysicalPosition::new(pos.x, pos.y));
        }
    }
    let Some(monitor) = win.primary_monitor()? else {
        return Ok(());
    };
    let size = win.outer_size()?;
    let pos = geometry::bottom_right(
        work_area(&monitor),
        (size.width, size.height),
        MARGIN,
        monitor.scale_factor(),
    );
    win.set_position(PhysicalPosition::new(pos.x, pos.y))
}

/// When the DPI changes Windows resizes the window, which knocks it off its spot. Re-place it once the
/// resize has landed (the event fires before the new size is applied). Not while you are dragging it across
/// monitors: the drag keeps it under the cursor.
///
/// A dragged cat keeps the window's bottom-right corner, where it stands: keeping the saved top-left would grow
/// the window (150% DPI: half as big again) right and down, off the screen and under the taskbar.
pub fn on_scale_changed(win: &WebviewWindow) {
    if drag::dragging() {
        return;
    }
    // Still the old geometry here.
    let before = match (win.outer_position(), win.outer_size()) {
        (Ok(p), Ok(s)) => Some((screen_pos(p), (s.width, s.height))),
        _ => None,
    };
    let win = win.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let app = win.app_handle();
        if app.state::<AppState>().settings().position.is_none() {
            let _ = place(&win, None);
            return;
        }
        if let Some(pos) = reanchor(&win, before) {
            log::info!("DPI changed: cat moved to {},{}", pos.x, pos.y);
            state::update_settings(app, |s| s.position = Some(pos));
        }
    });
}

/// Puts a dragged window back with the bottom-right corner it had `before` (or where it is, without that) at its
/// new size, with the cat inside the work area. Returns the new top-left.
fn reanchor(win: &WebviewWindow, before: Option<(WindowPos, (u32, u32))>) -> Option<WindowPos> {
    let size = win.outer_size().ok()?;
    let scale = win.scale_factor().ok()?;
    let pos = match before {
        Some((pos, old)) => geometry::keep_bottom_right(pos, old, (size.width, size.height)),
        None => screen_pos(win.outer_position().ok()?),
    };
    let cat = win
        .app_handle()
        .state::<AppState>()
        .hitboxes()
        .first()
        .map(|r| r.to_physical(Point::new(0.0, 0.0), scale));
    let area = win
        .app_handle()
        .monitor_from_point(
            f64::from(pos.x) + f64::from(size.width) / 2.0,
            f64::from(pos.y) + f64::from(size.height) / 2.0,
        )
        .ok()
        .flatten()
        .or_else(|| win.primary_monitor().ok().flatten())
        .map(|m| work_area(&m));
    let pos = match (cat, area) {
        (Some(cat), Some(area)) => geometry::clamp_window(pos, cat, area),
        _ => pos,
    };
    win.set_position(PhysicalPosition::new(pos.x, pos.y)).ok()?;
    Some(pos)
}

fn initial_position(app: &AppHandle, window: LogicalSize<f64>) -> Option<LogicalPosition<f64>> {
    let monitor = app.primary_monitor().ok()??;
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let origin = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);
    Some(LogicalPosition::new(
        origin.x + size.width - window.width - MARGIN,
        origin.y + size.height - window.height - MARGIN,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_grows_with_the_cat_scale() {
        assert_eq!(logical_size(1.0), LogicalSize::new(360.0, 330.0));
        assert_eq!(logical_size(1.5), LogicalSize::new(540.0, 495.0));
    }
}

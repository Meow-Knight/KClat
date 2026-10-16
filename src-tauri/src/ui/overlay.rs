//! Transparent, always-on-top overlay window: at the spot you dragged the cat to, or pinned to the bottom-right
//! of the primary work area.
//!
//! Cat size (`catScale`): the window is WIDTH×HEIGHT times the scale and its webview is zoomed by the same
//! factor, so the page always lays out at WIDTH×HEIGHT CSS px and everything in it grows together. Hitboxes
//! and cursor points are converted between the two (with `zoom()`) in `OverlayState::hitboxes` and the cursor poller.
//!
//! Dragging the size slider (ClauCat-jds): resizing, moving and zooming the window are separate asynchronous
//! steps, and doing them on every step of a drag made the cat flicker and jump. While the slider is dragged the
//! window is set once to the largest size (`PREVIEW_ZOOM`) and the page scales the cat with a CSS transform
//! (`cat://scale-preview`); on release it is set once to the chosen size. The page hides the cat around both
//! switches (`preview_zoom`).

use crate::ipc::events::EVENT_SCALE_PREVIEW;
use crate::platform;
use crate::services::cat_service::CatService;
use klat_core::geometry::{self, Css, Logical, Physical, Point, Rect, WindowPos};
use klat_core::settings::CAT_SCALE;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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

/// The window's zoom while the size slider is dragged: the largest cat size, so every preview is a shrink.
const PREVIEW_ZOOM: f64 = CAT_SCALE.1 as f64 / 100.0;

/// The overlay window's own state, shared by the commands, the cursor poller and the drag loop (managed by Tauri
/// and handed to the tasks at setup).
pub struct OverlayState {
    /// Clickable regions of the overlay as the page reports them; see `hitboxes()`.
    page_hitboxes: Mutex<Vec<Rect<Css>>>,
    /// The webview zoom in effect (f64 bits): the cat scale, or `PREVIEW_ZOOM` during a preview.
    zoom: AtomicU64,
    /// The size slider is being dragged (set by `preview`, cleared by `end_preview`).
    previewing: AtomicBool,
    /// The window is following the cursor (see tasks::drag_loop).
    dragging: AtomicBool,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            page_hitboxes: Mutex::default(),
            zoom: AtomicU64::new(1.0_f64.to_bits()),
            previewing: AtomicBool::new(false),
            dragging: AtomicBool::new(false),
        }
    }
}

impl OverlayState {
    pub fn set_page_hitboxes(&self, rects: Vec<Rect<Css>>) {
        *self.page_hitboxes.lock().unwrap() = rects;
    }

    /// Clickable regions relative to the overlay's top-left corner.
    pub fn hitboxes(&self) -> Vec<Rect<Logical>> {
        let zoom = self.zoom();
        let rects = self.page_hitboxes.lock().unwrap();
        rects.iter().map(|r| r.to_logical(zoom)).collect()
    }

    /// The webview zoom in effect, for converting between the page's CSS px and the window's logical px.
    pub fn zoom(&self) -> f64 {
        f64::from_bits(self.zoom.load(Ordering::Relaxed))
    }

    fn set_zoom(&self, zoom: f64) {
        self.zoom.store(zoom.to_bits(), Ordering::Relaxed);
    }

    pub fn dragging(&self) -> bool {
        self.dragging.load(Ordering::Relaxed)
    }

    /// Returns whether it was being dragged before.
    pub fn set_dragging(&self, on: bool) -> bool {
        self.dragging.swap(on, Ordering::Relaxed)
    }
}

pub fn create(
    app: &AppHandle,
    cat: &CatService,
    overlay: &OverlayState,
) -> tauri::Result<WebviewWindow> {
    let settings = cat.settings();
    let zoom = settings.zoom();
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
        .additional_browser_args(platform::webview_memory::BROWSER_ARGS)
        .visible(true);
    builder = platform::on_every_desktop(builder);

    // Approximate logical position for creation; place() then corrects it in physical px at the real DPI.
    if let Some(pos) = initial_position(app, size) {
        builder = builder.position(pos.x, pos.y);
    }
    let win = builder.build()?;
    win.set_zoom(zoom)?;
    overlay.set_zoom(zoom);
    place(&win, settings.position)?;
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
pub fn rescale(
    app: &AppHandle,
    cat: &CatService,
    overlay: &OverlayState,
    zoom: f64,
) -> Option<WindowPos> {
    let pos = resize(app, overlay, zoom)?;
    let dragged = cat.settings().position.is_some();
    dragged.then_some(pos)
}

/// Sets the window's size and zoom for `zoom`, keeping its bottom-right corner. Returns the new top-left.
fn resize(app: &AppHandle, overlay: &OverlayState, zoom: f64) -> Option<WindowPos> {
    let win = get(app)?;
    let (pos, old, scale) = (
        win.outer_position().ok()?,
        win.outer_size().ok()?,
        win.scale_factor().ok()?,
    );
    let size = logical_size(zoom);
    let new = size.to_physical::<u32>(scale);
    let _ = win.set_zoom(zoom);
    overlay.set_zoom(zoom);
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
pub fn preview(app: &AppHandle, overlay: &OverlayState, scale: u32) {
    overlay.previewing.store(true, Ordering::SeqCst);
    let factor = f64::from(scale.clamp(CAT_SCALE.0, CAT_SCALE.1)) / 100.0 / PREVIEW_ZOOM;
    let _ = app.emit_to(LABEL, EVENT_SCALE_PREVIEW, Some(factor));
}

/// The drag is over (the value was saved, or the Settings window went away): the overlay hides the cat and asks
/// for its real size through `preview_zoom(false)`. Returns false when no preview was running.
pub fn end_preview(app: &AppHandle, overlay: &OverlayState) -> bool {
    if !overlay.previewing.swap(false, Ordering::SeqCst) {
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
pub fn preview_zoom(
    app: &AppHandle,
    cat: &CatService,
    overlay: &OverlayState,
    preview: bool,
) -> (ZoomChange, Option<WindowPos>) {
    let from = overlay.zoom();
    let (to, saved) = if preview {
        resize(app, overlay, PREVIEW_ZOOM);
        (PREVIEW_ZOOM, None)
    } else {
        let to = cat.settings().zoom();
        (to, rescale(app, cat, overlay, to))
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
pub fn on_scale_changed(win: &WebviewWindow, cat: Arc<CatService>, overlay: Arc<OverlayState>) {
    if overlay.dragging() {
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
        if cat.settings().position.is_none() {
            let _ = place(&win, None);
            return;
        }
        if let Some(pos) = reanchor(&win, &overlay, before) {
            log::info!("DPI changed: cat moved to {},{}", pos.x, pos.y);
            cat.update_settings(|s| s.position = Some(pos));
        }
    });
}

/// Puts a dragged window back with the bottom-right corner it had `before` (or where it is, without that) at its
/// new size, with the cat inside the work area. Returns the new top-left.
fn reanchor(
    win: &WebviewWindow,
    overlay: &OverlayState,
    before: Option<(WindowPos, (u32, u32))>,
) -> Option<WindowPos> {
    let size = win.outer_size().ok()?;
    let scale = win.scale_factor().ok()?;
    let pos = match before {
        Some((pos, old)) => geometry::keep_bottom_right(pos, old, (size.width, size.height)),
        None => screen_pos(win.outer_position().ok()?),
    };
    let cat = overlay
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

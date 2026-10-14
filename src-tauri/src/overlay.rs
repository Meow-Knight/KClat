//! Transparent, always-on-top overlay window: at the spot you dragged the cat to, or pinned to the bottom-right
//! of the primary work area.
//!
//! Cat size (`catScale`): the window is WIDTH×HEIGHT times the scale and its webview is zoomed by the same
//! factor, so the page always lays out at WIDTH×HEIGHT CSS px and everything in it grows together. Hitboxes
//! and cursor points are converted between the two in `AppState::hitboxes` and the cursor poller.

use crate::drag::{self, WindowPos};
use crate::state::AppState;
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 360.0;
// 330 = the cat's 280px framing plus 50px of headroom above it for the aura spikes.
const HEIGHT: f64 = 330.0;
const MARGIN: f64 = 16.0;

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
    place(&win, app.state::<AppState>().settings().position)?;
    // Click-through by default; the cursor poller turns it off over the cat and the bubble.
    win.set_ignore_cursor_events(true)?;
    Ok(win)
}

pub fn get(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// The window's size for a cat scale (`Settings::zoom`).
fn logical_size(zoom: f64) -> LogicalSize<f64> {
    LogicalSize::new(WIDTH * zoom, HEIGHT * zoom)
}

/// The cat scale changed: resize the window and zoom its page to match. The bottom-right corner stays where it
/// is, so the cat (on the right, standing on the bottom) stays put and the window grows up and to the left.
/// Returns the new top-left when the cat had been dragged somewhere, for the caller to save.
pub fn rescale(app: &AppHandle, zoom: f64) -> Option<WindowPos> {
    let win = get(app)?;
    let (pos, old, scale) = (
        win.outer_position().ok()?,
        win.outer_size().ok()?,
        win.scale_factor().ok()?,
    );
    let size = logical_size(zoom);
    let new = size.to_physical::<u32>(scale);
    let _ = win.set_zoom(zoom);
    let _ = win.set_size(size);
    let (x, y) = keep_bottom_right(
        (pos.x, pos.y),
        (old.width, old.height),
        (new.width, new.height),
    );
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let dragged = app.state::<AppState>().settings().position.is_some();
    dragged.then_some(WindowPos { x, y })
}

/// Top-left that keeps a window's bottom-right corner in place when it goes from `old` to `new` size.
pub fn keep_bottom_right(pos: (i32, i32), old: (u32, u32), new: (u32, u32)) -> (i32, i32) {
    (
        pos.0 + old.0 as i32 - new.0 as i32,
        pos.1 + old.1 as i32 - new.1 as i32,
    )
}

/// Put the window at the saved spot if it is still on a monitor, otherwise MARGIN logical px inside the
/// bottom-right of the primary monitor's work area.
pub fn place(win: &WebviewWindow, saved: Option<WindowPos>) -> tauri::Result<()> {
    if let Some(pos) = saved {
        let size = win.outer_size()?;
        let areas: Vec<drag::PxRect> = win
            .available_monitors()?
            .iter()
            .map(|m| {
                let a = m.work_area();
                (
                    a.position.x,
                    a.position.y,
                    a.size.width as i32,
                    a.size.height as i32,
                )
            })
            .collect();
        if drag::is_on_screen(pos, (size.width, size.height), &areas) {
            return win.set_position(PhysicalPosition::new(pos.x, pos.y));
        }
    }
    let Some(monitor) = win.primary_monitor()? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let pos = bottom_right(
        (area.position.x, area.position.y),
        area.size,
        win.outer_size()?,
        monitor.scale_factor(),
    );
    win.set_position(pos)
}

/// When the DPI changes Windows resizes the window, which knocks it off its spot. Re-place it once the
/// resize has landed (the event fires before the new size is applied). Not while you are dragging it across
/// monitors: the drag keeps it under the cursor.
pub fn on_scale_changed(win: &WebviewWindow) {
    if drag::dragging() {
        return;
    }
    let win = win.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let saved = win.app_handle().state::<AppState>().settings().position;
        let _ = place(&win, saved);
    });
}

/// Physical top-left for a window of `size` inside the bottom-right corner of a work area.
pub fn bottom_right(
    area_origin: (i32, i32),
    area: PhysicalSize<u32>,
    size: PhysicalSize<u32>,
    scale: f64,
) -> PhysicalPosition<i32> {
    let margin = (MARGIN * scale).round() as i32;
    PhysicalPosition::new(
        area_origin.0 + area.width as i32 - size.width as i32 - margin,
        area_origin.1 + area.height as i32 - size.height as i32 - margin,
    )
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
    fn rescaling_keeps_the_bottom_right_corner() {
        // 360×330 at (1730, 912) grown to 150% (540×495): the corner stays at (2090, 1242).
        assert_eq!(
            keep_bottom_right((1730, 912), (360, 330), (540, 495)),
            (1550, 747)
        );
        // And back again.
        assert_eq!(
            keep_bottom_right((1550, 747), (540, 495), (360, 330)),
            (1730, 912)
        );
        assert_eq!(logical_size(1.5), LogicalSize::new(540.0, 495.0));
    }

    #[test]
    fn bottom_right_at_100_percent() {
        // 2560×1440 with a 48px taskbar: work area 2560×1392.
        let pos = bottom_right(
            (0, 0),
            PhysicalSize::new(2560, 1392),
            PhysicalSize::new(360, 280),
            1.0,
        );
        assert_eq!(pos, PhysicalPosition::new(2184, 1096));
    }

    #[test]
    fn bottom_right_scales_size_and_margin() {
        // 150%: the window is 540×420 physical and the margin 24px.
        let pos = bottom_right(
            (0, 0),
            PhysicalSize::new(2560, 1368),
            PhysicalSize::new(540, 420),
            1.5,
        );
        assert_eq!(pos, PhysicalPosition::new(1996, 924));
        // 125%: 450×350, margin 20px.
        let pos = bottom_right(
            (0, 0),
            PhysicalSize::new(2560, 1380),
            PhysicalSize::new(450, 350),
            1.25,
        );
        assert_eq!(pos, PhysicalPosition::new(2090, 1010));
    }

    #[test]
    fn bottom_right_respects_a_work_area_offset() {
        // Taskbar on the left or top shifts the work area origin.
        let pos = bottom_right(
            (60, 40),
            PhysicalSize::new(2500, 1400),
            PhysicalSize::new(360, 280),
            1.0,
        );
        assert_eq!(
            pos,
            PhysicalPosition::new(60 + 2500 - 360 - 16, 40 + 1400 - 280 - 16)
        );
    }
}

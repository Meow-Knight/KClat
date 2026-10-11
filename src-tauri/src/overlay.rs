//! Transparent, always-on-top overlay window pinned to the bottom-right of the primary work area.

use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub const LABEL: &str = "overlay";
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 280.0;
const MARGIN: f64 = 16.0;

pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::default())
        .title("Claude Cat")
        .inner_size(WIDTH, HEIGHT)
        .resizable(false)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        // Not focused on creation, and WS_EX_NOACTIVATE so clicking the cat never takes focus from the terminal.
        .focused(false)
        .focusable(false)
        .visible(true);

    // Approximate logical position for creation; place() then corrects it in physical px at the real DPI.
    if let Some(pos) = initial_position(app) {
        builder = builder.position(pos.x, pos.y);
    }
    let win = builder.build()?;
    place(&win)?;
    // Click-through by default; the cursor poller turns it off over the cat and the bubble.
    win.set_ignore_cursor_events(true)?;
    Ok(win)
}

pub fn get(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Pin the window MARGIN logical px inside the bottom-right of the primary monitor's work area.
pub fn place(win: &WebviewWindow) -> tauri::Result<()> {
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

/// When the DPI changes Windows resizes the window, which knocks it off the corner. Re-pin it once the
/// resize has landed (the event fires before the new size is applied).
pub fn on_scale_changed(win: &WebviewWindow) {
    let win = win.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        let _ = place(&win);
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

fn initial_position(app: &AppHandle) -> Option<LogicalPosition<f64>> {
    let monitor = app.primary_monitor().ok()??;
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let origin = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);
    Some(LogicalPosition::new(
        origin.x + size.width - WIDTH - MARGIN,
        origin.y + size.height - HEIGHT - MARGIN,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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

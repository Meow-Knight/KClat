//! The overlay window: its clickable regions and the size slider preview.

use crate::services::cat_service::CatService;
use crate::ui::overlay::{self, OverlayState};
use klat_core::geometry::{Css, Rect};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Upper bound on reported hitboxes; the overlay sends at most cat + bubble.
const MAX_HITBOXES: usize = 8;

/// The size slider is being dragged: show `scale` (percent) in the overlay; nothing is saved until
/// `set_settings`.
#[tauri::command]
pub fn preview_cat_scale(app: AppHandle, overlay: State<'_, Arc<OverlayState>>, scale: u32) {
    overlay::preview(&app, &overlay, scale);
}

/// The overlay page, with the cat hidden, switches its window to the preview size (`preview`) or back to the
/// cat scale. See `overlay::preview_zoom`.
#[tauri::command]
pub fn overlay_preview_zoom(
    app: AppHandle,
    cat: State<'_, Arc<CatService>>,
    overlay_state: State<'_, Arc<OverlayState>>,
    preview: bool,
) -> overlay::ZoomChange {
    let (change, saved) = overlay::preview_zoom(&app, &cat, &overlay_state, preview);
    if let Some(pos) = saved {
        cat.update_settings(|s| s.position = Some(pos));
    }
    change
}

#[tauri::command]
pub fn set_hitboxes(overlay: State<'_, Arc<OverlayState>>, rects: Vec<Rect<Css>>) {
    let rects = rects
        .into_iter()
        .filter(|r| [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) && r.w > 0.0 && r.h > 0.0)
        .take(MAX_HITBOXES)
        .collect();
    overlay.set_page_hitboxes(rects);
}

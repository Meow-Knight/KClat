//! Names of the events the app emits to the webviews.

/// The cat's state (payload: `CatState`).
pub const EVENT_STATE: &str = "cat://state";
/// The settings in effect (payload: `Settings`).
pub const EVENT_SETTINGS: &str = "cat://settings";
/// Where the cursor is, for head tracking (see tasks::cursor_poller).
pub const EVENT_CURSOR: &str = "cat://cursor";
/// Dragging the cat with Space: "idle" | "armed" | "dragging" (see tasks::drag_loop).
pub const EVENT_GRAB: &str = "cat://grab";
/// macOS: the Accessibility permission the Space hook needs changed (payload: `platform::Permission`).
#[allow(dead_code)] // only macOS emits it
pub const EVENT_PERMISSION: &str = "cat://drag-permission";
/// Size slider being dragged: the scale to show, as a factor of the page's size at `PREVIEW_ZOOM`; null when
/// the drag ends.
pub const EVENT_SCALE_PREVIEW: &str = "cat://scale-preview";

//! The Platform Language in effect. The catalogs and lookups are klat_core::i18n.

// target: services (the settings service hands out the language)

use crate::services::cat_service::AppState;
use klat_core::settings::UiLanguage;
use tauri::{AppHandle, Manager};

/// The UI language in effect.
pub fn current(app: &AppHandle) -> UiLanguage {
    app.state::<AppState>().settings().ui_language
}

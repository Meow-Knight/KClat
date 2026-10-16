//! The Claude Code hooks: whether they are installed, installing and removing them.

use crate::services::cat_service::CatService;
use crate::services::hooks_service;
use klat_infra::claude_settings::{Action, Status};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct HooksInfo {
    pub status: Status,
    /// `~/.claude/settings.json`; None when the home folder is unknown.
    pub settings_path: Option<String>,
}

#[tauri::command]
pub fn hooks_status() -> HooksInfo {
    HooksInfo {
        status: hooks_service::current_status(),
        settings_path: hooks_service::paths().map(|(s, _)| s.display().to_string()),
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct HooksResult {
    pub ok: bool,
    /// Text to show the user, success or error.
    pub message: String,
}

/// `action`: "install" (also repairs) or "uninstall". File I/O runs off the main thread; the message is in
/// the Platform Language.
#[tauri::command]
pub async fn hooks_apply(app: AppHandle, action: Action) -> HooksResult {
    let lang = app.state::<Arc<CatService>>().settings().ui_language;
    let result = tauri::async_runtime::spawn_blocking(move || hooks_service::apply(lang, action))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match result {
        Ok(message) => HooksResult { ok: true, message },
        Err(message) => HooksResult { ok: false, message },
    }
}

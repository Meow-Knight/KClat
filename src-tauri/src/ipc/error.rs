//! What commands tell the webviews in words: a catalog key and its placeholders, never display text. The windows
//! translate it into the Platform Language (docs/ARCHITECTURE.md §7.3). Errors from klat-core and klat-infra become
//! an [`IpcError`] here, at the edge.

use klat_infra::hook_files::InstallError;
use serde::Serialize;
use std::collections::BTreeMap;

/// A text from the shared catalogs (src/i18n) and the values of its `{placeholders}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Message {
    pub key: String,
    pub vars: BTreeMap<String, String>,
}

impl Message {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            vars: BTreeMap::new(),
        }
    }

    /// Fills the placeholder `{name}`.
    pub fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.vars.insert(name.to_owned(), value.into());
        self
    }
}

/// What failed, for a window that wants to react to a kind of failure rather than show the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ErrorCode {
    /// No home folder, or the hook program is not next to the app.
    HooksNotFound,
    HooksBrokenJson,
    HooksUnexpectedShape,
    HooksExeMissing,
    HooksChangedConcurrently,
    HooksIo,
    /// The OS refused to add or remove the login item.
    StartAtLogin,
    /// A window could not be created or shown.
    Window,
}

/// A command's failure: what failed and what to tell the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: Message,
}

impl IpcError {
    pub fn new(code: ErrorCode, message: Message) -> Self {
        Self { code, message }
    }
}

impl From<InstallError> for IpcError {
    fn from(e: InstallError) -> Self {
        let (code, message) = match &e {
            InstallError::BrokenJson(detail) => (
                ErrorCode::HooksBrokenJson,
                Message::new("hooks.error.brokenJson").with("detail", detail.as_str()),
            ),
            InstallError::UnexpectedShape(what) => (
                ErrorCode::HooksUnexpectedShape,
                Message::new("hooks.error.unexpectedShape").with("what", *what),
            ),
            InstallError::ExeMissing(p) => (
                ErrorCode::HooksExeMissing,
                Message::new("hooks.error.exeMissing").with("path", p.display().to_string()),
            ),
            InstallError::ChangedConcurrently => (
                ErrorCode::HooksChangedConcurrently,
                Message::new("hooks.error.changedConcurrently"),
            ),
            InstallError::Io(err) => (
                ErrorCode::HooksIo,
                Message::new("hooks.error.io").with("detail", err.to_string()),
            ),
        };
        Self::new(code, message)
    }
}

impl From<tauri::Error> for IpcError {
    fn from(e: tauri::Error) -> Self {
        Self::new(
            ErrorCode::Window,
            Message::new("error.window").with("detail", e.to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klat_core::i18n::tf;
    use klat_core::settings::UiLanguage;

    #[test]
    fn errors_go_out_as_a_code_and_a_message() {
        let e = IpcError::new(
            ErrorCode::HooksIo,
            Message::new("hooks.error.io").with("detail", "disk full"),
        );
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({
                "code": "hooks-io",
                "message": { "key": "hooks.error.io", "vars": { "detail": "disk full" } }
            })
        );
    }

    #[test]
    fn the_window_error_has_a_text_in_both_languages() {
        let m = Message::new("error.window").with("detail", "boom");
        for lang in [UiLanguage::En, UiLanguage::Vi] {
            let vars: Vec<_> = m
                .vars
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let text = tf(lang, &m.key, &vars);
            assert!(
                text.contains("boom") && !text.starts_with("error."),
                "{text}"
            );
        }
    }
}

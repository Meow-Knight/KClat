//! Platform Language (docs/specs/settings-i18n-macos.md §3.1): the text of the tray, dialogs and hook messages.
//!
//! The catalogs are `src/i18n/{en,vi}.json`, shared with the frontend and compiled into the binary. A key
//! missing in Vietnamese falls back to English, and a key missing in both shows the key itself (the tests
//! make sure neither happens). `{name}` in a text is replaced by the matching variable.

use crate::state::AppState;
use klat_core::settings::UiLanguage;
use std::collections::HashMap;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

const EN: &str = include_str!("../../src/i18n/en.json");
const VI: &str = include_str!("../../src/i18n/vi.json");

type Catalog = HashMap<String, String>;

fn parse(json: &str) -> Catalog {
    serde_json::from_str(json).unwrap_or_default()
}

fn catalog(lang: UiLanguage) -> &'static Catalog {
    static EN_CAT: OnceLock<Catalog> = OnceLock::new();
    static VI_CAT: OnceLock<Catalog> = OnceLock::new();
    match lang {
        UiLanguage::En => EN_CAT.get_or_init(|| parse(EN)),
        UiLanguage::Vi => VI_CAT.get_or_init(|| parse(VI)),
    }
}

/// The UI language in effect.
pub fn current(app: &AppHandle) -> UiLanguage {
    app.state::<AppState>().settings().ui_language
}

pub fn t(lang: UiLanguage, key: &str) -> String {
    catalog(lang)
        .get(key)
        .or_else(|| catalog(UiLanguage::En).get(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// `t` with `{name}` placeholders filled in.
pub fn tf(lang: UiLanguage, key: &str, vars: &[(&str, &str)]) -> String {
    vars.iter().fold(t(lang, key), |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<String> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name.to_owned()))
            .collect()
    }

    #[test]
    fn both_catalogs_parse_and_have_the_same_keys() {
        let en = parse(EN);
        let vi = parse(VI);
        assert!(!en.is_empty());
        let en_keys: BTreeSet<_> = en.keys().collect();
        let vi_keys: BTreeSet<_> = vi.keys().collect();
        assert_eq!(en_keys, vi_keys);
    }

    #[test]
    fn no_text_is_empty_and_placeholders_match_across_languages() {
        let (en, vi) = (parse(EN), parse(VI));
        for (key, en_text) in &en {
            let vi_text = &vi[key];
            assert!(!en_text.trim().is_empty(), "{key} is empty in en");
            assert!(!vi_text.trim().is_empty(), "{key} is empty in vi");
            assert_eq!(
                placeholders(en_text),
                placeholders(vi_text),
                "placeholders differ for {key}"
            );
        }
    }

    #[test]
    fn looks_up_by_language_and_falls_back() {
        assert_eq!(t(UiLanguage::En, "tray.quit"), "Quit");
        assert_eq!(t(UiLanguage::Vi, "tray.quit"), "Thoát");
        assert_eq!(t(UiLanguage::Vi, "no.such.key"), "no.such.key");
    }

    #[test]
    fn fills_placeholders() {
        assert_eq!(
            tf(UiLanguage::En, "tray.tooltip.many", &[("count", "3")]),
            "Klat · 3 sessions waiting"
        );
        assert_eq!(
            tf(UiLanguage::Vi, "tray.tooltip.many", &[("count", "3")]),
            "Klat · 3 phiên đang chờ"
        );
    }
}

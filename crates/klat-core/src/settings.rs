//! The settings model (spec §10, docs/specs/settings-i18n-macos.md §2): fields, defaults, ranges, reading a
//! JSON object leniently, migrating old files, and applying a Settings window edit. Reading and writing
//! `settings.json` is the app's job.

use crate::geometry::WindowPos;
use crate::time::DurationMs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Platform Language: the tray, the Settings window and dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLanguage {
    #[default]
    En,
    Vi,
}

/// Klat Language: what the cat says. Auto mixes both languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KlatLanguage {
    En,
    Vi,
    #[default]
    Auto,
}

/// Who sits in the corner (src/characters in the frontend). The overlay draws it; Rust only stores the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Character {
    #[default]
    Cat,
    Snake,
    Dragon,
}

/// Where the cat sits until you drag it. Only one corner so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    #[default]
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub character: Character,
    pub notifications_enabled: bool,
    pub hide_while_sleeping: bool,
    pub sfw_mode: bool,
    pub ui_language: UiLanguage,
    pub klat_language: KlatLanguage,
    pub yell_interval_ms: DurationMs,
    pub max_yells_per_wake: u32,
    pub stale_after_ms: DurationMs,
    pub reduce_motion: bool,
    /// Size of the cat in percent (the whole overlay: cat, bubble and Ready button).
    pub cat_scale: u32,
    pub start_at_login: bool,
    pub corner: Corner,
    /// Where you dragged the cat (window top-left, physical px); None = the corner.
    pub position: Option<WindowPos>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            character: Character::Cat,
            notifications_enabled: true,
            hide_while_sleeping: false,
            sfw_mode: false,
            ui_language: UiLanguage::En,
            klat_language: KlatLanguage::Auto,
            yell_interval_ms: DurationMs(30_000),
            max_yells_per_wake: 10,
            stale_after_ms: DurationMs(1_800_000),
            reduce_motion: false,
            cat_scale: 100,
            start_at_login: false,
            corner: Corner::BottomRight,
            position: None,
        }
    }
}

const YELL_INTERVAL: (DurationMs, DurationMs) = (DurationMs(5_000), DurationMs(600_000));
const MAX_YELLS_PER_WAKE: (u32, u32) = (1, 50);
const STALE_AFTER: (DurationMs, DurationMs) = (DurationMs(300_000), DurationMs(14_400_000));
pub const CAT_SCALE: (u32, u32) = (50, 200);

impl Settings {
    /// `catScale` as a factor: the overlay window's size and its webview zoom.
    pub fn zoom(&self) -> f64 {
        f64::from(self.cat_scale) / 100.0
    }

    /// Builds settings from a JSON object field by field; anything missing or mistyped keeps its default, and
    /// numbers outside their range are clamped into it.
    pub fn from_object(obj: &Map<String, Value>) -> Self {
        let d = Self::default();
        Self {
            character: field(obj, "character", d.character),
            notifications_enabled: field(obj, "notificationsEnabled", d.notifications_enabled),
            hide_while_sleeping: field(obj, "hideWhileSleeping", d.hide_while_sleeping),
            sfw_mode: field(obj, "sfwMode", d.sfw_mode),
            ui_language: field(obj, "uiLanguage", d.ui_language),
            klat_language: if obj.contains_key("klatLanguage") {
                field(obj, "klatLanguage", d.klat_language)
            } else {
                klat_language_from_legacy(obj.get("languages"))
            },
            yell_interval_ms: field(obj, "yellIntervalMs", d.yell_interval_ms),
            max_yells_per_wake: field(obj, "maxYellsPerWake", d.max_yells_per_wake),
            stale_after_ms: field(obj, "staleAfterMs", d.stale_after_ms),
            reduce_motion: field(obj, "reduceMotion", d.reduce_motion),
            cat_scale: field(obj, "catScale", d.cat_scale),
            start_at_login: field(obj, "startAtLogin", d.start_at_login),
            corner: field(obj, "corner", d.corner),
            position: field(obj, "position", d.position),
        }
        .clamped()
    }

    /// A Settings window edit: the fields in the patch replace the current ones, and numbers are clamped into
    /// their range as when loading a file.
    pub fn apply(&self, patch: &SettingsPatch) -> Settings {
        // Named one by one, so a field added to the patch does not compile until it is applied here.
        let SettingsPatch {
            character,
            notifications_enabled,
            hide_while_sleeping,
            sfw_mode,
            ui_language,
            klat_language,
            yell_interval_ms,
            max_yells_per_wake,
            stale_after_ms,
            reduce_motion,
            cat_scale,
        } = *patch;
        let s = self.clone();
        Settings {
            character: character.unwrap_or(s.character),
            notifications_enabled: notifications_enabled.unwrap_or(s.notifications_enabled),
            hide_while_sleeping: hide_while_sleeping.unwrap_or(s.hide_while_sleeping),
            sfw_mode: sfw_mode.unwrap_or(s.sfw_mode),
            ui_language: ui_language.unwrap_or(s.ui_language),
            klat_language: klat_language.unwrap_or(s.klat_language),
            yell_interval_ms: yell_interval_ms.unwrap_or(s.yell_interval_ms),
            max_yells_per_wake: max_yells_per_wake.unwrap_or(s.max_yells_per_wake),
            stale_after_ms: stale_after_ms.unwrap_or(s.stale_after_ms),
            reduce_motion: reduce_motion.unwrap_or(s.reduce_motion),
            cat_scale: cat_scale.unwrap_or(s.cat_scale),
            ..s
        }
        .clamped()
    }

    fn clamped(self) -> Self {
        Self {
            yell_interval_ms: self
                .yell_interval_ms
                .clamp(YELL_INTERVAL.0, YELL_INTERVAL.1),
            max_yells_per_wake: self
                .max_yells_per_wake
                .clamp(MAX_YELLS_PER_WAKE.0, MAX_YELLS_PER_WAKE.1),
            stale_after_ms: self.stale_after_ms.clamp(STALE_AFTER.0, STALE_AFTER.1),
            cat_scale: self.cat_scale.clamp(CAT_SCALE.0, CAT_SCALE.1),
            ..self
        }
    }
}

/// What the Settings window may change (`set_settings`). Not `position` (dragging sets it), `corner` (one
/// value) or `startAtLogin` (it must go through the OS, see `autostart`): those keys are ignored, as are keys
/// this version does not know, so a newer or older Settings window never fails to save.
///
/// Read like the file: a value of the wrong type, `null` or an unknown name becomes that field's default, so
/// the patch resets the field rather than failing. `None` = the key was not in the patch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "Map<String, Value>")]
pub struct SettingsPatch {
    pub character: Option<Character>,
    pub notifications_enabled: Option<bool>,
    pub hide_while_sleeping: Option<bool>,
    pub sfw_mode: Option<bool>,
    pub ui_language: Option<UiLanguage>,
    pub klat_language: Option<KlatLanguage>,
    pub yell_interval_ms: Option<DurationMs>,
    pub max_yells_per_wake: Option<u32>,
    pub stale_after_ms: Option<DurationMs>,
    pub reduce_motion: Option<bool>,
    pub cat_scale: Option<u32>,
}

impl From<Map<String, Value>> for SettingsPatch {
    fn from(obj: Map<String, Value>) -> Self {
        let d = Settings::default();
        Self {
            character: patched(&obj, "character", d.character),
            notifications_enabled: patched(&obj, "notificationsEnabled", d.notifications_enabled),
            hide_while_sleeping: patched(&obj, "hideWhileSleeping", d.hide_while_sleeping),
            sfw_mode: patched(&obj, "sfwMode", d.sfw_mode),
            ui_language: patched(&obj, "uiLanguage", d.ui_language),
            klat_language: patched(&obj, "klatLanguage", d.klat_language),
            yell_interval_ms: patched(&obj, "yellIntervalMs", d.yell_interval_ms),
            max_yells_per_wake: patched(&obj, "maxYellsPerWake", d.max_yells_per_wake),
            stale_after_ms: patched(&obj, "staleAfterMs", d.stale_after_ms),
            reduce_motion: patched(&obj, "reduceMotion", d.reduce_motion),
            cat_scale: patched(&obj, "catScale", d.cat_scale),
        }
    }
}

/// Files written before Klat Language existed have `languages`: the phrase languages the cat may use.
fn klat_language_from_legacy(languages: Option<&Value>) -> KlatLanguage {
    let has = |lang: &str| {
        languages
            .and_then(Value::as_array)
            .is_some_and(|l| l.iter().any(|v| v.as_str() == Some(lang)))
    };
    match (has("en"), has("vi")) {
        (true, false) => KlatLanguage::En,
        (false, true) => KlatLanguage::Vi,
        _ => KlatLanguage::Auto,
    }
}

fn field<T: DeserializeOwned>(obj: &Map<String, Value>, key: &str, default: T) -> T {
    obj.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(default)
}

/// A patch field: None when the key is absent, otherwise read like [`field`].
fn patched<T: DeserializeOwned>(obj: &Map<String, Value>, key: &str, default: T) -> Option<T> {
    obj.contains_key(key).then(|| field(obj, key, default))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// What `set_settings` does with the window's JSON.
    fn edit(current: &Settings, patch: Value) -> Settings {
        current.apply(&serde_json::from_value(patch).unwrap())
    }

    /// Whether every field of the patch is set. Named one by one, so a new patch field fails to compile here
    /// until the tables below cover it.
    fn is_full(p: SettingsPatch) -> bool {
        let SettingsPatch {
            character,
            notifications_enabled,
            hide_while_sleeping,
            sfw_mode,
            ui_language,
            klat_language,
            yell_interval_ms,
            max_yells_per_wake,
            stale_after_ms,
            reduce_motion,
            cat_scale,
        } = p;
        character.is_some()
            && notifications_enabled.is_some()
            && hide_while_sleeping.is_some()
            && sfw_mode.is_some()
            && ui_language.is_some()
            && klat_language.is_some()
            && yell_interval_ms.is_some()
            && max_yells_per_wake.is_some()
            && stale_after_ms.is_some()
            && reduce_motion.is_some()
            && cat_scale.is_some()
    }

    #[test]
    fn defaults_match_the_spec() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(
            v,
            json!({
                "character": "cat",
                "notificationsEnabled": true, "hideWhileSleeping": false, "sfwMode": false,
                "uiLanguage": "en", "klatLanguage": "auto", "yellIntervalMs": 30000, "maxYellsPerWake": 10,
                "staleAfterMs": 1800000, "reduceMotion": false, "catScale": 100, "startAtLogin": false,
                "corner": "bottom-right",
                "position": null
            })
        );
    }

    #[test]
    fn mistyped_and_out_of_range_fields_fall_back_individually() {
        let obj = json!({
            "sfwMode": "yes", "reduceMotion": true, "yellIntervalMs": 10, "maxYellsPerWake": 0,
            "staleAfterMs": -5, "uiLanguage": "fr", "klatLanguage": 3, "corner": "top-left", "unknown": 1,
            "character": "unicorn"
        });
        let s = Settings::from_object(obj.as_object().unwrap());
        assert!(!s.sfw_mode);
        assert!(s.reduce_motion);
        assert_eq!(s.yell_interval_ms, YELL_INTERVAL.0);
        assert_eq!(s.max_yells_per_wake, 1);
        assert_eq!(s.stale_after_ms, DurationMs(1_800_000));
        assert_eq!(s.ui_language, UiLanguage::En);
        assert_eq!(s.klat_language, KlatLanguage::Auto);
        assert_eq!(s.corner, Corner::BottomRight);
        assert_eq!(s.character, Character::Cat);
    }

    #[test]
    fn numbers_are_clamped_into_their_range() {
        let obj = |y: u64, m: u32, st: u64| json!({ "yellIntervalMs": y, "maxYellsPerWake": m, "staleAfterMs": st });
        let low = Settings::from_object(obj(1_000, 0, 60_000).as_object().unwrap());
        assert_eq!(
            (
                low.yell_interval_ms,
                low.max_yells_per_wake,
                low.stale_after_ms
            ),
            (DurationMs(5_000), 1, DurationMs(300_000))
        );
        let high = Settings::from_object(obj(u64::MAX, 999, u64::MAX).as_object().unwrap());
        assert_eq!(
            (
                high.yell_interval_ms,
                high.max_yells_per_wake,
                high.stale_after_ms
            ),
            (DurationMs(600_000), 50, DurationMs(14_400_000))
        );
    }

    #[test]
    fn cat_scale_is_clamped_and_falls_back() {
        let scale = |v: Value| {
            Settings::from_object(json!({ "catScale": v }).as_object().unwrap()).cat_scale
        };
        assert_eq!(scale(json!(150)), 150);
        assert_eq!(scale(json!(10)), 50);
        assert_eq!(scale(json!(1000)), 200);
        assert_eq!(scale(json!("big")), 100);
        assert_eq!(scale(json!(-3)), 100);
        let patched = edit(&Settings::default(), json!({ "catScale": 125 }));
        assert_eq!(patched.cat_scale, 125);
        assert_eq!(patched.zoom(), 1.25);
    }

    #[test]
    fn languages_from_older_files_become_klat_language() {
        let klat = |v: Value| Settings::from_object(v.as_object().unwrap()).klat_language;
        assert_eq!(
            klat(json!({ "languages": ["en", "vi"] })),
            KlatLanguage::Auto
        );
        assert_eq!(
            klat(json!({ "languages": ["vi", "en", "fr"] })),
            KlatLanguage::Auto
        );
        assert_eq!(klat(json!({ "languages": ["en"] })), KlatLanguage::En);
        assert_eq!(
            klat(json!({ "languages": ["vi", "fr", "vi"] })),
            KlatLanguage::Vi
        );
        assert_eq!(klat(json!({ "languages": ["fr"] })), KlatLanguage::Auto);
        assert_eq!(klat(json!({ "languages": [] })), KlatLanguage::Auto);
        assert_eq!(klat(json!({ "languages": "vi" })), KlatLanguage::Auto);
        assert_eq!(klat(json!({})), KlatLanguage::Auto);
        // Once klatLanguage exists the old list is ignored, even when klatLanguage is invalid.
        assert_eq!(
            klat(json!({ "languages": ["vi"], "klatLanguage": "en" })),
            KlatLanguage::En
        );
        assert_eq!(
            klat(json!({ "languages": ["vi"], "klatLanguage": "xx" })),
            KlatLanguage::Auto
        );
    }

    #[test]
    fn patch_changes_only_whitelisted_fields_and_revalidates() {
        let current = Settings {
            position: Some(WindowPos::new(10, 20)),
            ..Settings::default()
        };
        let patch = json!({
            "character": "dragon", "sfwMode": true, "uiLanguage": "vi", "klatLanguage": "en", "yellIntervalMs": 1,
            "position": null, "startAtLogin": true, "corner": "top-left", "bogus": 1
        });
        let next = edit(&current, patch);
        assert_eq!(next.character, Character::Dragon);
        assert!(next.sfw_mode);
        assert_eq!(next.ui_language, UiLanguage::Vi);
        assert_eq!(next.klat_language, KlatLanguage::En);
        assert_eq!(next.yell_interval_ms, DurationMs(5_000));
        assert_eq!(next.position, current.position);
        assert!(!next.start_at_login);
        assert_eq!(next.corner, Corner::BottomRight);
    }

    #[test]
    fn a_mistyped_patch_value_falls_back_to_its_default() {
        let current = Settings {
            sfw_mode: true,
            max_yells_per_wake: 3,
            ..Settings::default()
        };
        let next = edit(&current, json!({ "maxYellsPerWake": 7 }));
        assert!(next.sfw_mode);
        assert_eq!(next.max_yells_per_wake, 7);
        let next = edit(&current, json!({ "sfwMode": "yes" }));
        assert!(!next.sfw_mode);
        assert_eq!(next.max_yells_per_wake, 3);
    }

    /// A patch key, a value for it, and what it must change.
    type PatchCase = (&'static str, Value, fn(&mut Settings));

    /// A patch key, and how to put its field back to the default.
    type FieldReset = (&'static str, fn(&mut Settings, &Settings));

    /// Settings with every patchable field away from its default, so a reset to the default shows.
    fn far_from_defaults() -> Settings {
        Settings {
            character: Character::Snake,
            notifications_enabled: false,
            hide_while_sleeping: true,
            sfw_mode: true,
            ui_language: UiLanguage::Vi,
            klat_language: KlatLanguage::Vi,
            yell_interval_ms: DurationMs(60_000),
            max_yells_per_wake: 3,
            stale_after_ms: DurationMs(600_000),
            reduce_motion: true,
            cat_scale: 150,
            start_at_login: true,
            corner: Corner::BottomRight,
            position: Some(WindowPos::new(-40, 70)),
        }
    }

    #[test]
    fn spec_keys_the_settings_window_may_not_change_are_ignored() {
        let current = far_from_defaults();
        for patch in [
            json!({ "position": { "x": 1, "y": 2 } }),
            json!({ "position": null }),
            json!({ "startAtLogin": false }),
            json!({ "corner": "top-left" }),
            json!({ "languages": ["en"] }),
            json!({ "bogus": 1, "": true }),
            // Keys are case-sensitive, as in the file.
            json!({ "SfwMode": false, "sfw_mode": false, "CATSCALE": 50 }),
            json!({}),
        ] {
            let next = edit(&current, patch.clone());
            assert_eq!(next, current, "{patch}");
        }
    }

    #[test]
    fn spec_each_patchable_key_changes_only_its_field() {
        let current = far_from_defaults();
        let cases: [PatchCase; 11] = [
            ("character", json!("dragon"), |s| {
                s.character = Character::Dragon
            }),
            ("notificationsEnabled", json!(true), |s| {
                s.notifications_enabled = true
            }),
            ("hideWhileSleeping", json!(false), |s| {
                s.hide_while_sleeping = false
            }),
            ("sfwMode", json!(false), |s| s.sfw_mode = false),
            ("uiLanguage", json!("en"), |s| {
                s.ui_language = UiLanguage::En
            }),
            ("klatLanguage", json!("auto"), |s| {
                s.klat_language = KlatLanguage::Auto
            }),
            ("yellIntervalMs", json!(45_000), |s| {
                s.yell_interval_ms = DurationMs(45_000)
            }),
            ("maxYellsPerWake", json!(20), |s| s.max_yells_per_wake = 20),
            ("staleAfterMs", json!(900_000), |s| {
                s.stale_after_ms = DurationMs(900_000)
            }),
            ("reduceMotion", json!(false), |s| s.reduce_motion = false),
            ("catScale", json!(75), |s| s.cat_scale = 75),
        ];
        let all: Map<String, Value> = cases
            .iter()
            .map(|(k, v, _)| (k.to_string(), v.clone()))
            .collect();
        assert!(
            is_full(SettingsPatch::from(all)),
            "a patch field has no case"
        );
        for (key, value, expect) in cases {
            let mut expected = current.clone();
            expect(&mut expected);
            let next = edit(&current, json!({ key: value }));
            assert_eq!(next, expected, "{key}");
        }
    }

    /// Records today's behaviour, not a goal: a value of the wrong type (or null) resets the field to its
    /// default, not to its current value, because the patch is validated like a hand-edited file.
    #[test]
    fn spec_a_mistyped_or_null_patch_value_resets_its_field_to_the_default() {
        let current = far_from_defaults();
        let d = Settings::default();
        let bad = [
            json!(null),
            json!("x"),
            json!([1]),
            json!({ "a": 1 }),
            json!(-1),
        ];
        // Copies the default into one field.
        let fields: [FieldReset; 11] = [
            ("character", |s, d| s.character = d.character),
            ("notificationsEnabled", |s, d| {
                s.notifications_enabled = d.notifications_enabled
            }),
            ("hideWhileSleeping", |s, d| {
                s.hide_while_sleeping = d.hide_while_sleeping
            }),
            ("sfwMode", |s, d| s.sfw_mode = d.sfw_mode),
            ("uiLanguage", |s, d| s.ui_language = d.ui_language),
            ("klatLanguage", |s, d| s.klat_language = d.klat_language),
            ("yellIntervalMs", |s, d| {
                s.yell_interval_ms = d.yell_interval_ms
            }),
            ("maxYellsPerWake", |s, d| {
                s.max_yells_per_wake = d.max_yells_per_wake
            }),
            ("staleAfterMs", |s, d| s.stale_after_ms = d.stale_after_ms),
            ("reduceMotion", |s, d| s.reduce_motion = d.reduce_motion),
            ("catScale", |s, d| s.cat_scale = d.cat_scale),
        ];
        let all: Map<String, Value> = fields
            .iter()
            .map(|(k, _)| (k.to_string(), Value::Null))
            .collect();
        assert!(
            is_full(SettingsPatch::from(all)),
            "a patch field has no case"
        );
        for (key, reset) in fields {
            let mut expected = current.clone();
            reset(&mut expected, &d);
            for value in &bad {
                let next = edit(&current, json!({ key: value }));
                // That field is back to its default; every other field keeps its current value.
                assert_eq!(next, expected, "{key}: {value}");
            }
        }
        // An unknown enum value counts as mistyped.
        let next = edit(&current, json!({ "character": "unicorn" }));
        assert_eq!(next.character, Character::Cat);
        assert_eq!(next.sfw_mode, current.sfw_mode);
    }

    #[test]
    fn spec_patched_numbers_are_clamped_not_reset() {
        let current = far_from_defaults();
        let patch = json!({ "yellIntervalMs": 1, "maxYellsPerWake": 1000, "staleAfterMs": u64::MAX, "catScale": 5 });
        let next = edit(&current, patch);
        assert_eq!(
            (
                next.yell_interval_ms,
                next.max_yells_per_wake,
                next.stale_after_ms,
                next.cat_scale
            ),
            (
                YELL_INTERVAL.0,
                MAX_YELLS_PER_WAKE.1,
                STALE_AFTER.1,
                CAT_SCALE.0
            )
        );
    }

    #[test]
    fn a_patch_is_an_object_of_known_keys() {
        assert_eq!(
            serde_json::from_value::<SettingsPatch>(json!({ "bogus": 1 })).unwrap(),
            SettingsPatch::default()
        );
        assert_eq!(
            serde_json::from_value::<SettingsPatch>(json!({ "sfwMode": true, "catScale": 75 }))
                .unwrap(),
            SettingsPatch {
                sfw_mode: Some(true),
                cat_scale: Some(75),
                ..SettingsPatch::default()
            }
        );
        // Not an object: the command fails, as it did when it took a JSON map.
        for not_object in [json!([1]), json!("sfwMode"), json!(null), json!(3)] {
            assert!(serde_json::from_value::<SettingsPatch>(not_object).is_err());
        }
    }
}

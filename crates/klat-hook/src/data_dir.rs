//! Where Klat keeps its files (settings.json, runtime.json, the log). Shared word for word by the app and the
//! hook adapter: the app includes this file with `#[path]`, so both always look in the same folder.
//!
//! - Windows: `%APPDATA%\Klat`
//! - macOS: `~/Library/Application Support/Klat`
//! - Other systems: `$XDG_DATA_HOME/Klat`, else `~/.local/share/Klat`
//!
//! When the base folder is unknown, settings and the log fall back to the temp folder, but runtime.json (which
//! holds the token) does not exist: the temp folder can be shared with other users.

use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Other,
}

pub const CURRENT_OS: Os = if cfg!(windows) {
    Os::Windows
} else if cfg!(target_os = "macos") {
    Os::Mac
} else {
    Os::Other
};

/// Klat's data folder on this machine, or None when the base folder is not set.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_for(CURRENT_OS, |key| std::env::var_os(key))
}

/// The data folder for `os`, reading environment variables through `env` (so every branch can be tested on any
/// machine).
pub fn data_dir_for(os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |key: &str| env(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = match os {
        Os::Windows => set("APPDATA")?,
        Os::Mac => set("HOME")?.join("Library").join("Application Support"),
        Os::Other => {
            set("XDG_DATA_HOME").or_else(|| Some(set("HOME")?.join(".local").join("share")))?
        }
    };
    Some(base.join("Klat"))
}

/// `runtime.json`: the port and token the running app listens with. None without a data folder.
pub fn runtime_path() -> Option<PathBuf> {
    Some(data_dir()?.join("runtime.json"))
}

#[cfg(test)]
mod data_dir_tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn windows_uses_appdata() {
        let e = env(&[
            ("APPDATA", r"C:\Users\me\AppData\Roaming"),
            ("HOME", "/ignored"),
        ]);
        assert_eq!(
            data_dir_for(Os::Windows, e),
            Some(PathBuf::from(r"C:\Users\me\AppData\Roaming").join("Klat"))
        );
        assert_eq!(
            data_dir_for(Os::Windows, env(&[("HOME", "/Users/me")])),
            None
        );
    }

    #[test]
    fn macos_uses_application_support() {
        let e = env(&[("HOME", "/Users/me"), ("APPDATA", "ignored")]);
        assert_eq!(
            data_dir_for(Os::Mac, e),
            Some(
                PathBuf::from("/Users/me")
                    .join("Library")
                    .join("Application Support")
                    .join("Klat")
            )
        );
        assert_eq!(data_dir_for(Os::Mac, env(&[])), None);
    }

    #[test]
    fn other_systems_prefer_xdg_data_home() {
        assert_eq!(
            data_dir_for(
                Os::Other,
                env(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/me")])
            ),
            Some(PathBuf::from("/data").join("Klat"))
        );
        assert_eq!(
            data_dir_for(Os::Other, env(&[("HOME", "/home/me")])),
            Some(
                PathBuf::from("/home/me")
                    .join(".local")
                    .join("share")
                    .join("Klat")
            )
        );
    }

    #[test]
    fn empty_variables_count_as_unset() {
        assert_eq!(data_dir_for(Os::Windows, env(&[("APPDATA", "")])), None);
    }

    #[test]
    fn this_machine_matches_its_os() {
        if let Some(dir) = data_dir() {
            assert!(dir.ends_with("Klat"));
        }
        if let Some(path) = runtime_path() {
            assert!(path.ends_with(PathBuf::from("Klat").join("runtime.json")));
        }
    }
}

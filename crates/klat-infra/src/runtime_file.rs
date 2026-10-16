//! `runtime.json` (`klat_protocol::runtime_path`): the port and token the hook adapter needs to reach the app.
//! Readable by this user only (0600 on Unix), never placed in the temp folder, and created fresh so a planted
//! file or link is not followed. Deleted when the app quits.

use klat_protocol::RuntimeInfo;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Writes `runtime.json` (`klat_protocol::runtime_path`, where the hook adapter reads it).
pub fn write_runtime(path: &Path, info: &RuntimeInfo) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let _ = fs::remove_file(&tmp);
    let mut file = private_file().create_new(true).open(&tmp)?;
    file.write_all(&serde_json::to_vec(info).map_err(io::Error::other)?)?;
    drop(file);
    fs::rename(&tmp, path)
}

/// Options for a file only this user can read: 0600 on Unix. On Windows the data folder (`%APPDATA%`) is
/// already private to the user.
fn private_file() -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
}

pub fn remove_runtime(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_file_round_trips_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Klat").join("runtime.json");
        let info = RuntimeInfo {
            port: 4242,
            token: "t".into(),
            pid: 7,
        };
        write_runtime(&path, &info).unwrap();
        let back: RuntimeInfo = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(back, info);
        remove_runtime(&path);
        assert!(!path.exists());
    }

    #[test]
    fn runtime_file_replaces_a_leftover_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.json");
        fs::write(dir.path().join("runtime.json.tmp"), "planted").unwrap();
        let info = RuntimeInfo {
            port: 1,
            token: "t".into(),
            pid: 2,
        };
        write_runtime(&path, &info).unwrap();
        let back: RuntimeInfo = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(back, info);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_file_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.json");
        let info = RuntimeInfo {
            port: 1,
            token: "t".into(),
            pid: 2,
        };
        write_runtime(&path, &info).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

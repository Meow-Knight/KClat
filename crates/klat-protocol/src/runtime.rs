//! `runtime.json`: how the hook finds the running app.

use serde::{Deserialize, Serialize};

/// Hex characters in a token (256 bits).
pub const TOKEN_LEN: usize = 64;

/// What the app writes to [`crate::runtime_path`] while it runs (private to the user, deleted when it quits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// Loopback port of the event endpoint.
    pub port: u16,
    /// Bearer token for `POST /event`, fresh for each app run.
    pub token: String,
    /// The app's process id. The hook does not need it.
    #[serde(default)]
    pub pid: u32,
}

/// A token the app could have made: [`TOKEN_LEN`] hex characters. The hook sends nothing with anything else.
pub fn is_valid_token(token: &str) -> bool {
    token.len() == TOKEN_LEN && token.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_format_is_exactly_this() {
        let info = RuntimeInfo {
            port: 4242,
            token: "t".into(),
            pid: 7,
        };
        let text = serde_json::to_string(&info).unwrap();
        assert_eq!(text, r#"{"port":4242,"token":"t","pid":7}"#);
        assert_eq!(serde_json::from_str::<RuntimeInfo>(&text).unwrap(), info);
    }

    #[test]
    fn a_port_out_of_range_is_not_read() {
        assert!(
            serde_json::from_str::<RuntimeInfo>(r#"{"port":70000,"token":"t","pid":1}"#).is_err()
        );
        assert!(serde_json::from_str::<RuntimeInfo>(r#"{"token":"t","pid":1}"#).is_err());
    }

    #[test]
    fn tokens_are_64_hex_characters() {
        assert!(is_valid_token(&"a".repeat(64)));
        assert!(is_valid_token(&"0123456789abcdefABCDEF".repeat(3)[..64]));
        assert!(!is_valid_token(&"a".repeat(63)));
        assert!(!is_valid_token(&"a".repeat(65)));
        assert!(!is_valid_token(&format!("{}g", "a".repeat(63))));
        assert!(!is_valid_token(""));
    }
}

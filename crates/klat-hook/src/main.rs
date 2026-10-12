//! klat-hook: Claude Code hook adapter (spec §8.3–8.4).
//!
//! Reads the hook JSON from stdin, keeps only the fields the cat needs, and POSTs them to the Klat
//! app over loopback. Whatever happens it prints nothing and exits 0, so Claude Code can never read its
//! output as a decision, and an app that is not running costs at most ~300ms.

use serde_json::{json, Value};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Stdin beyond this is not read (tool responses can be huge; the fields we need come first).
const MAX_STDIN: u64 = 256 * 1024;
/// Total budget for talking to the app.
const BUDGET: Duration = Duration::from_millis(300);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(150);

fn main() {
    // Never print anything, not even a panic message.
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(run);
    std::process::exit(0);
}

fn run() {
    let started = Instant::now();
    let mut input = Vec::new();
    let _ = std::io::stdin().take(MAX_STDIN).read_to_end(&mut input);
    let fallback_name = std::env::args().nth(1);
    let Some(event) = build_event(&input, fallback_name.as_deref(), now_ms(), &new_id()) else {
        return;
    };
    let Some((port, token)) = read_runtime() else {
        return; // app not running
    };
    let _ = post(port, &token, &event, started + BUDGET);
}

/// The §8.3 payload, or None when there is no session id to attribute the event to.
fn build_event(input: &[u8], fallback_name: Option<&str>, now: u64, id: &str) -> Option<Value> {
    let parsed = serde_json::from_slice::<Value>(input)
        .ok()
        .filter(Value::is_object);
    let field = |key: &str| -> Option<String> {
        match &parsed {
            Some(v) => v.get(key).and_then(Value::as_str).map(str::to_owned),
            None => scan_string_field(input, key),
        }
    };
    let session_id = field("session_id").filter(|s| !s.is_empty())?;
    let hook_event_name = field("hook_event_name").or_else(|| fallback_name.map(str::to_owned))?;
    let mut ev = json!({
        "eventId": id,
        "hookEventName": hook_event_name,
        "sessionId": session_id,
        "occurredAt": now,
    });
    // Only the folder name, never the full path; never tool_input, commands or transcripts.
    if let Some(project) = field("cwd").as_deref().and_then(basename) {
        ev["projectName"] = json!(project);
    }
    if let Some(tool) = field("tool_name") {
        ev["toolName"] = json!(tool);
    }
    if let Some(kind) = field("notification_type") {
        ev["notificationType"] = json!(kind);
    }
    Some(ev)
}

/// For input cut off at MAX_STDIN (so not valid JSON): find `"key": "value"` in the prefix. Claude Code
/// writes the small identifying fields before tool_input/tool_response, so the first match is the real one.
fn scan_string_field(input: &[u8], key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let text = String::from_utf8_lossy(input);
    let after_key = &text[text.find(&needle)? + needle.len()..];
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    serde_json::Deserializer::from_str(after_colon)
        .into_iter::<String>()
        .next()?
        .ok()
}

fn basename(path: &str) -> Option<String> {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty() && !s.ends_with(':'))
        .map(str::to_owned)
}

fn runtime_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Klat")
        .join("runtime.json")
}

fn read_runtime() -> Option<(u16, String)> {
    let v: Value = serde_json::from_slice(&std::fs::read(runtime_path()).ok()?).ok()?;
    let port = u16::try_from(v.get("port")?.as_u64()?).ok()?;
    let token = v.get("token")?.as_str()?;
    let valid = token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit());
    valid.then(|| (port, token.to_owned()))
}

fn post(port: u16, token: &str, event: &Value, deadline: Instant) -> std::io::Result<u16> {
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT.min(remaining()?))?;
    let body = serde_json::to_vec(event)?;
    // One write for head and body: one segment on loopback, one syscall.
    let mut request = format!(
        "POST /event HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    request.extend_from_slice(&body);
    stream.set_write_timeout(Some(remaining()?))?;
    stream.write_all(&request)?;
    stream.set_read_timeout(Some(remaining()?))?;
    let mut status = [0u8; 12];
    stream.read_exact(&mut status)?; // "HTTP/1.1 204"
    std::str::from_utf8(&status[9..12])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// UUID v4 format from the OS-seeded hasher keys; uniqueness is all the app needs (no crypto use).
fn new_id() -> String {
    let mut words = [0u64; 2];
    for (i, w) in words.iter_mut().enumerate() {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(now_ms() ^ ((std::process::id() as u64) << 20) ^ i as u64);
        *w = h.finish();
    }
    let b: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-4{:01x}{:02x}-{:01x}{:01x}{:02x}-{}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6] & 0x0f,
        b[7],
        8 | (b[8] & 0x03),
        b[8] >> 4,
        b[9],
        b[10..16]
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const STOP: &str = r#"{"session_id":"abc123","transcript_path":"C:\\Users\\me\\.claude\\projects\\x.jsonl","cwd":"D:\\Bohi\\workspace\\ClauCat","permission_mode":"default","hook_event_name":"Stop","stop_hook_active":false,"last_assistant_message":"secret text"}"#;

    #[test]
    fn keeps_only_the_contract_fields() {
        let ev = build_event(STOP.as_bytes(), None, 42, "id-1").unwrap();
        assert_eq!(
            ev,
            json!({"eventId":"id-1","hookEventName":"Stop","sessionId":"abc123","projectName":"ClauCat","occurredAt":42})
        );
        let text = ev.to_string();
        for leak in ["transcript", "secret", "Bohi", "permission_mode"] {
            assert!(!text.contains(leak), "{leak}");
        }
    }

    #[test]
    fn extracts_tool_and_notification_type_but_never_tool_input() {
        let input = r#"{"session_id":"s","cwd":"/home/me/proj","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf /"}}"#;
        let ev = build_event(input.as_bytes(), None, 1, "i").unwrap();
        assert_eq!(ev["toolName"], "Bash");
        assert_eq!(ev["projectName"], "proj");
        assert!(!ev.to_string().contains("rm -rf"));
        let note = r#"{"session_id":"s","cwd":"/p","hook_event_name":"Notification","message":"m","notification_type":"idle_prompt"}"#;
        assert_eq!(
            build_event(note.as_bytes(), None, 1, "i").unwrap()["notificationType"],
            "idle_prompt"
        );
    }

    #[test]
    fn falls_back_to_the_argument_for_the_event_name() {
        let input = r#"{"session_id":"s","cwd":"/p"}"#;
        assert_eq!(
            build_event(input.as_bytes(), Some("Stop"), 1, "i").unwrap()["hookEventName"],
            "Stop"
        );
        assert!(build_event(input.as_bytes(), None, 1, "i").is_none());
    }

    #[test]
    fn needs_a_session_id() {
        assert!(build_event(br#"{"hook_event_name":"Stop"}"#, None, 1, "i").is_none());
        assert!(build_event(
            br#"{"session_id":"","hook_event_name":"Stop"}"#,
            None,
            1,
            "i"
        )
        .is_none());
        assert!(build_event(b"", Some("Stop"), 1, "i").is_none());
        assert!(build_event(b"[1,2]", Some("Stop"), 1, "i").is_none());
    }

    #[test]
    fn scans_truncated_input() {
        // A PostToolUse whose tool_response blew past MAX_STDIN: the JSON is cut mid-string.
        let mut input = r#"{"session_id":"big","cwd":"C:\\work\\repo","hook_event_name":"PostToolUse","tool_name":"Read","tool_input":{"file_path":"x"},"tool_response":{"content":""#.to_string();
        input.push_str(&"a".repeat(1000));
        let ev = build_event(input.as_bytes(), None, 1, "i").unwrap();
        assert_eq!(ev["sessionId"], "big");
        assert_eq!(ev["hookEventName"], "PostToolUse");
        assert_eq!(ev["toolName"], "Read");
        assert_eq!(ev["projectName"], "repo");
    }

    #[test]
    fn basename_handles_both_separators_and_edges() {
        assert_eq!(
            basename(r"D:\Bohi\workspace\ClauCat").as_deref(),
            Some("ClauCat")
        );
        assert_eq!(
            basename(r"D:\Bohi\workspace\ClauCat\").as_deref(),
            Some("ClauCat")
        );
        assert_eq!(basename("/home/me/my proj/").as_deref(), Some("my proj"));
        assert_eq!(basename("C:\\"), None);
        assert_eq!(basename("/"), None);
        assert_eq!(basename(""), None);
    }

    #[test]
    fn ids_look_like_uuid_v4_and_differ() {
        let (a, b) = (new_id(), new_id());
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
        assert!("89ab".contains(&a[19..20]));
        assert_ne!(a, b);
    }

    /// Reads one HTTP request (head + Content-Length body), however it was split into segments.
    fn read_request(s: &mut TcpStream) -> String {
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = s.read(&mut buf).unwrap();
            data.extend_from_slice(&buf[..n]);
            let text = String::from_utf8_lossy(&data).into_owned();
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let len: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("Content-Length: "))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                if body.len() >= len || n == 0 {
                    return text;
                }
            }
            if n == 0 {
                return text;
            }
        }
    }

    #[test]
    fn post_sends_the_bearer_token_and_reads_the_status() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let req = read_request(&mut s);
            s.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            req
        });
        let ev = json!({"hookEventName":"Stop"});
        let status = post(port, "tok", &ev, Instant::now() + BUDGET).unwrap();
        assert_eq!(status, 204);
        let req = server.join().unwrap();
        assert!(req.starts_with("POST /event HTTP/1.1\r\n"));
        assert!(req.contains("Authorization: Bearer tok\r\n"));
        assert!(req.ends_with(r#"{"hookEventName":"Stop"}"#));
    }

    #[test]
    fn post_gives_up_within_the_budget_when_the_app_hangs() {
        // Accepts the connection but never answers.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let _hold = std::thread::spawn(move || {
            let conn = listener.accept();
            std::thread::sleep(Duration::from_secs(3));
            drop(conn);
        });
        let started = Instant::now();
        assert!(post(port, "tok", &json!({}), started + BUDGET).is_err());
        assert!(
            started.elapsed() < BUDGET + Duration::from_millis(50),
            "{:?}",
            started.elapsed()
        );
    }
}

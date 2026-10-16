//! Loopback HTTP endpoint for the hook adapter (spec §8.3–8.4).
//!
//! - Binds 127.0.0.1 on a random port; a fresh 256-bit bearer token per app run.
//! - `{port, token}` goes to `%APPDATA%\Klat\runtime.json`, deleted when the app quits. Readable by this user only
//!   (0600 on Unix), never placed in the temp folder, and created fresh so a planted file or link is not followed.
//! - Accepts only `POST /event` with the right token, no `Origin` header (no browser pages), a body of at
//!   most 64 KB, valid JSON, and an allowlisted `hookEventName`. Everything else gets a 4xx and is dropped.
//! - Each request must arrive whole within `REQUEST_DEADLINE`, so nobody can hold a connection slot open by
//!   sending a byte now and then, and rejections are logged in short bursts, so a flood cannot rotate the
//!   real entries out of the capped log.

use crate::session_store::{HookEvent, ALLOWED_HOOKS};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_BODY: usize = 64 * 1024;
const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_CONNECTIONS: usize = 16;
/// For writing the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(1);
/// The whole request (head and body) must arrive within this. The adapter sends it in one write and gives up
/// after 300ms itself; a per-read timeout alone let one byte a second keep a connection open for hours.
const REQUEST_DEADLINE: Duration = Duration::from_millis(500);
/// At most this many rejected requests are logged per window; the rest are only counted.
const REJECT_LOG_BURST: u32 = 5;
const REJECT_LOG_WINDOW_MS: u64 = 60_000;
const MAX_FIELD: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

/// `runtime.json` in the data folder; the hook adapter reads the same path (data_dir.rs is shared). None when
/// there is no data folder.
pub fn runtime_path() -> Option<PathBuf> {
    crate::data_dir::runtime_path()
}

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

pub fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Starts the server thread and returns the bound port. `on_event` runs on a connection thread.
pub fn start<F>(token: String, on_event: F) -> io::Result<u16>
where
    F: Fn(HookEvent) + Send + Sync + 'static,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let token = Arc::new(token);
    let on_event = Arc::new(on_event);
    let active = Arc::new(AtomicUsize::new(0));
    thread::Builder::new()
        .name("event-server".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                    continue; // dropping the stream closes it
                }
                active.fetch_add(1, Ordering::Relaxed);
                let (token, on_event, counter) = (token.clone(), on_event.clone(), active.clone());
                let spawned = thread::Builder::new()
                    .name("event-conn".into())
                    .spawn(move || {
                        let _ = serve(stream, &token, &*on_event);
                        counter.fetch_sub(1, Ordering::Relaxed);
                    });
                if spawned.is_err() {
                    active.fetch_sub(1, Ordering::Relaxed);
                }
            }
        })?;
    Ok(port)
}

/// Reads from the connection until `until`, however the bytes are spread out.
struct Deadline {
    stream: TcpStream,
    until: Instant,
}

impl Read for Deadline {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self
            .until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}

/// Rate limit for the rejection warnings (`REJECT_LOG_BURST` per `REJECT_LOG_WINDOW_MS`).
#[derive(Debug, Default)]
struct RejectLog {
    window_start: u64,
    logged: u32,
    suppressed: u32,
}

impl RejectLog {
    const fn new() -> Self {
        Self {
            window_start: 0,
            logged: 0,
            suppressed: 0,
        }
    }

    /// The log lines for a rejection at `now` (unix ms): none once the burst is used up.
    fn on_reject(&mut self, status: u16, now: u64) -> Vec<String> {
        let mut lines = Vec::new();
        if now.saturating_sub(self.window_start) >= REJECT_LOG_WINDOW_MS {
            if self.suppressed > 0 {
                lines.push(format!(
                    "{} more rejected requests to the event endpoint were not logged",
                    self.suppressed
                ));
            }
            *self = Self {
                window_start: now,
                ..Self::new()
            };
        }
        if self.logged < REJECT_LOG_BURST {
            self.logged += 1;
            let mut line = format!("rejected a request to the event endpoint: HTTP {status}");
            if self.logged == REJECT_LOG_BURST {
                line.push_str(" (more this minute are counted, not logged)");
            }
            lines.push(line);
        } else {
            self.suppressed += 1;
        }
        lines
    }
}

static REJECTS: Mutex<RejectLog> = Mutex::new(RejectLog::new());

fn serve(stream: TcpStream, token: &str, on_event: &dyn Fn(HookEvent)) -> io::Result<()> {
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(Deadline {
        stream: stream.try_clone()?,
        until: Instant::now() + REQUEST_DEADLINE,
    });
    let status = match handle(&mut reader, token) {
        Ok(ev) => {
            on_event(ev);
            204
        }
        Err(status) => {
            let lines = match REJECTS.lock() {
                Ok(mut log) => log.on_reject(status, crate::state::now_ms()),
                Err(_) => Vec::new(),
            };
            for line in lines {
                crate::log::warn(line);
            }
            status
        }
    };
    let mut out = stream;
    out.write_all(&response(status))?;
    out.flush()
}

/// The whole answer, so it goes out in one write. `write!` straight to the socket sent each piece on its own,
/// and when a client still sending was cut off, the reset that followed the close could discard the pieces it
/// had not read yet (Windows), leaving it with just "HTTP/1.1 ".
fn response(status: u16) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        reason(status)
    )
    .into_bytes()
}

fn reason(status: u16) -> &'static str {
    match status {
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        _ => "Error",
    }
}

/// Parses and validates one request. Err carries the HTTP status to answer with.
pub fn handle(reader: &mut impl BufRead, token: &str) -> Result<HookEvent, u16> {
    let head = read_head(reader)?;
    let mut lines = head.lines();
    let mut request_line = lines.next().unwrap_or("").split(' ');
    let (method, path) = (
        request_line.next().unwrap_or(""),
        request_line.next().unwrap_or(""),
    );

    let mut content_length = None;
    let mut authorization = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(400);
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => content_length = Some(value.parse::<usize>().map_err(|_| 400u16)?),
            "authorization" => authorization = Some(value.to_owned()),
            // Browsers always send Origin on cross-site POSTs; the hook adapter never does.
            "origin" => return Err(403),
            "transfer-encoding" => return Err(400),
            _ => {}
        }
    }

    if path != "/event" {
        return Err(404);
    }
    if method != "POST" {
        return Err(405);
    }
    let presented = authorization
        .as_deref()
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq(presented.as_bytes(), token.as_bytes()) {
        return Err(401);
    }
    let len = content_length.ok_or(411u16)?;
    if len > MAX_BODY {
        return Err(413);
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).map_err(|_| 400u16)?;

    let ev: HookEvent = serde_json::from_slice(&body).map_err(|_| 400u16)?;
    validate(&ev)?;
    Ok(ev)
}

fn validate(ev: &HookEvent) -> Result<(), u16> {
    if !ALLOWED_HOOKS.contains(&ev.hook_event_name.as_str()) {
        return Err(422);
    }
    let too_long = |s: &str| s.chars().count() > MAX_FIELD;
    if ev.session_id.is_empty() || too_long(&ev.session_id) || too_long(&ev.event_id) {
        return Err(422);
    }
    for field in [&ev.project_name, &ev.tool_name, &ev.notification_type]
        .into_iter()
        .flatten()
    {
        if too_long(field) {
            return Err(422);
        }
    }
    Ok(())
}

/// Request line + headers, up to the blank line.
fn read_head(reader: &mut impl BufRead) -> Result<String, u16> {
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take((MAX_HEADER_BYTES + 1 - head.len()) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| 400u16)?;
        if n == 0 {
            return Err(400);
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        head.extend_from_slice(&line);
        if head.len() > MAX_HEADER_BYTES {
            return Err(400);
        }
    }
    // str::lines() later splits on "\n" and drops a trailing "\r", so CRLF and bare LF both work.
    String::from_utf8(head).map_err(|_| 400u16)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::mpsc;

    const TOKEN: &str = "s3cret";

    fn body(name: &str) -> String {
        format!(
            r#"{{"eventId":"1","hookEventName":"{name}","sessionId":"abc","projectName":"ClauCat","occurredAt":1}}"#
        )
    }

    fn request(method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
        let mut r = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        for (k, v) in headers {
            r.push_str(&format!("{k}: {v}\r\n"));
        }
        r.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        r.into_bytes()
    }

    fn ok_request(body: &str) -> Vec<u8> {
        request(
            "POST",
            "/event",
            &[("Authorization", "Bearer s3cret")],
            body,
        )
    }

    fn run(raw: Vec<u8>) -> Result<HookEvent, u16> {
        handle(&mut Cursor::new(raw), TOKEN)
    }

    #[test]
    fn accepts_a_valid_event() {
        let ev = run(ok_request(&body("Stop"))).unwrap();
        assert_eq!(ev.hook_event_name, "Stop");
        assert_eq!(ev.session_id, "abc");
        assert_eq!(ev.project_name.as_deref(), Some("ClauCat"));
    }

    #[test]
    fn rejects_a_wrong_or_missing_token() {
        let b = body("Stop");
        assert_eq!(
            run(request(
                "POST",
                "/event",
                &[("Authorization", "Bearer nope")],
                &b
            )),
            Err(401)
        );
        assert_eq!(
            run(request(
                "POST",
                "/event",
                &[("Authorization", "s3cret")],
                &b
            )),
            Err(401)
        );
        assert_eq!(run(request("POST", "/event", &[], &b)), Err(401));
        assert_eq!(
            run(request(
                "POST",
                "/event",
                &[("Authorization", "Bearer s3cret0")],
                &b
            )),
            Err(401)
        );
    }

    #[test]
    fn rejects_a_body_over_64kb_without_reading_it() {
        let raw = format!(
            "POST /event HTTP/1.1\r\nAuthorization: Bearer s3cret\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        assert_eq!(run(raw.into_bytes()), Err(413));
        // Exactly 64 KB is accepted (trailing whitespace is valid JSON).
        let padded = format!(
            "{}{}",
            body("Stop"),
            " ".repeat(MAX_BODY - body("Stop").len())
        );
        assert!(run(ok_request(&padded)).is_ok());
    }

    #[test]
    fn rejects_unknown_events() {
        assert_eq!(run(ok_request(&body("PreToolUse"))), Err(422));
        assert_eq!(run(ok_request(&body("rm -rf"))), Err(422));
        for name in ALLOWED_HOOKS {
            assert!(run(ok_request(&body(name))).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_broken_json_and_bad_fields() {
        assert_eq!(run(ok_request("{not json")), Err(400));
        assert_eq!(run(ok_request(r#"{"hookEventName":"Stop"}"#)), Err(400)); // missing fields
        let empty_session =
            r#"{"eventId":"1","hookEventName":"Stop","sessionId":"","occurredAt":1}"#;
        assert_eq!(run(ok_request(empty_session)), Err(422));
        let long = format!(
            r#"{{"eventId":"1","hookEventName":"Stop","sessionId":"a","projectName":"{}","occurredAt":1}}"#,
            "x".repeat(201)
        );
        assert_eq!(run(ok_request(&long)), Err(422));
    }

    #[test]
    fn rejects_other_methods_paths_browsers_and_chunking() {
        let b = body("Stop");
        let auth = ("Authorization", "Bearer s3cret");
        assert_eq!(run(request("GET", "/event", &[auth], "")), Err(405));
        assert_eq!(run(request("POST", "/", &[auth], &b)), Err(404));
        assert_eq!(
            run(request(
                "POST",
                "/event",
                &[auth, ("Origin", "https://evil.example")],
                &b
            )),
            Err(403)
        );
        assert_eq!(
            run(request(
                "POST",
                "/event",
                &[auth, ("Transfer-Encoding", "chunked")],
                &b
            )),
            Err(400)
        );
        let no_len = b"POST /event HTTP/1.1\r\nAuthorization: Bearer s3cret\r\n\r\n".to_vec();
        assert_eq!(run(no_len), Err(411));
        assert_eq!(run(b"garbage".to_vec()), Err(400));
    }

    #[test]
    fn rejects_oversized_headers() {
        let raw = format!(
            "POST /event HTTP/1.1\r\nX-Pad: {}\r\n\r\n",
            "a".repeat(MAX_HEADER_BYTES)
        );
        assert_eq!(run(raw.into_bytes()), Err(400));
    }

    #[test]
    fn the_answer_is_one_complete_message() {
        assert_eq!(
            response(204),
            b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        assert_eq!(
            response(400),
            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        assert!(response(418).starts_with(b"HTTP/1.1 418 Error\r\n"));
    }

    #[test]
    fn tokens_are_256_bit_hex_and_unique() {
        let (a, b) = (new_token().unwrap(), new_token().unwrap());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

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

    #[test]
    fn serves_over_real_loopback_sockets() {
        let (tx, rx) = mpsc::channel();
        let port = start(TOKEN.into(), move |ev| tx.send(ev).unwrap()).unwrap();
        let send = |raw: Vec<u8>| -> String {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(&raw).unwrap();
            let mut resp = String::new();
            s.read_to_string(&mut resp).unwrap();
            resp
        };
        assert!(send(ok_request(&body("Stop"))).starts_with("HTTP/1.1 204"));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2))
                .unwrap()
                .hook_event_name,
            "Stop"
        );
        assert!(send(request(
            "POST",
            "/event",
            &[("Authorization", "Bearer x")],
            &body("Stop")
        ))
        .starts_with("HTTP/1.1 401"));
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_slow_drip_is_cut_off_at_the_request_deadline() {
        let port = start(TOKEN.into(), |_| {}).unwrap();
        let mut conn = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        conn.write_all(b"POST /event HTTP/1.1\r\n").unwrap();
        // One header byte every 100ms: each read finishes well inside a per-read timeout.
        let mut drip = conn.try_clone().unwrap();
        thread::spawn(move || {
            for _ in 0..40 {
                thread::sleep(Duration::from_millis(100));
                if drip.write_all(b"a").is_err() {
                    return;
                }
            }
        });
        let started = std::time::Instant::now();
        conn.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        let mut resp = String::new();
        let _ = conn.read_to_string(&mut resp);
        assert!(resp.starts_with("HTTP/1.1 400"), "{resp:?}");
        assert!(
            started.elapsed() < REQUEST_DEADLINE + Duration::from_millis(500),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn rejections_are_logged_in_bursts_and_the_rest_counted() {
        let mut log = RejectLog::default();
        let t0 = 1_000_000;
        let lines: Vec<_> = (0..20).flat_map(|i| log.on_reject(401, t0 + i)).collect();
        assert_eq!(lines.len(), REJECT_LOG_BURST as usize);
        assert!(lines.last().unwrap().contains("not logged"), "{lines:?}");
        // Next window: one line for what was dropped, then logging resumes.
        let next = log.on_reject(404, t0 + REJECT_LOG_WINDOW_MS);
        assert_eq!(next.len(), 2, "{next:?}");
        assert!(next[0].contains("15 more"), "{next:?}");
        assert!(next[1].contains("HTTP 404"), "{next:?}");
    }
}

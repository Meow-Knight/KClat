//! Loopback HTTP endpoint for the hook adapter (spec §8.3–8.4).
//!
//! - Binds 127.0.0.1 on a random port; a fresh 256-bit bearer token per app run.
//! - `{port, token}` goes to `%APPDATA%\Klat\runtime.json` (runtime_file.rs), deleted when the app quits.
//! - Accepts only `POST /event` with the right token, no `Origin` header (no browser pages), a body of at
//!   most 64 KB, valid JSON, and an allowlisted `hookEventName`. Everything else gets a 4xx and is dropped.
//! - Each request must arrive whole within `REQUEST_DEADLINE`, so nobody can hold a connection slot open by
//!   sending a byte now and then, and rejections are logged in short bursts, so a flood cannot rotate the
//!   real entries out of the capped log.

use klat_core::ratelimit::RejectLog;
use klat_core::time::UnixMs;
use klat_protocol::{HookEvent, EVENT_PATH, MAX_BODY};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_CONNECTIONS: usize = 16;
/// For writing the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(1);
/// The whole request (head and body) must arrive within this. The adapter sends it in one write and gives up
/// after 300ms itself; a per-read timeout alone let one byte a second keep a connection open for hours.
const REQUEST_DEADLINE: Duration = Duration::from_millis(500);
const MAX_FIELD: usize = 200;

pub fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Why a request was refused. Each answers with its own 4xx.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Not HTTP as the adapter sends it: a broken head, chunked body, short body, bad JSON. 400
    BadRequest,
    /// Missing or wrong bearer token. 401
    Unauthorized,
    /// Sent by a browser page (it has an `Origin`). 403
    Forbidden,
    /// Not `/event`. 404
    NotFound,
    /// Not POST. 405
    MethodNotAllowed,
    /// No `Content-Length`. 411
    LengthRequired,
    /// A body over 64 KB. 413
    PayloadTooLarge,
    /// Valid JSON, but a hook this version does not know, or fields out of bounds. 422
    Unprocessable,
}

impl Reject {
    pub const fn status(self) -> u16 {
        match self {
            Reject::BadRequest => 400,
            Reject::Unauthorized => 401,
            Reject::Forbidden => 403,
            Reject::NotFound => 404,
            Reject::MethodNotAllowed => 405,
            Reject::LengthRequired => 411,
            Reject::PayloadTooLarge => 413,
            Reject::Unprocessable => 422,
        }
    }
}

/// What every connection of one server shares.
struct Server<C, F> {
    token: String,
    clock: C,
    on_event: F,
    /// Rate limit for the rejection warnings of this server.
    rejects: Mutex<RejectLog>,
}

/// Starts the server thread and returns the bound port. `on_event` runs on a connection thread; `clock` gives
/// the time for the rejection log's rate limit.
pub fn start<C, F>(token: String, clock: C, on_event: F) -> io::Result<u16>
where
    C: Fn() -> UnixMs + Send + Sync + 'static,
    F: Fn(HookEvent) + Send + Sync + 'static,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let server = Arc::new(Server {
        token,
        clock,
        on_event,
        rejects: Mutex::new(RejectLog::new()),
    });
    let active = Arc::new(AtomicUsize::new(0));
    thread::Builder::new()
        .name("event-server".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if active.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                    continue; // dropping the stream closes it
                }
                active.fetch_add(1, Ordering::Relaxed);
                let (server, counter) = (server.clone(), active.clone());
                let spawned = thread::Builder::new()
                    .name("event-conn".into())
                    .spawn(move || {
                        let _ = serve(stream, &server);
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

fn serve<C, F>(stream: TcpStream, server: &Server<C, F>) -> io::Result<()>
where
    C: Fn() -> UnixMs,
    F: Fn(HookEvent),
{
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(Deadline {
        stream: stream.try_clone()?,
        until: Instant::now() + REQUEST_DEADLINE,
    });
    let status = match handle(&mut reader, &server.token) {
        Ok(ev) => {
            (server.on_event)(ev);
            204
        }
        Err(reject) => {
            let status = reject.status();
            let lines = match server.rejects.lock() {
                Ok(mut log) => log.on_reject(status, (server.clock)()),
                Err(_) => Vec::new(),
            };
            for line in lines {
                log::warn!("{line}");
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

/// Parses and validates one request.
pub fn handle(reader: &mut impl BufRead, token: &str) -> Result<HookEvent, Reject> {
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
            return Err(Reject::BadRequest);
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => {
                content_length = Some(value.parse::<usize>().map_err(|_| Reject::BadRequest)?)
            }
            "authorization" => authorization = Some(value.to_owned()),
            // Browsers always send Origin on cross-site POSTs; the hook adapter never does.
            "origin" => return Err(Reject::Forbidden),
            "transfer-encoding" => return Err(Reject::BadRequest),
            _ => {}
        }
    }

    if path != EVENT_PATH {
        return Err(Reject::NotFound);
    }
    if method != "POST" {
        return Err(Reject::MethodNotAllowed);
    }
    let presented = authorization
        .as_deref()
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq(presented.as_bytes(), token.as_bytes()) {
        return Err(Reject::Unauthorized);
    }
    let len = content_length.ok_or(Reject::LengthRequired)?;
    if len > MAX_BODY {
        return Err(Reject::PayloadTooLarge);
    }
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .map_err(|_| Reject::BadRequest)?;

    let ev: HookEvent = serde_json::from_slice(&body).map_err(|_| Reject::BadRequest)?;
    validate(&ev)?;
    Ok(ev)
}

fn validate(ev: &HookEvent) -> Result<(), Reject> {
    if ev.hook().is_none() {
        return Err(Reject::Unprocessable);
    }
    let too_long = |s: &str| s.chars().count() > MAX_FIELD;
    if ev.session_id.is_empty() || too_long(&ev.session_id) || too_long(&ev.event_id) {
        return Err(Reject::Unprocessable);
    }
    for field in [&ev.project_name, &ev.tool_name, &ev.notification_type]
        .into_iter()
        .flatten()
    {
        if too_long(field) {
            return Err(Reject::Unprocessable);
        }
    }
    Ok(())
}

/// Request line + headers, up to the blank line.
fn read_head(reader: &mut impl BufRead) -> Result<String, Reject> {
    let mut head = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take((MAX_HEADER_BYTES + 1 - head.len()) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| Reject::BadRequest)?;
        if n == 0 {
            return Err(Reject::BadRequest);
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
        head.extend_from_slice(&line);
        if head.len() > MAX_HEADER_BYTES {
            return Err(Reject::BadRequest);
        }
    }
    // str::lines() later splits on "\n" and drops a trailing "\r", so CRLF and bare LF both work.
    String::from_utf8(head).map_err(|_| Reject::BadRequest)
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
    use klat_protocol::HookName;
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

    /// The status a request gets, as the tests have always stated it.
    fn run(raw: Vec<u8>) -> Result<HookEvent, u16> {
        handle(&mut Cursor::new(raw), TOKEN).map_err(Reject::status)
    }

    fn clock() -> UnixMs {
        UnixMs(0)
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
        for name in HookName::ALL.map(HookName::as_str) {
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

    /// xorshift64*: a fixed seed gives the same requests on every run and every machine, without a dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Pieces of requests, valid and not, so the junk gets past the first checks often enough to reach the
    /// later ones. No piece is a whole valid body, so no request may be accepted.
    const PIECES: &[&[u8]] = &[
        b"POST",
        b"GET",
        b"post",
        b" /event",
        b" /",
        b" /event?x=1",
        b" HTTP/1.1",
        b"\r\n",
        b"\n",
        b"\r\n\r\n",
        b"Content-Length: ",
        b"content-length:",
        b"Authorization: Bearer ",
        b"Authorization: Bearer s3cre",
        b"Origin: x",
        b"Transfer-Encoding: chunked",
        b"X-Pad: ",
        b": ",
        b":",
        b"0",
        b"17",
        b"65536",
        b"65537",
        b"-1",
        b"99999999999999999999999",
        b"{",
        b"}",
        br#""hookEventName":"Stop""#,
        br#""sessionId":"""#,
        br#""eventId":"1","occurredAt":1"#,
        b"\xff\xfe",
        b"\0",
        "é漢".as_bytes(),
    ];

    #[test]
    fn junk_never_panics_and_always_gets_a_4xx() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut statuses = std::collections::BTreeSet::new();
        for _ in 0..2000 {
            // Two in three start like the adapter's requests, half of those with the right token, so the junk
            // also reaches the token, length and body checks.
            let mut raw = match rng.below(3) {
                0 => Vec::new(),
                1 => b"POST /event HTTP/1.1\r\n".to_vec(),
                _ => b"POST /event HTTP/1.1\r\nAuthorization: Bearer s3cret\r\n".to_vec(),
            };
            for _ in 0..rng.below(40) {
                if rng.below(4) == 0 {
                    // Raw bytes, sometimes a long run of them.
                    let len = if rng.below(10) == 0 {
                        rng.below(10_000)
                    } else {
                        rng.below(16)
                    };
                    raw.extend((0..len).map(|_| rng.next() as u8));
                } else {
                    raw.extend_from_slice(PIECES[rng.below(PIECES.len())]);
                }
            }
            let got = std::panic::catch_unwind(|| run(raw.clone()));
            let Ok(got) = got else {
                panic!("panicked on {:?}", String::from_utf8_lossy(&raw));
            };
            match got {
                Err(status) => {
                    assert!((400..500).contains(&status), "{status}");
                    statuses.insert(status);
                }
                Ok(ev) => panic!("accepted junk: {ev:?}"),
            }
        }
        // The junk reaches the later checks too (400, 401, 403, 404, 405, 411 with this seed).
        assert!(statuses.len() >= 6, "{statuses:?}");
    }

    #[test]
    fn each_rejection_has_its_own_status() {
        let all = [
            Reject::BadRequest,
            Reject::Unauthorized,
            Reject::Forbidden,
            Reject::NotFound,
            Reject::MethodNotAllowed,
            Reject::LengthRequired,
            Reject::PayloadTooLarge,
            Reject::Unprocessable,
        ];
        let statuses: Vec<u16> = all.iter().map(|r| r.status()).collect();
        assert_eq!(statuses, [400, 401, 403, 404, 405, 411, 413, 422]);
        for r in all {
            assert_ne!(reason(r.status()), "Error", "{r:?}");
        }
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
        assert!(klat_protocol::is_valid_token(&a));
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn serves_over_real_loopback_sockets() {
        let (tx, rx) = mpsc::channel();
        let port = start(TOKEN.into(), clock, move |ev| tx.send(ev).unwrap()).unwrap();
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
        let port = start(TOKEN.into(), clock, |_| {}).unwrap();
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
}

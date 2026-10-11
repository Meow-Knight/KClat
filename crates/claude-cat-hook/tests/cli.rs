//! Runs the real binary: it must always exit 0, print nothing, and be quick when the app is not running.

use std::io::Write;
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const STOP: &str = r#"{"session_id":"abc","cwd":"D:\\work\\ClauCat","hook_event_name":"Stop"}"#;
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Run {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    elapsed: Duration,
}

fn run_hook(appdata: &Path, stdin: &str, arg: &str) -> Run {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_claude-cat-hook"))
        .arg(arg)
        .env("APPDATA", appdata)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Run {
        code: out.status.code(),
        stdout: out.stdout,
        stderr: out.stderr,
        elapsed: started.elapsed(),
    }
}

fn write_runtime(appdata: &Path, port: u16) {
    let dir = appdata.join("ClaudeCat");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("runtime.json"),
        format!(r#"{{"port":{port},"token":"{TOKEN}","pid":1}}"#),
    )
    .unwrap();
}

fn assert_silent_success(r: &Run) {
    assert_eq!(r.code, Some(0));
    assert!(
        r.stdout.is_empty(),
        "stdout: {:?}",
        String::from_utf8_lossy(&r.stdout)
    );
    assert!(
        r.stderr.is_empty(),
        "stderr: {:?}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn app_not_running_no_runtime_file() {
    let dir = tempfile::tempdir().unwrap();
    let r = run_hook(dir.path(), STOP, "Stop");
    assert_silent_success(&r);
    assert!(r.elapsed < Duration::from_millis(300), "{:?}", r.elapsed);
}

#[test]
fn app_quit_but_stale_runtime_file_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    // A port nothing listens on.
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    write_runtime(dir.path(), port);
    let r = run_hook(dir.path(), STOP, "Stop");
    assert_silent_success(&r);
    assert!(r.elapsed < Duration::from_millis(300), "{:?}", r.elapsed);
}

#[test]
fn app_hangs_hook_still_exits_within_budget() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    write_runtime(dir.path(), listener.local_addr().unwrap().port());
    let _hold = std::thread::spawn(move || {
        let conn = listener.accept();
        std::thread::sleep(Duration::from_secs(3));
        drop(conn);
    });
    let r = run_hook(dir.path(), STOP, "Stop");
    assert_silent_success(&r);
    // 300ms budget plus process start-up.
    assert!(r.elapsed < Duration::from_millis(450), "{:?}", r.elapsed);
}

#[test]
fn garbage_and_empty_stdin_are_fine() {
    let dir = tempfile::tempdir().unwrap();
    for input in ["", "not json", "{", "[]", "{\"session_id\": 5}"] {
        assert_silent_success(&run_hook(dir.path(), input, "Stop"));
    }
}

#[test]
fn delivers_to_a_listening_app() {
    let dir = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    write_runtime(dir.path(), listener.local_addr().unwrap().port());
    let server = std::thread::spawn(move || {
        use std::io::Read;
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        // Read until the JSON body is complete, however the request was segmented.
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        while !data.ends_with(b"}") {
            let n = s.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
        }
        s.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        String::from_utf8_lossy(&data).into_owned()
    });
    let r = run_hook(dir.path(), STOP, "Stop");
    assert_silent_success(&r);
    let req = server.join().unwrap();
    assert!(req.contains(&format!("Authorization: Bearer {TOKEN}")));
    assert!(req.contains(r#""hookEventName":"Stop""#));
    assert!(req.contains(r#""projectName":"ClauCat""#));
    assert!(!req.contains("D:\\\\work"));
}

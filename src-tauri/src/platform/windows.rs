//! Windows side of dragging the cat with Space: a low-level keyboard hook (WH_KEYBOARD_LL). Contract in mod.rs.

use super::{Msg, Permission, GRABBING, ZONE};
use klat_core::geometry::{Physical, Point};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RBUTTON, VK_RWIN, VK_SHIFT, VK_SPACE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetMessageW, GetSystemMetrics, SetWindowsHookExW, HC_ACTION,
    KBDLLHOOKSTRUCT, MSG, SM_SWAPBUTTON, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

/// Where the hook reports grabs. A static (docs/ARCHITECTURE.md §7.2): WH_KEYBOARD_LL calls `hook` with no user
/// data.
static TX: OnceLock<Sender<Msg>> = OnceLock::new();

/// Tags our replayed tap ("KLAT"), so the hook lets it through. Filtering on LLKHF_INJECTED instead would also
/// drop the Space that remote-control apps (TeamViewer, AnyDesk, UltraViewer...) send with SendInput.
const REPLAY_MARK: usize = 0x4B4C_4154;

/// The path with its folders in 8.3 short names when they have spaces (`C:\PROGRA~1\Klat\klat-hook.exe`), so the
/// Codex hooks can name it without quotes (klat_core::agent_hooks). The file name stays long: it is how Install
/// recognises its own entries. The path itself when nothing needs shortening or the volume keeps no short names.
pub fn cmd_safe_path(path: &Path) -> PathBuf {
    let has_space = |p: &Path| {
        p.as_os_str()
            .to_string_lossy()
            .contains(char::is_whitespace)
    };
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    if !has_space(dir) || has_space(Path::new(name)) {
        return path.to_path_buf();
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and `buf` holds `buf.len()` u16s; the call writes at most that many.
    let len =
        unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    if len == 0 || len >= buf.len() {
        return path.to_path_buf();
    }
    let short = PathBuf::from(std::ffi::OsString::from_wide(&buf[..len]));
    if has_space(&short) {
        return path.to_path_buf();
    }
    short.join(name)
}

fn held(vk: u16) -> bool {
    // SAFETY: plain Win32 call with no pointers.
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

pub fn cursor() -> Option<Point<Physical, i32>> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: `p` is a valid, writable POINT.
    (unsafe { GetCursorPos(&mut p) } != 0).then_some(Point::new(p.x, p.y))
}

/// The physical button that acts as "left" (swapped for left-handed setups).
pub fn primary_button_down() -> bool {
    // SAFETY: plain Win32 call with no pointers.
    let swapped = unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0;
    held(if swapped { VK_RBUTTON } else { VK_LBUTTON })
}

pub fn replay_space() {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_SPACE,
                wScan: 0x39,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: REPLAY_MARK,
            },
        },
    };
    let inputs = [key(0), key(KEYEVENTF_KEYUP)];
    // SAFETY: `inputs` is a valid array of INPUT structs of the size passed.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
}

fn pointer_in_zone() -> bool {
    let Some(p) = cursor() else { return false };
    // Never block inside the hook: if the poller is writing right now, let this press through.
    ZONE.try_read()
        .map(|z| z.iter().any(|r| r.contains(p)))
        .unwrap_or(false)
}

fn modifier_held() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
        .into_iter()
        .any(held)
}

/// Runs for every key press in the session, so it only touches atomics and returns at once.
unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        // Our own replayed tap passes; injected presses from other apps (remote control) are handled.
        if kb.vkCode == VK_SPACE as u32 && kb.dwExtraInfo != REPLAY_MARK {
            let msg = wparam as u32;
            let send = |m| {
                if let Some(tx) = TX.get() {
                    let _ = tx.send(m);
                }
            };
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                if GRABBING.load(Ordering::Relaxed) {
                    return 1; // auto-repeat while grabbing
                }
                if !modifier_held() && pointer_in_zone() {
                    GRABBING.store(true, Ordering::Relaxed);
                    send(Msg::GrabStarted);
                    return 1;
                }
            } else if (msg == WM_KEYUP || msg == WM_SYSKEYUP)
                && GRABBING.swap(false, Ordering::Relaxed)
            {
                send(Msg::GrabEnded);
                return 1;
            }
        }
    }
    // SAFETY: forwarding the unchanged arguments to the next hook.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

pub fn permission() -> Permission {
    Permission::NotNeeded
}

pub fn request_permission() {}

pub fn install_hook(
    _app: tauri::AppHandle,
    tx: Sender<Msg>,
    _on_permission: impl Fn(Permission) + Send + 'static,
) {
    let _ = TX.set(tx);
    std::thread::Builder::new()
        .name("space-hook".into())
        .spawn(|| {
            // SAFETY: a low-level hook needs a message loop on the thread that installed it; this thread
            // does nothing else and lives as long as the app.
            unsafe {
                let handle = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(hook),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                );
                if handle.is_null() {
                    log::warn!("could not install the Space hook; dragging the cat is off");
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            }
        })
        .expect("failed to start the Space hook thread");
}

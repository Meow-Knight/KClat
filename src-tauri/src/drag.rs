//! Moving the cat: hold Space with the pointer over the cat, press the mouse button and move; releasing Space
//! drops it there, and the spot is saved.
//!
//! The overlay never takes focus (WS_EX_NOACTIVATE), so it never sees the keyboard. A low-level keyboard hook
//! watches Space, and nothing else:
//! - Space pressed while the pointer is over the cat (or its bubble or button), with no modifier held: the press
//!   is kept from the focused app and "grab" starts. A press anywhere else passes through untouched.
//! - Space released: grab ends. If the mouse was never pressed (a plain tap while the pointer happened to rest
//!   on the cat), the tap is replayed to the focused app, so typing is not lost.
//!
//! While grabbing, pressing the primary mouse button starts the drag: the window follows the cursor (~120Hz)
//! until Space is released. The cat is kept inside the work area of the monitor under the cursor.
//! The overlay is told through `cat://grab` ("idle" | "armed" | "dragging"), so it can ignore the click.

use crate::log;
use crate::overlay;
use crate::state::{self, AppState};
use klat_core::geometry::{clamp_window, in_rect, to_px, PxRect, Rect, WindowPos};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_GRAB: &str = "cat://grab";
/// macOS: the Accessibility permission the Space hook needs changed (payload: [`Permission`]).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const EVENT_PERMISSION: &str = "cat://drag-permission";
const DRAG_PERIOD: Duration = Duration::from_millis(8);

/// Whether the Space hook may run. Only macOS asks (Accessibility); elsewhere it is not needed.
// Each platform builds only some of these: macOS never says NotNeeded, the others say nothing else.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    NotNeeded,
    Granted,
    Missing,
}

pub fn permission() -> Permission {
    platform::permission()
}

/// macOS: shows the system prompt and opens Privacy & Security > Accessibility. Elsewhere: nothing.
pub fn request_permission() {
    platform::request_permission();
}

/// Where Space starts a grab: the overlay's clickable regions in physical screen px (kept by the cursor poller).
static ZONE: RwLock<Vec<PxRect>> = RwLock::new(Vec::new());
/// Space is held and kept from the focused app.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
static GRABBING: AtomicBool = AtomicBool::new(false);
/// The window is following the cursor.
static DRAGGING: AtomicBool = AtomicBool::new(false);

pub fn dragging() -> bool {
    DRAGGING.load(Ordering::Relaxed)
}

/// Called by the cursor poller with the overlay's geometry and its clickable regions (logical px).
pub fn set_zone(origin: (f64, f64), scale: f64, rects: &[Rect]) {
    let zone: Vec<PxRect> = rects.iter().map(|r| to_px(origin, scale, r)).collect();
    if let Ok(mut z) = ZONE.write() {
        if *z != zone {
            *z = zone;
        }
    }
}

/// Sent by the platform hook.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
enum Msg {
    GrabStarted,
    GrabEnded,
}

pub fn start(app: AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let worker = app.clone();
    std::thread::Builder::new()
        .name("drag".into())
        .spawn(move || {
            while let Ok(msg) = rx.recv() {
                if let Msg::GrabStarted = msg {
                    run_grab(&worker, &rx);
                }
            }
        })
        .expect("failed to start the drag worker");
    platform::install_hook(app, tx);
}

/// One Space hold: wait for the mouse button, follow the cursor, save on release.
fn run_grab(app: &AppHandle, rx: &std::sync::mpsc::Receiver<Msg>) {
    let _ = app.emit(EVENT_GRAB, "armed");
    let mut pressed = false;
    // Logical px from the window's top-left to the grab point, so a DPI change mid-drag keeps the cat in hand.
    let mut grip: Option<(f64, f64)> = None;
    let win = overlay::get(app);
    loop {
        match rx.recv_timeout(DRAG_PERIOD) {
            Ok(Msg::GrabEnded) => break,
            Ok(Msg::GrabStarted) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let Some(win) = win.as_ref() else { continue };
        let Some(cursor) = platform::cursor() else {
            continue;
        };
        if grip.is_none() && platform::primary_button_down() {
            pressed = true;
            if let (Ok(origin), Ok(scale)) = (win.outer_position(), win.scale_factor()) {
                grip = Some((
                    (cursor.0 - origin.x) as f64 / scale,
                    (cursor.1 - origin.y) as f64 / scale,
                ));
                DRAGGING.store(true, Ordering::Relaxed);
                let _ = app.emit(EVENT_GRAB, "dragging");
            }
        }
        if let Some((gx, gy)) = grip {
            let scale = win.scale_factor().unwrap_or(1.0);
            let mut pos = (
                cursor.0 - (gx * scale).round() as i32,
                cursor.1 - (gy * scale).round() as i32,
            );
            let cat = app
                .state::<AppState>()
                .hitboxes()
                .first()
                .map(|r| to_px((0.0, 0.0), scale, r));
            let area = app
                .monitor_from_point(cursor.0 as f64, cursor.1 as f64)
                .ok()
                .flatten()
                .map(|m| {
                    let a = m.work_area();
                    (
                        a.position.x,
                        a.position.y,
                        a.size.width as i32,
                        a.size.height as i32,
                    )
                });
            if let (Some(cat), Some(area)) = (cat, area) {
                pos = clamp_window(pos, cat, area);
            }
            let _ = win.set_position(tauri::PhysicalPosition::new(pos.0, pos.1));
        }
    }

    let moved = DRAGGING.swap(false, Ordering::Relaxed);
    let _ = app.emit(EVENT_GRAB, "idle");
    if moved {
        if let Some(Ok(p)) = win.as_ref().map(|w| w.outer_position()) {
            log::info(format!("cat moved to {},{}", p.x, p.y));
            state::update_settings(app, |s| s.position = Some(WindowPos { x: p.x, y: p.y }));
        }
    } else if !pressed {
        // A plain tap while the pointer rested on the cat: give it back to the app you were typing in.
        platform::replay_space();
    }
}

#[cfg(windows)]
mod platform {
    use super::{in_rect, Msg, Permission, GRABBING, ZONE};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::Sender;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
        VK_CONTROL, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RBUTTON, VK_RWIN, VK_SHIFT, VK_SPACE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetCursorPos, GetMessageW, GetSystemMetrics, SetWindowsHookExW, HC_ACTION,
        KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, SM_SWAPBUTTON, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static TX: OnceLock<Sender<Msg>> = OnceLock::new();

    fn held(vk: u16) -> bool {
        // SAFETY: plain Win32 call with no pointers.
        unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
    }

    pub fn cursor() -> Option<(i32, i32)> {
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: `p` is a valid, writable POINT.
        (unsafe { GetCursorPos(&mut p) } != 0).then_some((p.x, p.y))
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
                    dwExtraInfo: 0,
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
            .map(|z| z.iter().any(|r| in_rect(p, *r)))
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
            // Injected presses pass, including our own replayed tap.
            if kb.vkCode == VK_SPACE as u32 && kb.flags & LLKHF_INJECTED == 0 {
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

    pub fn install_hook(_app: tauri::AppHandle, tx: Sender<Msg>) {
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
                        crate::log::warn(
                            "could not install the Space hook; dragging the cat is off",
                        );
                        return;
                    }
                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
                }
            })
            .expect("failed to start the Space hook thread");
    }
}

#[cfg(target_os = "macos")]
#[path = "drag_macos.rs"]
mod platform;

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::{Msg, Permission};
    use std::sync::mpsc::Sender;

    pub fn cursor() -> Option<(i32, i32)> {
        None
    }
    pub fn primary_button_down() -> bool {
        false
    }
    pub fn replay_space() {}
    pub fn permission() -> Permission {
        Permission::NotNeeded
    }
    pub fn request_permission() {}
    pub fn install_hook(_app: tauri::AppHandle, _tx: Sender<Msg>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_uses_the_names_the_settings_window_expects() {
        let names: Vec<serde_json::Value> = [
            Permission::NotNeeded,
            Permission::Granted,
            Permission::Missing,
        ]
        .iter()
        .map(|p| serde_json::to_value(p).unwrap())
        .collect();
        assert_eq!(names, ["not-needed", "granted", "missing"]);
    }

    #[test]
    fn this_platform_reports_its_permission() {
        let expected = if cfg!(target_os = "macos") {
            [Permission::Granted, Permission::Missing].contains(&permission())
        } else {
            permission() == Permission::NotNeeded
        };
        assert!(expected);
    }
}

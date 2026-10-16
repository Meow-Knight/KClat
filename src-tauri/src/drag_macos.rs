//! macOS side of dragging the cat with Space (docs/specs/settings-i18n-macos.md §5.1). Same contract as the
//! Windows hook in drag.rs: Space pressed over the cat with no modifier starts a grab and is kept from the
//! focused app; its release ends it; a plain tap is replayed.
//!
//! A CGEventTap that can drop key events needs the Accessibility permission. Without it the tap is not created,
//! the rest of the app works, and Settings shows a button that asks for it. The hook thread checks again every
//! 2s and installs the tap as soon as the permission is there, without a restart.

use super::{Msg, Permission, EVENT_PERMISSION, GRABBING, ZONE};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CGMouseButton, CallbackResult, EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use klat_core::geometry::{Physical, Point};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// kVK_Space.
const SPACE: i64 = 49;
/// Marks the Space taps we replay, so the tap lets them through.
const OUR_EVENT: i64 = 0x4b4c_4154; // "KLAT"
const RECHECK: Duration = Duration::from_secs(2);
const PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

static APP: OnceLock<AppHandle> = OnceLock::new();
static TX: OnceLock<Sender<Msg>> = OnceLock::new();
/// The tap's mach port, to switch it back on when macOS disables it (slow callback, secure input).
static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
/// 0 = not known yet, 1 = granted, 2 = missing.
static PERMISSION: AtomicU8 = AtomicU8::new(0);

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceButtonState(state: CGEventSourceStateID, button: CGMouseButton) -> bool;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
}

/// Cursor in the same physical px as the overlay's position (see cursor.rs), not CoreGraphics points.
pub fn cursor() -> Option<Point<Physical, i32>> {
    let p = APP.get()?.cursor_position().ok()?;
    Some(Point::new(p.x.round() as i32, p.y.round() as i32))
}

/// macOS already maps "left" to the primary button for left-handed setups.
pub fn primary_button_down() -> bool {
    // SAFETY: plain CoreGraphics call with value arguments.
    unsafe {
        CGEventSourceButtonState(
            CGEventSourceStateID::CombinedSessionState,
            CGMouseButton::Left,
        )
    }
}

pub fn replay_space() {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else {
        return;
    };
    for down in [true, false] {
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), SPACE as u16, down) {
            event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, OUR_EVENT);
            event.post(CGEventTapLocation::Session);
        }
    }
}

pub fn permission() -> Permission {
    match PERMISSION.load(Ordering::Relaxed) {
        1 => Permission::Granted,
        2 => Permission::Missing,
        // SAFETY: plain ApplicationServices call.
        _ if unsafe { AXIsProcessTrusted() } => Permission::Granted,
        _ => Permission::Missing,
    }
}

/// Shows macOS's own prompt (once per app) and opens Privacy & Security > Accessibility.
pub fn request_permission() {
    // SAFETY: kAXTrustedCheckOptionPrompt is a constant CFString owned by the framework.
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options =
        CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    // SAFETY: `options` is a valid CFDictionary for the duration of the call.
    let _ = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
    let _ = std::process::Command::new("open").arg(PANE).spawn();
}

fn set_permission(granted: bool) {
    let value = if granted { 1 } else { 2 };
    if PERMISSION.swap(value, Ordering::Relaxed) != value {
        if let Some(app) = APP.get() {
            let _ = app.emit(EVENT_PERMISSION, permission());
        }
    }
}

fn pointer_in_zone() -> bool {
    let Some(p) = cursor() else { return false };
    ZONE.try_read()
        .map(|z| z.iter().any(|r| r.contains(p)))
        .unwrap_or(false)
}

fn modifier_held(event: &CGEvent) -> bool {
    event.get_flags().intersects(
        CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand,
    )
}

fn send(msg: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(msg);
    }
}

/// Runs for every key press in the session: keep it short.
fn on_event(kind: CGEventType, event: &CGEvent) -> CallbackResult {
    match kind {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let port = TAP_PORT.load(Ordering::Relaxed);
            if !port.is_null() {
                // SAFETY: the port belongs to the tap, which lives for the rest of the app.
                unsafe { CGEventTapEnable(port, true) };
            }
            return CallbackResult::Keep;
        }
        CGEventType::KeyDown | CGEventType::KeyUp => {}
        _ => return CallbackResult::Keep,
    }
    if event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) != SPACE
        || event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == OUR_EVENT
    {
        return CallbackResult::Keep;
    }
    if matches!(kind, CGEventType::KeyDown) {
        if GRABBING.load(Ordering::Relaxed) {
            return CallbackResult::Drop; // auto-repeat while grabbing
        }
        if event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) == 0
            && !modifier_held(event)
            && pointer_in_zone()
        {
            GRABBING.store(true, Ordering::Relaxed);
            send(Msg::GrabStarted);
            return CallbackResult::Drop;
        }
    } else if GRABBING.swap(false, Ordering::Relaxed) {
        send(Msg::GrabEnded);
        return CallbackResult::Drop;
    }
    CallbackResult::Keep
}

pub fn install_hook(app: AppHandle, tx: Sender<Msg>) {
    let _ = APP.set(app);
    let _ = TX.set(tx);
    std::thread::Builder::new()
        .name("space-hook".into())
        .spawn(|| {
            // SAFETY: plain ApplicationServices call.
            while !unsafe { AXIsProcessTrusted() } {
                set_permission(false);
                std::thread::sleep(RECHECK);
            }
            set_permission(true);
            let tap = match CGEventTap::new(
                CGEventTapLocation::Session,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![CGEventType::KeyDown, CGEventType::KeyUp],
                |_proxy, kind, event| on_event(kind, event),
            ) {
                Ok(tap) => tap,
                Err(()) => {
                    log::warn!("could not create the Space event tap; dragging the cat is off");
                    return;
                }
            };
            let Ok(source) = tap.mach_port().create_runloop_source(0) else {
                log::warn!("could not attach the Space event tap; dragging the cat is off");
                return;
            };
            TAP_PORT.store(
                tap.mach_port().as_concrete_TypeRef() as *mut c_void,
                Ordering::Relaxed,
            );
            // SAFETY: kCFRunLoopCommonModes is a constant owned by CoreFoundation.
            CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
            tap.enable();
            log::info!("Space event tap installed");
            // The tap lives as long as this thread's run loop, which is the rest of the app.
            CFRunLoop::run_current();
            drop(tap);
        })
        .expect("failed to start the Space hook thread");
}

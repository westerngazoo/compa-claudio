//! Global cursor position — used by the click-through poller to decide when
//! the transparent window should catch clicks vs. let them pass through.
//!
//! macOS gives no mouse events to a click-through window, so we can't rely on
//! the webview; we query the OS cursor position directly instead.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::ptr;

    type CGEventRef = *const c_void;
    type CGEventSourceRef = *const c_void;

    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
        fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    /// Current cursor position in global display points (top-left origin),
    /// which lines up with Tauri's logical window coordinates.
    pub fn cursor_position() -> Option<(f64, f64)> {
        unsafe {
            // CGEventCreate(NULL) snapshots the current input state.
            let event = CGEventCreate(ptr::null());
            if event.is_null() {
                return None;
            }
            let p = CGEventGetLocation(event);
            CFRelease(event);
            Some((p.x, p.y))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn cursor_position() -> Option<(f64, f64)> {
        None
    }
}

pub use imp::cursor_position;

/// Highest NSWindow level among on-screen windows owned by the given pids
/// (regular GUI apps), excluding our own. Lets Claudio sit one notch above
/// whatever the tallest app window is (e.g. Ghostty's non-native fullscreen)
/// instead of hardcoding a level and hoping.
///
/// Uses CGWindowListCopyWindowInfo — pid + layer are readable without any
/// special permission (only window *names* need Screen Recording).
#[cfg(target_os = "macos")]
pub fn max_app_window_level(exclude_pid: i32, candidate_pids: &[i32]) -> Option<i64> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use std::collections::HashSet;
    use std::ffi::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: isize, value_ptr: *mut c_void) -> u8;
        fn CFRelease(cf: *const c_void);
    }
    const ON_SCREEN_ONLY: u32 = 1 << 0;
    const SINT64: isize = 4; // kCFNumberSInt64Type

    let allowed: HashSet<i32> = candidate_pids
        .iter()
        .copied()
        .filter(|&p| p != exclude_pid)
        .collect();

    unsafe {
        let arr = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY, 0);
        if arr.is_null() {
            return None;
        }

        // The kCGWindow* constants' values are literally their own names.
        let pid_key = CFString::new("kCGWindowOwnerPID");
        let layer_key = CFString::new("kCGWindowLayer");
        let pid_key_ref = pid_key.as_concrete_TypeRef() as *const c_void;
        let layer_key_ref = layer_key.as_concrete_TypeRef() as *const c_void;

        let mut max_level: Option<i64> = None;
        for i in 0..CFArrayGetCount(arr) {
            let dict = CFArrayGetValueAtIndex(arr, i);
            if dict.is_null() {
                continue;
            }
            let pid_num = CFDictionaryGetValue(dict, pid_key_ref);
            let layer_num = CFDictionaryGetValue(dict, layer_key_ref);
            if pid_num.is_null() || layer_num.is_null() {
                continue;
            }
            let mut pid: i64 = 0;
            let mut layer: i64 = 0;
            if CFNumberGetValue(pid_num, SINT64, &mut pid as *mut i64 as *mut c_void) == 0 {
                continue;
            }
            if CFNumberGetValue(layer_num, SINT64, &mut layer as *mut i64 as *mut c_void) == 0 {
                continue;
            }
            if !allowed.contains(&(pid as i32)) {
                continue;
            }
            max_level = Some(max_level.map_or(layer, |m| m.max(layer)));
        }
        CFRelease(arr);
        max_level
    }
}

#[cfg(not(target_os = "macos"))]
pub fn max_app_window_level(_exclude_pid: i32, _candidate_pids: &[i32]) -> Option<i64> {
    None
}

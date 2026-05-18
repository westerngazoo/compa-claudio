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

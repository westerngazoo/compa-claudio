//! macOS Accessibility (AX) integration.
//!
//! Manual FFI to the four AX functions we need, with `core-foundation` handling
//! CFType refcounts. Memory rules per Apple's "Create Rule":
//! - `Create`/`Copy` named functions return +1; we must `CFRelease` (or wrap with create_rule)
//! - other returns are +0; use `get_rule`
//! - the global constant `kAXTrustedCheckOptionPrompt` is +0 → get_rule

use crate::backends::ChatContext;
use super::AccessibilityStatus;

use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};

use std::ffi::c_void;

// ---- Raw FFI ----

type AXError = i32;
type AXUIElementRef = *const c_void;

const AX_ERROR_SUCCESS: AXError = 0;
const MAX_TEXT_CHARS: usize = 4000;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> u8;

    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

// ---- RAII wrapper for AXUIElement refs we own (+1) ----

struct AxElement(AXUIElementRef);

impl AxElement {
    fn as_ref(&self) -> AXUIElementRef {
        self.0
    }
}

impl Drop for AxElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as CFTypeRef) }
        }
    }
}

// ---- Attribute helpers ----

fn copy_attr_raw(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
    if element.is_null() {
        return None;
    }
    let key = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, key.as_concrete_TypeRef(), &mut value)
    };
    if err == AX_ERROR_SUCCESS && !value.is_null() {
        Some(value)
    } else {
        None
    }
}

fn copy_attr_string(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let raw = copy_attr_raw(element, attribute)?;
    // Verify it's actually a CFString before wrapping — defensive against
    // attribute-type mismatches (rare but possible across host apps).
    unsafe {
        let s = CFString::wrap_under_create_rule(raw as CFStringRef);
        Some(s.to_string())
    }
}

fn copy_attr_element(element: AXUIElementRef, attribute: &str) -> Option<AxElement> {
    let raw = copy_attr_raw(element, attribute)?;
    Some(AxElement(raw as AXUIElementRef))
}

fn element_pid(element: AXUIElementRef) -> Option<i32> {
    if element.is_null() {
        return None;
    }
    let mut pid: i32 = 0;
    let err = unsafe { AXUIElementGetPid(element, &mut pid) };
    if err == AX_ERROR_SUCCESS {
        Some(pid)
    } else {
        None
    }
}

fn truncate(mut s: String) -> String {
    if s.chars().count() > MAX_TEXT_CHARS {
        let taken: String = s.chars().take(MAX_TEXT_CHARS).collect();
        s = format!("{}\n…(truncated)", taken);
    }
    s
}

// ---- Public API ----

pub fn check_accessibility(prompt: bool) -> AccessibilityStatus {
    let trusted = unsafe {
        if prompt {
            // Build { kAXTrustedCheckOptionPrompt: kCFBooleanTrue }
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let value = CFBoolean::true_value();
            let pairs: Vec<(CFString, CFType)> = vec![(key, value.as_CFType())];
            let dict = CFDictionary::from_CFType_pairs(&pairs);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as CFTypeRef)
        } else {
            AXIsProcessTrusted()
        }
    };
    if trusted != 0 {
        AccessibilityStatus::Trusted
    } else {
        AccessibilityStatus::NotTrusted
    }
}

pub fn read_focused_context() -> ChatContext {
    let mut ctx = ChatContext::default();

    // No permission → return empty, the mascot stays blindfolded politely.
    if !matches!(check_accessibility(false), AccessibilityStatus::Trusted) {
        return ctx;
    }

    let system_raw = unsafe { AXUIElementCreateSystemWide() };
    if system_raw.is_null() {
        return ctx;
    }
    let system = AxElement(system_raw);

    // Walk: system → focused app → focused UI element. App name comes from
    // the focused app element's AXTitle. Window title from AXFocusedWindow.AXTitle.
    let focused_app = match copy_attr_element(system.as_ref(), "AXFocusedApplication") {
        Some(a) => a,
        None => return ctx,
    };

    // Skip when WE are the focused app — otherwise opening chat would echo
    // back our own UI text instead of whatever the user was studying.
    let our_pid = std::process::id() as i32;
    if let Some(pid) = element_pid(focused_app.as_ref()) {
        if pid == our_pid {
            return ctx;
        }
    }

    ctx.focused_app = copy_attr_string(focused_app.as_ref(), "AXTitle");

    // Focused window title → use as a breadcrumb when there's no good selection
    let focused_window = copy_attr_element(focused_app.as_ref(), "AXFocusedWindow");
    let window_title = focused_window
        .as_ref()
        .and_then(|w| copy_attr_string(w.as_ref(), "AXTitle"));

    // Focused element is where the cursor / selection lives. Fall back to the
    // window itself if no element is focused (e.g. read-only PDF viewers).
    let focused_element = copy_attr_element(focused_app.as_ref(), "AXFocusedUIElement")
        .or(focused_window);

    if let Some(elem) = focused_element {
        // Selection wins — it's the most precise signal of "the user's looking
        // at this exact thing."
        if let Some(sel) = copy_attr_string(elem.as_ref(), "AXSelectedText") {
            let trimmed = sel.trim();
            if !trimmed.is_empty() {
                ctx.selection = Some(truncate(trimmed.to_string()));
            }
        }

        // AXValue holds the full text of text-bearing elements (editors,
        // text fields, source-code areas). Bigger blob, lower precision.
        if let Some(val) = copy_attr_string(elem.as_ref(), "AXValue") {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                ctx.focused_text = Some(truncate(trimmed.to_string()));
            }
        }
    }

    // Decorate the app label with window title when we have it — gives the
    // model more breadcrumbs to reason from (e.g. "VS Code — main.rs").
    if let (Some(app), Some(title)) = (ctx.focused_app.as_deref(), window_title.as_deref()) {
        if !title.is_empty() && title != app {
            ctx.focused_app = Some(format!("{app} — {title}"));
        }
    } else if ctx.focused_app.is_none() {
        ctx.focused_app = window_title;
    }

    ctx
}

use crate::backends::ChatContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // NotApplicable only constructed on non-macOS targets
pub enum AccessibilityStatus {
    Trusted,
    NotTrusted,
    NotApplicable,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
use stub as imp;

/// Read the currently focused application's text + selection.
/// Returns an empty `ChatContext` if accessibility is denied or we hit any error —
/// the mascot stays blindfolded rather than the whole app falling over.
pub fn read_focused_context() -> ChatContext {
    imp::read_focused_context()
}

/// Read context from a specific app by pid (the user has targeted this app
/// explicitly via the "Look at…" menu).
pub fn read_context_for_pid(pid: i32) -> ChatContext {
    imp::read_context_for_pid(pid)
}

/// Check whether the process has been granted Accessibility permission.
/// When `prompt` is true and not yet granted, macOS opens its trust prompt.
pub fn check_accessibility(prompt: bool) -> AccessibilityStatus {
    imp::check_accessibility(prompt)
}

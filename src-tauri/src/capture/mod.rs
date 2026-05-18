//! Screen capture — an on-demand *capability* (not a push sensor).
//!
//! Claudio captures the screen only when explicitly asked. The result is a
//! base64-encoded PNG, attached to a chat message for a vision-capable backend
//! to look at. Needs the macOS Screen Recording permission, which is separate
//! from Accessibility.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // NotApplicable only constructed on non-macOS targets
pub enum ScreenPermission {
    Granted,
    Denied,
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

/// Check Screen Recording permission. When `prompt` is true and not granted,
/// macOS shows its permission prompt.
pub fn check_screen_permission(prompt: bool) -> ScreenPermission {
    imp::check(prompt)
}

/// Capture the main display, returning a base64-encoded PNG.
/// Blocking — call from a blocking context (e.g. `spawn_blocking`).
pub fn capture_screen() -> Result<String, String> {
    imp::capture()
}

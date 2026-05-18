//! macOS screen capture.
//!
//! Permission via CoreGraphics' screen-capture-access APIs. The capture itself
//! shells out to Apple's built-in `screencapture` (no FFI, no extra deps) and
//! downscales with `sips` so the image is a friendly size for vision models.

use base64::Engine;
use std::process::Command;

use super::ScreenPermission;

/// Longest-edge size to downscale captures to — a good balance for vision
/// models (Claude recommends ~1568px max).
const MAX_EDGE: &str = "1568";

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

pub fn check(prompt: bool) -> ScreenPermission {
    let granted = unsafe {
        if CGPreflightScreenCaptureAccess() {
            true
        } else if prompt {
            // Shows the system prompt. Returns the (still-false) current state;
            // the user must grant in System Settings and relaunch.
            CGRequestScreenCaptureAccess()
        } else {
            false
        }
    };
    if granted {
        ScreenPermission::Granted
    } else {
        ScreenPermission::Denied
    }
}

pub fn capture() -> Result<String, String> {
    if !unsafe { CGPreflightScreenCaptureAccess() } {
        return Err(
            "Screen Recording permission isn't granted yet. Turn it on in System Settings → Privacy & Security → Screen Recording, then relaunch me."
                .to_string(),
        );
    }

    let tmp = std::env::temp_dir().join("claudio-screen.png");
    let tmp_str = tmp.to_string_lossy().to_string();

    // -x: silent, -m: main display only, -t png: format.
    let status = Command::new("screencapture")
        .args(["-x", "-m", "-t", "png", &tmp_str])
        .status()
        .map_err(|e| format!("couldn't run screencapture: {e}"))?;
    if !status.success() {
        return Err("screencapture exited with an error".to_string());
    }

    // Downscale in place — keeps the base64 payload reasonable.
    let _ = Command::new("sips")
        .args(["-Z", MAX_EDGE, &tmp_str])
        .status();

    let bytes = std::fs::read(&tmp).map_err(|e| format!("couldn't read the capture: {e}"))?;
    let _ = std::fs::remove_file(&tmp);

    if bytes.is_empty() {
        return Err("the capture came back empty".to_string());
    }

    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

use super::ScreenPermission;

pub fn check(_prompt: bool) -> ScreenPermission {
    ScreenPermission::NotApplicable
}

pub fn capture() -> Result<String, String> {
    Err("screen capture is not supported on this platform yet".to_string())
}

//! List running GUI applications (macOS) — used by the "Look at…" menu so the
//! user can explicitly point Claudio at a specific app instead of relying on
//! whatever happens to have keyboard focus.

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    pub name: String,
    pub pid: i32,
}

#[cfg(target_os = "macos")]
pub fn list_apps() -> Vec<AppInfo> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CStr;

    let mut out: Vec<AppInfo> = Vec::new();
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return out;
        }
        let apps: *mut Object = msg_send![workspace, runningApplications];
        if apps.is_null() {
            return out;
        }
        let count: usize = msg_send![apps, count];
        for i in 0..count {
            let app: *mut Object = msg_send![apps, objectAtIndex: i];
            if app.is_null() {
                continue;
            }
            // activationPolicy: 0 = regular (GUI app), 1 = accessory, 2 = prohibited
            let policy: i64 = msg_send![app, activationPolicy];
            if policy != 0 {
                continue;
            }
            let pid: i32 = msg_send![app, processIdentifier];
            let name_obj: *mut Object = msg_send![app, localizedName];
            if name_obj.is_null() {
                continue;
            }
            let utf8: *const i8 = msg_send![name_obj, UTF8String];
            if utf8.is_null() {
                continue;
            }
            let name = match CStr::from_ptr(utf8).to_str() {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };
            if name.is_empty() {
                continue;
            }
            out.push(AppInfo { name, pid });
        }
    }
    // Don't list ourselves.
    let our_pid = std::process::id() as i32;
    out.retain(|a| a.pid != our_pid);
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[cfg(not(target_os = "macos"))]
pub fn list_apps() -> Vec<AppInfo> {
    Vec::new()
}

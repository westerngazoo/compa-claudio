mod apps;
mod backends;
mod capture;
mod context;
mod events;
mod overlay;
mod sensors;

use backends::{BackendInfo, BackendRegistry, ChatContext, ChatMessage};
use context::AccessibilityStatus;
use events::{ContextEvent, EventBus};
use sensors::accessibility::AccessibilitySensor;
use sensors::Sensor;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

/// Cached snapshot of the most recently focused (non-self) context.
/// Fed by the bus fan-out task so `get_focused_context` is an instant read
/// rather than a blocking AX call.
struct ContextCache(Arc<Mutex<ChatContext>>);

/// Add `fullScreenAuxiliary` (plus `canJoinAllSpaces` + `stationary`) to the
/// window's NSWindow `collectionBehavior` — Tauri's `set_visible_on_all_workspaces`
/// only sets `canJoinAllSpaces`, which isn't enough to float over a fullscreen
/// app's Space. `fullScreenAuxiliary` is THE flag that lets a floating window
/// appear over a fullscreen app (it's how Picture-in-Picture and floating
/// tool palettes do it).
#[cfg(target_os = "macos")]
fn apply_fullscreen_auxiliary(window: &tauri::WebviewWindow) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    let ns_window: *mut Object = match window.ns_window() {
        Ok(ptr) => ptr as *mut Object,
        Err(e) => {
            eprintln!("[overlay] ns_window err: {e}");
            return;
        }
    };
    if ns_window.is_null() {
        eprintln!("[overlay] ns_window null");
        return;
    }

    const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
    const STATIONARY: usize = 1 << 4;
    const FULLSCREEN_AUXILIARY: usize = 1 << 8;
    // NSPopUpMenuWindowLevel = 101. Level 25 (NSStatusWindowLevel) lost to
    // Ghostty's fullscreen window, which sits at/above it — 101 outranks
    // fullscreen apps while staying under system-critical overlays.
    const STATUS_LEVEL: i64 = 101;

    unsafe {
        let before: usize = msg_send![ns_window, collectionBehavior];
        let new_cb = before | CAN_JOIN_ALL_SPACES | STATIONARY | FULLSCREEN_AUXILIARY;
        let _: () = msg_send![ns_window, setCollectionBehavior: new_cb];
        let after: usize = msg_send![ns_window, collectionBehavior];
        let _: () = msg_send![ns_window, setLevel: STATUS_LEVEL];
        let level: i64 = msg_send![ns_window, level];
        eprintln!(
            "[overlay] collectionBehavior {before:#x} -> {after:#x}  (want fullScreenAux=0x100); level={level}"
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_fullscreen_auxiliary(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
fn set_window_level(window: &tauri::WebviewWindow, level: i64) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    if let Ok(ptr) = window.ns_window() {
        let ns_window = ptr as *mut Object;
        if !ns_window.is_null() {
            unsafe {
                let _: () = msg_send![ns_window, setLevel: level];
            }
            eprintln!("[overlay] window level -> {level}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_window_level(_window: &tauri::WebviewWindow, _level: i64) {}

/// Keep Claudio one notch above the tallest regular-app window on screen.
/// Hardcoded levels kept losing (Ghostty's fullscreen sat above whatever we
/// picked), so instead: every 1.5s, scan on-screen windows of regular GUI
/// apps, take the max level, sit at max+1. Clamped to [101, 400] so Claudio
/// beats fullscreen apps but never fights system dialogs or the screensaver.
fn spawn_level_poller(app_handle: tauri::AppHandle, window: tauri::WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        let our_pid = std::process::id() as i32;
        let mut last: i64 = 0;
        loop {
            tokio::time::sleep(Duration::from_millis(1500)).await;

            // Both calls touch OS APIs that can stall — keep them off the
            // async runtime thread.
            let max = tokio::task::spawn_blocking(move || {
                let pids: Vec<i32> = apps::list_apps().into_iter().map(|a| a.pid).collect();
                overlay::max_app_window_level(our_pid, &pids)
            })
            .await
            .ok()
            .flatten();

            let desired = max.map_or(101, |m| (m + 1).clamp(101, 400));
            if desired != last {
                last = desired;
                let w = window.clone();
                // NSWindow ops must run on the main thread.
                let _ = app_handle.run_on_main_thread(move || {
                    set_window_level(&w, desired);
                });
            }
        }
    });
}

/// When `Some(pid)`, the AccessibilitySensor reads context from that app
/// explicitly instead of whatever has focus — used by the "Look at…" menu so
/// the user can pin Claudio's attention to a specific app.
struct TargetPid(Arc<Mutex<Option<i32>>>);

/// Whether a panel (chat/settings/onboarding) is currently open. The
/// click-through poller reads this: panel open → the whole window catches
/// clicks; otherwise only Claudio's body and the corner controls do.
struct PanelOpen(Arc<AtomicBool>);

/// Poll the global cursor position and toggle window click-through, so the
/// transparent window only intercepts clicks over Claudio (or an open panel)
/// and lets every other click fall through to the app behind him.
///
/// Currently unused — only relevant for the transparent floating window, which
/// is shelved while we run in a normal decorated window. Kept for when/if the
/// floating look is revisited.
#[allow(dead_code)]
fn spawn_click_through_poller(window: tauri::WebviewWindow, panel_open: Arc<AtomicBool>) {
    tauri::async_runtime::spawn(async move {
        let mut last_ignore: Option<bool> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(60)).await;

            let (cx, cy) = match overlay::cursor_position() {
                Some(p) => p,
                None => continue,
            };
            let scale = window.scale_factor().unwrap_or(1.0);
            let pos = match window.outer_position() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let size = match window.outer_size() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let wx = pos.x as f64 / scale;
            let wy = pos.y as f64 / scale;
            let ww = size.width as f64 / scale;
            let wh = size.height as f64 / scale;

            let interactive = if panel_open.load(Ordering::Relaxed) {
                // A panel is open — the whole window should catch clicks.
                cx >= wx && cx <= wx + ww && cy >= wy && cy <= wy + wh
            } else {
                // Idle — only Claudio (centered, near the top) and the
                // top-right corner controls are interactive.
                let mascot_cx = wx + ww / 2.0;
                let over_mascot =
                    (cx - mascot_cx).abs() <= 70.0 && cy >= wy && cy <= wy + 150.0;
                let over_controls = cx >= wx + ww - 64.0
                    && cx <= wx + ww
                    && cy >= wy
                    && cy <= wy + 36.0;
                over_mascot || over_controls
            };

            let ignore = !interactive;
            if last_ignore != Some(ignore) {
                let _ = window.set_ignore_cursor_events(ignore);
                last_ignore = Some(ignore);
            }
        }
    });
}

#[tauri::command]
async fn send_message(
    messages: Vec<ChatMessage>,
    context: Option<ChatContext>,
    registry: State<'_, BackendRegistry>,
) -> Result<String, String> {
    let backend = registry.current().await;
    backend.send(messages, context).await
}

#[tauri::command]
async fn list_backends(registry: State<'_, BackendRegistry>) -> Result<Vec<BackendInfo>, String> {
    Ok(registry.list().await)
}

#[tauri::command]
async fn get_current_backend(registry: State<'_, BackendRegistry>) -> Result<String, String> {
    Ok(registry.current_id().await)
}

#[tauri::command]
async fn set_backend(id: String, registry: State<'_, BackendRegistry>) -> Result<(), String> {
    registry.set_current(&id).await
}

#[tauri::command]
fn accessibility_status() -> AccessibilityStatus {
    context::check_accessibility(false)
}

#[tauri::command]
fn request_accessibility() -> AccessibilityStatus {
    context::check_accessibility(true)
}

#[tauri::command]
fn get_focused_context(cache: State<'_, ContextCache>) -> ChatContext {
    cache.0.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
fn screen_permission_status() -> capture::ScreenPermission {
    capture::check_screen_permission(false)
}

#[tauri::command]
fn request_screen_permission() -> capture::ScreenPermission {
    capture::check_screen_permission(true)
}

#[tauri::command]
fn set_panel_open(open: bool, state: State<'_, PanelOpen>) {
    state.0.store(open, Ordering::Relaxed);
}

#[tauri::command]
fn list_apps() -> Vec<apps::AppInfo> {
    apps::list_apps()
}

#[tauri::command]
fn set_target_app(pid: Option<i32>, state: State<'_, TargetPid>) {
    if let Ok(mut g) = state.0.lock() {
        *g = pid;
    }
}

#[tauri::command]
fn get_target_app(state: State<'_, TargetPid>) -> Option<i32> {
    state.0.lock().ok().and_then(|g| *g)
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    // Hard exit — `window.close()` only sends a close *request*, which macOS
    // Tahoe can silently swallow. `exit` terminates the process for sure.
    app.exit(0);
}

#[tauri::command]
async fn capture_screen() -> Result<String, String> {
    // Capture is blocking (shells out to `screencapture` + `sips`) — keep it
    // off the async runtime thread.
    tokio::task::spawn_blocking(capture::capture_screen)
        .await
        .map_err(|e| format!("capture task failed: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cache = Arc::new(Mutex::new(ChatContext::default()));
    let bus = EventBus::new();
    let panel_open = Arc::new(AtomicBool::new(false));
    let target_pid: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(BackendRegistry::new())
        .manage(ContextCache(cache.clone()))
        .manage(PanelOpen(panel_open.clone()))
        .manage(TargetPid(target_pid.clone()))
        .manage(bus.clone())
        .setup(move |app| {
            // Force "visible on all workspaces" at runtime. The declarative
            // tauri.conf.json flag of the same name isn't reliable on macOS
            // Tahoe (26+) — the window gets pinned to one Space.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_visible_on_all_workspaces(true);
                apply_fullscreen_auxiliary(&window);

                // The setup-time calls above can run before the window is fully
                // realized on macOS — re-apply a couple of times once the window
                // has settled, so Claudio shows on whichever Space the user is
                // actually looking at AND can float over fullscreen apps.
                let w = window.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    for delay in [900u64, 1600] {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        let _ = w.set_visible_on_all_workspaces(true);
                        // NSWindow ops must run on the main thread.
                        let w_main = w.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            apply_fullscreen_auxiliary(&w_main);
                        });
                    }
                });

                // Keep Claudio's level above whatever app window is tallest
                // (Ghostty fullscreen, PiP videos, etc.) — dynamic, not guessed.
                spawn_level_poller(app.handle().clone(), window.clone());

                // Click-through poller is shelved with the transparent window.
                // Re-enable if the floating look gets click-through:
                // spawn_click_through_poller(window, panel_open.clone());
            }

            // Menu-bar (tray) icon — Claudio's permanent home base. Even if
            // the floating window is ever covered or lost, one click in the
            // menu bar summons him to the current Space.
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let toggle =
                    MenuItem::with_id(app, "toggle", "Show / Hide Claudio", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle, &quit])?;

                TrayIconBuilder::with_id("claudio")
                    .icon(app.default_window_icon().expect("window icon").clone())
                    .menu(&menu)
                    .show_menu_on_left_click(true)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "toggle" => {
                            if let Some(w) = app.get_webview_window("main") {
                                if w.is_visible().unwrap_or(false) {
                                    let _ = w.hide();
                                } else {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                    // Re-assert Space/level behavior on re-show.
                                    let _ = w.set_visible_on_all_workspaces(true);
                                    apply_fullscreen_auxiliary(&w);
                                }
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .build(app)?;
            }

            // Spawn each sensor's observation loop. Sensors publish to the bus;
            // they know nothing about who consumes their events. Adding a new
            // sensor = add one entry to this list.
            let all_sensors: Vec<Box<dyn Sensor>> =
                vec![Box::new(AccessibilitySensor::new(target_pid.clone()))];
            for sensor in all_sensors {
                println!("[sensors] starting: {}", sensor.id());
                let sensor_bus = bus.clone();
                tauri::async_runtime::spawn(async move {
                    sensor.run(sensor_bus).await;
                });
            }

            // Fan-out: drain the bus and route events to (1) the context
            // cache that `get_focused_context` reads, and (2) the frontend
            // via a Tauri event, so the UI can react without polling.
            let mut rx = bus.subscribe();
            let cache_for_fanout = cache.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    match &event {
                        ContextEvent::FocusChanged {
                            app,
                            text,
                            selection,
                        } => {
                            if let Ok(mut guard) = cache_for_fanout.lock() {
                                guard.focused_app = app.clone();
                                guard.focused_text = text.clone();
                                guard.selection = selection.clone();
                            }
                        }
                    }
                    let _ = app_handle.emit("context-event", &event);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            send_message,
            list_backends,
            get_current_backend,
            set_backend,
            accessibility_status,
            request_accessibility,
            get_focused_context,
            screen_permission_status,
            request_screen_permission,
            capture_screen,
            quit_app,
            set_panel_open,
            list_apps,
            set_target_app,
            get_target_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

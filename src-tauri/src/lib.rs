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

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(BackendRegistry::new())
        .manage(ContextCache(cache.clone()))
        .manage(PanelOpen(panel_open.clone()))
        .manage(bus.clone())
        .setup(move |app| {
            // Force "visible on all workspaces" at runtime. The declarative
            // tauri.conf.json flag of the same name isn't reliable on macOS
            // Tahoe (26+) — the window gets pinned to one Space.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_visible_on_all_workspaces(true);
                // Click-through poller is shelved with the transparent window.
                // Re-enable both together if the floating look is revisited:
                // spawn_click_through_poller(window, panel_open.clone());
            }

            // Spawn each sensor's observation loop. Sensors publish to the bus;
            // they know nothing about who consumes their events. Adding a new
            // sensor = add one entry to this list.
            let all_sensors: Vec<Box<dyn Sensor>> = vec![Box::new(AccessibilitySensor)];
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

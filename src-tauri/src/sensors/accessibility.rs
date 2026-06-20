//! Accessibility sensor — watches the focused app/window/selection via the
//! macOS AX APIs and publishes `FocusChanged` events when something changes.
//!
//! If the user has explicitly targeted an app (via the "Look at…" menu), the
//! sensor reads context from THAT app's pid instead of whatever's focused.

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::Sensor;
use crate::context;
use crate::events::{ContextEvent, EventBus};

pub struct AccessibilitySensor {
    /// When `Some(pid)`, read context from that app explicitly (the user has
    /// "Look at…"-targeted it). When `None`, fall back to focus-based reading.
    target_pid: Arc<Mutex<Option<i32>>>,
}

impl AccessibilitySensor {
    pub fn new(target_pid: Arc<Mutex<Option<i32>>>) -> Self {
        Self { target_pid }
    }
}

type Snapshot = (Option<String>, Option<String>, Option<String>);

#[async_trait]
impl Sensor for AccessibilitySensor {
    fn id(&self) -> &'static str {
        "accessibility"
    }

    async fn run(&self, bus: EventBus) {
        let mut last: Option<Snapshot> = None;

        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;

            // Snapshot the current target (don't hold the lock across the
            // blocking AX call).
            let target = self
                .target_pid
                .lock()
                .map(|g| *g)
                .ok()
                .flatten();

            // AX calls can block on unresponsive apps — keep them off the
            // async runtime thread.
            let ctx = match target {
                Some(pid) => {
                    tokio::task::spawn_blocking(move || context::read_context_for_pid(pid))
                        .await
                        .unwrap_or_default()
                }
                None => tokio::task::spawn_blocking(context::read_focused_context)
                    .await
                    .unwrap_or_default(),
            };

            let has_signal = ctx.focused_app.is_some()
                || ctx.focused_text.is_some()
                || ctx.selection.is_some();
            if !has_signal {
                continue;
            }

            // Dedup: only publish when something actually changed.
            let snapshot: Snapshot = (
                ctx.focused_app.clone(),
                ctx.focused_text.clone(),
                ctx.selection.clone(),
            );
            if last.as_ref() == Some(&snapshot) {
                continue;
            }
            last = Some(snapshot);

            bus.publish(ContextEvent::focus_from(&ctx));
        }
    }
}

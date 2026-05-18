//! Accessibility sensor — watches the focused app/window/selection via the
//! macOS AX APIs and publishes `FocusChanged` events when something changes.
//!
//! This wraps the raw AX reader in `crate::context`; the polling + dedup logic
//! that used to live inline in `lib.rs` now lives here, behind the Sensor trait.

use async_trait::async_trait;
use std::time::Duration;

use super::Sensor;
use crate::context;
use crate::events::{ContextEvent, EventBus};

pub struct AccessibilitySensor;

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

            // AX calls can block on unresponsive apps — keep them off the
            // async runtime thread.
            let ctx = tokio::task::spawn_blocking(context::read_focused_context)
                .await
                .unwrap_or_default();

            let has_signal = ctx.focused_app.is_some()
                || ctx.focused_text.is_some()
                || ctx.selection.is_some();
            if !has_signal {
                // Empty read = no perms, or we're the focused app. Skip.
                continue;
            }

            // Dedup: only publish when the focus actually changed, so the bus
            // isn't spammed with identical events four times a second.
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

//! Internal event bus.
//!
//! Sensors observe the user's environment and publish `ContextEvent`s here.
//! Subscribers (the context cache, the frontend bridge) react. This keeps
//! sensors decoupled from consumers — adding a new sensor never touches the
//! brain, and adding a new consumer never touches the sensors.
//!
//! Deliberately in-process: a single `tokio::broadcast` channel, no IPC, no
//! broker. Cross-process integration (music players, third-party tools) is a
//! separate concern handled via MCP, not this bus.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::backends::ChatContext;

/// A signal about the user's environment, emitted by a sensor.
/// Serialized to the frontend as `{ "kind": "...", ... }`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContextEvent {
    /// The focused application / window / selection changed.
    FocusChanged {
        app: Option<String>,
        text: Option<String>,
        selection: Option<String>,
    },
}

impl ContextEvent {
    pub fn focus_from(ctx: &ChatContext) -> Self {
        ContextEvent::FocusChanged {
            app: ctx.focused_app.clone(),
            text: ctx.focused_text.clone(),
            selection: ctx.selection.clone(),
        }
    }
}

/// Cloneable handle to the broadcast bus. Cheap to clone — every clone shares
/// the same underlying channel.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<ContextEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(64);
        Self { tx }
    }

    /// Publish an event. A send error just means no subscribers right now —
    /// not a real failure, so it's ignored.
    pub fn publish(&self, event: ContextEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ContextEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

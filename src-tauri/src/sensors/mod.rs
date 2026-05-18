//! Sensors — things that observe the user's environment and publish events.
//!
//! Each sensor runs its own async loop for the lifetime of the app and pushes
//! `ContextEvent`s onto the `EventBus`. A sensor knows nothing about who
//! consumes its events.
//!
//! To add a sensor: implement `Sensor` in a new file here, then register it in
//! the sensor list in `lib.rs`. Nothing else changes.

use async_trait::async_trait;

use crate::events::EventBus;

pub mod accessibility;

#[async_trait]
pub trait Sensor: Send + Sync + 'static {
    /// Stable identifier, for logging/debugging.
    fn id(&self) -> &'static str;

    /// Run the sensor's observation loop, publishing to the bus.
    /// Expected to run for the whole lifetime of the app.
    async fn run(&self, bus: EventBus);
}

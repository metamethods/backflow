//! Dummy output backend for testing and demonstration purposes.
use crate::input::InputEventStream;
use crate::output::OutputBackend;
use eyre::Result;

pub struct DummyOutput {
    stream: InputEventStream,
}

impl DummyOutput {
    /// Creates a new `DummyOutput` with the given input event stream.
    #[cfg(test)] // Part of public API, may be used in tests or alternative configurations
    pub fn new(stream: InputEventStream) -> Self {
        Self { stream }
    }
}

// #[async_trait]
impl OutputBackend for DummyOutput {
    async fn run(&mut self) -> Result<()> {
        tracing::info!("Dummy output backend started!");

        loop {
            match self.stream.receive().await {
                Some(packet) => {
                    tracing::info!(
                        "🎮 Dummy backend received packet from device '{}' with {} events",
                        packet.device_id,
                        packet.events.len()
                    );

                    for (i, event) in packet.events.iter().enumerate() {
                        tracing::info!("  Event {}: {:?}", i + 1, event);
                    }
                }
                None => {
                    tracing::info!("Input stream closed, stopping dummy backend");
                    break;
                }
            }
        }

        Ok(())
    }
}

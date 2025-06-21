//! Dummy output backend for testing and demonstration purposes.
use crate::input::{InputEventPacket, InputEventStream};
use crate::output::OutputBackend;
use eyre::Result;

pub struct DummyOutput {
    stream: crossbeam::channel::Receiver<InputEventPacket>,
}

impl DummyOutput {
    /// Creates a new `DummyOutput` with the given input event stream.
    pub fn new(stream: InputEventStream) -> Self {
        Self { stream: stream.rx }
    }
}

// #[async_trait]
impl OutputBackend for DummyOutput {
    async fn run(&mut self) -> Result<()> {
        tracing::info!("Dummy output backend started!");

        loop {
            match self.stream.recv() {
                Ok(packet) => {
                    tracing::info!(
                        "🎮 Dummy backend received packet from device '{}' with {} events",
                        packet.device_id,
                        packet.events.len()
                    );

                    for (i, event) in packet.events.iter().enumerate() {
                        tracing::info!("  Event {}: {:?}", i + 1, event);
                    }
                }
                Err(e) => {
                    tracing::error!("Dummy backend failed to receive packet: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

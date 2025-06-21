//! InputPlumber D-Bus output backend, for sending messages to the InputPlumber D-Bus service.
use crate::input::{InputEventPacket, InputEventStream};
use crate::output::OutputBackend;
use eyre::Result;


/// initialize by calling org.shadowblip.InputManager CreateTargetDevice <input_type>
pub struct InputPlumberTargetDev {
    // SendKey sb KEY_ENTER 1 for example
    pub endpoint: String,
}

/// a virtual target device to send input events to for InputPlumber
pub struct InputPlumberTarget {
    pub keyboard: Option<InputPlumberTargetDev>,
}

pub struct DbusPlumberOutput {
    // Receiver for input event packets
    pub stream: crossbeam::channel::Receiver<InputEventPacket>,
    // sender for D-Bus messages
    // ...
}

impl DbusPlumberOutput {
    /// Creates a new `DbusPlumberOutput` with the given input event stream.
    pub fn new(stream: InputEventStream) -> Self {
        Self {
            stream: stream.rx,
            // Initialize D-Bus connection and sender here
        }
    }
}

// #[async_trait]
impl OutputBackend for DbusPlumberOutput {
    async fn run(&mut self) -> Result<()> {
        // Main loop for processing input events and sending D-Bus messages
        loop {
            match self.stream.recv() {
                Ok(packet) => {
                    // Process the packet and send D-Bus messages
                    // For example, serialize the packet and send it over D-Bus
                    tracing::info!("Received input event packet: {:?}", packet);
                }
                Err(e) => {
                    // Handle errors, such as logging or retrying
                    tracing::error!("Failed to receive input event packet: {}", e);
                }
            }
        }
    }
}

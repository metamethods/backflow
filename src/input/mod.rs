//! Input handling module, for handling input device types and events.
//! This is the top-level module for all input backends,
//! such as WebSockets, MIDI, RS232, and others.

use serde::{Deserialize, Serialize};
pub mod web;

/// Represents a packet of input events, sent over a network or any other communication channel.
/// (i.e WebSocket, Unix Domain Socket, etc.)
///
/// The packet contains a device identifier, timestamp and a list of input events to be processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEventPacket {
    /// The device identifier for this packet, used for routing events.
    pub device_id: String,

    /// The timestamp of the packet, in epoch milliseconds.
    pub timestamp: u64,

    /// List of input events that occured in this packet.
    pub events: Vec<InputEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    /// A keyboard event, such as a key press or release.
    Keyboard(KeyboardEvent),
    /// A pointer event, such as a mouse button click, release, move, or scroll.
    Pointer(PointerEvent),
    /// A joystick event, such as a button press or release, or an axis movement.
    Joystick(JoystickEvent),
}

#[async_trait::async_trait]
pub trait InputBackend: Send {
    /// Starts the input backend, processing input events and sending them to the appropriate destination.
    async fn run(&mut self) -> eyre::Result<()>;
}

/// A keyboard event, such as a key press or release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyboardEvent {
    /// A key press event.
    /// The `key` is the keymap code of the key that was pressed.
    ///
    /// This is an evdev code, which is usually a string representation of the key.
    KeyPress { key: String },
    /// A key release event.
    /// The `key` is the keymap code of the key that was released.
    KeyRelease { key: String },
}

/// A pointer event, such as a mouse button click, release, move, or scroll.
/// The pointer can be a mouse, trackpad, or any other relative pointing device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PointerEvent {
    /// A mouse button event.
    ///
    /// The `button` is the button code of the mouse button that was pressed.
    /// This is usually a number from 1 to 5, where 1 is the left button,
    Click { button: u8 },
    /// A mouse button release event.
    ///
    /// Similar to `Click`, the `button` is the button code of the mouse button that was released.
    /// This is usually a number from 1 to 5, where 1 is the left button,
    ClickRelease { button: u8 },
    /// A mouse move event.
    ///
    /// The `x_delta` and `y_delta` are the changes in position of the mouse pointer.
    /// Positive values indicate movement to the right or down, while negative values indicate movement to the left or up.
    Move { x_delta: i32, y_delta: i32 },

    /// A mouse scroll event.
    ///
    /// Can be either horizontal or vertical. Most of the time you probably want
    /// vertical, so use `y_delta` for that.
    Scroll { x_delta: i32, y_delta: i32 },
}

/// A joystick  event, such as a button press or release, or an axis movement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JoystickEvent {
    /// A joystick button press event.
    ButtonPress { button: u8 },
    /// A joystick button release event.
    ButtonRelease { button: u8 },
    /// A joystick axis movement event.
    AxisMovement { stick: u8, x: i16, y: i16 },
}
#[cfg(test)]
impl InputEventPacket {
    /// Creates a new `InputEventPacket` with the given device ID and timestamp.
    pub fn new(device_id: String, timestamp: u64) -> Self {
        Self {
            device_id,
            timestamp,
            events: Vec::new(),
        }
    }

    /// Adds an event to the packet.
    pub fn add_event(&mut self, event: InputEvent) {
        self.events.push(event);
    }
}

/// Represents a stream of input event packets, using tokio mpsc channels for async communication.
#[derive(Clone)]
pub struct InputEventStream {
    /// Sender for input event packets.
    pub tx: tokio::sync::mpsc::Sender<InputEventPacket>,
    /// Receiver for input event packets.
    pub rx: std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<InputEventPacket>>>,
}

impl InputEventStream {
    /// Creates a new `InputEventStream` with a tokio mpsc channel.
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        Self {
            tx,
            rx: std::sync::Arc::new(tokio::sync::Mutex::new(rx)),
        }
    }

    /// Sends an input event packet through the stream.
    pub async fn send(
        &self,
        packet: InputEventPacket,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<InputEventPacket>> {
        self.tx.send(packet).await
    }

    /// Receives an input event packet from the stream.
    /// This is an async method that will await for a packet to be available.
    pub async fn receive(&self) -> Option<InputEventPacket> {
        let mut rx = self.rx.lock().await;
        rx.recv().await
    }
}

impl Default for InputEventStream {
    fn default() -> Self {
        Self::new()
    }
}

// todo: route to inputplumber dbus

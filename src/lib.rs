//! PlumberShim - A Rust library for handling input events through various backends
//! and routing them to output destinations.
//!
//! This library provides a flexible framework for accepting input events from various
//! sources (WebSocket, MIDI, RS232, etc.) and routing them to appropriate output
//! backends (D-Bus, direct input injection, etc.).

pub mod input;
pub mod output;

pub use input::{InputEvent, InputEventPacket, InputEventStream, InputBackend};
pub use output::OutputBackend;

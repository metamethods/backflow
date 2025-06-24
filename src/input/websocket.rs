//! WebSocket input handling utilities, accepts a stream of JSON-encoded input events,
//! serialized from [`InputEventPacket`](crate::input::InputEventPacket).
//!
//! This module provides reusable WebSocket handlers for use with Axum web servers.
//! It focuses on processing the WebSocket messages and integrating with the input
//! event stream system.
use crate::input::{InputEventPacket, InputEventStream};
use axum::{
    extract::{
        ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use eyre::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tracing::{debug, error, info, warn};

/// Process an Axum WebSocket connection
pub async fn handle_axum_websocket(
    socket: WebSocket,
    event_stream: &InputEventStream,
    addr: SocketAddr,
) -> Result<()> {
    let (mut sender, mut receiver) = socket.split();

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received text message from {}: {}", addr, text);

                match process_input_message(event_stream, &text).await {
                    Ok(_) => {
                        // Send acknowledgment back to client
                        if let Err(e) = sender.send(Message::Text("ok".into())).await {
                            warn!("Failed to send acknowledgment to {}: {}", addr, e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to process input message from {}: {}", addr, e);
                        // Send error back to client
                        let error_msg = format!("error: {}", e);
                        if let Err(e) = sender.send(Message::Text(error_msg.into())).await {
                            warn!("Failed to send error message to {}: {}", addr, e);
                            break;
                        }
                    }
                }
            }
            Ok(Message::Binary(_)) => {
                warn!(
                    "Received binary message from {}, but only text messages are supported",
                    addr
                );
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket connection from {} closed", addr);
                break;
            }
            Ok(Message::Ping(payload)) => {
                debug!("Received ping from {}", addr);
                if let Err(e) = sender.send(Message::Pong(payload)).await {
                    warn!("Failed to send pong to {}: {}", addr, e);
                    break;
                }
            }
            Ok(Message::Pong(_)) => {
                debug!("Received pong from {}", addr);
            }
            Err(e) => {
                error!("WebSocket error from {}: {}", addr, e);
                break;
            }
        }
    }

    info!("WebSocket connection from {} terminated", addr);
    Ok(())
}

/// WebSocket handler function for Axum
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    event_stream: axum::extract::State<InputEventStream>,
) -> impl IntoResponse {
    info!("New WebSocket connection from {}", addr);
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_axum_websocket(socket, &event_stream, addr).await {
            error!("Error handling WebSocket connection from {}: {}", addr, e);
        }
    })
}

/// Processes an input message received via WebSocket.
pub async fn process_input_message(event_stream: &InputEventStream, message: &str) -> Result<()> {
    let packet: InputEventPacket =
        serde_json::from_str(message).context("Failed to deserialize input event packet")?;

    debug!(
        "Processing input packet from device '{}' with {} events",
        packet.device_id,
        packet.events.len()
    );

    event_stream
        .send(packet)
        .context("Failed to send input event packet to stream")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{InputEvent, KeyboardEvent, PointerEvent};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Creates a sample input event packet for testing.
    fn create_sample_packet() -> InputEventPacket {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut packet = InputEventPacket::new("test-device".to_string(), timestamp);

        // Add some sample events
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
            key: 65.to_string(),
        })); // 'A' key
        packet.add_event(InputEvent::Pointer(PointerEvent::Move {
            x_delta: 10,
            y_delta: -5,
        }));
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
            key: 65.to_string(),
        }));

        packet
    }

    #[test]
    fn test_packet_serialization() {
        let packet = create_sample_packet();
        let json = serde_json::to_string(&packet).expect("Failed to serialize packet");

        // Ensure we can deserialize it back
        let _deserialized: InputEventPacket =
            serde_json::from_str(&json).expect("Failed to deserialize packet");
    }

    #[test]
    fn test_sample_json_format() {
        let packet = create_sample_packet();
        let json = serde_json::to_string_pretty(&packet).expect("Failed to serialize packet");

        println!("Sample JSON format for WebSocket input:");
        println!("{}", json);
    }
}

/// Documentation for WebSocket input handling.
///
/// # WebSocket Input Protocol
///
/// The WebSocket endpoint expects JSON messages in the format of [`InputEventPacket`].
/// Each message should contain a device ID, timestamp, and list of input events.
///
/// ## Example JSON Message
///
/// ```json
/// {
///   "device_id": "my-device",
///   "timestamp": 1672531200000,
///   "events": [
///     {
///       "Keyboard": {
///         "KeyPress": {
///           "key": 65
///         }
///       }
///     },
///     {
///       "Pointer": {
///         "Move": {
///           "x_delta": 10,
///           "y_delta": -5
///         }
///       }
///     }
///   ]
/// }
/// ```
///
/// ## Client Connection
///
/// Clients can connect using any WebSocket library. For example, with JavaScript:
///
/// ```javascript
/// const ws = new WebSocket('ws://localhost:8080/ws');
///
/// ws.onopen = function() {
///     const packet = {
///         device_id: "web-client",
///         timestamp: Date.now(),
///         events: [
///             {
///                 Keyboard: {
///                     KeyPress: { key: 65 }
///                 }
///             }
///         ]
///     };
///
///     ws.send(JSON.stringify(packet));
/// };
///
/// ws.onmessage = function(event) {
///     console.log('Server response:', event.data);
/// };
/// ```
pub mod examples {}

//! Websocket input handling backend, accepts a stream of JSON-encoded input events,
//! serialized from [`InputEventPacket`](crate::input::InputEventPacket).
use crate::input::{InputBackend, InputEventPacket, InputEventStream};
use eyre::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// WebSocket input backend that listens for connections and processes input events.
pub struct WebSocketInputBackend {
    /// The address to bind the WebSocket server to.
    bind_addr: SocketAddr,
    /// The input event stream to send received events to.
    event_stream: InputEventStream,
}

impl WebSocketInputBackend {
    /// Creates a new WebSocket input backend.
    ///
    /// # Arguments
    /// * `bind_addr` - The address to bind the WebSocket server to
    /// * `event_stream` - The input event stream to send received events to
    pub fn new(bind_addr: SocketAddr, event_stream: InputEventStream) -> Self {
        Self {
            bind_addr,
            event_stream,
        }
    }

    /// Handles a single WebSocket connection.
    async fn handle_connection(&self, stream: TcpStream, addr: SocketAddr) -> Result<()> {
        info!("New WebSocket connection from {}", addr);

        let ws_stream = accept_async(stream)
            .await
            .context("Failed to accept WebSocket connection")?;

        self.handle_websocket(ws_stream, addr).await
    }

    /// Handles messages from a WebSocket connection.
    async fn handle_websocket(
        &self,
        mut ws_stream: WebSocketStream<TcpStream>,
        addr: SocketAddr,
    ) -> Result<()> {
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received text message from {}: {}", addr, text);

                    match self.process_input_message(&text).await {
                        Ok(_) => {
                            // Send acknowledgment back to client
                            if let Err(e) = ws_stream.send(Message::Text("ok".into())).await {
                                warn!("Failed to send acknowledgment to {}: {}", addr, e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to process input message from {}: {}", addr, e);
                            // Send error back to client
                            let error_msg = format!("error: {}", e);
                            if let Err(e) = ws_stream.send(Message::Text(error_msg.into())).await {
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
                    if let Err(e) = ws_stream.send(Message::Pong(payload)).await {
                        warn!("Failed to send pong to {}: {}", addr, e);
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from {}", addr);
                }
                Ok(Message::Frame(_)) => {
                    debug!("Received raw frame from {}", addr);
                    // Raw frames are typically handled internally by the WebSocket library
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

    /// Processes an input message received via WebSocket.
    async fn process_input_message(&self, message: &str) -> Result<()> {
        let packet: InputEventPacket =
            serde_json::from_str(message).context("Failed to deserialize input event packet")?;

        debug!(
            "Processing input packet from device '{}' with {} events",
            packet.device_id,
            packet.events.len()
        );

        self.event_stream
            .send(packet)
            .context("Failed to send input event packet to stream")?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl InputBackend for WebSocketInputBackend {
    async fn run(&mut self) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr)
            .await
            .context(format!("Failed to bind to {}", self.bind_addr))?;

        info!("WebSocket input backend listening on {}", self.bind_addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let backend = self.clone();

            // Spawn a new task for each connection
            tokio::spawn(async move {
                if let Err(e) = backend.handle_connection(stream, addr).await {
                    error!("Error handling connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }
}

// Implement Clone for WebSocketInputBackend to allow spawning tasks
impl Clone for WebSocketInputBackend {
    fn clone(&self) -> Self {
        Self {
            bind_addr: self.bind_addr,
            event_stream: InputEventStream {
                tx: self.event_stream.tx.clone(),
                rx: self.event_stream.rx.clone(),
            },
        }
    }
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
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease { key: 65.to_string() }));

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

/// Example usage and documentation for the WebSocket input backend.
///
/// # WebSocket Input Protocol
///
/// The WebSocket server expects JSON messages in the format of [`InputEventPacket`].
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
/// ## Usage Example
///
/// ```rust,no_run
/// use std::net::SocketAddr;
/// use plumbershim::input::{InputEventStream, websocket::WebSocketInputBackend, InputBackend};
///
/// #[tokio::main]
/// async fn main() -> eyre::Result<()> {
///     let bind_addr: SocketAddr = "127.0.0.1:8080".parse()?;
///     let event_stream = InputEventStream::new();
///     
///     let mut backend = WebSocketInputBackend::new(bind_addr, event_stream);
///     backend.run().await?;
///     
///     Ok(())
/// }
/// ```
///
/// ## Client Connection
///
/// Clients can connect using any WebSocket library. For example, with JavaScript:
///
/// ```javascript
/// const ws = new WebSocket('ws://localhost:8080');
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

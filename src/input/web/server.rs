//! Web server implementation for both WebSocket input handling and web UI serving.
//! Uses axum framework with tower middleware support.

use crate::feedback::{FeedbackEventPacket, FeedbackEventStream};
use crate::input::{InputBackend, InputEventPacket, InputEventStream};
use axum::{
    Router,
    extract::{
        ConnectInfo, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{Json, Response},
    routing::get,
};
use eyre::{Context, Result};
use futures_util::{sink::SinkExt, stream::StreamExt};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{debug, error, info, trace, warn};

use super::frontend;

/// Shared state for bidirectional WebSocket communication
#[derive(Clone)]
pub struct WebSocketState {
    pub input_stream: InputEventStream,
    pub feedback_stream: FeedbackEventStream,
    pub connected_clients:
        Arc<RwLock<Vec<tokio::sync::mpsc::UnboundedSender<FeedbackEventPacket>>>>,
}

/// Web server that handles both WebSocket connections for input events
/// and serves static files for the web UI.
pub struct WebServer {
    /// The address to bind the server to
    bind_addr: SocketAddr,
    /// The input event stream to send received events to
    event_stream: InputEventStream,
    /// The feedback event stream to receive feedback events from
    feedback_stream: FeedbackEventStream,
    /// Path to the directory containing web UI assets
    web_assets_path: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebTemplateResponse {
    /// Relative web path to the custom template
    pub path: String,
    /// Name of the custom template, usually the name of the directory
    pub name: String,
}

impl WebServer {
    /// Automatically detect and use web UI if available.
    ///
    /// # Arguments
    /// * `bind_addr` - The address to bind the server to
    /// * `event_stream` - The input event stream to send received events to
    /// * `feedback_stream` - The feedback event stream to receive feedback events from
    ///
    /// This method checks the `WEB_UI_PATH` environment variable for a configured path,
    /// and falls back to searching for a `web` directory in the current working directory
    pub fn auto_detect_web_ui(
        bind_addr: SocketAddr,
        event_stream: InputEventStream,
        feedback_stream: FeedbackEventStream,
    ) -> Self {
        let configured_path = std::env::var("WEB_UI_PATH").ok().map(PathBuf::from);

        let web_assets_path = frontend::find_web_ui_dir(configured_path);
        Self {
            bind_addr,
            event_stream,
            feedback_stream,
            web_assets_path,
        }
    }

    pub fn list_custom_layouts(&self) -> Vec<WebTemplateResponse> {
        if let Some(assets_path) = &self.web_assets_path {
            let custom_dir = assets_path.join("custom");
            if custom_dir.is_dir() {
                match std::fs::read_dir(&custom_dir) {
                    Ok(entries) => entries
                        .filter_map(|entry| {
                            entry.ok().and_then(|e| {
                                let path = e.path();
                                if path.is_dir() {
                                    // Look for the first HTML file in this directory
                                    if let Ok(files) = std::fs::read_dir(&path) {
                                        for file in files.flatten() {
                                            let file_path = file.path();
                                            if let Some(ext) = file_path.extension() {
                                                if ext == "html" {
                                                    if let Some(dir_name) =
                                                        path.file_name().and_then(|n| n.to_str())
                                                    {
                                                        if let Some(file_name) = file_path
                                                            .file_name()
                                                            .and_then(|n| n.to_str())
                                                        {
                                                            return Some(WebTemplateResponse {
                                                                path: format!(
                                                                    "custom/{}/{}",
                                                                    dir_name, file_name
                                                                ),
                                                                name: dir_name.to_string(),
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    None
                                } else {
                                    None
                                }
                            })
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    /// Build the application router with all routes and return both the router and WebSocket state
    fn build_router(&self) -> (Router, WebSocketState) {
        // Create shared state for bidirectional WebSocket communication
        let ws_state = WebSocketState {
            input_stream: self.event_stream.clone(),
            feedback_stream: self.feedback_stream.clone(),
            connected_clients: Arc::new(RwLock::new(Vec::new())),
        };

        // Create the WebSocket router with WebSocket state
        let ws_router = Router::new()
            .route("/ws", get(bidirectional_ws_handler))
            .with_state(ws_state.clone());

        // Create API router with server state
        let server_arc = Arc::new(self.clone());
        let api_router = Router::new()
            .route("/api/layouts", get(get_custom_layouts))
            .with_state(server_arc);

        // Merge routers
        let mut router = ws_router.merge(api_router);

        // Add static file serving if web UI is enabled
        if let Some(assets_path) = &self.web_assets_path {
            if frontend::is_valid_web_ui(assets_path) {
                // Create a service for serving static files with SPA support
                let serve_dir = tower_http::services::ServeDir::new(assets_path)
                    .append_index_html_on_directories(true);

                // Add the static file service as a fallback service
                router = router.fallback_service(serve_dir);

                info!("Web UI serving enabled from {}", assets_path.display());
            } else {
                warn!(
                    "Web UI directory exists but doesn't contain index.html: {}",
                    assets_path.display()
                );
            }
        } else {
            info!("Web UI serving disabled (no web assets path configured)");
        }

        // Add tracing middleware
        let router = router.layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

        (router, ws_state)
    }
}

#[async_trait::async_trait]
impl InputBackend for WebServer {
    async fn run(&mut self) -> Result<()> {
        info!("Starting web server on {}", self.bind_addr);

        // Build the router and get the WebSocket state
        let (router, ws_state) = self.build_router();

        // Clone the connected_clients reference for the feedback task
        let connected_clients_for_broadcast = ws_state.connected_clients.clone();
        let feedback_stream = self.feedback_stream.clone();

        let feedback_task = {
            let feedback_stream = feedback_stream.clone();
            let clients = connected_clients_for_broadcast;
            tokio::spawn(async move {
                loop {
                    // Use async receive method - much cleaner than try_recv with sleep
                    match feedback_stream.receive().await {
                        Some(feedback_packet) => {
                            let start_time = std::time::Instant::now();
                            trace!(
                                target: crate::PACKET_PROCESSING_TARGET,
                                "Broadcasting feedback packet to all clients: device_id={}, events={}",
                                feedback_packet.device_id,
                                feedback_packet.events.len()
                            );

                            // Send to all connected clients with minimal lock time
                            let clients_to_send = {
                                let clients_guard = clients.read().await;
                                clients_guard.clone() // Clone the senders to release the lock quickly
                            };

                            let mut successful_sends = 0;
                            let mut failed_sends = 0;

                            for tx in &clients_to_send {
                                if !tx.is_closed() {
                                    match tx.send(feedback_packet.clone()) {
                                        Ok(_) => successful_sends += 1,
                                        Err(_) => failed_sends += 1,
                                    }
                                } else {
                                    failed_sends += 1;
                                }
                            }

                            // Only cleanup disconnected clients if there were failures
                            if failed_sends > 0 {
                                let mut clients_guard = clients.write().await;
                                let initial_count = clients_guard.len();
                                clients_guard.retain(|tx| !tx.is_closed());
                                let final_count = clients_guard.len();

                                if initial_count != final_count {
                                    debug!(
                                        "Cleaned up {} disconnected clients (was {}, now {})",
                                        initial_count - final_count,
                                        initial_count,
                                        final_count
                                    );
                                }
                            }

                            let elapsed = start_time.elapsed();

                            if successful_sends > 0 {
                                trace!(
                                    target: crate::PACKET_PROCESSING_TARGET,
                                    "Feedback broadcast sent to {} clients in {}μs",
                                    successful_sends,
                                    elapsed.as_micros()
                                );
                            }
                        }
                        None => {
                            info!("Feedback stream closed, stopping broadcast task");
                            break;
                        }
                    }
                }
            })
        };

        // Create TCP listener
        let listener = tokio::net::TcpListener::bind(self.bind_addr)
            .await
            .context("Failed to bind to address")?;

        // Run the server with graceful shutdown
        let server_task = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl+c");
            info!("Shutting down web server...");
        });

        // Run both the server and feedback broadcast task
        tokio::select! {
            result = server_task => {
                result.context("Server error")?;
            }
            _ = feedback_task => {
                warn!("Feedback broadcast task ended unexpectedly");
            }
        }

        Ok(())
    }
}

impl Clone for WebServer {
    fn clone(&self) -> Self {
        Self {
            bind_addr: self.bind_addr,
            event_stream: self.event_stream.clone(),
            feedback_stream: FeedbackEventStream {
                tx: self.feedback_stream.tx.clone(),
                rx: self.feedback_stream.rx.clone(),
            },
            web_assets_path: self.web_assets_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackEvent, LedEvent};
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
            key: "a".to_string(),
        }));
        packet.add_event(InputEvent::Pointer(PointerEvent::Move {
            x_delta: 10,
            y_delta: -5,
        }));
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
            key: "a".to_string(),
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

    // Test function to demonstrate feedback functionality
    // #[cfg(test)]
    // pub async fn test_feedback_broadcast(feedback_stream: &FeedbackEventStream) -> Result<()> {
    //     use crate::feedback::{FeedbackEvent, FeedbackEventPacket, HapticEvent, LedEvent};
    //     use std::time::{SystemTime, UNIX_EPOCH};

    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     // Create a test feedback packet with LED and haptic events
    //     let mut feedback_packet =
    //         FeedbackEventPacket::new("test-controller".to_string(), timestamp);

    //     // Add LED event - turn on red LED
    //     feedback_packet.add_event(FeedbackEvent::Led(LedEvent::Set {
    //         led_id: 1,
    //         on: true,
    //         brightness: Some(255),
    //         rgb: Some((255, 0, 0)), // Red
    //     }));

    //     // Add haptic event - vibrate motor 0
    //     feedback_packet.add_event(FeedbackEvent::Haptic(HapticEvent::Vibrate {
    //         motor_id: 0,
    //         intensity: 128,
    //         duration_ms: 500,
    //     }));

    //     // Send the feedback packet
    //     feedback_stream.send(feedback_packet).await?;
    //     info!("Test feedback packet sent successfully");

    //     Ok(())
    // }

    #[tokio::test]
    async fn test_unified_websocket_state() {
        let input_stream = InputEventStream::new();
        let feedback_stream = FeedbackEventStream::new();

        let state = WebSocketState {
            input_stream: input_stream.clone(),
            feedback_stream: feedback_stream.clone(),
            connected_clients: Arc::new(RwLock::new(Vec::new())),
        };

        // Test that we can create channels for both types
        let (feedback_tx, _feedback_rx) =
            tokio::sync::mpsc::unbounded_channel::<FeedbackEventPacket>();

        // Add client to broadcast list
        {
            let mut clients = state.connected_clients.write().await;
            clients.push(feedback_tx);
            assert_eq!(clients.len(), 1);
        }

        // Test input packet creation
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut input_packet = InputEventPacket::new("test-device".to_string(), timestamp);
        input_packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
            key: "a".to_string(),
        }));

        // Test feedback packet creation
        let mut feedback_packet = FeedbackEventPacket::new("test-device".to_string(), timestamp);
        feedback_packet.add_event(FeedbackEvent::Led(LedEvent::Set {
            led_id: 1,
            on: true,
            brightness: Some(255),
            rgb: Some((255, 0, 0)),
        }));

        // Verify serialization works for both packet types
        let input_json = serde_json::to_string(&input_packet).unwrap();
        let feedback_json = serde_json::to_string(&feedback_packet).unwrap();

        // Verify we can parse them back
        let parsed_input: InputEventPacket = serde_json::from_str(&input_json).unwrap();
        let parsed_feedback: FeedbackEventPacket = serde_json::from_str(&feedback_json).unwrap();

        assert_eq!(parsed_input.device_id, "test-device");
        assert_eq!(parsed_feedback.device_id, "test-device");
        assert_eq!(parsed_input.events.len(), 1);
        assert_eq!(parsed_feedback.events.len(), 1);
    }

    #[tokio::test]
    async fn test_api_layouts_endpoint() {
        use std::fs;

        // Create a temporary directory structure for testing
        let temp_dir = std::env::temp_dir().join("backflow_test_layouts");
        let web_dir = &temp_dir;
        let custom_dir = web_dir.join("custom");
        let layout1_dir = custom_dir.join("chuni");
        let layout2_dir = custom_dir.join("keyboard");

        // Clean up any existing test directory
        let _ = fs::remove_dir_all(&temp_dir);

        fs::create_dir_all(&layout1_dir).unwrap();
        fs::create_dir_all(&layout2_dir).unwrap();

        // Create HTML files in each layout directory
        fs::write(
            layout1_dir.join("chuni.html"),
            "<html><body>Chuni Layout</body></html>",
        )
        .unwrap();
        fs::write(
            layout2_dir.join("keys.html"),
            "<html><body>Keyboard Layout</body></html>",
        )
        .unwrap();

        // Create a web server with the temp directory
        let input_stream = InputEventStream::new();
        let feedback_stream = FeedbackEventStream::new();
        let mut server = WebServer::auto_detect_web_ui(
            "127.0.0.1:0".parse().unwrap(),
            input_stream,
            feedback_stream,
        );
        // Manually set the web assets path for testing
        server.web_assets_path = Some(web_dir.to_path_buf());

        // Test the list_custom_layouts method
        let layouts = server.list_custom_layouts();

        assert_eq!(layouts.len(), 2);

        // Sort to ensure consistent ordering for testing
        let mut sorted_layouts = layouts;
        sorted_layouts.sort_by(|a, b| a.name.cmp(&b.name));

        assert_eq!(sorted_layouts[0].name, "chuni");
        assert_eq!(sorted_layouts[0].path, "custom/chuni/chuni.html");

        assert_eq!(sorted_layouts[1].name, "keyboard");
        assert_eq!(sorted_layouts[1].path, "custom/keyboard/keys.html");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_web_template_response_serialization() {
        let response = WebTemplateResponse {
            path: "custom/chuni/layout.html".to_string(),
            name: "chuni".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        let expected = r#"{"path":"custom/chuni/layout.html","name":"chuni"}"#;
        assert_eq!(json, expected);

        // Test deserialization
        let deserialized: WebTemplateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, "custom/chuni/layout.html");
        assert_eq!(deserialized.name, "chuni");
    }
}

/// Example usage of the WebServer.
pub mod examples {
    /// WebSocket Bidirectional Protocol
    ///
    /// The WebSocket endpoint at `/ws` supports bidirectional communication:
    /// - **Input (Client → Server)**: Clients send JSON messages in the format of [`InputEventPacket`] for user input processing
    /// - **Feedback (Client → All Clients)**: Clients can send JSON messages in the format of [`FeedbackEventPacket`] to be broadcasted to all connected clients
    /// - **Feedback (Server → Client)**: Server broadcasts JSON messages in the format of [`FeedbackEventPacket`] to all connected clients
    ///
    /// ## Input Message Example (Client sends to Server)
    ///
    /// Send user input events like keyboard presses, mouse movements, etc:
    ///
    /// ```json
    /// {
    ///   "device_id": "my-device",
    ///   "timestamp": 1672531200000,
    ///   "events": [
    ///     {
    ///       "Keyboard": {
    ///         "KeyPress": {
    ///           "key": "a"
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
    /// ## Client Feedback Broadcast Example (Client broadcasts to All Clients)
    ///
    /// Send feedback events to be announced to all connected clients:
    ///
    /// ```json
    /// {
    ///   "device_id": "my-custom-device",
    ///   "timestamp": 1672531200000,
    ///   "events": [
    ///     {
    ///       "Led": {
    ///         "Set": {
    ///           "led_id": 42,
    ///           "on": true,
    ///           "brightness": 255,
    ///           "rgb": [255, 0, 0]
    ///         }
    ///       }
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// ## Feedback Message Example (Server broadcasts to Clients)
    ///
    /// Server-generated feedback events are automatically sent to all clients:
    ///
    /// ```json
    /// {
    ///   "device_id": "controller-001",
    ///   "timestamp": 1672531205000,
    ///   "events": [
    ///     {
    ///       "Led": {
    ///         "Set": {
    ///           "led_id": 1,
    ///           "on": true,
    ///           "brightness": 200,
    ///           "rgb": [255, 0, 0]
    ///         }
    ///       }
    ///     },
    ///     {
    ///       "Haptic": {
    ///         "Vibrate": {
    ///           "motor_id": 0,
    ///           "intensity": 128,
    ///           "duration_ms": 500
    ///         }
    ///       }
    ///     }
    ///   ]
    /// }
    /// ```
    ///
    /// ## Connection Behavior
    ///
    /// - All feedback messages are broadcast to ALL connected WebSocket clients
    /// - Clients can send input events at any time
    /// - Server will send feedback events to all clients when available
    /// - Connection supports standard WebSocket ping/pong for keepalive
    pub fn _doc_example() {}
}

/// Bidirectional WebSocket handler that processes input events from clients
/// and broadcasts feedback events to all connected clients.
pub async fn bidirectional_ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<WebSocketState>,
) -> Response {
    info!("New WebSocket connection from {}", addr);
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

/// Handles a single WebSocket connection for bidirectional communication.
async fn handle_socket(socket: WebSocket, addr: SocketAddr, state: WebSocketState) {
    let (sender, mut receiver) = socket.split();

    // Create a channel for this specific client to receive feedback
    let (feedback_tx, mut feedback_rx) =
        tokio::sync::mpsc::unbounded_channel::<FeedbackEventPacket>();

    // Add this client to the list of connected clients for feedback broadcasting
    {
        let mut clients = state.connected_clients.write().await;
        clients.push(feedback_tx);
        info!(
            "Client {} added to feedback broadcast list. Total clients: {}",
            addr,
            clients.len()
        );
    }

    // Clone state for the tasks
    let input_stream = state.input_stream.clone();
    let feedback_stream_for_client = state.feedback_stream.clone();

    // Wrap sender in Arc<Mutex> to share between tasks
    let sender = Arc::new(tokio::sync::Mutex::new(sender));

    // Task to handle incoming messages (input events from client)
    let input_task = {
        let sender = sender.clone();
        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received message from {}: {}", addr, text);

                        // Try to parse as InputEventPacket first
                        if let Ok(packet) = serde_json::from_str::<InputEventPacket>(&text) {
                            debug!(
                                "Parsed input packet from {}: device_id={}, events={}",
                                addr,
                                packet.device_id,
                                packet.events.len()
                            );

                            if let Err(e) = input_stream.send(packet).await {
                                error!("Failed to send input event to stream: {}", e);
                            }
                        }
                        // If not an input packet, try to parse as FeedbackEventPacket for broadcasting
                        else if let Ok(feedback_packet) =
                            serde_json::from_str::<FeedbackEventPacket>(&text)
                        {
                            info!(
                                "Received feedback packet from {} for broadcasting: device_id={}, events={}",
                                addr,
                                feedback_packet.device_id,
                                feedback_packet.events.len()
                            );

                            // Instead of broadcasting directly here, send it through the main feedback stream
                            // This ensures all feedback goes through the same unified broadcast mechanism
                            if let Err(e) = feedback_stream_for_client.send(feedback_packet).await {
                                error!("Failed to send client feedback to main stream: {}", e);
                            } else {
                                debug!(
                                    "Client feedback packet from {} forwarded to main broadcast system",
                                    addr
                                );
                            }
                        } else {
                            warn!(
                                "Failed to parse message from {} as either input or feedback packet",
                                addr
                            );
                        }
                    }
                    Ok(Message::Binary(_)) => {
                        warn!("Received unexpected binary message from {}", addr);
                    }
                    Ok(Message::Ping(data)) => {
                        debug!("Received ping from {}", addr);
                        let mut sender_guard = sender.lock().await;
                        if let Err(e) = sender_guard.send(Message::Pong(data)).await {
                            warn!("Failed to send pong to {}: {}", addr, e);
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {
                        debug!("Received pong from {}", addr);
                    }
                    Ok(Message::Close(_)) => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                    Err(e) => {
                        warn!("WebSocket error from {}: {}", addr, e);
                        break;
                    }
                }
            }

            // Note: Client cleanup from the broadcast list happens automatically
            // when the feedback channel is dropped and detected as closed
            debug!("Input task for client {} finished", addr);
        })
    };

    // Task to handle outgoing messages (feedback events to client)
    let output_task = {
        let sender = sender.clone();
        tokio::spawn(async move {
            while let Some(feedback_packet) = feedback_rx.recv().await {
                let start_time = std::time::Instant::now();
                trace!(
                    target: crate::PACKET_PROCESSING_TARGET,
                    "Sending feedback packet to {}: device_id={}, events={}",
                    addr,
                    feedback_packet.device_id,
                    feedback_packet.events.len()
                );

                match serde_json::to_string(&feedback_packet) {
                    Ok(json) => {
                        let mut sender_guard = sender.lock().await;
                        if let Err(e) = sender_guard.send(Message::Text(json.into())).await {
                            warn!("Failed to send feedback to {}: {}", addr, e);
                            break;
                        }
                        // Explicitly flush the WebSocket to ensure immediate sending
                        if let Err(e) = sender_guard.flush().await {
                            warn!("Failed to flush WebSocket for {}: {}", addr, e);
                            break;
                        }

                        let elapsed = start_time.elapsed();
                        if elapsed.as_millis() > 5 {
                            debug!("Slow WebSocket send to {}: {}ms", addr, elapsed.as_millis());
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize feedback packet for {}: {}", addr, e);
                    }
                }
            }
        })
    };

    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        _ = input_task => {
            debug!("Input task completed for {}", addr);
        }
        _ = output_task => {
            debug!("Output task completed for {}", addr);
        }
    }

    info!("WebSocket connection {} closed", addr);
}

/// REST API handler for listing custom layouts
async fn get_custom_layouts(
    State(server): State<Arc<WebServer>>,
) -> Json<Vec<WebTemplateResponse>> {
    Json(server.list_custom_layouts())
}

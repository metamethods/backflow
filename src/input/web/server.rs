//! Web server implementation for both WebSocket input handling and web UI serving.
//! Uses axum framework with tower middleware support.

use crate::input::websocket::ws_handler;
use crate::input::{InputBackend, InputEventPacket, InputEventStream};
use axum::{
    Router,
    extract::{
        ConnectInfo, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    routing::get,
};
use eyre::{Context, Result};
use std::{net::SocketAddr, path::PathBuf};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};
use tracing::{info, warn};

use super::frontend;

/// Web server that handles both WebSocket connections for input events
/// and serves static files for the web UI.
pub struct WebServer {
    /// The address to bind the server to
    bind_addr: SocketAddr,
    /// The input event stream to send received events to
    event_stream: InputEventStream,
    /// Path to the directory containing web UI assets
    web_assets_path: Option<PathBuf>,
}

impl WebServer {
    /// Creates a new web server for WebSocket input handling only.
    ///
    /// # Arguments
    /// * `bind_addr` - The address to bind the server to
    /// * `event_stream` - The input event stream to send received events to
    pub fn new(bind_addr: SocketAddr, event_stream: InputEventStream) -> Self {
        Self {
            bind_addr,
            event_stream,
            web_assets_path: None,
        }
    }

    /// Creates a new web server with web UI hosting.
    ///
    /// # Arguments
    /// * `bind_addr` - The address to bind the server to
    /// * `event_stream` - The input event stream to send received events to
    /// * `web_assets_path` - Path to the directory containing web UI assets
    pub fn with_web_ui(
        bind_addr: SocketAddr,
        event_stream: InputEventStream,
        web_assets_path: PathBuf,
    ) -> Self {
        Self {
            bind_addr,
            event_stream,
            web_assets_path: Some(web_assets_path),
        }
    }

    /// Automatically detect and use web UI if available.
    ///
    /// # Arguments
    /// * `bind_addr` - The address to bind the server to
    /// * `event_stream` - The input event stream to send received events to
    ///
    /// This method checks the `WEB_UI_PATH` environment variable for a configured path,
    /// and falls back to searching for a `web` directory in the current working directory
    pub fn auto_detect_web_ui(bind_addr: SocketAddr, event_stream: InputEventStream) -> Self {
        let configured_path = std::env::var("WEB_UI_PATH").ok().map(PathBuf::from);

        let web_assets_path = frontend::find_web_ui_dir(configured_path);
        Self {
            bind_addr,
            event_stream,
            web_assets_path,
        }
    }

    /// Build the application router with all routes
    fn build_router(&self) -> Router {
        // Create the base router with the WebSocket endpoint
        let mut router = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(self.event_stream.clone());

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
        router = router.layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

        router
    }
}

#[async_trait::async_trait]
impl InputBackend for WebServer {
    async fn run(&mut self) -> Result<()> {
        let router = self.build_router();

        info!("Starting web server on {}", self.bind_addr);

        // Create TCP listener
        let listener = tokio::net::TcpListener::bind(self.bind_addr)
            .await
            .context("Failed to bind to address")?;

        // Run the server with graceful shutdown
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl+c");
            info!("Shutting down web server...");
        })
        .await
        .context("Server error")?;

        Ok(())
    }
}

impl Clone for WebServer {
    fn clone(&self) -> Self {
        Self {
            bind_addr: self.bind_addr,
            event_stream: InputEventStream {
                tx: self.event_stream.tx.clone(),
                rx: self.event_stream.rx.clone(),
            },
            web_assets_path: self.web_assets_path.clone(),
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
}

/// Example usage of the WebServer.
pub mod examples {
    /// WebSocket Input Protocol
    ///
    /// The WebSocket endpoint at `/ws` expects JSON messages in the format of [`InputEventPacket`].
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
    pub fn _doc_example() {}
}

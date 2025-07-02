//! proxy socket for chuniio passthrough
//!
//! This backend output exposes a dedicated Unix Domain Socket
//! that can be used to send input events as a virtual CHUNITHM controller.
//!
//!
//! This requires a dedicated chuniio.dll implementation that actually proxies
//! all input events to this socket.
//!
//! The socket is created at `/run/user/<uid>/backflow_chuniio`
//! and will pass through all the normal chuniio poll and feedback events.
//!
//! ## Special Keycodes
//!
//! The following keycodes are exclusive to this backend and when passed,
//! will be sent to the game directly through the socket/chuniio
//!
//! - `CHUNIIO_COIN` - Coin input, equivalent to inserting a credit
//! - `CHUNIIO_TEST` - Test mode input, equivalent to pressing the test button
//! - `CHUNIIO_SERVICE` - Service mode input, equivalent to pressing the service button
//! - `CHUNIIO_SLIDER_[0-31]` - Slider input, equivalent to pressing one of the 32 slider touch zones
//! - `CHUNIIO_IR_[0-5]` - IR input, equivalent to blocking one of the 6 IR sensors
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use plumbershim::output::chuniio_proxy::{ChuniioProxyServer, create_chuniio_channels};
//! use plumbershim::protos::chuniio::{ChuniInputEvent, ChuniFeedbackEvent};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     // Create channels for input/output communication
//!     let (input_tx, mut input_rx, feedback_tx, _feedback_rx) = create_chuniio_channels();
//!
//!     // Create proxy server
//!     let mut server = ChuniioProxyServer::new(
//!         Some(PathBuf::from("/tmp/chuniio.sock")),
//!         input_tx.clone(),
//!         feedback_tx.clone(),
//!     );
//!
//!     // Start server in background task
//!     tokio::spawn(async move {
//!         if let Err(e) = server.start().await {
//!             eprintln!("Server error: {}", e);
//!         }
//!     });
//!
//!     // Handle input events from clients
//!     while let Some(event) = input_rx.recv().await {
//!         match event {
//!             ChuniInputEvent::CoinInsert => println!("Coin inserted!"),
//!             ChuniInputEvent::OperatorButton { button, pressed } => {
//!                 println!("Button {} {}", button, if pressed { "pressed" } else { "released" });
//!             }
//!             ChuniInputEvent::SliderTouch { region, pressure } => {
//!                 println!("Slider region {} touched with pressure {}", region, pressure);
//!             }
//!             ChuniInputEvent::IrBeam { beam, broken } => {
//!                 println!("IR beam {} {}", beam, if broken { "broken" } else { "restored" });
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Protocol
//!
//! The chuniio proxy uses a binary protocol with the following message types:
//!
//! - `JvsPoll` (0x01) - Request current JVS state from proxy
//! - `JvsPollResponse` (0x02) - Response with operator buttons and IR beam state  
//! - `CoinCounterRead` (0x03) - Request current coin count
//! - `CoinCounterReadResponse` (0x04) - Response with coin count
//! - `SliderInput` (0x05) - Slider pressure data (32 bytes, one per region)
//! - `SliderLedUpdate` (0x06) - Update slider LEDs (RGB data)
//! - `LedUpdate` (0x07) - Update billboard/air tower LEDs
//! - `Ping` (0x08) - Keepalive ping
//! - `Pong` (0x09) - Keepalive response
//!
//! All multi-byte integers are transmitted in little-endian format.

use crate::feedback::FeedbackEventStream;
use crate::input::{InputEvent, InputEventPacket, InputEventStream, KeyboardEvent};
use crate::output::OutputBackend;
use crate::protos::chuniio::{
    ChuniFeedbackEvent, ChuniInputEvent, ChuniMessage, ChuniProtocolState,
    CHUNI_IO_OPBTN_COIN, CHUNI_IO_OPBTN_SERVICE, CHUNI_IO_OPBTN_TEST,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

/// Default socket path for chuniio proxy
const DEFAULT_SOCKET_PATH: &str = "/tmp/backflow_chuniio.sock";

/// Chuniio proxy server that handles bidirectional communication
pub struct ChuniioProxyServer {
    socket_path: PathBuf,
    protocol_state: Arc<RwLock<ChuniProtocolState>>,
    next_client_id: Arc<RwLock<u64>>,
    input_stream: InputEventStream,
    feedback_stream: FeedbackEventStream,
}

impl ChuniioProxyServer {
    /// Create a new chuniio proxy server
    pub fn new(
        socket_path: Option<PathBuf>,
        input_stream: InputEventStream,
        feedback_stream: FeedbackEventStream,
    ) -> Self {
        let socket_path = socket_path.unwrap_or_else(|| {
            // Try to use user runtime directory, fallback to /tmp
            if let Ok(uid) = std::env::var("UID") {
                let runtime_path = format!("/run/user/{}/backflow_chuniio.sock", uid);
                if std::path::Path::new(&format!("/run/user/{}", uid)).exists() {
                    PathBuf::from(runtime_path)
                } else {
                    PathBuf::from(DEFAULT_SOCKET_PATH)
                }
            } else {
                PathBuf::from(DEFAULT_SOCKET_PATH)
            }
        });

        Self {
            socket_path,
            protocol_state: Arc::new(RwLock::new(ChuniProtocolState::new())),
            next_client_id: Arc::new(RwLock::new(0)),
            input_stream,
            feedback_stream,
        }
    }
}

impl OutputBackend for ChuniioProxyServer {
    async fn run(&mut self) -> eyre::Result<()> {
        tracing::info!("Starting chuniio proxy output backend");
        
        // Create channels for internal communication
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<ChuniInputEvent>();
        let (feedback_tx, mut feedback_rx) = mpsc::unbounded_channel::<ChuniFeedbackEvent>();
        
        // Start the socket server in a background task
        let socket_path = self.socket_path.clone();
        let protocol_state = Arc::clone(&self.protocol_state);
        let next_client_id = Arc::clone(&self.next_client_id);
        let input_tx_clone = input_tx.clone();
        let feedback_tx_clone = feedback_tx.clone();
        
        let server_handle = tokio::spawn(async move {
            let mut server = InternalChuniioProxyServer {
                socket_path,
                protocol_state,
                next_client_id,
                input_tx: input_tx_clone,
                feedback_tx: feedback_tx_clone,
            };
            if let Err(e) = server.start_server().await {
                tracing::error!("Chuniio proxy server error: {}", e);
            }
        });
        
        // Start feedback handler task
        let feedback_handle = tokio::spawn(async move {
            while let Some(feedback_event) = feedback_rx.recv().await {
                // Convert feedback to chuniio events and forward to feedback stream
                match feedback_event {
                    ChuniFeedbackEvent::SliderLeds { rgb_data } => {
                        // Convert to feedback stream format if needed
                        // For now, just log it
                        tracing::debug!("Received slider LED feedback: {} bytes", rgb_data.len());
                    }
                    ChuniFeedbackEvent::LedBoard { board, rgb_data } => {
                        tracing::debug!("Received LED board {} feedback: {} bytes", board, rgb_data.len());
                    }
                }
            }
        });
        
        // Main loop: convert input events to chuniio events
        loop {
            tokio::select! {
                // Handle input events from the application
                packet = self.input_stream.receive() => {
                    match packet {
                        Some(packet) => {
                            if let Err(e) = self.process_input_packet(packet, &input_tx).await {
                                tracing::error!("Failed to process input packet: {}", e);
                            }
                        }
                        None => {
                            tracing::info!("Input stream closed, stopping chuniio proxy backend");
                            break;
                        }
                    }
                }
                
                // Handle chuniio input events from clients
                chuni_event = input_rx.recv() => {
                    match chuni_event {
                        Some(event) => {
                            // Update internal state
                            let mut state = self.protocol_state.write().await;
                            state.process_input_event(event);
                        }
                        None => {
                            tracing::debug!("Chuniio input channel closed");
                        }
                    }
                }
            }
        }
        
        // Cleanup
        server_handle.abort();
        feedback_handle.abort();
        
        Ok(())
    }
    
    async fn stop(&mut self) -> eyre::Result<()> {
        tracing::info!("Stopping chuniio proxy output backend");
        Ok(())
    }
}

impl ChuniioProxyServer {
    /// Create internal server with channels (used by the OutputBackend implementation)
    fn new_internal(
        socket_path: PathBuf,
        input_tx: mpsc::UnboundedSender<ChuniInputEvent>,
        feedback_tx: mpsc::UnboundedSender<ChuniFeedbackEvent>,
        protocol_state: Arc<RwLock<ChuniProtocolState>>,
        next_client_id: Arc<RwLock<u64>>,
    ) -> InternalChuniioProxyServer {
        InternalChuniioProxyServer {
            socket_path,
            protocol_state,
            next_client_id,
            input_tx,
            feedback_tx,
        }
    }
    
    /// Process input packets from the application and convert to chuniio events
    async fn process_input_packet(
        &mut self,
        packet: InputEventPacket,
        input_tx: &mpsc::UnboundedSender<ChuniInputEvent>,
    ) -> eyre::Result<()> {
        for event in packet.events {
            match event {
                InputEvent::Keyboard(KeyboardEvent::KeyPress { key }) => {
                    if let Some(chuni_event) = self.keyboard_to_chuniio_event(&key, true).await {
                        let _ = input_tx.send(chuni_event);
                    }
                }
                InputEvent::Keyboard(KeyboardEvent::KeyRelease { key }) => {
                    if let Some(chuni_event) = self.keyboard_to_chuniio_event(&key, false).await {
                        let _ = input_tx.send(chuni_event);
                    }
                }
                // Handle other input types as needed
                _ => {}
            }
        }
        Ok(())
    }
    
    /// Convert keyboard events to chuniio events
    async fn keyboard_to_chuniio_event(&mut self, key: &str, pressed: bool) -> Option<ChuniInputEvent> {
        match key {
            "CHUNIIO_COIN" => Some(ChuniInputEvent::CoinInsert),
            "CHUNIIO_TEST" => Some(ChuniInputEvent::OperatorButton {
                button: CHUNI_IO_OPBTN_TEST,
                pressed,
            }),
            "CHUNIIO_SERVICE" => Some(ChuniInputEvent::OperatorButton {
                button: CHUNI_IO_OPBTN_SERVICE,
                pressed,
            }),
            key if key.starts_with("CHUNIIO_SLIDER_") => {
                if let Ok(region) = key.strip_prefix("CHUNIIO_SLIDER_")?.parse::<u8>() {
                    if region < 32 {
                        Some(ChuniInputEvent::SliderTouch {
                            region,
                            pressure: if pressed { 255 } else { 0 },
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            key if key.starts_with("CHUNIIO_IR_") => {
                if let Ok(beam) = key.strip_prefix("CHUNIIO_IR_")?.parse::<u8>() {
                    if beam < 6 {
                        Some(ChuniInputEvent::IrBeam {
                            beam,
                            broken: pressed,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Internal server structure that matches the original implementation
struct InternalChuniioProxyServer {
    socket_path: PathBuf,
    protocol_state: Arc<RwLock<ChuniProtocolState>>,
    next_client_id: Arc<RwLock<u64>>,
    input_tx: mpsc::UnboundedSender<ChuniInputEvent>,
    feedback_tx: mpsc::UnboundedSender<ChuniFeedbackEvent>,
}

impl InternalChuniioProxyServer {
    /// Create internal server instance
    fn new(
        socket_path: PathBuf,
        protocol_state: Arc<RwLock<ChuniProtocolState>>,
        next_client_id: Arc<RwLock<u64>>,
        input_tx: mpsc::UnboundedSender<ChuniInputEvent>,
        feedback_tx: mpsc::UnboundedSender<ChuniFeedbackEvent>,
    ) -> Self {
        Self {
            socket_path,
            protocol_state,
            next_client_id,
            input_tx,
            feedback_tx,
        }
    }

    /// Start the internal server
    async fn start_server(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Remove existing socket file if it exists
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Create the socket listener
        let listener = UnixListener::bind(&self.socket_path)?;
        info!("Chuniio proxy server listening on {:?}", self.socket_path);

        // Make socket accessible to everyone (for compatibility with Windows DLL)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&self.socket_path)?.permissions();
            perms.set_mode(0o666);
            std::fs::set_permissions(&self.socket_path, perms)?;
        }

        // Accept client connections
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let client_id = {
                        let mut next_id = self.next_client_id.write().await;
                        let id = *next_id;
                        *next_id += 1;
                        id
                    };

                    info!("New chuniio client connected: {}", client_id);

                    // Spawn client handler
                    let protocol_state = Arc::clone(&self.protocol_state);
                    let input_tx = self.input_tx.clone();
                    let feedback_tx = self.feedback_tx.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(
                            client_id,
                            stream,
                            protocol_state,
                            input_tx,
                            feedback_tx,
                        )
                        .await
                        {
                            warn!("Client {} error: {}", client_id, e);
                        }
                        info!("Client {} disconnected", client_id);
                    });
                }
                Err(e) => {
                    error!("Failed to accept client connection: {}", e);
                }
            }
        }
    }

    /// Handle a single client connection with bidirectional communication
    async fn handle_client(
        client_id: u64,
        stream: UnixStream,
        protocol_state: Arc<RwLock<ChuniProtocolState>>,
        input_tx: mpsc::UnboundedSender<ChuniInputEvent>,
        _feedback_tx: mpsc::UnboundedSender<ChuniFeedbackEvent>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, writer) = stream.into_split();
        let mut reader = tokio::io::BufReader::new(reader);
        let writer = Arc::new(tokio::sync::Mutex::new(writer));

        // Channel for receiving feedback events to send to this client
        let (client_feedback_tx, mut client_feedback_rx) =
            mpsc::unbounded_channel::<ChuniFeedbackEvent>();

        // Spawn task to handle outgoing feedback messages
        let writer_clone = Arc::clone(&writer);
        let feedback_task = tokio::spawn(async move {
            while let Some(feedback_event) = client_feedback_rx.recv().await {
                if let Some(message) = Self::feedback_event_to_message(feedback_event) {
                    let mut writer_guard = writer_clone.lock().await;
                    if let Err(e) = message.write_to(&mut *writer_guard).await {
                        warn!("Failed to send feedback to client {}: {}", client_id, e);
                        break;
                    }
                }
            }
        });

        // Handle incoming messages from client
        loop {
            match ChuniMessage::from_reader(&mut reader).await {
                Ok(message) => {
                    match Self::handle_client_message(
                        client_id,
                        message,
                        &protocol_state,
                        &input_tx,
                        &client_feedback_tx,
                    )
                    .await
                    {
                        Ok(Some(response)) => {
                            // Send response back to client
                            let mut writer_guard = writer.lock().await;
                            if let Err(e) = response.write_to(&mut *writer_guard).await {
                                warn!("Failed to send response to client {}: {}", client_id, e);
                                break;
                            }
                        }
                        Ok(None) => {
                            // No response needed
                        }
                        Err(e) => {
                            warn!("Error handling message from client {}: {}", client_id, e);
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        info!("Client {} disconnected", client_id);
                    } else {
                        warn!("Error reading from client {}: {}", client_id, e);
                    }
                    break;
                }
            }
        }

        feedback_task.abort();
        Ok(())
    }

    /// Handle a message from a client and optionally return a response
    async fn handle_client_message(
        _client_id: u64,
        message: ChuniMessage,
        protocol_state: &Arc<RwLock<ChuniProtocolState>>,
        input_tx: &mpsc::UnboundedSender<ChuniInputEvent>,
        _feedback_tx: &mpsc::UnboundedSender<ChuniFeedbackEvent>,
    ) -> Result<Option<ChuniMessage>, Box<dyn std::error::Error + Send + Sync>> {
        match message {
            ChuniMessage::JvsPoll => {
                // Return current JVS state
                let state = protocol_state.read().await;
                Ok(Some(ChuniMessage::JvsPollResponse {
                    opbtn: state.jvs_state.opbtn,
                    beams: state.jvs_state.beams,
                }))
            }
            ChuniMessage::CoinCounterRead => {
                // Return current coin count
                let state = protocol_state.read().await;
                Ok(Some(ChuniMessage::CoinCounterReadResponse {
                    count: state.coin_counter,
                }))
            }
            ChuniMessage::SliderInput { pressure } => {
                // Update slider state and send input events
                {
                    let mut state = protocol_state.write().await;
                    state.slider_state.pressure = pressure;
                }

                // Send slider touch events for changed regions
                for (region, &pressure_value) in pressure.iter().enumerate() {
                    if pressure_value > 0 {
                        let _ = input_tx.send(ChuniInputEvent::SliderTouch {
                            region: region as u8,
                            pressure: pressure_value,
                        });
                    }
                }
                Ok(None)
            }
            ChuniMessage::Ping => {
                Ok(Some(ChuniMessage::Pong))
            }
            _ => {
                // Other messages are responses or feedback, not handled here
                Ok(None)
            }
        }
    }

    /// Convert feedback event to chuniio message
    fn feedback_event_to_message(event: ChuniFeedbackEvent) -> Option<ChuniMessage> {
        match event {
            ChuniFeedbackEvent::SliderLeds { rgb_data } => {
                Some(ChuniMessage::SliderLedUpdate { rgb_data })
            }
            ChuniFeedbackEvent::LedBoard { board, rgb_data } => {
                Some(ChuniMessage::LedUpdate { board, rgb_data })
            }
        }
    }

    /// Send feedback event to all connected clients
    pub async fn send_feedback(
        &self,
        event: ChuniFeedbackEvent,
    ) -> Result<(), mpsc::error::SendError<ChuniFeedbackEvent>> {
        self.feedback_tx.send(event)
    }

    /// Send input event (for external use)
    pub async fn send_input(
        &self,
        event: ChuniInputEvent,
    ) -> Result<(), mpsc::error::SendError<ChuniInputEvent>> {
        // Update internal state
        {
            let mut state = self.protocol_state.write().await;
            state.process_input_event(event.clone());
        }

        // Forward to input handler
        self.input_tx.send(event)
    }
}

/// Helper function to create channels for the proxy server
pub fn create_chuniio_channels() -> (
    mpsc::UnboundedSender<ChuniInputEvent>,
    mpsc::UnboundedReceiver<ChuniInputEvent>,
    mpsc::UnboundedSender<ChuniFeedbackEvent>,
    mpsc::UnboundedReceiver<ChuniFeedbackEvent>,
) {
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (feedback_tx, feedback_rx) = mpsc::unbounded_channel();
    (input_tx, input_rx, feedback_tx, feedback_rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let input_stream = InputEventStream::new();
        let feedback_stream = FeedbackEventStream::new();
        let server = ChuniioProxyServer::new(
            Some(PathBuf::from("/tmp/test_chuniio.sock")),
            input_stream,
            feedback_stream,
        );

        assert!(
            server
                .socket_path
                .to_str()
                .unwrap()
                .contains("test_chuniio.sock")
        );
    }

    #[tokio::test]
    async fn test_feedback_message_conversion() {
        let slider_event = ChuniFeedbackEvent::SliderLeds {
            rgb_data: vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
        };
        let message = InternalChuniioProxyServer::feedback_event_to_message(slider_event);

        assert!(matches!(
            message,
            Some(ChuniMessage::SliderLedUpdate { .. })
        ));
    }
}

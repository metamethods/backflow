//! WebSocket feedback handling utilities, sends JSON-encoded feedback events
//! to connected WebSocket clients.
//!
//! This module provides functionality to broadcast feedback events to multiple
//! WebSocket connections, enabling real-time feedback delivery to devices.

use crate::feedback::{FeedbackEventPacket, FeedbackEventStream};
use axum::extract::ws::{Message, WebSocket};
use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, warn};

/// Connection ID type for tracking WebSocket connections
pub type ConnectionId = u64;

/// Represents an active WebSocket connection for feedback delivery
#[derive(Debug)]
pub struct FeedbackConnection {
    /// Unique connection ID
    pub id: ConnectionId,
    /// Client address
    pub addr: SocketAddr,
    /// Device ID filter - if Some, only send feedback for this device
    pub device_filter: Option<String>,
}

/// Manager for WebSocket feedback connections
#[derive(Debug, Clone)]
pub struct FeedbackWebSocketManager {
    /// Broadcast sender for feedback events
    feedback_tx: broadcast::Sender<FeedbackEventPacket>,
    /// Map of active connections
    connections: Arc<RwLock<HashMap<ConnectionId, FeedbackConnection>>>,
    /// Counter for generating unique connection IDs
    next_connection_id: Arc<AtomicU64>,
}

impl FeedbackWebSocketManager {
    /// Creates a new feedback WebSocket manager
    pub fn new() -> Self {
        let (feedback_tx, _) = broadcast::channel(1000); // Buffer for 1000 feedback events
        Self {
            feedback_tx,
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_connection_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Broadcasts a feedback event packet to all connected clients
    pub async fn broadcast_feedback(&self, packet: FeedbackEventPacket) -> Result<()> {
        let sent_count = self.feedback_tx.send(packet)?;
        debug!("Broadcasted feedback to {} connections", sent_count);
        Ok(())
    }

    /// Registers a new WebSocket connection for feedback delivery
    pub async fn register_connection(
        &self,
        addr: SocketAddr,
        device_filter: Option<String>,
    ) -> ConnectionId {
        let id = self.next_connection_id.fetch_add(1, Ordering::Relaxed);
        let connection = FeedbackConnection {
            id,
            addr,
            device_filter: device_filter.clone(),
        };

        let mut connections = self.connections.write().await;
        connections.insert(id, connection);

        info!(
            "Registered feedback connection {} from {} (device filter: {:?})",
            id, addr, device_filter
        );

        id
    }

    /// Unregisters a WebSocket connection
    pub async fn unregister_connection(&self, id: ConnectionId) {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(&id) {
            info!(
                "Unregistered feedback connection {} from {}",
                id, connection.addr
            );
        }
    }

    /// Creates a feedback receiver for a specific connection
    pub fn create_feedback_receiver(&self) -> broadcast::Receiver<FeedbackEventPacket> {
        self.feedback_tx.subscribe()
    }

    /// Gets the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

impl Default for FeedbackWebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Handles a WebSocket connection for feedback delivery
pub async fn handle_feedback_websocket(
    socket: WebSocket,
    manager: &FeedbackWebSocketManager,
    addr: SocketAddr,
    device_filter: Option<String>,
) -> Result<()> {
    let connection_id = manager
        .register_connection(addr, device_filter.clone())
        .await;
    let mut feedback_rx = manager.create_feedback_receiver();
    let (mut sender, mut receiver) = socket.split();

    // Create a channel for sending control messages (like pong) from the incoming task
    let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    // Spawn a task to handle incoming messages (for potential device filter updates)
    let incoming_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!(
                        "Received message from feedback connection {}: {}",
                        connection_id, text
                    );
                    // Could handle device filter updates here in the future
                }
                Ok(Message::Close(_)) => {
                    info!("Feedback WebSocket connection {} closed", connection_id);
                    break;
                }
                Ok(Message::Ping(payload)) => {
                    if let Err(e) = control_tx.send(Message::Pong(payload)) {
                        warn!(
                            "Failed to queue pong for feedback connection {}: {}",
                            connection_id, e
                        );
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    debug!("Received pong from feedback connection {}", connection_id);
                }
                Ok(Message::Binary(_)) => {
                    warn!(
                        "Received binary message from feedback connection {}, ignoring",
                        connection_id
                    );
                }
                Err(e) => {
                    error!(
                        "WebSocket error from feedback connection {}: {}",
                        connection_id, e
                    );
                    break;
                }
            }
        }
    });

    // Handle outgoing feedback events and control messages
    let outgoing_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Handle feedback packets
                feedback_result = feedback_rx.recv() => {
                    match feedback_result {
                        Ok(packet) => {
                            // Apply device filter if set
                            if let Some(ref filter) = device_filter {
                                if packet.device_id != *filter {
                                    continue;
                                }
                            }

                            // Serialize and send the feedback packet
                            match serde_json::to_string(&packet) {
                                Ok(json) => {
                                    if let Err(e) = sender.send(Message::Text(json.into())).await {
                                        error!(
                                            "Failed to send feedback to connection {}: {}",
                                            connection_id, e
                                        );
                                        break;
                                    }
                                    debug!("Sent feedback packet to connection {}", connection_id);
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to serialize feedback packet for connection {}: {}",
                                        connection_id, e
                                    );
                                }
                            }
                        }
                        Err(_) => {
                            debug!("Feedback channel closed for connection {}", connection_id);
                            break;
                        }
                    }
                }
                // Handle control messages (like pong)
                control_msg = control_rx.recv() => {
                    match control_msg {
                        Some(msg) => {
                            if let Err(e) = sender.send(msg).await {
                                warn!(
                                    "Failed to send control message to connection {}: {}",
                                    connection_id, e
                                );
                                break;
                            }
                        }
                        None => {
                            debug!("Control channel closed for connection {}", connection_id);
                            break;
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        _ = incoming_task => {},
        _ = outgoing_task => {},
    }

    // Clean up the connection
    manager.unregister_connection(connection_id).await;
    Ok(())
}

/// Background task to process feedback events from the feedback stream and broadcast them
pub async fn feedback_broadcaster_task(
    feedback_stream: FeedbackEventStream,
    manager: FeedbackWebSocketManager,
) -> Result<()> {
    info!("Starting feedback broadcaster task");

    loop {
        match feedback_stream.receive() {
            Ok(packet) => {
                debug!(
                    "Broadcasting feedback packet from device '{}' with {} events",
                    packet.device_id,
                    packet.events.len()
                );

                if let Err(e) = manager.broadcast_feedback(packet).await {
                    error!("Failed to broadcast feedback: {}", e);
                }
            }
            Err(e) => {
                error!("Error receiving feedback from stream: {}", e);
                // On channel closed, exit the task
                break;
            }
        }
    }

    info!("Feedback broadcaster task terminated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::{FeedbackEvent, LedEvent};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn test_feedback_manager_creation() {
        let manager = FeedbackWebSocketManager::new();
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_registration() {
        let manager = FeedbackWebSocketManager::new();
        let addr = "127.0.0.1:8080".parse().unwrap();

        let id1 = manager.register_connection(addr, None).await;
        let id2 = manager
            .register_connection(addr, Some("device-1".to_string()))
            .await;

        assert_eq!(manager.connection_count().await, 2);
        assert_ne!(id1, id2);

        manager.unregister_connection(id1).await;
        assert_eq!(manager.connection_count().await, 1);
    }

    #[tokio::test]
    async fn test_feedback_broadcast() {
        let manager = FeedbackWebSocketManager::new();
        let mut rx = manager.create_feedback_receiver();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut packet = FeedbackEventPacket::new("test-device".to_string(), timestamp);
        packet.add_event(FeedbackEvent::Led(LedEvent::Set {
            led_id: 1,
            on: true,
            brightness: Some(255),
            rgb: None,
        }));

        // Broadcast the packet
        manager.broadcast_feedback(packet.clone()).await.unwrap();

        // Receive and verify
        let received = rx.recv().await.unwrap();
        assert_eq!(received.device_id, packet.device_id);
        assert_eq!(received.events.len(), packet.events.len());
    }
}

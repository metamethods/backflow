use crate::feedback::{FeedbackEventPacket, FeedbackEventStream};
use crate::input::{InputBackend, InputEventPacket, InputEventStream};
use eyre::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub struct UnixSocketServer {
    pub socket_path: PathBuf,
    pub event_stream: InputEventStream,
    pub feedback_stream: FeedbackEventStream,
    pub connected_clients:
        Arc<RwLock<Vec<tokio::sync::mpsc::UnboundedSender<FeedbackEventPacket>>>>,
}

impl UnixSocketServer {
    pub fn new(
        socket_path: PathBuf,
        event_stream: InputEventStream,
        feedback_stream: FeedbackEventStream,
    ) -> Self {
        Self {
            socket_path,
            event_stream,
            feedback_stream,
            connected_clients: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn handle_client(&self, stream: UnixStream) {
        let (reader, writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let mut writer = BufWriter::new(writer);
        let (feedback_tx, mut feedback_rx) =
            tokio::sync::mpsc::unbounded_channel::<FeedbackEventPacket>();

        // Add this client to the feedback broadcast list
        {
            let mut clients = self.connected_clients.write().await;
            clients.push(feedback_tx);
            info!(
                "UNIX socket client added to feedback broadcast list. Total clients: {}",
                clients.len()
            );
        }

        let input_stream = self.event_stream.clone();
        let feedback_stream_for_client = self.feedback_stream.clone();
        let connected_clients = self.connected_clients.clone();

        // Task to handle incoming messages (input/feedback from client)
        let input_task = tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                // Try to parse as InputEventPacket first
                if let Ok(packet) = serde_json::from_str::<InputEventPacket>(&line) {
                    debug!(
                        "Received input packet from unix socket: device_id={}, events={}",
                        packet.device_id,
                        packet.events.len()
                    );
                    if let Err(e) = input_stream.send(packet).await {
                        error!("Failed to send input event to stream: {}", e);
                    }
                } else if let Ok(feedback_packet) =
                    serde_json::from_str::<FeedbackEventPacket>(&line)
                {
                    info!(
                        "Received feedback packet from unix socket client for broadcast: device_id={}, events={}",
                        feedback_packet.device_id,
                        feedback_packet.events.len()
                    );
                    if let Err(e) = feedback_stream_for_client.send(feedback_packet).await {
                        error!("Failed to send client feedback to main stream: {}", e);
                    }
                } else {
                    warn!("Failed to parse message from unix socket as input or feedback packet");
                }
            }
            debug!("UNIX socket client input task finished");
        });

        // Task to handle outgoing feedback events to client
        let output_task = tokio::spawn(async move {
            while let Some(feedback_packet) = feedback_rx.recv().await {
                match serde_json::to_string(&feedback_packet) {
                    Ok(json) => {
                        if let Err(e) = writer.write_all(json.as_bytes()).await {
                            warn!("Failed to send feedback to unix socket client: {}", e);
                            break;
                        }
                        if let Err(e) = writer.write_all(b"\n").await {
                            warn!("Failed to send newline to unix socket client: {}", e);
                            break;
                        }
                        if let Err(e) = writer.flush().await {
                            warn!("Failed to flush unix socket client: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to serialize feedback packet for unix socket client: {}",
                            e
                        );
                    }
                }
            }
            debug!("UNIX socket client output task finished");
        });

        // Wait for either task to complete
        let _ = tokio::join!(input_task, output_task);

        // Remove client from broadcast list
        {
            let mut clients = connected_clients.write().await;
            clients.retain(|tx| !tx.is_closed());
            info!(
                "UNIX socket client removed from feedback broadcast list. Remaining: {}",
                clients.len()
            );
        }
    }
}

#[async_trait::async_trait]
impl InputBackend for UnixSocketServer {
    async fn run(&mut self) -> Result<()> {
        // Remove any existing socket file
        let _ = std::fs::remove_file(&self.socket_path);
        let listener = UnixListener::bind(&self.socket_path).with_context(|| {
            format!("Failed to bind unix socket: {}", self.socket_path.display())
        })?;
        info!(
            "UNIX socket input backend listening at {}",
            self.socket_path.display()
        );

        // Feedback broadcast task
        let clients = self.connected_clients.clone();
        let feedback_stream = self.feedback_stream.clone();
        tokio::spawn(async move {
            while let Some(feedback_packet) = feedback_stream.receive().await {
                let clients_to_send = {
                    let clients_guard = clients.read().await;
                    clients_guard.clone()
                };
                for tx in &clients_to_send {
                    if !tx.is_closed() {
                        let _ = tx.send(feedback_packet.clone());
                    }
                }
            }
        });

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let handler = self.clone();
                    tokio::spawn(async move {
                        handler.handle_client(stream).await;
                    });
                }
                Err(e) => {
                    error!("Failed to accept unix socket connection: {}", e);
                }
            }
        }
    }
}

impl Clone for UnixSocketServer {
    fn clone(&self) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            event_stream: self.event_stream.clone(),
            feedback_stream: self.feedback_stream.clone(),
            connected_clients: self.connected_clients.clone(),
        }
    }
}

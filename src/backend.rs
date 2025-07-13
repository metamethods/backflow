//! Backend module for Plumbershim
//!
//! This module handles taking in a configuration, parsing it, and setting up the backend services.

use crate::config::AppConfig;
use crate::device_filter::DeviceFilter;
use crate::feedback::FeedbackEventStream;
use crate::feedback::generators::chuni_jvs::ChuniLedDataPacket;
use crate::input::{InputBackend, InputEventStream};
use crate::output::{OutputBackend, OutputBackendType};
use eyre::Result;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Message streams container for passing around shared streams
pub struct MessageStreams {
    pub input: InputEventStream,
    pub feedback: FeedbackEventStream,
    /// Transformed input stream (after device filtering)
    pub transformed_input: InputEventStream,
}

/// Represents the actual backend service
pub struct Backend {
    config: AppConfig,
    streams: MessageStreams,
    device_filter: DeviceFilter,

    // Service handles for graceful shutdown
    input_handles: Vec<JoinHandle<()>>,
    filter_handle: Option<JoinHandle<()>>,
    output_handles: Vec<JoinHandle<()>>,
    feedback_handles: Vec<JoinHandle<()>>,
}

impl Backend {
    /// Create a new backend from configuration
    pub fn new(config: AppConfig) -> Self {
        let device_filter = DeviceFilter::new(&config);
        let streams = MessageStreams {
            input: InputEventStream::new(),
            feedback: FeedbackEventStream::new(),
            transformed_input: InputEventStream::new(),
        };

        Self {
            config,
            streams,
            device_filter,
            input_handles: Vec::new(),
            filter_handle: None,
            output_handles: Vec::new(),
            feedback_handles: Vec::new(),
        }
    }

    /// Start all configured backend services
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting backend services...");

        let (led_packet_tx, led_packet_rx) =
            crate::feedback::generators::chuni_jvs::create_chuni_led_channel();

        // Start device filter service
        self.start_device_filter_service().await?;

        // Start input services
        self.start_input_services().await?;

        // Start output services
        self.start_output_services(led_packet_tx).await?;

        // Start feedback services
        self.start_feedback_services(led_packet_rx).await?;

        tracing::info!("All backend services started successfully");
        Ok(())
    }

    /// Start input services based on configuration
    async fn start_input_services(&mut self) -> Result<()> {
        // Start web input backend if configured and enabled
        if let Some(web_config) = &self.config.input.web {
            if web_config.enabled {
                tracing::info!(
                    "Starting web input backend on {}:{}",
                    web_config.host,
                    web_config.port
                );

                let bind_addr: SocketAddr = format!("{}:{}", web_config.host, web_config.port)
                    .parse()
                    .map_err(|e| eyre::eyre!("Invalid web backend address: {}", e))?;

                let input_stream = self.streams.input.clone();
                let feedback_stream = self.streams.feedback.clone();

                let handle = tokio::spawn(async move {
                    use crate::input::web::WebServer;
                    let mut ws_backend =
                        WebServer::auto_detect_web_ui(bind_addr, input_stream, feedback_stream);
                    if let Err(e) = ws_backend.run().await {
                        tracing::error!("WebSocket backend error: {}", e);
                    }
                });

                self.input_handles.push(handle);
            } else {
                tracing::info!("Web input backend is disabled");
            }
        }

        // TODO: Add unix domain socket input backend
        if let Some(unix_config) = &self.config.input.unix {
            tracing::info!(
                "Starting unix socket input backend at {}",
                unix_config.path.display()
            );
            let input_stream = self.streams.input.clone();
            let feedback_stream = self.streams.feedback.clone();
            let socket_path = unix_config.path.clone();
            let handle = tokio::spawn(async move {
                use crate::input::unix_socket::UnixSocketServer;
                let mut unix_backend =
                    UnixSocketServer::new(socket_path, input_stream, feedback_stream);
                if let Err(e) = unix_backend.run().await {
                    tracing::error!("Unix socket backend error: {}", e);
                }
            });
            self.input_handles.push(handle);
        }

        // Start Brokenithm TCP client backend if configured and enabled
        if let Some(brokenithm_config) = &self.config.input.brokenithm {
            if brokenithm_config.enabled {
                tracing::info!(
                    "Starting Brokenithm TCP client backend to {}:{}",
                    brokenithm_config.host,
                    brokenithm_config.port
                );
                let connect_addr: SocketAddr =
                    format!("{}:{}", brokenithm_config.host, brokenithm_config.port)
                        .parse()
                        .map_err(|e| {
                            eyre::eyre!("Invalid Brokenithm TCP backend address: {}", e)
                        })?;
                let input_stream = self.streams.input.clone();
                let handle = tokio::spawn(async move {
                    use crate::input::brokenithm::BrokenithmTcpClient;
                    let mut backend = BrokenithmTcpClient::new(connect_addr, input_stream);
                    if let Err(e) = backend.run().await {
                        tracing::error!("Brokenithm TCP client backend error: {}", e);
                    }
                });
                self.input_handles.push(handle);
            } else {
                tracing::info!("Brokenithm TCP client backend is disabled");
            }
        }

        // Start Brokenithm iDevice client backend if configured and enabled
        if let Some(brokenithm_config) = &self.config.input.brokenithm {
            if let Some(idevice_config) = &brokenithm_config.idevice {
                if idevice_config.enabled {
                    tracing::info!(
                        "Starting Brokenithm iDevice client backend: device_port={} local_port={} udid={:?}",
                        idevice_config.device_port,
                        idevice_config.local_port,
                        idevice_config.udid
                    );
                    let input_stream = self.streams.input.clone();
                    let local_port = idevice_config.local_port;
                    let device_port = idevice_config.device_port;
                    let udid = idevice_config.udid.clone();
                    let handle = tokio::spawn(async move {
                        use crate::input::brokenithm::BrokenithmIdeviceClient;
                        let mut backend = BrokenithmIdeviceClient::new(
                            local_port,
                            device_port,
                            udid,
                            input_stream,
                        );
                        if let Err(e) = backend.run().await {
                            tracing::error!("Brokenithm iDevice client backend error: {}", e);
                        }
                    });
                    self.input_handles.push(handle);
                } else {
                    tracing::info!("Brokenithm iDevice client backend is disabled");
                }
            }
        }

        Ok(())
    }

    /// Start device filter service that transforms raw input events
    async fn start_device_filter_service(&mut self) -> Result<()> {
        tracing::info!("Starting device filter service");

        let raw_input_stream = self.streams.input.clone();
        let transformed_output_stream = self.streams.transformed_input.clone();
        let device_filter = self.device_filter.clone();

        let handle = tokio::spawn(async move {
            loop {
                // Receive from raw input stream
                if let Some(packet) = raw_input_stream.receive().await {
                    match device_filter.transform_packet(packet) {
                        Ok(transformed_packet) => {
                            // Send to transformed stream
                            if let Err(e) = transformed_output_stream.send(transformed_packet).await
                            {
                                tracing::error!("Failed to send transformed packet: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to transform packet: {}", e);
                        }
                    }
                } else {
                    tracing::debug!("Input stream closed, shutting down device filter");
                    break;
                }
            }
        });

        self.filter_handle = Some(handle);
        Ok(())
    }

    /// Start output services based on configuration
    async fn start_output_services(
        &mut self,
        led_packet_tx: mpsc::Sender<ChuniLedDataPacket>, // switched to bounded Sender
    ) -> Result<()> {
        if self.config.output.uinput.enabled {
            tracing::info!("Starting uinput output backend");

            let input_stream = self.streams.transformed_input.clone();
            let handle = tokio::spawn(async move {
                let mut output = OutputBackendType::Udev(
                    crate::output::udev::UdevOutput::new(input_stream)
                        .expect("Failed to create UdevOutput"),
                );

                if let Err(e) = output.run().await {
                    tracing::error!("Output backend error: {}", e);
                }
            });

            self.output_handles.push(handle);
        } else {
            tracing::info!("Uinput output backend is disabled");
        }

        // Start chuniio proxy backend if configured and enabled
        if let Some(chuniio_config) = &self.config.output.chuniio_proxy {
            if chuniio_config.enabled {
                tracing::info!(
                    "Starting chuniio proxy output backend on socket: {:?}",
                    chuniio_config.socket_path
                );

                let input_stream = self.streams.transformed_input.clone();
                let feedback_stream = self.streams.feedback.clone();
                let socket_path = chuniio_config.socket_path.clone();

                let handle = tokio::spawn(async move {
                    let mut output = OutputBackendType::ChuniioProxy(
                        crate::output::chuniio_proxy::ChuniioProxyServer::new(
                            Some(socket_path),
                            input_stream,
                            feedback_stream,
                            led_packet_tx,
                        ),
                    );

                    if let Err(e) = output.run().await {
                        tracing::error!("Chuniio proxy backend error: {}", e);
                    }
                });

                self.output_handles.push(handle);
            } else {
                tracing::info!("Chuniio proxy output backend is disabled");
            }
        }

        Ok(())
    }

    /// Start feedback services based on configuration
    async fn start_feedback_services(
        &mut self,
        led_packet_rx: mpsc::Receiver<ChuniLedDataPacket>, // switched to bounded Receiver
    ) -> Result<()> {
        // Check if chuniio_proxy is enabled
        let chuniio_proxy_enabled = self
            .config
            .output
            .chuniio_proxy
            .as_ref()
            .map(|config| config.enabled)
            .unwrap_or(false);

        if let Some(chuniio_config) = &self.config.feedback.chuniio {
            if chuniio_proxy_enabled {
                tracing::info!(
                    "Starting unified ChuniIO RGB feedback service (fed by chuniio_proxy)"
                );

                // Start the unified service that processes packets from chuniio_proxy
                let config = chuniio_config.clone();
                let feedback_stream = self.streams.feedback.clone();

                let handle = tokio::spawn(async move {
                    use crate::feedback::generators::chuni_jvs::run_chuniio_service;
                    if let Err(e) =
                        run_chuniio_service(config, feedback_stream, led_packet_rx).await
                    {
                        tracing::error!("ChuniIO RGB feedback service error: {}", e);
                    }
                });

                self.feedback_handles.push(handle);
            } else {
                tracing::info!(
                    "Starting ChuniIO RGB feedback service with JVS reader on socket: {:?}",
                    chuniio_config.socket_path
                );

                // Start both the JVS reader and the RGB service
                let config = chuniio_config.clone();
                let feedback_stream = self.streams.feedback.clone();
                let socket_path = chuniio_config.socket_path.clone();

                // Create a new channel for the JVS reader to send packets
                let (jvs_led_tx, jvs_led_rx) =
                    crate::feedback::generators::chuni_jvs::create_chuni_led_channel();

                // Start JVS reader
                let jvs_handle = tokio::spawn(async move {
                    use crate::feedback::generators::chuni_jvs::ChuniJvsReader;
                    let mut reader = ChuniJvsReader::new(socket_path, jvs_led_tx);
                    if let Err(e) = reader.run().await {
                        tracing::error!("ChuniIO JVS reader error: {}", e);
                    }
                });

                // Start RGB service
                let rgb_handle = tokio::spawn(async move {
                    use crate::feedback::generators::chuni_jvs::run_chuniio_service;
                    if let Err(e) = run_chuniio_service(config, feedback_stream, jvs_led_rx).await {
                        tracing::error!("ChuniIO RGB feedback service error: {}", e);
                    }
                });

                self.feedback_handles.push(jvs_handle);
                self.feedback_handles.push(rgb_handle);
            }
        }

        Ok(())
    }

    /// Wait for all services to complete or handle shutdown
    pub async fn wait_for_shutdown(&mut self) -> Result<()> {
        tracing::info!("Waiting for shutdown signal...");

        tokio::select! {
            // Wait for Ctrl+C
            signal_result = tokio::signal::ctrl_c() => {
                match signal_result {
                    Ok(_) => tracing::info!("Received Ctrl+C, shutting down gracefully..."),
                    Err(e) => tracing::error!("Failed to listen for Ctrl+C: {}", e),
                }
                self.shutdown().await?;
            }
            // Wait for any service to complete (which might indicate an error)
            _ = self.wait_for_any_service() => {
                tracing::warn!("One or more services completed unexpectedly");
                self.shutdown().await?;
            }
        }

        Ok(())
    }

    /// Wait for any service to complete
    async fn wait_for_any_service(&mut self) {
        // Simple approach: just wait for any output service if any exist
        // In practice, services shouldn't complete unless there's an error
        if !self.output_handles.is_empty() {
            let mut handles = std::mem::take(&mut self.output_handles);
            tokio::select! {
                _ = async {
                    for handle in handles.iter_mut() {
                        if handle.is_finished() {
                            let _ = handle.await;
                            return;
                        }
                    }
                    // If none are finished, wait for the first one
                    if let Some(handle) = handles.pop() {
                        let _ = handle.await;
                    }
                } => {
                    tracing::warn!("An output service completed unexpectedly");
                }
            }
            self.output_handles = handles;
        } else {
            // No output services, just wait indefinitely
            // In a real implementation, you might want to monitor input/feedback services too
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                tracing::debug!("Backend still running...");
            }
        }
    }

    /// Gracefully shutdown all services
    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down backend services...");

        // Abort all input service tasks
        for handle in &self.input_handles {
            handle.abort();
        }
        tracing::info!("Aborted {} input service tasks", self.input_handles.len());

        // Abort device filter service task
        if let Some(ref handle) = self.filter_handle {
            handle.abort();
            tracing::info!("Aborted device filter service task");
        }

        // Abort all output service tasks
        for handle in &self.output_handles {
            handle.abort();
        }
        if !self.output_handles.is_empty() {
            tracing::info!(
                "Aborted {} output service task(s)",
                self.output_handles.len()
            );
        }

        // Abort all feedback service tasks
        for handle in &self.feedback_handles {
            handle.abort();
        }
        tracing::info!(
            "Aborted {} feedback service tasks",
            self.feedback_handles.len()
        );

        tracing::info!("Backend shutdown complete");
        Ok(())
    }
}

/// Convenience function to create and start a backend from configuration
pub async fn setup_and_run_backend(config: AppConfig) -> Result<()> {
    let mut backend = Backend::new(config);
    backend.start().await?;
    backend.wait_for_shutdown().await?;
    Ok(())
}

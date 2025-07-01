//! Backend module for Plumbershim
//!
//! This module handles taking in a configuration, parsing it, and setting up the backend services.

use crate::config::AppConfig;
use crate::feedback::FeedbackEventStream;
use crate::input::{InputBackend, InputEventStream};
use crate::output::{OutputBackend, OutputBackendType};
use eyre::Result;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

/// Message streams container for passing around shared streams
pub struct MessageStreams {
    pub input: InputEventStream,
    pub feedback: FeedbackEventStream,
}

/// Represents the actual backend service
pub struct Backend {
    config: AppConfig,
    streams: MessageStreams,

    // Service handles for graceful shutdown
    input_handles: Vec<JoinHandle<()>>,
    output_handle: Option<JoinHandle<()>>,
    feedback_handles: Vec<JoinHandle<()>>,
}

impl Backend {
    /// Create a new backend from configuration
    pub fn new(config: AppConfig) -> Self {
        let streams = MessageStreams {
            input: InputEventStream::new(),
            feedback: FeedbackEventStream::new(),
        };

        Self {
            config,
            streams,
            input_handles: Vec::new(),
            output_handle: None,
            feedback_handles: Vec::new(),
        }
    }

    /// Start all configured backend services
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting backend services...");

        // Start input services
        self.start_input_services().await?;

        // Start output services
        self.start_output_services().await?;

        // Start feedback services
        self.start_feedback_services().await?;

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
        if let Some(_unix_config) = &self.config.input.unix {
            tracing::warn!("Unix domain socket input backend not yet implemented");
        }

        Ok(())
    }

    /// Start output services based on configuration
    async fn start_output_services(&mut self) -> Result<()> {
        if self.config.output.uinput.enabled {
            tracing::info!("Starting uinput output backend");

            let input_stream = self.streams.input.clone();
            let handle = tokio::spawn(async move {
                let mut output = OutputBackendType::Udev(
                    crate::output::udev::UdevOutput::new(input_stream)
                        .expect("Failed to create UdevOutput"),
                );

                if let Err(e) = output.run().await {
                    tracing::error!("Output backend error: {}", e);
                }
            });

            self.output_handle = Some(handle);
        } else {
            tracing::info!("Output backend is disabled");
        }

        Ok(())
    }

    /// Start feedback services based on configuration
    async fn start_feedback_services(&mut self) -> Result<()> {
        if let Some(chuniio_config) = &self.config.feedback.chuniio {
            tracing::info!(
                "Starting ChuniIO RGB feedback service on socket: {:?}",
                chuniio_config.socket_path
            );

            // TODO: Implement ChuniIO RGB feedback service
            tracing::warn!("ChuniIO RGB feedback service not yet implemented");
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
        // Simple approach: just wait for the output service if it exists
        // In practice, services shouldn't complete unless there's an error
        if let Some(output_handle) = self.output_handle.take() {
            let _ = output_handle.await;
            tracing::warn!("Output service completed unexpectedly");
        } else {
            // No output service, just wait indefinitely
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

        // Abort output service task
        if let Some(ref handle) = self.output_handle {
            handle.abort();
            tracing::info!("Aborted output service task");
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

mod input;
mod output;
use eyre::Result;

use crate::output::OutputBackend;

pub fn build_logger() -> Result<()> {
    // Create an env filter that defaults to "info" level if RUST_LOG is not set
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE) // Global ceiling - allows up to TRACE
        .with_env_filter(env_filter) // Runtime filtering - defaults to INFO
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .try_init()
        .map_err(|e| eyre::eyre!("Failed to initialize logger: {}", e))?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    build_logger()?;
    tracing::debug!("Debug logging is enabled");
    tracing::info!("Backflow version: {}", env!("CARGO_PKG_VERSION"));

    let input_stream = input::InputEventStream::new();

    // Clone the input stream for different tasks
    let ws_input_stream = input_stream.clone();
    let dbus_input_stream = input_stream.clone();

    // Clone the sender for the test task (commented out)
    // let test_tx = input_stream.tx.clone();

    // Spawn a task to inject test events
    // tokio::spawn(async move {
    //     use input::{InputEvent, InputEventPacket, KeyboardEvent, PointerEvent};
    //     use std::time::{SystemTime, UNIX_EPOCH};

    //     tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    //     tracing::info!("Starting to inject test events...");

    //     // Create a test packet with keyboard press
    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     let mut press_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //     press_packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
    //         key: 65.to_string(),
    //     })); // 'A' key

    //     if let Err(e) = test_tx.send(press_packet) {
    //         tracing::error!("Failed to send test keyboard press: {}", e);
    //     } else {
    //         tracing::info!("Sent test keyboard press (A key)");
    //     }

    //     // Wait a bit, then send the release
    //     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     let mut release_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //     release_packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
    //         key: 65.to_string(),
    //     })); // 'A' key

    //     if let Err(e) = test_tx.send(release_packet) {
    //         tracing::error!("Failed to send test keyboard release: {}", e);
    //     } else {
    //         tracing::info!("Sent test keyboard release (A key)");
    //     }

    //     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    //     // Create a test packet with pointer events
    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     let mut move_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //     move_packet.add_event(InputEvent::Pointer(PointerEvent::Move {
    //         x_delta: 10,
    //         y_delta: -5,
    //     }));

    //     if let Err(e) = test_tx.send(move_packet) {
    //         tracing::error!("Failed to send test pointer move: {}", e);
    //     } else {
    //         tracing::info!("Sent test pointer move event");
    //     }

    //     tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    //     // Send click press
    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     let mut click_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //     click_packet.add_event(InputEvent::Pointer(PointerEvent::Click { button: 1 })); // Left click

    //     if let Err(e) = test_tx.send(click_packet) {
    //         tracing::error!("Failed to send test pointer click: {}", e);
    //     } else {
    //         tracing::info!("Sent test pointer click");
    //     }

    //     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    //     // Send click release
    //     let timestamp = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_millis() as u64;

    //     let mut release_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //     release_packet.add_event(InputEvent::Pointer(PointerEvent::ClickRelease {
    //         button: 1,
    //     }));

    //     if let Err(e) = test_tx.send(release_packet) {
    //         tracing::error!("Failed to send test pointer release: {}", e);
    //     } else {
    //         tracing::info!("Sent test pointer release");
    //     }

    //     // Continue sending periodic test events
    //     let mut counter = 0;
    //     loop {
    //         tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    //         counter += 1;

    //         let timestamp = SystemTime::now()
    //             .duration_since(UNIX_EPOCH)
    //             .unwrap()
    //             .as_millis() as u64;

    //         let key = 48 + (counter % 10) as u8; // Number keys 0-9
    //         let mut press_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //         press_packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
    //             key: key.to_string(),
    //         }));

    //         if let Err(e) = test_tx.send(press_packet) {
    //             tracing::error!("Failed to send periodic test press: {}", e);
    //             break;
    //         } else {
    //             tracing::info!(
    //                 "Sent periodic test press #{} (number key {})",
    //                 counter,
    //                 counter % 10
    //             );
    //         }

    //         // Wait a bit, then send the release
    //         tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    //         let timestamp = SystemTime::now()
    //             .duration_since(UNIX_EPOCH)
    //             .unwrap()
    //             .as_millis() as u64;

    //         let mut release_packet = InputEventPacket::new("test_device".to_string(), timestamp);
    //         release_packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
    //             key: key.to_string(),
    //         }));

    //         if let Err(e) = test_tx.send(release_packet) {
    //             tracing::error!("Failed to send periodic test release: {}", e);
    //             break;
    //         } else {
    //             tracing::info!(
    //                 "Sent periodic test release #{} (number key {})",
    //                 counter,
    //                 counter % 10
    //             );
    //         }
    //     }
    // });

    // let's listen to websocket input events
    let event_task = tokio::spawn(async move {
        use input::InputBackend;
        use input::websocket::WebSocketInputBackend;
        use std::net::SocketAddr;
        let bind_addr: SocketAddr = "127.0.0.1:8000".parse().expect("Invalid address");
        let mut ws_backend = WebSocketInputBackend::new(bind_addr, ws_input_stream);
        if let Err(e) = ws_backend.run().await {
            tracing::error!("WebSocket backend error: {}", e);
        }
    });

    // You can easily switch between different output backends here
    let backend_type = std::env::var("OUTPUT_BACKEND").unwrap_or_else(|_| "dbus".to_string());

    let mut dbus_output = output::OutputBackendType::Dbus(
        output::plumber_dbus::DbusPlumberOutput::new(dbus_input_stream).await,
    );

    tracing::info!("Starting {} output backend...", backend_type);
    tracing::info!("Press Ctrl+C to gracefully shutdown the application");

    // Set up ctrl+c handler
    tokio::select! {
        result = dbus_output.run() => {
            match result {
                Ok(_) => tracing::info!("D-Bus output backend finished"),
                Err(e) => tracing::error!("D-Bus output backend error: {}", e),
            }
        }
        signal_result = tokio::signal::ctrl_c() => {
            match signal_result {
                Ok(_) => tracing::info!("Received Ctrl+C, shutting down gracefully..."),
                Err(e) => tracing::error!("Failed to listen for Ctrl+C: {}", e),
            }

            tracing::info!("Aborting WebSocket task...");
            // Abort the WebSocket task
            event_task.abort();

            tracing::info!("Stopping D-Bus backend...");
            // Try to stop the backend gracefully
            if let Err(e) = dbus_output.stop().await {
                tracing::warn!("Error stopping D-Bus backend: {}", e);
            }

            tracing::info!("Shutdown complete");
        }
    }

    Ok(())
}

//! A TCP server implementation specifically for Brokenithm clients
//!
//! Brokenithm is a UDP-based mobile virtual controller for CHUNITHM-style games.
//!
//! This module provides a TCP server/client that connects to Brokenithm clients,
//! allowing them to send input events and receive updates.

use crate::input::{
    AnalogEvent, InputBackend, InputEvent, InputEventPacket, InputEventStream, KeyboardEvent,
};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, trace};

mod idevice_proxy;
use idevice_proxy::spawn_iproxy;

pub struct BrokenithmTcpServer {
    pub bind_addr: SocketAddr,
    pub input_stream: InputEventStream,
}

impl BrokenithmTcpServer {
    pub fn new(bind_addr: SocketAddr, input_stream: InputEventStream) -> Self {
        Self {
            bind_addr,
            input_stream,
        }
    }
}

#[async_trait::async_trait]
impl InputBackend for BrokenithmTcpServer {
    async fn run(&mut self) -> eyre::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        tracing::info!("Brokenithm TCP backend listening on {}", self.bind_addr);
        loop {
            let (mut socket, addr) = listener.accept().await?;
            tracing::info!("Accepted TCP connection from {}", addr);
            let input_stream = self.input_stream.clone();
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut read_buf = [0u8; 256];
                let mut state_tracker = BrokenithmInputStateTracker::new();
                loop {
                    match socket.read(&mut read_buf).await {
                        Ok(0) => {
                            tracing::info!("Connection closed by {}", addr);
                            break;
                        }
                        Ok(len) => {
                            // Append new data to buffer
                            buffer.extend_from_slice(&read_buf[..len]);

                            // Extract and process all complete messages
                            let (messages, consumed) = extract_brokenithm_messages(&buffer);
                            for message in messages {
                                tracing::trace!("Parsed message from {}: {:?}", addr, message);
                                match message {
                                    BrokenithmMessage::Input {
                                        air,
                                        slider,
                                        test_button,
                                        service_button,
                                    } => {
                                        if let Some(packet) = state_tracker.diff_and_packet(
                                            &air,
                                            &slider,
                                            test_button,
                                            service_button,
                                        ) {
                                            let _ = input_stream.send(packet).await;
                                        }
                                    }
                                    BrokenithmMessage::InsertCoin => {
                                        // Avoid double borrow: pulse, then copy state to locals, then call diff_and_packet
                                        state_tracker.pulse_coin();
                                        let air = state_tracker.prev_air.clone();
                                        let slider = state_tracker.prev_slider.clone();
                                        let test_button = state_tracker.prev_test;
                                        let service_button = state_tracker.prev_service;
                                        if let Some(packet) = state_tracker.diff_and_packet(
                                            &air,
                                            &slider,
                                            test_button,
                                            service_button,
                                        ) {
                                            let _ = input_stream.send(packet).await;
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            // Remove consumed bytes from buffer
                            if consumed > 0 {
                                buffer.drain(..consumed);
                            }
                        }
                        Err(e) => {
                            tracing::error!("TCP read error from {}: {}", addr, e);
                            break;
                        }
                    }
                }
            });
        }
    }
}

pub struct BrokenithmTcpClient {
    pub connect_addr: SocketAddr,
    pub input_stream: InputEventStream,
}

impl BrokenithmTcpClient {
    pub fn new(connect_addr: SocketAddr, input_stream: InputEventStream) -> Self {
        Self {
            connect_addr,
            input_stream,
        }
    }
}

#[async_trait::async_trait]
impl InputBackend for BrokenithmTcpClient {
    async fn run(&mut self) -> eyre::Result<()> {
        let mut state_tracker = BrokenithmInputStateTracker::new();
        loop {
            match TcpStream::connect(self.connect_addr).await {
                Ok(mut socket) => {
                    tracing::info!("Connected to Brokenithm device at {}", self.connect_addr);
                    let mut buffer = Vec::new();
                    let mut read_buf = [0u8; 256];
                    loop {
                        match socket.read(&mut read_buf).await {
                            Ok(0) => {
                                tracing::info!("Connection closed by remote");
                                break;
                            }
                            Ok(len) => {
                                // Append new data to buffer
                                buffer.extend_from_slice(&read_buf[..len]);

                                // Extract and process all complete messages
                                let (messages, consumed) = extract_brokenithm_messages(&buffer);
                                for message in messages {
                                    tracing::info!("Parsed message: {:?}", message);
                                    match message {
                                        BrokenithmMessage::Input {
                                            air,
                                            slider,
                                            test_button,
                                            service_button,
                                        } => {
                                            if let Some(packet) = state_tracker.diff_and_packet(
                                                &air,
                                                &slider,
                                                test_button,
                                                service_button,
                                            ) {
                                                let _ = self.input_stream.send(packet).await;
                                            }
                                        }
                                        BrokenithmMessage::InsertCoin => {
                                            state_tracker.pulse_coin();
                                            let air = state_tracker.prev_air.clone();
                                            let slider = state_tracker.prev_slider.clone();
                                            let test_button = state_tracker.prev_test;
                                            let service_button = state_tracker.prev_service;
                                            if let Some(packet) = state_tracker.diff_and_packet(
                                                &air,
                                                &slider,
                                                test_button,
                                                service_button,
                                            ) {
                                                let _ = self.input_stream.send(packet).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // Remove consumed bytes from buffer
                                if consumed > 0 {
                                    buffer.drain(..consumed);
                                }
                            }
                            Err(e) => {
                                tracing::error!("TCP read error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to {}: {}. Retrying in 5s...",
                        self.connect_addr,
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }
}

pub struct BrokenithmIdeviceClient {
    pub local_port: u16,
    pub device_port: u16,
    pub udid: Option<String>,
    pub input_stream: InputEventStream,
}

impl BrokenithmIdeviceClient {
    pub fn new(
        local_port: u16,
        device_port: u16,
        udid: Option<String>,
        input_stream: InputEventStream,
    ) -> Self {
        Self {
            local_port,
            device_port,
            udid,
            input_stream,
        }
    }
}

#[async_trait::async_trait]
impl InputBackend for BrokenithmIdeviceClient {
    async fn run(&mut self) -> eyre::Result<()> {
        // Start iproxy (libimobiledevice) to forward device_port to local_port
        let udid_ref = self.udid.as_deref();
        let mut iproxy_child = spawn_iproxy(self.local_port, self.device_port, udid_ref).await?;
        tracing::info!(
            "Started iproxy for device port {} -> localhost:{}",
            self.device_port,
            self.local_port
        );
        // Connect to the local forwarded port
        let connect_addr = format!("127.0.0.1:{}", self.local_port);
        let mut state_tracker = BrokenithmInputStateTracker::new();
        loop {
            match TcpStream::connect(&connect_addr).await {
                Ok(mut socket) => {
                    tracing::info!(
                        "Connected to Brokenithm device via iproxy at {}",
                        connect_addr
                    );
                    let mut buffer = Vec::new();
                    let mut read_buf = [0u8; 256];
                    loop {
                        match socket.read(&mut read_buf).await {
                            Ok(0) => {
                                tracing::info!("Connection closed by remote");
                                break;
                            }
                            Ok(len) => {
                                // Append new data to buffer
                                buffer.extend_from_slice(&read_buf[..len]);

                                // Extract and process all complete messages
                                let (messages, consumed) = extract_brokenithm_messages(&buffer);
                                for message in messages {
                                    tracing::trace!("Parsed message: {:?}", message);
                                    match message {
                                        BrokenithmMessage::Input {
                                            air,
                                            slider,
                                            test_button,
                                            service_button,
                                        } => {
                                            if let Some(packet) = state_tracker.diff_and_packet(
                                                &air,
                                                &slider,
                                                test_button,
                                                service_button,
                                            ) {
                                                let _ = self.input_stream.send(packet).await;
                                            }
                                        }
                                        BrokenithmMessage::InsertCoin => {
                                            state_tracker.pulse_coin();
                                            let air = state_tracker.prev_air.clone();
                                            let slider = state_tracker.prev_slider.clone();
                                            let test_button = state_tracker.prev_test;
                                            let service_button = state_tracker.prev_service;
                                            if let Some(packet) = state_tracker.diff_and_packet(
                                                &air,
                                                &slider,
                                                test_button,
                                                service_button,
                                            ) {
                                                let _ = self.input_stream.send(packet).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // Remove consumed bytes from buffer
                                if consumed > 0 {
                                    buffer.drain(..consumed);
                                }
                            }
                            Err(e) => {
                                tracing::error!("TCP read error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to connect to {}: {}. Retrying in 5s...",
                        connect_addr,
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
        // Optionally: handle iproxy_child termination/cleanup
    }
}

#[derive(Debug)]
pub enum BrokenithmMessage {
    Welcome,
    InsertCoin,
    TapCard,
    EnableAir(bool),
    Input {
        air: Vec<u8>,
        slider: Vec<u8>,
        test_button: bool,
        service_button: bool,
    },
    Unknown(Vec<u8>),
}

fn parse_brokenithm_message(data: &[u8]) -> Option<BrokenithmMessage> {
    if data.len() < 4 {
        return None;
    }

    let length = data[0] as usize;
    let packet_name = &data[1..4];

    // Check if we have enough data for the full message
    if data.len() < length + 1 {
        return None;
    }

    // Debug logging
    trace!(
        "Parsing message: len={}, name={:?} (ascii: {}), data length={}",
        length,
        packet_name,
        String::from_utf8_lossy(packet_name),
        data.len()
    );

    match packet_name {
        [0x57, 0x45, 0x4C] if length == 3 => Some(BrokenithmMessage::Welcome), // "WEL"
        [0x46, 0x4E, 0x43] if length >= 4 => {
            // "FNC" - Function button
            let func_btn = data[4];
            match func_btn {
                1 => Some(BrokenithmMessage::InsertCoin),
                2 => Some(BrokenithmMessage::TapCard),
                _ => Some(BrokenithmMessage::Unknown(data.to_vec())),
            }
        }
        [0x41, 0x49, 0x52] if length >= 4 => {
            // "AIR" - Air enable/disable
            let air_enabled = data[4] != 0;
            // Restore to just EnableAir, let state tracker handle event emission
            Some(BrokenithmMessage::EnableAir(air_enabled))
        }
        [0x49, 0x4E, 0x50] if length >= 38 => {
            // "INP" - Input packet
            // Minimum: 6 air + 32 slider = 38 bytes payload
            // Full: 6 air + 32 slider + 1 test + 1 service = 40 bytes payload
            let payload = &data[4..];
            let payload_len = payload.len();

            trace!(
                "INP message: expected_payload_len={}, actual_payload_len={}, total_len={}",
                length - 3,
                payload_len,
                data.len()
            );

            if payload_len >= 38 {
                let air = reorder_air_zones(&payload[0..6]);
                let slider = payload[6..38].to_vec();

                // Test and service buttons are optional (may not be present in shorter messages)
                let test_button = if payload_len >= 39 {
                    payload[38] != 0
                } else {
                    false
                };
                let service_button = if payload_len >= 40 {
                    payload[39] != 0
                } else {
                    false
                };

                // tracing::debug!(
                //     "INP message: payload_len={}, air={:?}, slider_len={}, test={}, service={}",
                //     payload_len,
                //     air,
                //     slider.len(),
                //     test_button,
                //     service_button
                // );

                Some(BrokenithmMessage::Input {
                    air,
                    slider,
                    test_button,
                    service_button,
                })
            } else {
                tracing::warn!(
                    "INP message too short: {} bytes (need at least 38)",
                    payload.len()
                );
                Some(BrokenithmMessage::Unknown(data.to_vec()))
            }
        }
        _ => {
            tracing::debug!(
                "Unknown message: len={} name={:?} data={:x?}",
                length,
                packet_name,
                data
            );
            Some(BrokenithmMessage::Unknown(data.to_vec()))
        }
    }
}

/// Tracks previous input state to emit only changed events
struct BrokenithmInputStateTracker {
    prev_air: Vec<u8>,
    prev_slider: Vec<u8>,
    prev_test: bool,
    prev_service: bool,
    coin_pulse: bool,
}

impl BrokenithmInputStateTracker {
    fn new() -> Self {
        Self {
            prev_air: vec![0; 6],
            prev_slider: vec![0; 32],
            prev_test: false,
            prev_service: false,
            coin_pulse: false,
        }
    }

    /// Returns only the changed events as InputEventPacket
    fn diff_and_packet(
        &mut self,
        air: &[u8],
        slider: &[u8],
        test_button: bool,
        service_button: bool,
    ) -> Option<InputEventPacket> {
        let mut events = Vec::new();
        // Air zones (CHUNIIO_IR_N as KeyPress/KeyRelease)
        for (i, (&prev, &curr)) in self.prev_air.iter().zip(air.iter()).enumerate() {
            let key = format!("CHUNIIO_IR_{}", i + 1);
            if prev == 0 && curr > 0 {
                events.push(InputEvent::Keyboard(KeyboardEvent::KeyPress {
                    key: key.clone(),
                }));
            } else if prev > 0 && curr == 0 {
                events.push(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
                    key: key.clone(),
                }));
            }
        }
        // Slider
        for (i, (&prev, &curr)) in self.prev_slider.iter().zip(slider.iter()).enumerate() {
            // CHUNIIO_SLIDER_0 to CHUNIIO_SLIDER_31
            let key = format!("CHUNIIO_SLIDER_{}", i);
            if prev < 128 && curr >= 128 {
                events.push(InputEvent::Keyboard(KeyboardEvent::KeyPress {
                    key: key.clone(),
                }));
            } else if prev >= 128 && curr < 128 {
                events.push(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
                    key: key.clone(),
                }));
            }
        }
        // Test button
        const TEST_BUTTON: &str = "CHUNIIO_TEST";
        if self.prev_test != test_button {
            events.push(InputEvent::Keyboard(if test_button {
                KeyboardEvent::KeyPress {
                    key: TEST_BUTTON.to_string(),
                }
            } else {
                KeyboardEvent::KeyRelease {
                    key: TEST_BUTTON.to_string(),
                }
            }));
        }
        // Service button
        const SERVICE_BUTTON: &str = "CHUNIIO_SERVICE";
        if self.prev_service != service_button {
            events.push(InputEvent::Keyboard(if service_button {
                KeyboardEvent::KeyPress {
                    key: SERVICE_BUTTON.to_string(),
                }
            } else {
                KeyboardEvent::KeyRelease {
                    key: SERVICE_BUTTON.to_string(),
                }
            }));
        }
        // Coin input (pulse)
        if self.coin_pulse {
            events.push(InputEvent::Keyboard(KeyboardEvent::KeyPress {
                key: "CHUNIIO_COIN".to_string(),
            }));
            events.push(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
                key: "CHUNIIO_COIN".to_string(),
            }));
            self.coin_pulse = false;
        }
        // Update state
        self.prev_air.copy_from_slice(air);
        self.prev_slider.copy_from_slice(slider);
        self.prev_test = test_button;
        self.prev_service = service_button;
        if events.is_empty() {
            None
        } else {
            let device_id = "brokenithm".to_string();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            Some(InputEventPacket {
                device_id,
                timestamp,
                events,
            })
        }
    }

    /// Call this to pulse a coin event
    fn pulse_coin(&mut self) {
        self.coin_pulse = true;
    }
}

fn extract_brokenithm_messages(buf: &[u8]) -> (Vec<BrokenithmMessage>, usize) {
    let mut messages = Vec::new();
    let mut consumed = 0;
    let mut pos = 0;

    while pos < buf.len() {
        // Need at least 4 bytes for length + 3-byte packet name
        if pos + 4 > buf.len() {
            break;
        }

        let length = buf[pos] as usize;

        // Check if we have enough data for the full message
        if pos + 1 + length > buf.len() {
            break;
        }

        let message_data = &buf[pos..pos + 1 + length];
        if let Some(parsed) = parse_brokenithm_message(message_data) {
            messages.push(parsed);
        }

        pos += 1 + length;
        consumed = pos;
    }

    (messages, consumed)
}

// Reorder air zones to logical order: 1,2,3,4,5,6 from the received 2,1,4,3,6,5
fn reorder_air_zones(air: &[u8]) -> Vec<u8> {
    match air.len() {
        6 => vec![air[1], air[0], air[3], air[2], air[5], air[4]],
        _ => air.to_vec(),
    }
}

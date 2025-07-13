//! A TCP server implementation specifically for Brokenithm clients
//!
//! Brokenithm is a UDP-based mobile virtual controller for CHUNITHM-style games.
//!
//! This module provides a TCP server/client that connects to Brokenithm clients,
//! allowing them to send input events and receive updates.

use crate::input::{InputBackend, InputEventStream};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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
            tokio::spawn(async move {
                let mut buf = [0u8; 256];
                loop {
                    match socket.read(&mut buf).await {
                        Ok(0) => {
                            tracing::info!("Connection closed by {}", addr);
                            break;
                        }
                        Ok(len) => {
                            let msg = &buf[..len];
                            if let Some(parsed) = parse_brokenithm_message(msg) {
                                tracing::info!("Parsed message from {}: {:?}", addr, parsed);
                            } else {
                                tracing::info!("Received {} bytes from {}: {:x?}", len, addr, msg);
                            }
                            // TODO: Forward as InputEventPacket
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
        loop {
            match TcpStream::connect(self.connect_addr).await {
                Ok(mut socket) => {
                    tracing::info!("Connected to Brokenithm device at {}", self.connect_addr);
                    let mut buf = [0u8; 256];
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) => {
                                tracing::info!("Connection closed by remote");
                                break;
                            }
                            Ok(len) => {
                                let msg = &buf[..len];
                                if let Some(parsed) = parse_brokenithm_message(msg) {
                                    tracing::info!("Parsed message: {:?}", parsed);
                                } else {
                                    tracing::info!("Received {} bytes: {:x?}", len, msg);
                                }
                                // TODO: Forward as InputEventPacket
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
        loop {
            match TcpStream::connect(&connect_addr).await {
                Ok(mut socket) => {
                    tracing::info!(
                        "Connected to Brokenithm device via iproxy at {}",
                        connect_addr
                    );
                    let mut buf = [0u8; 256];
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) => {
                                tracing::info!("Connection closed by remote");
                                break;
                            }
                            Ok(len) => {
                                tracing::info!("Received {} bytes: {:x?}", len, &buf[..len]);
                                // TODO: Parse and forward as InputEventPacket
                                if let Some(parsed) = parse_brokenithm_message(&buf[..len]) {
                                    tracing::info!("Parsed message: {:?}", parsed);
                                } else {
                                    tracing::info!("Received {} bytes: {:x?}", len, &buf[..len]);
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
    Input { air: Vec<u8>, lane: Vec<u8> },
    Unknown(Vec<u8>),
}

fn parse_brokenithm_message(data: &[u8]) -> Option<BrokenithmMessage> {
    match data {
        [3, 87, 69, 76] => Some(BrokenithmMessage::Welcome),
        [4, 70, 78, 67, 1] => Some(BrokenithmMessage::InsertCoin),
        [4, 70, 78, 67, 2] => Some(BrokenithmMessage::TapCard),
        [4, 65, 73, 82, b] => Some(BrokenithmMessage::EnableAir(*b != 0)),
        [41, 73, 78, 80, rest @ ..] => {
            // INP message: [41, 73, 78, 80] + arrayAir + arrayLane
            // If 32 or more bytes, last 32 are lanes, rest is air. If less, all is lane.
            if rest.len() >= 32 {
                let (air, lane) = rest.split_at(rest.len() - 32);
                Some(BrokenithmMessage::Input {
                    air: reorder_air_zones(air),
                    lane: lane.to_vec(),
                })
            } else {
                Some(BrokenithmMessage::Input {
                    air: vec![],
                    lane: rest.to_vec(),
                })
            }
        }
        _ => Some(BrokenithmMessage::Unknown(data.to_vec())),
    }
}

// Reorder air zones to logical order: 1,2,3,4,5,6 from the received 2,1,4,3,6,5
fn reorder_air_zones(air: &[u8]) -> Vec<u8> {
    match air.len() {
        6 => vec![air[1], air[0], air[3], air[2], air[5], air[4]],
        _ => air.to_vec(),
    }
}

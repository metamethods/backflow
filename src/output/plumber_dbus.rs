//! InputPlumber D-Bus output backend, for sending messages to the InputPlumber D-Bus service.
use std::collections::HashMap;

use eyre::Result;

use crate::input::{InputEvent, InputEventPacket, InputEventStream, KeyboardEvent};
use crate::output::OutputBackend;
use zbus_inputplumber::interface::{
    gamepad::GamepadProxy,
    input_manager::{self, InputManagerProxy, TargetDevice},
    keyboard::KeyboardProxy,
};

pub enum DeviceProxy {
    Keyboard(KeyboardProxy<'static>),
    Gamepad(GamepadProxy<'static>),
}

pub struct DeviceEntry {
    pub target_device: TargetDevice,
    pub proxy: DeviceProxy,
}

pub struct PlumberOutputBackend {
    pub connection: zbus::Connection,
    pub input_manager: InputManagerProxy<'static>,
    pub devices: HashMap<&'static str, DeviceEntry>,
}

impl PlumberOutputBackend {
    /// Creates a new `PlumberOutputBackend` with the given D-Bus connection.
    pub async fn new() -> Result<Self> {
        let connection = zbus::Connection::system().await?;
        tracing::info!("Connected to D-Bus system bus");
        let input_manager = InputManagerProxy::new(&connection).await?;

        let mut devices = HashMap::new();

        // Create devices using helper function
        Self::create_keyboard_device(&connection, &input_manager, &mut devices).await?;
        Self::create_gamepad_device(&connection, &input_manager, &mut devices).await?;

        Ok(Self {
            connection,
            input_manager,
            devices,
        })
    }
    async fn create_keyboard_device(
        connection: &zbus::Connection,
        input_manager: &InputManagerProxy<'_>,
        devices: &mut HashMap<&'static str, DeviceEntry>,
    ) -> Result<()> {
        tracing::info!("Creating keyboard device...");
        let device_type = input_manager::TargetDeviceType::Keyboard;
        let target_device = TargetDevice::new(input_manager, device_type).await?;
        tracing::info!("Keyboard target device created: {}", target_device.path);
        let proxy = KeyboardProxy::builder(connection)
            .path(target_device.path.clone())?
            .interface("org.shadowblip.Input.Keyboard")?
            
            .build()
            .await?;
        tracing::info!("Keyboard proxy created at path: {}", target_device.path);

        devices.insert(
            device_type.into(),
            DeviceEntry {
                target_device,
                proxy: DeviceProxy::Keyboard(proxy),
            },
        );
        tracing::debug!("Keyboard device added to devices map");
        Ok(())
    }

    async fn create_gamepad_device(
        connection: &zbus::Connection,
        input_manager: &InputManagerProxy<'_>,
        devices: &mut HashMap<&'static str, DeviceEntry>,
    ) -> Result<()> {
        tracing::info!("Creating gamepad device...");
        let device_type = input_manager::TargetDeviceType::Gamepad;
        let target_device = TargetDevice::new(input_manager, device_type).await?;
        tracing::info!("Gamepad target device created: {}", target_device.path);
        let proxy = GamepadProxy::builder(connection)
            .path(target_device.path.clone())?
            .build()
            .await?;
        tracing::info!("Gamepad proxy created at path: {}", target_device.path);

        devices.insert(
            device_type.into(),
            DeviceEntry {
                target_device,
                proxy: DeviceProxy::Gamepad(proxy),
            },
        );
        tracing::debug!("Gamepad device added to devices map");
        Ok(())
    }

    /// Get a device by its type string (e.g., "keyboard", "gamepad")
    pub fn get_device(&self, device_type: &str) -> Option<&DeviceEntry> {
        self.devices.get(device_type)
    }

    /// Get a mutable reference to a device by its type string
    pub fn get_device_mut(&mut self, device_type: &str) -> Option<&mut DeviceEntry> {
        self.devices.get_mut(device_type)
    }

    /// Get the keyboard device if it exists
    pub fn keyboard(&self) -> Option<&DeviceEntry> {
        self.get_device("keyboard")
    }

    /// Get the gamepad device if it exists
    pub fn gamepad(&self) -> Option<&DeviceEntry> {
        self.get_device("gamepad")
    }
}

pub struct DbusPlumberOutput {
    // Receiver for input event packets
    pub stream: crossbeam::channel::Receiver<InputEventPacket>,
    // sender for D-Bus messages
    pub backend: PlumberOutputBackend,
}

impl DbusPlumberOutput {
    /// Creates a new `DbusPlumberOutput` with the given input event stream.
    pub async fn new(stream: InputEventStream) -> Self {
        Self {
            stream: stream.rx,
            backend: PlumberOutputBackend::new().await.unwrap(),
        }
    }

    async fn process_packet(&mut self, packet: InputEventPacket) -> Result<()> {
        tracing::info!("Received input event packet: {:?}", packet);

        for event in packet.events {
            self.handle_event(event).await?;
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: InputEvent) -> Result<()> {
        match event {
            InputEvent::Keyboard(keyboard_event) => {
                self.handle_keyboard_event(keyboard_event).await
            }
            _ => {
                // Skip other event types for now
                tracing::debug!("Skipping unsupported event type");
                Ok(())
            }
        }
    }

    async fn handle_keyboard_event(&mut self, event: KeyboardEvent) -> Result<()> {
        let Some(kb_device) = self.backend.keyboard() else {
            tracing::warn!("No keyboard device available");
            return Ok(());
        };

        let DeviceProxy::Keyboard(proxy) = &kb_device.proxy else {
            tracing::error!("Expected keyboard proxy but got different device type");
            return Ok(());
        };

        match event {
            KeyboardEvent::KeyPress { key } => {
                proxy.send_key(&key, true).await?;
            }
            KeyboardEvent::KeyRelease { key } => {
                proxy.send_key(&key, false).await?;
            }
        }
        Ok(())
    }
}

impl OutputBackend for DbusPlumberOutput {
    async fn run(&mut self) -> Result<()> {
        loop {
            // Use try_recv with a small sleep to make this cancellable
            match self.stream.try_recv() {
                Ok(packet) => {
                    if let Err(e) = self.process_packet(packet).await {
                        tracing::error!("Failed to process packet: {}", e);
                    }
                }
                Err(crossbeam::channel::TryRecvError::Empty) => {
                    // No data available, sleep briefly to yield control
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                Err(crossbeam::channel::TryRecvError::Disconnected) => {
                    tracing::info!("Input stream disconnected, stopping D-Bus backend");
                    break;
                }
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("Stopping D-Bus Plumber output backend");
        // iterate each device and call stop on each device proxy
        let proxy = &self.backend.input_manager;
        // NOTE: drain() here consumes the whole devices list, so we can clean it up
        for device in self.backend.devices.drain().map(|(_, device)| device) {
            let target = &device.target_device;
            tracing::debug!("Stopping device: {}", target.path);
            target.stop(proxy).await?;
            tracing::debug!("Stopped device: {}", target.path);
        }
        Ok(())
    }
}

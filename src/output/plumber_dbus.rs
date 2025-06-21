//! InputPlumber D-Bus output backend, for sending messages to the InputPlumber D-Bus service.
use crate::input::InputEvent;
use crate::input::{InputEventPacket, InputEventStream, KeyboardEvent};
use crate::output::OutputBackend;
use eyre::Result;
use std::collections::HashMap;
use zbus_inputplumber::interface::input_manager::{self, InputManagerProxy, TargetDevice};
use zbus_inputplumber::interface::{gamepad::GamepadProxy, keyboard::KeyboardProxy};

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
        let input_manager = InputManagerProxy::new(&connection).await?;

        let mut devices = HashMap::new();

        // Create keyboard device
        let kb_device =
            TargetDevice::new(&input_manager, input_manager::TargetDeviceType::Keyboard).await?;
        let kb_proxy = KeyboardProxy::builder(&connection)
            .path(kb_device.path.clone())
            .unwrap()
            .build()
            .await?;
        devices.insert(
            input_manager::TargetDeviceType::Keyboard.into(),
            DeviceEntry {
                target_device: kb_device,
                proxy: DeviceProxy::Keyboard(kb_proxy),
            },
        );

        // Create gamepad device
        let gp_device =
            TargetDevice::new(&input_manager, input_manager::TargetDeviceType::Gamepad).await?;
        let gp_proxy = GamepadProxy::builder(&connection)
            .path(gp_device.path.clone())
            .unwrap()
            .build()
            .await?;
        devices.insert(
            input_manager::TargetDeviceType::Gamepad.into(),
            DeviceEntry {
                target_device: gp_device,
                proxy: DeviceProxy::Gamepad(gp_proxy),
            },
        );

        Ok(Self {
            connection,
            input_manager,
            devices,
        })
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
            // Initialize D-Bus connection and sender here
            backend: PlumberOutputBackend::new().await.unwrap(),
        }
    }
}

// #[async_trait]
impl OutputBackend for DbusPlumberOutput {
    async fn run(&mut self) -> Result<()> {
        // Main loop for processing input events and sending D-Bus messages
        loop {
            match self.stream.recv() {
                Ok(packet) => {
                    // Process the packet and send D-Bus messages
                    // For example, serialize the packet and send it over D-Bus
                    tracing::info!("Received input event packet: {:?}", packet);

                    // Now we can handle the data

                    for event in packet.events {
                        match &event {
                            // Handle keyboard events
                            InputEvent::Keyboard(keyboard_event) => {
                                if let Some(kb_device) = self.backend.keyboard() {
                                    match &kb_device.proxy {
                                        DeviceProxy::Keyboard(proxy) => match keyboard_event {
                                            KeyboardEvent::KeyPress { key } => {
                                                proxy.send_key(key, true).await?;
                                            }
                                            KeyboardEvent::KeyRelease { key } => {
                                                proxy.send_key(key, false).await?;
                                            }
                                        },
                                        // todo: Handle other device types
                                        _ => continue, // Skip if not a keyboard proxy
                                    }
                                }
                            }
                            // Handle gamepad events
                            _ => continue, // Skip other event types for now
                        }
                    }
                }
                Err(e) => {
                    // Handle errors, such as logging or retrying
                    tracing::error!("Failed to receive input event packet: {}", e);
                }
            }
        }
    }
}

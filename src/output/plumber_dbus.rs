//! InputPlumber D-Bus output backend, for sending messages to the InputPlumber D-Bus service.
use std::collections::HashMap;
// todo: Use our new virtual evdev
use eyre::Result;
use zbus_inputplumber::interface::mouse::MouseProxy;

use crate::input::{InputEvent, InputEventPacket, InputEventStream, KeyboardEvent, PointerEvent};

#[derive(Debug, Clone)]
struct KeyState {
    pressed: bool,
    last_timestamp: u64,
}
use crate::output::OutputBackend;
use zbus_inputplumber::interface::{
    gamepad::GamepadProxy,
    input_manager::{self, InputManagerProxy, TargetDevice},
    keyboard::KeyboardProxy,
};

pub enum DeviceProxy {
    Keyboard(KeyboardProxy<'static>),
    Mouse(MouseProxy<'static>),
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
    #[cfg(test)] // Part of public API, may be used when D-Bus output is enabled
    pub async fn new() -> Result<Self> {
        let connection = zbus::Connection::system().await?;
        tracing::info!("Connected to D-Bus system bus");
        let input_manager = InputManagerProxy::new(&connection).await?;

        let mut devices = HashMap::new();

        // Create devices using helper function
        Self::create_keyboard_device(&connection, &input_manager, &mut devices).await?;
        Self::create_gamepad_device(&connection, &input_manager, &mut devices).await?;
        Self::create_mouse_device(&connection, &input_manager, &mut devices).await?;

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

    async fn create_mouse_device(
        connection: &zbus::Connection,
        input_manager: &InputManagerProxy<'_>,
        devices: &mut HashMap<&'static str, DeviceEntry>,
    ) -> Result<()> {
        tracing::info!("Creating mouse device...");
        let device_type = input_manager::TargetDeviceType::Mouse;
        let target_device = TargetDevice::new(input_manager, device_type).await?;
        tracing::info!("Mouse target device created: {}", target_device.path);
        let proxy = MouseProxy::builder(connection)
            .path(target_device.path.clone())?
            .build()
            .await?;
        tracing::info!("Mouse proxy created at path: {}", target_device.path);

        devices.insert(
            device_type.into(),
            DeviceEntry {
                target_device,
                proxy: DeviceProxy::Mouse(proxy),
            },
        );
        tracing::debug!("Mouse device added to devices map");
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
    // Input event stream
    pub stream: InputEventStream,
    // sender for D-Bus messages
    pub backend: PlumberOutputBackend,
    // Key state tracking to prevent out-of-order events
    key_states: HashMap<String, KeyState>,
}

impl DbusPlumberOutput {
    /// Creates a new `DbusPlumberOutput` with the given input event stream.
    #[cfg(test)] // Part of public API, may be used when D-Bus output is enabled
    pub async fn new(stream: InputEventStream) -> Self {
        Self {
            stream,
            backend: PlumberOutputBackend::new().await.unwrap(),
            key_states: HashMap::new(),
        }
    }

    async fn process_packet(&mut self, packet: InputEventPacket) -> Result<()> {
        tracing::info!("Received input event packet: {:?}", packet);

        for event in packet.events {
            self.handle_event(event, packet.timestamp).await?;
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: InputEvent, timestamp: u64) -> Result<()> {
        match event {
            InputEvent::Keyboard(keyboard_event) => {
                self.handle_keyboard_event(keyboard_event, timestamp).await
            }
            InputEvent::Pointer(pointer_event) => self.handle_pointer_event(pointer_event).await,
            _ => {
                // Skip other event types for now
                tracing::debug!("Skipping unsupported event type");
                Ok(())
            }
        }
    }

    async fn handle_keyboard_event(&mut self, event: KeyboardEvent, timestamp: u64) -> Result<()> {
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
                // Get the current state for this specific key only
                let current_state = self.key_states.get(&key);

                // Check if we should ignore this KeyPress due to out-of-order timing
                // Only ignore if:
                // 1. We have a previous state for this key
                // 2. The key is currently not pressed
                // 3. This timestamp is older than the last timestamp for THIS SPECIFIC KEY
                if let Some(state) = current_state {
                    if !state.pressed && timestamp < state.last_timestamp {
                        tracing::warn!(
                            "Ignoring out-of-order KeyPress for key '{}' (timestamp: {}, last: {})",
                            key,
                            timestamp,
                            state.last_timestamp
                        );
                        return Ok(());
                    }
                }

                // Update state for this specific key and send the key press
                self.key_states.insert(
                    key.clone(),
                    KeyState {
                        pressed: true,
                        last_timestamp: timestamp,
                    },
                );
                proxy.send_key(&key, true).await?;
            }
            KeyboardEvent::KeyRelease { key } => {
                // Always allow KeyRelease events and update state for this specific key
                self.key_states.insert(
                    key.clone(),
                    KeyState {
                        pressed: false,
                        last_timestamp: timestamp,
                    },
                );
                proxy.send_key(&key, false).await?;
            }
        }
        Ok(())
    }

    async fn handle_pointer_event(&mut self, event: PointerEvent) -> Result<()> {
        let Some(mouse_device) = self.backend.get_device("mouse") else {
            tracing::warn!("No mouse device available");
            return Ok(());
        };

        let Some(kb_device) = self.backend.get_device("keyboard") else {
            tracing::warn!("No keyboard device available for pointer events");
            return Ok(());
        };

        let DeviceProxy::Mouse(proxy) = &mouse_device.proxy else {
            tracing::error!("Expected mouse proxy but got different device type");
            return Ok(());
        };

        let DeviceProxy::Keyboard(kb_proxy) = &kb_device.proxy else {
            tracing::error!("Expected keyboard proxy but got different device type");
            return Ok(());
        };

        const POINTER_EVENTS: [(PointerEvent, &str); 5] = [
            (PointerEvent::Click { button: 1 }, "BTN_LEFT"),
            (PointerEvent::Click { button: 2 }, "BTN_RIGHT"),
            (PointerEvent::Click { button: 3 }, "BTN_MIDDLE"),
            (PointerEvent::Click { button: 4 }, "BTN_SIDE"),
            (PointerEvent::Click { button: 5 }, "BTN_EXTRA"),
        ];
        // click order is left, right, middle, side, extra

        match event {
            PointerEvent::Move { x_delta, y_delta } => {
                proxy.move_cursor(x_delta, y_delta).await?;
            }
            PointerEvent::Click { button } => {
                if let Some((_, key)) = POINTER_EVENTS.iter().find(
                    |(event, _)| matches!(event, PointerEvent::Click { button: b } if *b == button),
                ) {
                    kb_proxy.send_key(key, true).await?;
                } else {
                    tracing::warn!("Unsupported pointer button: {}", button);
                    return Ok(());
                }
            }
            PointerEvent::ClickRelease { button } => {
                if let Some((_, key)) = POINTER_EVENTS.iter().find(
                    |(event, _)| matches!(event, PointerEvent::Click { button: b } if *b == button),
                ) {
                    kb_proxy.send_key(key, false).await?;
                } else {
                    tracing::warn!("Unsupported pointer button release: {}", button);
                    return Ok(());
                }
            }
            PointerEvent::Scroll { x_delta, y_delta } => {
                if y_delta > 0 {
                    kb_proxy.send_key("KEY_SCROLLUP", true).await?;
                    kb_proxy.send_key("KEY_SCROLLUP", false).await?;
                } else if y_delta < 0 {
                    kb_proxy.send_key("KEY_SCROLLDOWN", true).await?;
                    kb_proxy.send_key("KEY_SCROLLDOWN", false).await?;
                }

                if x_delta > 0 {
                    kb_proxy.send_key("KEY_SCROLLRIGHT", true).await?;
                    kb_proxy.send_key("KEY_SCROLLRIGHT", false).await?;
                } else if x_delta < 0 {
                    kb_proxy.send_key("KEY_SCROLLLEFT", true).await?;
                    kb_proxy.send_key("KEY_SCROLLLEFT", false).await?;
                }
            }
        }
        Ok(())
    }
}

impl OutputBackend for DbusPlumberOutput {
    async fn run(&mut self) -> Result<()> {
        loop {
            // Use async receive method - much cleaner than try_recv with sleep
            match self.stream.receive().await {
                Some(packet) => {
                    if let Err(e) = self.process_packet(packet).await {
                        tracing::error!("Failed to process packet: {}", e);
                    }
                }
                None => {
                    tracing::info!("Input stream closed, stopping D-Bus backend");
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

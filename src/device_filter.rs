//! Device filtering and transformation module
//!
//! This module provides per-device filtering and key remapping functionality.
//! It acts as an intermediate layer between input and output backends,
//! allowing custom keycodes to be remapped to standard evdev codes
//! and routing events to specific output backends based on device configuration.

use crate::config::{AppConfig, DeviceConfig};
use crate::input::{InputEvent, InputEventPacket, KeyboardEvent, PointerEvent, JoystickEvent};
use eyre::Result;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Device filter that applies per-device transformations and routing
pub struct DeviceFilter {
    /// Device configurations mapped by device ID
    device_configs: HashMap<String, DeviceConfig>,
}

impl DeviceFilter {
    /// Create a new device filter from application configuration
    pub fn new(config: &AppConfig) -> Self {
        Self {
            device_configs: config.device.clone(),
        }
    }

    /// Transform an input event packet according to device-specific rules
    pub fn transform_packet(&self, mut packet: InputEventPacket) -> Result<InputEventPacket> {
        // Get device configuration for this device ID
        let device_config = self.device_configs.get(&packet.device_id);
        
        if let Some(config) = device_config {
            debug!(
                "Applying device config for device '{}': backend={}, type={}",
                packet.device_id, config.map_backend, config.device_type
            );
            
            // Transform all events in the packet
            for event in &mut packet.events {
                self.transform_event(event, config)?;
            }
        } else {
            debug!(
                "No device config found for device '{}', using default transformation",
                packet.device_id
            );
        }

        Ok(packet)
    }

    /// Transform a single input event based on device configuration
    fn transform_event(&self, event: &mut InputEvent, config: &DeviceConfig) -> Result<()> {
        match event {
            InputEvent::Keyboard(keyboard_event) => {
                self.transform_keyboard_event(keyboard_event, config)?;
            }
            InputEvent::Pointer(_) => {
                // Pointer events don't typically need key remapping
                // but could be extended in the future for device-specific transformations
            }
            InputEvent::Joystick(_) => {
                // Joystick events could be remapped to keyboard events
                // or other transformations in the future
            }
        }
        Ok(())
    }

    /// Transform keyboard events based on device remapping rules
    fn transform_keyboard_event(&self, event: &mut KeyboardEvent, config: &DeviceConfig) -> Result<()> {
        let key = match event {
            KeyboardEvent::KeyPress { key } => key,
            KeyboardEvent::KeyRelease { key } => key,
        };

        // Check if this key needs to be remapped
        if let Some(remapped_key) = config.remap.get(key) {
            debug!("Remapping key '{}' to '{}' for device", key, remapped_key);
            *key = remapped_key.clone();
        } else if !Self::is_standard_evdev_key(key) {
            // If it's not a standard evdev key and no remapping is defined, warn
            warn!(
                "Custom key '{}' has no remapping defined for device. Event may be ignored by output backend.",
                key
            );
        }

        Ok(())
    }

    /// Check if a key string appears to be a standard evdev key code
    fn is_standard_evdev_key(key: &str) -> bool {
        // Standard evdev keys typically start with KEY_, BTN_, etc.
        key.starts_with("KEY_") || 
        key.starts_with("BTN_") || 
        key.starts_with("ABS_") || 
        key.starts_with("REL_")
    }

    /// Get the target backend for a specific device
    pub fn get_device_backend(&self, device_id: &str) -> Option<&str> {
        self.device_configs
            .get(device_id)
            .map(|config| config.map_backend.as_str())
    }

    /// Get the device type for a specific device
    pub fn get_device_type(&self, device_id: &str) -> Option<&str> {
        self.device_configs
            .get(device_id)
            .map(|config| config.device_type.as_str())
    }

    /// Check if a device has any configuration
    pub fn has_device_config(&self, device_id: &str) -> bool {
        self.device_configs.contains_key(device_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, DeviceConfig};
    use crate::input::{InputEvent, InputEventPacket, KeyboardEvent};
    use std::collections::HashMap;

    fn create_test_config() -> AppConfig {
        let mut device_configs = HashMap::new();
        
        // Create a test device config with remapping
        let mut remap = HashMap::new();
        remap.insert("SLIDER_1".to_string(), "KEY_A".to_string());
        remap.insert("SLIDER_2".to_string(), "KEY_B".to_string());
        remap.insert("GAME_1".to_string(), "KEY_SPACE".to_string());
        
        let device_config = DeviceConfig {
            map_backend: "uinput".to_string(),
            device_type: "keyboard".to_string(),
            remap,
        };
        
        device_configs.insert("test_device".to_string(), device_config);
        
        AppConfig {
            device: device_configs,
            ..Default::default()
        }
    }

    #[test]
    fn test_device_filter_creation() {
        let config = create_test_config();
        let filter = DeviceFilter::new(&config);
        
        assert!(filter.has_device_config("test_device"));
        assert!(!filter.has_device_config("nonexistent_device"));
        assert_eq!(filter.get_device_backend("test_device"), Some("uinput"));
        assert_eq!(filter.get_device_type("test_device"), Some("keyboard"));
    }

    #[test]
    fn test_key_remapping() {
        let config = create_test_config();
        let filter = DeviceFilter::new(&config);
        
        // Create a test packet with custom keys
        let mut packet = InputEventPacket::new("test_device".to_string(), 12345);
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
            key: "SLIDER_1".to_string(),
        }));
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyRelease {
            key: "GAME_1".to_string(),
        }));
        
        let transformed_packet = filter.transform_packet(packet).unwrap();
        
        // Check that keys were remapped
        match &transformed_packet.events[0] {
            InputEvent::Keyboard(KeyboardEvent::KeyPress { key }) => {
                assert_eq!(key, "KEY_A");
            }
            _ => panic!("Expected KeyPress event"),
        }
        
        match &transformed_packet.events[1] {
            InputEvent::Keyboard(KeyboardEvent::KeyRelease { key }) => {
                assert_eq!(key, "KEY_SPACE");
            }
            _ => panic!("Expected KeyRelease event"),
        }
    }

    #[test]
    fn test_standard_keys_passthrough() {
        let config = create_test_config();
        let filter = DeviceFilter::new(&config);
        
        // Create a packet with standard evdev keys
        let mut packet = InputEventPacket::new("test_device".to_string(), 12345);
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
            key: "KEY_A".to_string(),
        }));
        
        let transformed_packet = filter.transform_packet(packet).unwrap();
        
        // Standard keys should pass through unchanged
        match &transformed_packet.events[0] {
            InputEvent::Keyboard(KeyboardEvent::KeyPress { key }) => {
                assert_eq!(key, "KEY_A");
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    #[test]
    fn test_unconfigured_device() {
        let config = create_test_config();
        let filter = DeviceFilter::new(&config);
        
        // Create a packet for a device without configuration
        let mut packet = InputEventPacket::new("unknown_device".to_string(), 12345);
        packet.add_event(InputEvent::Keyboard(KeyboardEvent::KeyPress {
            key: "SLIDER_1".to_string(),
        }));
        
        let transformed_packet = filter.transform_packet(packet).unwrap();
        
        // Keys should remain unchanged for unconfigured devices
        match &transformed_packet.events[0] {
            InputEvent::Keyboard(KeyboardEvent::KeyPress { key }) => {
                assert_eq!(key, "SLIDER_1");
            }
            _ => panic!("Expected KeyPress event"),
        }
    }

    #[test]
    fn test_is_standard_evdev_key() {
        assert!(DeviceFilter::is_standard_evdev_key("KEY_A"));
        assert!(DeviceFilter::is_standard_evdev_key("BTN_LEFT"));
        assert!(DeviceFilter::is_standard_evdev_key("ABS_X"));
        assert!(DeviceFilter::is_standard_evdev_key("REL_X"));
        
        assert!(!DeviceFilter::is_standard_evdev_key("SLIDER_1"));
        assert!(!DeviceFilter::is_standard_evdev_key("GAME_1"));
        assert!(!DeviceFilter::is_standard_evdev_key("CUSTOM_KEY"));
    }
}
//! Config modules for the application.

// todo: finish this file

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub feedback: FeedbackConfig,
    /// Per-device configurations for filtering and remapping
    #[serde(default)]
    pub device: HashMap<String, DeviceConfig>,
}

impl AppConfig {
    pub fn from_toml_str(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: AppConfig = Self::from_toml_str(&contents)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;
        Ok(config)
    }

    /// Load configuration with fallback to defaults
    pub fn load_or_default() -> Self {
        // Try to load from standard locations
        // for now, backflow.toml is the only config file in the current directory
        let config_paths = [std::path::PathBuf::from("backflow.toml")];

        for path in &config_paths {
            if let Ok(config) = Self::from_file(path) {
                tracing::info!("Loaded configuration from: {}", path.display());
                return config;
            }
        }

        tracing::info!("No configuration file found, using defaults");
        Self::default()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InputConfig {
    #[serde(default = "default_web_enabled")]
    pub web: Option<WebBackend>,
    #[serde(default)]
    pub unix: Option<UnixDomainSocketConfig>,
    // #[serde(default)]
    // pub chuniio: Option<ChuniIoSerialConfig>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            web: default_web_enabled(),
            unix: None,
        }
    }
}

fn default_web_enabled() -> Option<WebBackend> {
    Some(WebBackend::default())
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct OutputConfig {
    #[serde(default)]
    pub uinput: UInputConfig,
    #[serde(default)]
    pub chuniio_proxy: Option<ChuniioProxyConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct UnixDomainSocketConfig {
    #[serde(default = "default_unix_socket_path")]
    pub path: PathBuf,
}
fn default_unix_socket_path() -> PathBuf {
    use std::env;
    // Check environment variable first
    if let Ok(env_path) = env::var("BACKFLOW_UNIX_SOCKET") {
        return PathBuf::from(env_path);
    }
    let uid = nix::unistd::Uid::effective().as_raw();
    PathBuf::from(format!("/run/user/{}/backflow", uid))
}

// set web.enabled = false in [input.web] to explicitly disable the web backend
#[derive(Debug, Deserialize, Serialize)]
pub struct WebBackend {
    #[serde(default = "default_web_enabled_bool")]
    pub enabled: bool,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default = "default_web_host")]
    pub host: String,
}

impl Default for WebBackend {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 8000,
            host: "0.0.0.0".to_string(),
        }
    }
}

fn default_web_enabled_bool() -> bool {
    true
}

fn default_web_port() -> u16 {
    8000
}

fn default_web_host() -> String {
    "0.0.0.0".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UInputConfig {
    pub enabled: bool,
}

impl Default for UInputConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct FeedbackConfig {
    // CHUNIIO RGB feedback socket
    pub chuniio: Option<ChuniIoRgbConfig>,
    // pub rgb:
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChuniIoRgbConfig {
    /// Path to the Unix domain socket for ChuniIo RGB feedback, usually from Outflow bridge from inside Wine
    pub socket_path: PathBuf,
    /// Number of RGB outputs to clamp to
    /// Default will be at 32
    #[serde(default = "default_slider_lights")]
    pub slider_clamp_lights: u32,

    /// The offset of the light ID, defaults to 0 (no offset, emit light events from 0-31)
    /// Useful if you want to route to specific lights
    #[serde(default)]
    pub slider_id_offset: u32,
}

fn default_slider_lights() -> u32 {
    32
}

/// Configuration for a specific device, including backend routing and key remapping
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceConfig {
    /// Which output backend this device should route to (e.g., "uinput", "inputplumber")
    pub map_backend: String,
    /// The type of device this represents (e.g., "keyboard", "mouse", "gamepad")
    pub device_type: String,
    /// Key remapping from custom keys to evdev codes
    #[serde(default)]
    pub remap: HashMap<String, String>,
    /// When true, only keys defined in the remap table are allowed through (whitelist mode)
    /// When false (default), undefined keys pass through unchanged
    #[serde(default)]
    pub remap_whitelist: bool,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            map_backend: "uinput".to_string(),
            device_type: "keyboard".to_string(),
            remap: HashMap::new(),
            remap_whitelist: false,
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct ChuniioProxyConfig {
    #[serde(default = "default_chuniio_proxy_enabled")]
    pub enabled: bool,
    #[serde(default = "default_chuniio_proxy_socket_path")]
    pub socket_path: PathBuf,
}

impl Default for ChuniioProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            socket_path: default_chuniio_proxy_socket_path(),
        }
    }
}

fn default_chuniio_proxy_enabled() -> bool {
    false
}

fn default_chuniio_proxy_socket_path() -> PathBuf {
    use std::env;
    // Check environment variable first
    if let Ok(env_path) = env::var("CHUNIIO_PROXY_SOCKET") {
        return PathBuf::from(env_path);
    }
    // Try to use user runtime directory, fallback to /tmp
    if let Ok(uid) = env::var("UID") {
        let runtime_path = format!("/run/user/{}/backflow_chuniio", uid);
        if std::path::Path::new(&format!("/run/user/{}", uid)).exists() {
            PathBuf::from(runtime_path)
        } else {
            PathBuf::from("/tmp/backflow_chuniio")
        }
    } else {
        PathBuf::from("/tmp/backflow_chuniio")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_app_config() {
        let config = AppConfig::default();
        // InputConfig.web should be Some(WebBackend::default())
        let web = config.input.web.as_ref().unwrap();
        assert!(web.enabled);
        assert_eq!(web.port, 8000);
        assert_eq!(web.host, "0.0.0.0");
        // OutputConfig.uinput.enabled should be true
        assert!(config.output.uinput.enabled);
        // FeedbackConfig.chuniio should be None
        assert!(config.feedback.chuniio.is_none());
    }

    #[test]
    fn test_disable_web_backend() {
        let toml_str = r#"
            [input.web]
            enabled = false
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let web = config.input.web.unwrap();
        assert!(!web.enabled);
    }

    #[test]
    fn test_custom_web_backend() {
        let toml_str = r#"
            [input.web]
            port = 1234
            host = "127.0.0.1"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let web = config.input.web.unwrap();
        assert!(web.enabled); // Should default to true
        assert_eq!(web.port, 1234);
        assert_eq!(web.host, "127.0.0.1");
    }

    #[test]
    fn test_default_chuniio_rgb_config() {
        let toml_str = r#"
            [feedback.chuniio]
            socket_path = "/tmp/chuniio.sock"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let chuniio = config.feedback.chuniio.unwrap();
        assert_eq!(chuniio.socket_path, PathBuf::from("/tmp/chuniio.sock"));
        assert_eq!(chuniio.slider_clamp_lights, 32);
        assert_eq!(chuniio.slider_id_offset, 0);
    }

    #[test]
    fn test_custom_chuniio_rgb_config() {
        let toml_str = r#"
            [feedback.chuniio]
            socket_path = "/tmp/chuniio.sock"
            slider_clamp_lights = 16
            slider_id_offset = 2
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let chuniio = config.feedback.chuniio.unwrap();
        assert_eq!(chuniio.slider_clamp_lights, 16);
        assert_eq!(chuniio.slider_id_offset, 2);
    }

    #[test]
    fn test_default_uinput_config() {
        let config = OutputConfig::default();
        assert!(config.uinput.enabled);
    }

    #[test]
    fn test_disable_uinput() {
        let toml_str = r#"
            [output.uinput]
            enabled = false
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.output.uinput.enabled);
    }

    #[test]
    fn test_device_config_basic() {
        let toml_str = r#"
            [device."test_device"]
            map_backend = "uinput"
            device_type = "keyboard"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let device_config = config.device.get("test_device").unwrap();
        assert_eq!(device_config.map_backend, "uinput");
        assert_eq!(device_config.device_type, "keyboard");
        assert!(device_config.remap.is_empty());
    }

    #[test]
    fn test_device_config_with_remapping() {
        let toml_str = r#"
            [device."slider_device"]
            map_backend = "uinput"
            device_type = "keyboard"

            [device."slider_device".remap]
            "SLIDER_1" = "KEY_A"
            "SLIDER_2" = "KEY_B"
            "GAME_1" = "KEY_SPACE"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let device_config = config.device.get("slider_device").unwrap();
        assert_eq!(device_config.map_backend, "uinput");
        assert_eq!(device_config.device_type, "keyboard");
        assert_eq!(
            device_config.remap.get("SLIDER_1"),
            Some(&"KEY_A".to_string())
        );
        assert_eq!(
            device_config.remap.get("SLIDER_2"),
            Some(&"KEY_B".to_string())
        );
        assert_eq!(
            device_config.remap.get("GAME_1"),
            Some(&"KEY_SPACE".to_string())
        );
    }

    #[test]
    fn test_multiple_devices() {
        let toml_str = r#"
            [device."keyboard_device"]
            map_backend = "uinput"
            device_type = "keyboard"

            [device."gamepad_device"]
            map_backend = "inputplumber"
            device_type = "gamepad"

            [device."gamepad_device".remap]
            "BUTTON_A" = "BTN_A"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();

        let keyboard_config = config.device.get("keyboard_device").unwrap();
        assert_eq!(keyboard_config.map_backend, "uinput");
        assert_eq!(keyboard_config.device_type, "keyboard");

        let gamepad_config = config.device.get("gamepad_device").unwrap();
        assert_eq!(gamepad_config.map_backend, "inputplumber");
        assert_eq!(gamepad_config.device_type, "gamepad");
        assert_eq!(
            gamepad_config.remap.get("BUTTON_A"),
            Some(&"BTN_A".to_string())
        );
    }

    #[test]
    fn test_device_example_config_format() {
        let toml_str = r#"
            [input.web]
            enabled = true
            port = 8000
            host = "0.0.0.0"

            [output.uinput]
            enabled = true

            [device."slider_controller"]
            map_backend = "uinput"
            device_type = "keyboard"

            [device."slider_controller".remap]
            "SLIDER_1" = "KEY_A"
            "SLIDER_2" = "KEY_S"
            "SLIDER_3" = "KEY_D"

            [device."custom_gamepad"]
            map_backend = "uinput"
            device_type = "keyboard"

            [device."custom_gamepad".remap]
            "GAME_1" = "KEY_SPACE"
            "BUTTON_A" = "KEY_Z"
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        // Test slider controller
        let slider_config = config.device.get("slider_controller").unwrap();
        assert_eq!(slider_config.map_backend, "uinput");
        assert_eq!(slider_config.device_type, "keyboard");
        assert_eq!(
            slider_config.remap.get("SLIDER_1"),
            Some(&"KEY_A".to_string())
        );
        assert_eq!(
            slider_config.remap.get("SLIDER_2"),
            Some(&"KEY_S".to_string())
        );
        assert_eq!(
            slider_config.remap.get("SLIDER_3"),
            Some(&"KEY_D".to_string())
        );
        assert!(!slider_config.remap_whitelist); // Should default to false

        // Test custom gamepad
        let gamepad_config = config.device.get("custom_gamepad").unwrap();
        assert_eq!(gamepad_config.map_backend, "uinput");
        assert_eq!(gamepad_config.device_type, "keyboard");
        assert_eq!(
            gamepad_config.remap.get("GAME_1"),
            Some(&"KEY_SPACE".to_string())
        );
        assert_eq!(
            gamepad_config.remap.get("BUTTON_A"),
            Some(&"KEY_Z".to_string())
        );
        assert!(!gamepad_config.remap_whitelist); // Should default to false

        // Test other configuration sections remain working
        assert!(config.input.web.is_some());
        assert!(config.output.uinput.enabled);
    }

    #[test]
    fn test_device_config_with_whitelist_enabled() {
        let toml_str = r#"
            [device."whitelist_device"]
            map_backend = "uinput"
            device_type = "keyboard"
            remap_whitelist = true

            [device."whitelist_device".remap]
            "SLIDER_1" = "KEY_A"
            "GAME_1" = "KEY_SPACE"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let device_config = config.device.get("whitelist_device").unwrap();
        assert_eq!(device_config.map_backend, "uinput");
        assert_eq!(device_config.device_type, "keyboard");
        assert!(device_config.remap_whitelist);
        assert_eq!(
            device_config.remap.get("SLIDER_1"),
            Some(&"KEY_A".to_string())
        );
        assert_eq!(
            device_config.remap.get("GAME_1"),
            Some(&"KEY_SPACE".to_string())
        );
    }

    #[test]
    fn test_device_config_with_whitelist_enabled_no_remap() {
        let toml_str = r#"
            [device."ignore_all_device"]
            map_backend = "uinput"
            device_type = "keyboard"
            remap_whitelist = true
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let device_config = config.device.get("ignore_all_device").unwrap();
        assert_eq!(device_config.map_backend, "uinput");
        assert_eq!(device_config.device_type, "keyboard");
        assert!(device_config.remap_whitelist);
        assert!(device_config.remap.is_empty());
    }

    #[test]
    fn test_device_config_with_whitelist_explicitly_disabled() {
        let toml_str = r#"
            [device."passthrough_device"]
            map_backend = "uinput"
            device_type = "keyboard"
            remap_whitelist = false

            [device."passthrough_device".remap]
            "SLIDER_1" = "KEY_A"
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let device_config = config.device.get("passthrough_device").unwrap();
        assert_eq!(device_config.map_backend, "uinput");
        assert_eq!(device_config.device_type, "keyboard");
        assert!(!device_config.remap_whitelist);
        assert_eq!(
            device_config.remap.get("SLIDER_1"),
            Some(&"KEY_A".to_string())
        );
    }
}

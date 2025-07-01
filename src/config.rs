//! Config modules for the application.

// todo: finish this file

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub feedback: FeedbackConfig,
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
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct UnixDomainSocketConfig {
    pub path: PathBuf,
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
}

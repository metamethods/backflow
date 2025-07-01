//! Config modules for the application.

// todo: finish this file

use eyre::{ContextCompat, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub feedback: FeedbackConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InputConfig {
    #[serde(default = "default_web_enabled")]
    pub web: Option<WebBackend>,
    #[serde(default)]
    pub unix: Option<UnixDomainSocketConfig>,
    // #[serde(default)]
    // pub chuniio: Option<ChuniIoSerialConfig>,
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


// set web = null in [input] to explicitly disable the web backend
#[derive(Debug, Deserialize, Serialize)]
pub struct WebBackend {
    pub port: u16,
    pub host: String,
}

impl Default for WebBackend {
    fn default() -> Self {
        Self {
            port: 8000,
            host: "0.0.0.0".to_string(),
        }
    }
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

#[derive(Debug, Deserialize, Serialize)]
pub struct ChuniIoRgbConfig {
    /// Path to the Unix domain socket for ChuniIo RGB feedback, usually from Outflow bridge from inside Wine
    pub socket_path: PathBuf,
}

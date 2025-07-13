//! Utility for forwarding a TCP port from an iOS device to localhost using libimobiledevice (idevice crate)

use std::process::Stdio;
use tokio::process::Command;

/// Spawns an `iproxy` process to forward a device port to a local port.
/// Returns the child process handle. The caller is responsible for keeping it alive.
pub async fn spawn_iproxy(
    local_port: u16,
    device_port: u16,
    udid: Option<&str>,
) -> std::io::Result<tokio::process::Child> {
    let mut cmd = Command::new("iproxy");
    cmd.arg(local_port.to_string()).arg(device_port.to_string());
    if let Some(udid) = udid {
        cmd.arg("--udid").arg(udid);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn()
}

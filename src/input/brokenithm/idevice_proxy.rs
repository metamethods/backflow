//! Utility for forwarding a TCP port from an iOS device to localhost using libimobiledevice (idevice crate)

use nix::libc;
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
    match cmd.spawn() {
        Ok(child) => Ok(child),
        Err(e) => {
            if let Some(raw_os_error) = e.raw_os_error() {
                if raw_os_error == libc::ENOENT {
                    tracing::error!(
                        "Failed to spawn 'iproxy': command not found. Is usbmuxd-utils installed?"
                    );
                }
            }
            Err(e)
        }
    }
}

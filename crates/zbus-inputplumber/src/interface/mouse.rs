//! # D-Bus interface proxy for: `org.shadowblip.Input.Mouse`
//! Source: `org.shadowblip.Input.Mouse.xml`.
//!
//! You may prefer to adapt it, instead of using it verbatim.
//!
//! More information can be found in the [Writing a client proxy] section of the zbus
//! documentation.
//!
//! This type implements the [D-Bus standard interfaces], (`org.freedesktop.DBus.*`) for which the
//! following zbus API can be used:
//!
//! * [`zbus::fdo::IntrospectableProxy`]
//! * [`zbus::fdo::PeerProxy`]
//! * [`zbus::fdo::PropertiesProxy`]
//!
//! Consequently `zbus-xmlgen` did not generate code for the above interfaces.
//!
//! [Writing a client proxy]: https://dbus2.github.io/zbus/client.html
//! [D-Bus standard interfaces]: https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces,
use zbus::proxy;
#[proxy(
    interface = "org.shadowblip.Input.Mouse",
    assume_defaults = true,
    default_service = "org.shadowblip.InputPlumber"
)]
pub trait Mouse {
    /// MoveCursor method
    fn move_cursor(&self, x: i32, y: i32) -> zbus::Result<()>;

    /// Name property
    #[zbus(property)]
    fn name(&self) -> zbus::Result<String>;
}

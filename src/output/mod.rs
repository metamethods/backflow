pub mod dummy;
pub mod plumber_dbus;
pub mod udev;

// Re-export the concrete types for external use
pub use dummy::DummyOutput;
pub use plumber_dbus::DbusPlumberOutput;
pub use udev::UdevOutput;

// even though our MSRV is Rust 1.75 and Rust 2024,
// we use async_trait because of trait object safety
pub trait OutputBackend: Send {
    /// Starts the output backend, processing input events and sending them to the appropriate destination.
    async fn run(&mut self) -> eyre::Result<()>;
    async fn stop(&mut self) -> eyre::Result<()> {
        // Default implementation does nothing
        Ok(())
    }
}

pub enum OutputBackendType {
    Dummy(DummyOutput),
    Dbus(DbusPlumberOutput),
    Udev(UdevOutput),
}

impl OutputBackend for OutputBackendType {
    async fn run(&mut self) -> eyre::Result<()> {
        match self {
            OutputBackendType::Dummy(backend) => backend.run().await,
            OutputBackendType::Dbus(backend) => backend.run().await,
            OutputBackendType::Udev(backend) => backend.run().await,
        }
    }

    async fn stop(&mut self) -> eyre::Result<()> {
        match self {
            OutputBackendType::Dummy(backend) => backend.stop().await,
            OutputBackendType::Dbus(backend) => backend.stop().await,
            OutputBackendType::Udev(backend) => backend.stop().await,
        }
    }
}

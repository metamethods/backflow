pub mod dummy;
pub mod plumber_dbus;

// Re-export the concrete types for external use
pub use dummy::DummyOutput;
pub use plumber_dbus::DbusPlumberOutput;

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
}

impl OutputBackend for OutputBackendType {
    async fn run(&mut self) -> eyre::Result<()> {
        match self {
            OutputBackendType::Dummy(backend) => backend.run().await,
            OutputBackendType::Dbus(backend) => backend.run().await,
        }
    }

    async fn stop(&mut self) -> eyre::Result<()> {
        match self {
            OutputBackendType::Dummy(backend) => backend.stop().await,
            OutputBackendType::Dbus(backend) => backend.stop().await,
        }
    }
}

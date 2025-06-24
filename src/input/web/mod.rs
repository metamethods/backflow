//! The web UI backend, running with the WebSocket backend in a nested path.
//!
//! This module provides an axum-based HTTP server that:
//! 1. Handles WebSocket connections for input events
//! 2. Optionally serves static files for the web UI frontend
//!
//! The implementation uses tower middleware for extensibility and axum
//! for its ergonomic routing API.

pub mod frontend;
mod server;

pub use server::WebServer;

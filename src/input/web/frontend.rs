//! Frontend file serving implementation for the web UI.
//!
//! This module provides utilities for serving static files for the web UI.

// use axum::Router;
use std::path::{Path, PathBuf};
// use tower_http::services::fs::ServeDir;
use tracing::{info, warn};

/// Attempts to find a valid web UI directory, checking common locations.
///
/// Checks the following locations in order:
/// 1. The provided path (if any)
/// 2. A `web` directory in the current working directory
/// 3. A `web` directory in the executable's directory
pub fn find_web_ui_dir(configured_path: Option<PathBuf>) -> Option<PathBuf> {
    // First, check if a path was explicitly configured
    if let Some(path) = configured_path {
        if path.exists() && path.is_dir() {
            info!("Using configured web UI directory: {}", path.display());
            return Some(path);
        } else {
            warn!(
                "Configured web UI directory does not exist: {}",
                path.display()
            );
        }
    }

    // Try to find a web directory in the current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let web_dir = cwd.join("web");
        if web_dir.exists() && web_dir.is_dir() {
            info!(
                "Found web UI directory in current working directory: {}",
                web_dir.display()
            );
            return Some(web_dir);
        }
    }

    // Try to find a web directory relative to the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let web_dir = exe_dir.join("web");
            if web_dir.exists() && web_dir.is_dir() {
                info!(
                    "Found web UI directory next to executable: {}",
                    web_dir.display()
                );
                return Some(web_dir);
            }
        }
    }

    warn!("No web UI directory found");
    None
}

// /// Builds a router for serving the web UI if the directory exists.
// pub fn build_frontend_router(web_dir: &Path) -> Router {
//     info!(
//         "Setting up frontend file serving from {}",
//         web_dir.display()
//     );

//     // Create a service for serving static files with SPA support
//     let serve_dir = ServeDir::new(web_dir).append_index_html_on_directories(true);

//     Router::new().fallback_service(serve_dir)
// }

/// Checks if an HTML file exists in the web directory to confirm it's a valid web UI.
pub fn is_valid_web_ui(web_dir: &Path) -> bool {
    let index_html = web_dir.join("index.html");
    index_html.exists() && index_html.is_file()
}

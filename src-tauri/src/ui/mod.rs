/// UI bridge layer: Tauri commands and events
///
/// This module provides the bridge between the Rust backend and
/// JavaScript frontend via Tauri's IPC system.

pub mod commands;
pub mod events;
pub mod state;

// Re-export commonly used types
pub use commands::*;
pub use events::*;
pub use state::AppState;

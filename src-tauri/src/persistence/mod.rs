/// Data persistence: world files, logs, preferences
///
/// This module handles all data storage and retrieval:
/// - World files (XML format)
/// - Session logging
/// - Application preferences

pub mod world_file;
pub mod logging;

// Re-export commonly used types
pub use world_file::WorldFile;
pub use logging::SessionLogger;

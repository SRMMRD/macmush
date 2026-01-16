/// Core domain logic for MACMush
///
/// This module contains the fundamental types and logic for MUD connections,
/// world configuration, and session management.

pub mod connection;
pub mod world;
pub mod session;
pub mod events;

// Re-export commonly used types
pub use connection::Connection;
pub use world::{World, WorldConfig};
pub use session::Session;
pub use events::{MudEvent, EventBus};

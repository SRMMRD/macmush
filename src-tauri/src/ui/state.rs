/// Application state management for Tauri
///
/// Manages the active MUD session and event bus with thread-safe access.

use crate::core::{EventBus, Session};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Global application state shared across Tauri commands
#[derive(Clone)]
pub struct AppState {
    /// Current active session (None when disconnected)
    /// Uses tokio::sync::Mutex to allow holding across await points
    pub session: Arc<Mutex<Option<Session>>>,

    /// Event bus for session events
    pub event_bus: Arc<EventBus>,

    /// Current log file path (None when not logging)
    pub log_file: Arc<Mutex<Option<String>>>,

    /// Log format (plain, html, raw)
    pub log_format: Arc<Mutex<String>>,
}

impl AppState {
    /// Create new application state
    pub fn new() -> Self {
        let event_bus = Arc::new(EventBus::new());

        Self {
            session: Arc::new(Mutex::new(None)),
            event_bus,
            log_file: Arc::new(Mutex::new(None)),
            log_format: Arc::new(Mutex::new("plain".to_string())),
        }
    }

    /// Check if a session is currently active
    pub async fn is_connected(&self) -> bool {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|s| s.is_connected())
            .unwrap_or(false)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::World;

    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new();
        assert!(!state.is_connected().await, "Should start disconnected");
    }

    #[tokio::test]
    async fn test_app_state_default() {
        let state = AppState::default();
        assert!(!state.is_connected().await, "Default should be disconnected");
    }

    #[tokio::test]
    async fn test_app_state_clone() {
        let state1 = AppState::new();
        let state2 = state1.clone();

        // Both should be disconnected
        assert!(!state1.is_connected().await);
        assert!(!state2.is_connected().await);
    }

    #[tokio::test]
    async fn test_app_state_session_management() {
        let state = AppState::new();

        // Start with no session
        assert!(!state.is_connected().await);

        // Create a session (not connected yet)
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let session = Session::new(world, state.event_bus.clone()).unwrap();

        // Store session
        *state.session.lock().await = Some(session);

        // Still not connected (session exists but not started)
        assert!(!state.is_connected().await);
    }
}

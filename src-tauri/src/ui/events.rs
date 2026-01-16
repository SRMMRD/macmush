/// Event streaming to frontend
///
/// Handles background data reception and forwarding events to the frontend.

use crate::automation::HighlightStyle;
use crate::core::MudEvent;
use crate::ui::state::AppState;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, timeout, Duration};
use tracing::{debug, error, info, warn};

/// Event payload sent to frontend
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrontendEvent {
    /// Text data received from MUD server
    DataReceived { text: String },

    /// Connection status changed
    ConnectionStatus { connected: bool, world_name: Option<String> },

    /// Error occurred
    Error { message: String },

    /// Highlight matched incoming text
    HighlightMatched { matches: Vec<(usize, usize, HighlightStyle)> },

    /// Trigger matched incoming text
    TriggerMatched { trigger_name: String, matched_text: String },

    /// Trigger executed commands
    TriggerExecuted { commands: Vec<String> },

    /// Trigger error occurred
    TriggerError { error: String },

    /// Alias matched user input
    AliasMatched { alias_name: String, matched_text: String },

    /// Alias executed commands
    AliasExecuted { commands: Vec<String> },

    /// Alias error occurred
    AliasError { error: String },

    /// Timer executed commands
    TimerExecuted { commands: Vec<String> },

    /// Timer error occurred
    TimerError { error: String },
}

/// Data receiver loop state
static RECEIVER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Start background data receiver loop
///
/// This spawns a tokio task that continuously receives data from the MUD
/// and forwards it to the frontend via Tauri events.
pub fn start_data_receiver(app_handle: AppHandle, state: AppState) {
    // Prevent multiple receivers
    if RECEIVER_RUNNING.swap(true, Ordering::SeqCst) {
        warn!("Data receiver already running");
        return;
    }

    info!("Starting data receiver loop");

    tokio::spawn(async move {
        loop {
            // Check if still connected
            if !state.is_connected().await {
                debug!("Not connected, stopping receiver");
                RECEIVER_RUNNING.store(false, Ordering::SeqCst);
                break;
            }

            // Try to receive data with timeout to allow send_command to acquire lock
            let receive_result = timeout(Duration::from_millis(100), async {
                let mut session_guard = state.session.lock().await;
                match session_guard.as_mut() {
                    Some(session) => session.process_incoming_data().await,
                    None => {
                        debug!("No session, stopping receiver");
                        RECEIVER_RUNNING.store(false, Ordering::SeqCst);
                        Err(crate::error::MushError::NotConnected)
                    }
                }
            })
            .await;

            // Handle result
            match receive_result {
                Ok(Ok(())) => {
                    // Data processed successfully, continue loop
                }
                Ok(Err(e)) => {
                    error!("Error receiving data: {}", e);

                    // Emit error to frontend
                    let _ = app_handle.emit(
                        "mud-event",
                        FrontendEvent::Error {
                            message: format!("Connection error: {}", e),
                        },
                    );

                    // Connection likely closed, stop receiver
                    RECEIVER_RUNNING.store(false, Ordering::SeqCst);
                    break;
                }
                Err(_) => {
                    // Timeout - no data available, loop again
                    // This releases the lock and gives send_command a chance to run
                    sleep(Duration::from_millis(10)).await;
                }
            }

            // Process timers periodically
            let timer_result = timeout(Duration::from_millis(50), async {
                let mut session_guard = state.session.lock().await;
                match session_guard.as_mut() {
                    Some(session) => session.process_timers().await,
                    None => Ok(()),
                }
            })
            .await;

            if let Ok(Err(e)) = timer_result {
                error!("Error processing timers: {}", e);
            }
        }

        info!("Data receiver loop stopped");
    });
}

/// Forward EventBus events to frontend
///
/// Subscribes to the EventBus and forwards relevant events to the frontend.
pub fn start_event_forwarder(app_handle: AppHandle, state: AppState) {
    tokio::spawn(async move {
        let mut rx = state.event_bus.subscribe();

        info!("Starting event forwarder");

        while let Ok(event) = rx.recv().await {
            let frontend_event = match event {
                MudEvent::DataReceived { text, .. } => Some(FrontendEvent::DataReceived { text }),

                MudEvent::Connected { .. } => {
                    // Update status
                    let world_name = state
                        .session
                        .lock()
                        .await
                        .as_ref()
                        .map(|s| s.world().name.clone());

                    Some(FrontendEvent::ConnectionStatus {
                        connected: true,
                        world_name,
                    })
                }

                MudEvent::Disconnected { reason, .. } => {
                    info!("Disconnected: {}", reason);
                    Some(FrontendEvent::ConnectionStatus {
                        connected: false,
                        world_name: None,
                    })
                }

                MudEvent::ConnectionError { error, .. } => Some(FrontendEvent::Error {
                    message: format!("Connection error: {}", error),
                }),

                MudEvent::HighlightMatched { matches, .. } => {
                    Some(FrontendEvent::HighlightMatched { matches })
                }

                MudEvent::TriggerMatched { trigger_name, matched_text, .. } => {
                    Some(FrontendEvent::TriggerMatched { trigger_name, matched_text })
                }

                MudEvent::TriggerExecuted { commands, .. } => {
                    Some(FrontendEvent::TriggerExecuted { commands })
                }

                MudEvent::TriggerError { error, .. } => {
                    Some(FrontendEvent::TriggerError { error })
                }

                MudEvent::AliasMatched { alias_name, matched_text, .. } => {
                    Some(FrontendEvent::AliasMatched { alias_name, matched_text })
                }

                MudEvent::AliasExecuted { commands, .. } => {
                    Some(FrontendEvent::AliasExecuted { commands })
                }

                MudEvent::AliasError { error, .. } => {
                    Some(FrontendEvent::AliasError { error })
                }

                MudEvent::TimerExecuted { commands, .. } => {
                    Some(FrontendEvent::TimerExecuted { commands })
                }

                MudEvent::TimerError { error, .. } => {
                    Some(FrontendEvent::TimerError { error })
                }

                // CommandSent is logged but not forwarded to frontend
                MudEvent::CommandSent { .. } => None,
            };

            // Emit to frontend
            if let Some(fe) = frontend_event {
                if let Err(e) = app_handle.emit("mud-event", fe) {
                    error!("Failed to emit event to frontend: {}", e);
                }
            }
        }

        warn!("Event forwarder stopped");
    });
}

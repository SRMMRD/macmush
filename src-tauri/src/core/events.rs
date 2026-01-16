/// Event system for MUD client communication
///
/// Provides type-safe event bus for communication between components:
/// - TCP layer generates DataReceived events
/// - Trigger system generates TriggerMatched events
/// - Session generates connection state events
/// - UI subscribes to all events for display

use crate::automation::HighlightStyle;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Maximum number of events buffered per subscriber
const EVENT_BUFFER_SIZE: usize = 100;

/// Events that occur during MUD session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MudEvent {
    /// Connection established
    Connected {
        world_id: Uuid,
        host: String,
        port: u16,
    },

    /// Connection closed
    Disconnected {
        world_id: Uuid,
        reason: String,
    },

    /// Data received from MUD server
    DataReceived {
        world_id: Uuid,
        data: Vec<u8>,
        text: String,
    },

    /// Command sent to MUD server
    CommandSent {
        world_id: Uuid,
        command: String,
    },

    /// Trigger matched incoming text
    TriggerMatched {
        world_id: Uuid,
        trigger_id: Uuid,
        trigger_name: String,
        matched_text: String,
    },

    /// Trigger executed action
    TriggerExecuted {
        world_id: Uuid,
        trigger_id: Uuid,
        commands: Vec<String>,
    },

    /// Connection error occurred
    ConnectionError {
        world_id: Uuid,
        error: String,
    },

    /// Trigger error occurred
    TriggerError {
        world_id: Uuid,
        trigger_id: Uuid,
        error: String,
    },

    /// Alias matched user input
    AliasMatched {
        world_id: Uuid,
        alias_id: Uuid,
        alias_name: String,
        matched_text: String,
    },

    /// Alias executed commands
    AliasExecuted {
        world_id: Uuid,
        alias_id: Uuid,
        commands: Vec<String>,
    },

    /// Alias error occurred
    AliasError {
        world_id: Uuid,
        alias_id: Uuid,
        error: String,
    },

    /// Timer executed commands
    TimerExecuted {
        world_id: Uuid,
        timer_id: Uuid,
        commands: Vec<String>,
    },

    /// Timer error occurred
    TimerError {
        world_id: Uuid,
        timer_id: Uuid,
        error: String,
    },

    /// Highlight matched incoming text
    HighlightMatched {
        world_id: Uuid,
        matches: Vec<(usize, usize, HighlightStyle)>,
    },
}

impl MudEvent {
    /// Get the world ID associated with this event
    pub fn world_id(&self) -> Uuid {
        match self {
            MudEvent::Connected { world_id, .. }
            | MudEvent::Disconnected { world_id, .. }
            | MudEvent::DataReceived { world_id, .. }
            | MudEvent::CommandSent { world_id, .. }
            | MudEvent::TriggerMatched { world_id, .. }
            | MudEvent::TriggerExecuted { world_id, .. }
            | MudEvent::ConnectionError { world_id, .. }
            | MudEvent::TriggerError { world_id, .. }
            | MudEvent::AliasMatched { world_id, .. }
            | MudEvent::AliasExecuted { world_id, .. }
            | MudEvent::AliasError { world_id, .. }
            | MudEvent::TimerExecuted { world_id, .. }
            | MudEvent::TimerError { world_id, .. }
            | MudEvent::HighlightMatched { world_id, .. } => *world_id,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            MudEvent::ConnectionError { .. }
            | MudEvent::TriggerError { .. }
            | MudEvent::AliasError { .. }
            | MudEvent::TimerError { .. }
        )
    }
}

/// Event bus for pub/sub communication
pub struct EventBus {
    sender: broadcast::Sender<MudEvent>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_BUFFER_SIZE);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: MudEvent) -> Result<()> {
        // broadcast::send returns error only if no receivers exist, which is ok
        let _ = self.sender.send(event);
        Ok(())
    }

    /// Subscribe to events
    ///
    /// Returns a receiver that will receive all future events.
    /// The receiver will buffer up to EVENT_BUFFER_SIZE events.
    pub fn subscribe(&self) -> broadcast::Receiver<MudEvent> {
        self.sender.subscribe()
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_event_bus() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0, "Should start with no subscribers");
    }

    #[test]
    fn test_subscribe_to_events() {
        let bus = EventBus::new();
        let _rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1, "Should have 1 subscriber");

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2, "Should have 2 subscribers");
    }

    #[tokio::test]
    async fn test_publish_event() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let world_id = Uuid::new_v4();
        let event = MudEvent::Connected {
            world_id,
            host: "mud.example.com".to_string(),
            port: 4000,
        };

        bus.publish(event.clone()).unwrap();

        // Receive the event
        let received = rx.recv().await.unwrap();
        assert_eq!(received, event, "Should receive published event");
    }

    #[tokio::test]
    async fn test_multiple_subscribers_receive_same_event() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        let world_id = Uuid::new_v4();
        let event = MudEvent::DataReceived {
            world_id,
            data: b"test data".to_vec(),
            text: "test data".to_string(),
        };

        bus.publish(event.clone()).unwrap();

        // All subscribers receive the same event
        assert_eq!(rx1.recv().await.unwrap(), event);
        assert_eq!(rx2.recv().await.unwrap(), event);
        assert_eq!(rx3.recv().await.unwrap(), event);
    }

    #[tokio::test]
    async fn test_event_world_id_extraction() {
        let world_id = Uuid::new_v4();

        let events = vec![
            MudEvent::Connected {
                world_id,
                host: "test.mud".to_string(),
                port: 4000,
            },
            MudEvent::Disconnected {
                world_id,
                reason: "User disconnect".to_string(),
            },
            MudEvent::DataReceived {
                world_id,
                data: vec![],
                text: String::new(),
            },
            MudEvent::CommandSent {
                world_id,
                command: "look".to_string(),
            },
        ];

        for event in events {
            assert_eq!(event.world_id(), world_id, "Should extract correct world_id");
        }
    }

    #[test]
    fn test_event_is_error() {
        let world_id = Uuid::new_v4();

        // Error events
        assert!(
            MudEvent::ConnectionError {
                world_id,
                error: "test".to_string()
            }
            .is_error()
        );

        assert!(
            MudEvent::TriggerError {
                world_id,
                trigger_id: Uuid::new_v4(),
                error: "test".to_string()
            }
            .is_error()
        );

        // Non-error events
        assert!(
            !MudEvent::Connected {
                world_id,
                host: "test".to_string(),
                port: 4000
            }
            .is_error()
        );

        assert!(
            !MudEvent::DataReceived {
                world_id,
                data: vec![],
                text: String::new()
            }
            .is_error()
        );
    }

    #[tokio::test]
    async fn test_publish_to_dropped_subscribers() {
        let bus = EventBus::new();
        let rx = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        // Drop subscriber
        drop(rx);

        // Publishing should still succeed even with no subscribers
        let world_id = Uuid::new_v4();
        let event = MudEvent::Connected {
            world_id,
            host: "test.mud".to_string(),
            port: 4000,
        };

        let result = bus.publish(event);
        assert!(result.is_ok(), "Publishing should succeed with no subscribers");
    }

    #[tokio::test]
    async fn test_late_subscriber_misses_old_events() {
        let bus = EventBus::new();

        let world_id = Uuid::new_v4();
        let event1 = MudEvent::Connected {
            world_id,
            host: "test.mud".to_string(),
            port: 4000,
        };

        // Publish before subscribing
        bus.publish(event1).unwrap();

        // Late subscriber
        let mut rx = bus.subscribe();

        let event2 = MudEvent::DataReceived {
            world_id,
            data: b"test".to_vec(),
            text: "test".to_string(),
        };

        bus.publish(event2.clone()).unwrap();

        // Should only receive event2, not event1
        let received = rx.recv().await.unwrap();
        assert_eq!(received, event2, "Late subscriber should only receive new events");
    }

    #[tokio::test]
    async fn test_json_serialization() {
        let world_id = Uuid::new_v4();
        let event = MudEvent::Connected {
            world_id,
            host: "mud.example.com".to_string(),
            port: 4000,
        };

        let json = serde_json::to_string(&event);
        assert!(json.is_ok(), "Should serialize to JSON");

        let deserialized: MudEvent = serde_json::from_str(&json.unwrap()).unwrap();
        assert_eq!(deserialized, event, "Should deserialize correctly");
    }

    #[test]
    fn test_default_event_bus() {
        let bus = EventBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }
}

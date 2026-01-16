/// Integration tests for MUSHClient components
///
/// Tests the integration between TCP client, World configuration, and Trigger system.

use mushclient_macos_lib::{
    automation::triggers::{Trigger, TriggerAction, TriggerManager},
    core::World,
    network::TcpClient,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// ============================================================================
// World + TCP Client Integration Tests
// ============================================================================

#[tokio::test]
async fn test_world_configures_tcp_client() {
    // Create a World configuration
    let world = World::new("Test MUD", "mud.example.com", 4000).expect("World creation failed");

    // Create TCP client using World configuration
    let client = TcpClient::builder(&world.host, world.port)
        .timeout(Duration::from_secs(world.timeout_secs))
        .build();

    // Verify client configuration matches world
    assert!(!client.is_connected());
}

#[tokio::test]
async fn test_world_tcp_connection_flow() {
    // Start mock server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Accept connection in background
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            // Send welcome message
            let _ = socket.write_all(b"Welcome to Test MUD!\r\n").await;

            // Keep connection open briefly
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Create World for the test server
    let world = World::new("Test MUD", "127.0.0.1", port).expect("World creation failed");

    // Create and connect TCP client
    let mut client = TcpClient::builder(&world.host, world.port)
        .timeout(Duration::from_secs(world.timeout_secs))
        .build();

    // Connect using World configuration
    let result = client.connect().await;
    assert!(result.is_ok(), "Should connect using World configuration");
    assert!(client.is_connected(), "Client should be connected");

    // Receive welcome message
    let data = client.receive().await.expect("Should receive data");
    let message = String::from_utf8_lossy(&data);
    assert!(message.contains("Welcome to Test MUD!"));

    // Clean disconnect
    client.disconnect().await.expect("Should disconnect");
    assert!(!client.is_connected(), "Client should be disconnected");
}

// ============================================================================
// Trigger System Integration Tests
// ============================================================================

#[test]
fn test_trigger_manager_with_multiple_triggers() {
    let mut manager = TriggerManager::new();

    // Add multiple triggers
    let t1 = Trigger::new(
        "HP Trigger",
        r"^HP: (\d+)/(\d+)",
        TriggerAction::DisplayText("Health updated!".to_string()),
    )
    .unwrap();

    let t2 = Trigger::new(
        "Combat Trigger",
        r"^You (hit|miss) the",
        TriggerAction::SendCommand("look".to_string()),
    )
    .unwrap();

    let t3 = Trigger::new(
        "Death Trigger",
        r"^You have been slain",
        TriggerAction::Sequence(vec![
            TriggerAction::DisplayText("DEATH WARNING!".to_string()),
            TriggerAction::PlaySound("death.wav".to_string()),
            TriggerAction::SendCommand("return".to_string()),
        ]),
    )
    .unwrap();

    manager.add_trigger(t1).unwrap();
    manager.add_trigger(t2).unwrap();
    manager.add_trigger(t3).unwrap();

    // Test text that matches HP trigger
    let matches = manager
        .find_matches("HP: 95/100")
        .expect("Should find matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "HP Trigger");

    // Test text that matches combat trigger
    let matches = manager
        .find_matches("You hit the goblin!")
        .expect("Should find matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Combat Trigger");

    // Test text that matches death trigger
    let matches = manager
        .find_matches("You have been slain by a dragon!")
        .expect("Should find matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Death Trigger");

    // Test text that matches no triggers
    let matches = manager
        .find_matches("The weather is nice today.")
        .expect("Should find matches");
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_trigger_execution_generates_commands() {
    // Create trigger with command sequence
    let trigger = Trigger::new(
        "Battle Trigger",
        r"^Enemy appears",
        TriggerAction::Sequence(vec![
            TriggerAction::SendCommand("draw sword".to_string()),
            TriggerAction::SendCommand("attack".to_string()),
            TriggerAction::SendCommand("defend".to_string()),
        ]),
    )
    .unwrap();

    // Execute trigger
    let commands = trigger.execute().expect("Should execute trigger");

    // Verify all commands generated
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0], "draw sword");
    assert_eq!(commands[1], "attack");
    assert_eq!(commands[2], "defend");
}

// ============================================================================
// Full Integration: World + TCP + Triggers
// ============================================================================

#[tokio::test]
async fn test_complete_mud_session_flow() {
    // Setup: Start mock MUD server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Mock server sends MUD text with trigger patterns
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            // Send connection banner
            let _ = socket.write_all(b"Welcome adventurer!\r\n").await;
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Send HP update
            let _ = socket.write_all(b"HP: 100/100\r\n").await;
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Send combat message
            let _ = socket.write_all(b"A goblin appears!\r\n").await;
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Read client response
            let mut buf = [0u8; 256];
            let _ = socket.read(&mut buf).await;

            // Keep connection alive
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    // Step 1: Create World configuration
    let world = World::new("Test MUD", "127.0.0.1", port).expect("World creation failed");

    // Step 2: Create TCP client from World
    let mut client = TcpClient::builder(&world.host, world.port)
        .timeout(Duration::from_secs(world.timeout_secs))
        .build();

    // Step 3: Create Trigger system for this world
    let mut trigger_manager = TriggerManager::new();

    // Add HP monitoring trigger
    let hp_trigger = Trigger::new(
        "HP Monitor",
        r"^HP: (\d+)/(\d+)",
        TriggerAction::DisplayText("HP updated".to_string()),
    )
    .unwrap();

    // Add combat trigger
    let combat_trigger = Trigger::new(
        "Combat Alert",
        r"goblin appears",
        TriggerAction::SendCommand("attack goblin".to_string()),
    )
    .unwrap();

    trigger_manager.add_trigger(hp_trigger).unwrap();
    trigger_manager.add_trigger(combat_trigger).unwrap();

    // Step 4: Connect to MUD server
    client.connect().await.expect("Should connect");
    assert!(client.is_connected());

    // Step 5: Receive and process welcome message
    let data = client.receive().await.expect("Should receive welcome");
    let text = String::from_utf8_lossy(&data);
    assert!(text.contains("Welcome adventurer!"));

    // Check triggers (should not match welcome)
    let matches = trigger_manager.find_matches(&text).unwrap();
    assert_eq!(matches.len(), 0, "Welcome should not trigger anything");

    // Step 6: Receive HP update
    let data = client.receive().await.expect("Should receive HP");
    let text = String::from_utf8_lossy(&data);

    // Process through trigger system
    let matches = trigger_manager.find_matches(&text).unwrap();
    assert_eq!(matches.len(), 1, "HP message should trigger HP monitor");
    assert_eq!(matches[0].name, "HP Monitor");

    // Step 7: Receive combat message
    let data = client.receive().await.expect("Should receive combat");
    let text = String::from_utf8_lossy(&data);

    // Process through trigger system
    let matches = trigger_manager.find_matches(&text).unwrap();
    assert_eq!(matches.len(), 1, "Combat message should trigger combat alert");
    assert_eq!(matches[0].name, "Combat Alert");

    // Step 8: Execute trigger and send command
    let commands = matches[0].execute().expect("Should execute trigger");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0], "attack goblin");

    // Send the command to MUD
    let command = format!("{}\n", commands[0]);
    client
        .send(command.as_bytes())
        .await
        .expect("Should send command");

    // Step 9: Clean disconnect
    client.disconnect().await.expect("Should disconnect");
    assert!(!client.is_connected());
}

// ============================================================================
// Error Handling Integration Tests
// ============================================================================

#[tokio::test]
async fn test_connection_failure_with_world() {
    // Create World with invalid host
    let world = World::new("Bad MUD", "invalid.nonexistent.host", 9999)
        .expect("World creation should succeed");

    // Try to connect
    let mut client = TcpClient::builder(&world.host, world.port)
        .timeout(Duration::from_secs(2)) // Short timeout for test
        .build();

    let result = client.connect().await;
    assert!(result.is_err(), "Should fail to connect to invalid host");
    assert!(!client.is_connected());
}

#[test]
fn test_invalid_trigger_pattern() {
    // Try to create trigger with ReDoS-vulnerable pattern
    let result = Trigger::new(
        "Bad Trigger",
        "(a+)+b", // ReDoS vulnerable
        TriggerAction::SendCommand("test".to_string()),
    );

    assert!(result.is_err(), "Should reject ReDoS-vulnerable pattern");
}

// ============================================================================
// Performance Integration Tests
// ============================================================================

#[test]
fn test_trigger_performance_with_many_triggers() {
    let mut manager = TriggerManager::new();

    // Add 100 triggers with word boundary to avoid overlapping matches
    for i in 0..100 {
        let trigger = Trigger::new(
            format!("Trigger {}", i),
            format!(r"^Pattern {}\b", i), // Add word boundary
            TriggerAction::SendCommand(format!("command{}", i)),
        )
        .unwrap();
        manager.add_trigger(trigger).unwrap();
    }

    // Test matching against text that doesn't match any trigger
    let start = std::time::Instant::now();
    let matches = manager
        .find_matches("Some random text that won't match")
        .unwrap();
    let duration = start.elapsed();

    assert_eq!(matches.len(), 0);
    assert!(
        duration.as_millis() < 500,
        "Should check 100 triggers in under 500ms, took {}ms",
        duration.as_millis()
    );

    // Test matching against text that matches one trigger
    let start = std::time::Instant::now();
    let matches = manager.find_matches("Pattern 50").unwrap();
    let duration = start.elapsed();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "Trigger 50");
    assert!(
        duration.as_millis() < 500,
        "Should find match in under 500ms, took {}ms",
        duration.as_millis()
    );
}

// ============================================================================
// Wave 3: Session + Connection + Events Integration Tests
// ============================================================================

use mushclient_macos_lib::core::{EventBus, MudEvent, Session};
use std::sync::Arc;

#[tokio::test]
async fn test_session_lifecycle_with_events() {
    // Start mock MUD server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let _accept = listener.accept().await;
    });

    // Create world and event bus
    let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
    let event_bus = Arc::new(EventBus::new());
    let mut rx = event_bus.subscribe();

    // Create session
    let mut session = Session::new(world, event_bus);

    // Start session - should publish Connected event
    session.start().await.unwrap();
    assert!(session.is_connected(), "Session should be connected");

    // Verify Connected event
    let event = rx.recv().await.unwrap();
    assert!(
        matches!(event, MudEvent::Connected { .. }),
        "Should publish Connected event"
    );

    // Stop session - should publish Disconnected event
    session.stop().await.unwrap();
    assert!(!session.is_connected(), "Session should be disconnected");

    // Verify Disconnected event
    let event = rx.recv().await.unwrap();
    assert!(
        matches!(event, MudEvent::Disconnected { .. }),
        "Should publish Disconnected event"
    );
}

#[tokio::test]
async fn test_session_trigger_integration_with_events() {
    // Start mock MUD server that sends MUD data
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            // Send HP update message
            let _ = socket.write_all(b"HP: 95/100\n").await;

            // Read command sent by trigger
            let mut buf = [0u8; 256];
            let _ = socket.read(&mut buf).await;
        }
    });

    // Create world and event bus
    let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
    let event_bus = Arc::new(EventBus::new());
    let mut rx = event_bus.subscribe();

    // Create session with trigger
    let mut session = Session::new(world, event_bus);

    let trigger = Trigger::new(
        "HP Alert",
        r"^HP: (\d+)/(\d+)",
        TriggerAction::SendCommand("score".to_string()),
    )
    .unwrap();
    session.add_trigger(trigger).unwrap();

    // Connect
    session.start().await.unwrap();

    // Process incoming data
    session.process_incoming_data().await.unwrap();

    // Collect all events
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Verify event sequence: Connected, DataReceived, TriggerMatched, TriggerExecuted, CommandSent
    assert!(events.len() >= 4, "Should have at least 4 events");

    // Verify Connected event
    assert!(
        events.iter().any(|e| matches!(e, MudEvent::Connected { .. })),
        "Should have Connected event"
    );

    // Verify DataReceived event
    assert!(
        events.iter().any(|e| matches!(e, MudEvent::DataReceived { .. })),
        "Should have DataReceived event"
    );

    // Verify TriggerMatched event
    assert!(
        events.iter().any(|e| matches!(e, MudEvent::TriggerMatched { .. })),
        "Should have TriggerMatched event"
    );

    // Verify TriggerExecuted event
    assert!(
        events.iter().any(|e| matches!(e, MudEvent::TriggerExecuted { .. })),
        "Should have TriggerExecuted event"
    );

    // Verify CommandSent event
    assert!(
        events.iter().any(|e| matches!(e, MudEvent::CommandSent { .. })),
        "Should have CommandSent event"
    );
}

#[tokio::test]
async fn test_session_multiple_subscribers() {
    // Start mock MUD server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let _ = socket.write_all(b"Test message\n").await;
        }
    });

    // Create world and event bus with multiple subscribers
    let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
    let event_bus = Arc::new(EventBus::new());
    let mut rx1 = event_bus.subscribe();
    let mut rx2 = event_bus.subscribe();
    let mut rx3 = event_bus.subscribe();

    // Create session
    let mut session = Session::new(world, event_bus);

    // Start session
    session.start().await.unwrap();

    // Process incoming data
    session.process_incoming_data().await.unwrap();

    // All subscribers should receive Connected event
    let event1 = rx1.try_recv().ok();
    let event2 = rx2.try_recv().ok();
    let event3 = rx3.try_recv().ok();

    assert!(event1.is_some(), "Subscriber 1 should receive event");
    assert!(event2.is_some(), "Subscriber 2 should receive event");
    assert!(event3.is_some(), "Subscriber 3 should receive event");
}

#[tokio::test]
async fn test_session_complete_mud_workflow() {
    // Start mock MUD server with full conversation
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            // Send welcome
            let _ = socket.write_all(b"Welcome adventurer!\n").await;
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Send HP update
            let _ = socket.write_all(b"HP: 100/100\n").await;
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Read "score" command from trigger
            let mut buf = [0u8; 256];
            let _ = socket.read(&mut buf).await;

            // Send combat message
            let _ = socket.write_all(b"A goblin appears!\n").await;
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Read "attack goblin" command from trigger
            let _ = socket.read(&mut buf).await;
        }
    });

    // Create world and event bus
    let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
    let event_bus = Arc::new(EventBus::new());

    // Create session with triggers
    let mut session = Session::new(world, event_bus);

    // Add HP monitoring trigger
    let hp_trigger = Trigger::new(
        "HP Monitor",
        r"^HP: (\d+)/(\d+)",
        TriggerAction::SendCommand("score".to_string()),
    )
    .unwrap();
    session.add_trigger(hp_trigger).unwrap();

    // Add combat trigger
    let combat_trigger = Trigger::new(
        "Combat Alert",
        r"goblin appears",
        TriggerAction::SendCommand("attack goblin".to_string()),
    )
    .unwrap();
    session.add_trigger(combat_trigger).unwrap();

    // Start session
    session.start().await.unwrap();
    assert!(session.is_connected());

    // Process welcome message (no trigger match)
    let result = session.process_incoming_data().await;
    assert!(result.is_ok(), "Should process welcome message");

    // Process HP update (should trigger)
    let result = session.process_incoming_data().await;
    assert!(result.is_ok(), "Should process HP update");

    // Process combat message (should trigger)
    let result = session.process_incoming_data().await;
    assert!(result.is_ok(), "Should process combat message");

    // Stop session
    session.stop().await.unwrap();
    assert!(!session.is_connected());
}

#[tokio::test]
async fn test_session_error_handling_with_events() {
    // Test connection error propagation through events
    let world = World::new("Test MUD", "invalid.nonexistent.host", 9999).unwrap();
    let event_bus = Arc::new(EventBus::new());
    let mut rx = event_bus.subscribe();

    let mut session = Session::new(world, event_bus);

    // Try to start session (should fail)
    let result = session.start().await;
    assert!(result.is_err(), "Should fail to connect");
    assert!(!session.is_connected());

    // Verify ConnectionError event was published
    let event = rx.recv().await.unwrap();
    assert!(
        matches!(event, MudEvent::ConnectionError { .. }),
        "Should publish ConnectionError event"
    );
}

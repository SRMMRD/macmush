/// Connection management with event generation
///
/// Wraps TcpClient and publishes events to EventBus for all connection activities.

use crate::core::{EventBus, MudEvent, World};
use crate::error::{MushError, Result};
use crate::network::{TcpClient, MccpHandler};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Connection state management with event publishing
pub struct Connection {
    world: World,
    client: TcpClient,
    event_bus: Arc<EventBus>,
    mccp: MccpHandler,
    telnet_buffer: Vec<u8>,
}

impl Connection {
    /// Create a new connection for a world
    pub fn new(world: World, event_bus: Arc<EventBus>) -> Self {
        debug!("Creating connection for world '{}'", world.name);

        let client = TcpClient::builder(&world.host, world.port)
            .timeout(Duration::from_secs(world.timeout_secs))
            .build();

        Self {
            world,
            client,
            event_bus,
            mccp: MccpHandler::new(),
            telnet_buffer: Vec::new(),
        }
    }

    /// Connect to the MUD server
    ///
    /// Publishes Connected event on success, ConnectionError event on failure.
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to {} ({}:{})", self.world.name, self.world.host, self.world.port);

        match self.client.connect().await {
            Ok(()) => {
                info!("Successfully connected to {}", self.world.name);

                // Publish Connected event
                self.event_bus
                    .publish(MudEvent::Connected {
                        world_id: self.world.id,
                        host: self.world.host.clone(),
                        port: self.world.port,
                    })?;

                Ok(())
            }
            Err(e) => {
                error!("Connection failed for {}: {}", self.world.name, e);

                // Publish ConnectionError event
                self.event_bus
                    .publish(MudEvent::ConnectionError {
                        world_id: self.world.id,
                        error: e.to_string(),
                    })?;

                Err(e)
            }
        }
    }

    /// Disconnect from the MUD server
    ///
    /// Publishes Disconnected event.
    pub async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from {}", self.world.name);

        self.client.disconnect().await?;

        // Publish Disconnected event
        self.event_bus
            .publish(MudEvent::Disconnected {
                world_id: self.world.id,
                reason: "User disconnect".to_string(),
            })?;

        debug!("Disconnected from {}", self.world.name);
        Ok(())
    }

    /// Send command to MUD server
    ///
    /// Publishes CommandSent event on success.
    /// Handles MCCP3 compression if active.
    pub async fn send_command(&mut self, command: impl AsRef<str>) -> Result<()> {
        let command = command.as_ref();
        debug!("Sending command: {}", command);

        let mut data = format!("{}\n", command).into_bytes();

        // Compress if MCCP3 is active
        if self.mccp.is_sending_compressed() {
            data = self.mccp.compress(&data)?;
            debug!("Compressed command to {} bytes", data.len());
        }

        self.client.send(&data).await?;

        // Publish CommandSent event
        self.event_bus
            .publish(MudEvent::CommandSent {
                world_id: self.world.id,
                command: command.to_string(),
            })?;

        Ok(())
    }

    /// Receive data from MUD server
    ///
    /// Returns raw bytes. Session layer publishes DataReceived event.
    /// Handles MCCP decompression and telnet negotiation.
    pub async fn receive(&mut self) -> Result<Vec<u8>> {
        match self.client.receive().await {
            Ok(mut data) => {
                debug!("Received {} bytes", data.len());

                // Decompress if MCCP2 is active
                if self.mccp.is_receiving_compressed() {
                    match self.mccp.decompress(&data) {
                        Ok(decompressed) => {
                            if !decompressed.is_empty() {
                                debug!("Decompressed {} bytes to {} bytes", data.len(), decompressed.len());
                                data = decompressed;
                            }
                        }
                        Err(e) => {
                            warn!("MCCP decompression error: {}", e);
                            // Disable compression on error
                            let response = self.mccp.disable_mccp2()?;
                            self.client.send(&response).await?;
                        }
                    }
                }

                // Process telnet IAC sequences
                self.process_telnet(&mut data).await?;

                Ok(data)
            }
            Err(e) => {
                match &e {
                    MushError::ConnectionClosed => {
                        info!("Connection closed by remote host");
                        // Publish Disconnected event
                        self.event_bus
                            .publish(MudEvent::Disconnected {
                                world_id: self.world.id,
                                reason: "Connection closed by remote host".to_string(),
                            })?;
                    }
                    _ => {
                        warn!("Receive error: {}", e);
                        // Publish ConnectionError event
                        self.event_bus
                            .publish(MudEvent::ConnectionError {
                                world_id: self.world.id,
                                error: e.to_string(),
                            })?;
                    }
                }

                Err(e)
            }
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    /// Get reference to the world
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Process telnet IAC sequences and MCCP negotiation
    async fn process_telnet(&mut self, data: &mut Vec<u8>) -> Result<()> {
        const IAC: u8 = 255;   // Interpret As Command
        const WILL: u8 = 251;
        const DO: u8 = 253;
        const DONT: u8 = 254;
        const SB: u8 = 250;    // Subnegotiation Begin
        const SE: u8 = 240;    // Subnegotiation End

        let mut i = 0;
        while i < data.len() {
            if data[i] == IAC && i + 2 < data.len() {
                let command = data[i + 1];
                let option = data[i + 2];

                // Handle MCCP2 negotiation (option 86)
                if command == WILL && option == 86 {
                    info!("Server offered MCCP2 compression");
                    let response = self.mccp.handle_mccp2_will()?;
                    self.client.send(&response).await?;

                    // Remove IAC WILL COMPRESS2 from data
                    data.drain(i..i+3);
                    continue;
                }

                // Handle MCCP2 subnegotiation
                if command == SB && option == 86 {
                    // Look for IAC SE
                    if let Some(se_pos) = data[i+3..].windows(2).position(|w| w == [IAC, SE]) {
                        let se_idx = i + 3 + se_pos;

                        // Start compression
                        self.mccp.handle_mccp2_subnegotiation()?;
                        info!("MCCP2 compression active");

                        // Remove IAC SB COMPRESS2 IAC SE from data
                        data.drain(i..se_idx+2);
                        continue;
                    }
                }

                // Handle MCCP3 negotiation (option 87)
                if command == WILL && option == 87 {
                    info!("Server offered MCCP3 compression");
                    let response = self.mccp.handle_mccp3_will()?;
                    self.client.send(&response).await?;
                    self.mccp.start_mccp3()?;

                    // Remove IAC WILL COMPRESS3 from data
                    data.drain(i..i+3);
                    continue;
                }
            }

            i += 1;
        }

        Ok(())
    }

    /// Get MCCP compression statistics
    pub fn get_compression_stats(&self) -> crate::network::CompressionStats {
        self.mccp.get_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Helper: Start a mock MUD server
    async fn start_mock_server() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn test_create_connection() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let conn = Connection::new(world.clone(), event_bus.clone());

        assert_eq!(conn.world().name, "Test MUD");
        assert_eq!(conn.world().host, "mud.example.com");
        assert_eq!(conn.world().port, 4000);
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_connect_publishes_event() {
        let (listener, port) = start_mock_server().await;

        // Accept connection in background
        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut conn = Connection::new(world, event_bus);

        // Connect
        let result = conn.connect().await;
        assert!(result.is_ok(), "Should connect successfully");

        // Verify Connected event was published
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, MudEvent::Connected { .. }),
            "Should publish Connected event"
        );

        if let MudEvent::Connected { host, port, .. } = event {
            assert_eq!(host, "127.0.0.1");
            assert_eq!(port, port);
        }
    }

    #[tokio::test]
    async fn test_disconnect_publishes_event() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut conn = Connection::new(world, event_bus.clone());
        conn.connect().await.unwrap();

        // Subscribe after connection to only see disconnect event
        let mut rx = event_bus.subscribe();

        // Disconnect
        let result = conn.disconnect().await;
        assert!(result.is_ok(), "Should disconnect successfully");

        // Verify Disconnected event was published
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, MudEvent::Disconnected { .. }),
            "Should publish Disconnected event"
        );
    }

    #[tokio::test]
    async fn test_send_command_publishes_event() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut conn = Connection::new(world, event_bus.clone());
        conn.connect().await.unwrap();

        // Subscribe after connection
        let mut rx = event_bus.subscribe();

        // Send command
        let result = conn.send_command("look").await;
        assert!(result.is_ok(), "Should send command successfully");

        // Verify CommandSent event was published
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, MudEvent::CommandSent { .. }),
            "Should publish CommandSent event"
        );

        if let MudEvent::CommandSent { command, .. } = event {
            assert_eq!(command, "look");
        }
    }

    #[tokio::test]
    async fn test_receive_data_publishes_event() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"Welcome to the MUD!\n").await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut conn = Connection::new(world, event_bus.clone());
        conn.connect().await.unwrap();

        // Subscribe after connection
        let mut rx = event_bus.subscribe();

        // Receive data
        let result = conn.receive().await;
        assert!(result.is_ok(), "Should receive data successfully");

        // Verify DataReceived event was published
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, MudEvent::DataReceived { .. }),
            "Should publish DataReceived event"
        );

        if let MudEvent::DataReceived { data, text, .. } = event {
            assert_eq!(data, b"Welcome to the MUD!\n");
            assert_eq!(text, "Welcome to the MUD!\n");
        }
    }

    #[tokio::test]
    async fn test_connection_error_publishes_event() {
        let world = World::new("Test MUD", "invalid.nonexistent.host", 9999).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut conn = Connection::new(world, event_bus);

        // Try to connect (should fail)
        let result = conn.connect().await;
        assert!(result.is_err(), "Should fail to connect");

        // Verify ConnectionError event was published
        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, MudEvent::ConnectionError { .. }),
            "Should publish ConnectionError event"
        );
    }

    #[tokio::test]
    async fn test_world_reference() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let conn = Connection::new(world.clone(), event_bus);

        assert_eq!(conn.world().id, world.id);
        assert_eq!(conn.world().name, world.name);
    }
}

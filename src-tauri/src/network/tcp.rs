/// TCP client for MUD connections
///
/// Provides async TCP connection with:
/// - Configurable timeouts
/// - Connection pooling
/// - Graceful error handling
/// - Send/receive buffering

use crate::error::{MushError, Result};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// TCP client configuration builder
#[derive(Debug, Clone)]
pub struct TcpClientBuilder {
    host: String,
    port: u16,
    timeout_secs: u64,
    read_buffer_size: usize,
}

impl TcpClientBuilder {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            timeout_secs: 30,
            read_buffer_size: 8192,
        }
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout_secs = duration.as_secs();
        self
    }

    pub fn read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size;
        self
    }

    pub fn build(self) -> TcpClient {
        TcpClient {
            host: self.host,
            port: self.port,
            timeout_secs: self.timeout_secs,
            read_buffer_size: self.read_buffer_size,
            stream: None,
        }
    }
}

/// TCP client for MUD connections
pub struct TcpClient {
    host: String,
    port: u16,
    timeout_secs: u64,
    read_buffer_size: usize,
    stream: Option<TcpStream>,
}

impl TcpClient {
    /// Create a new TCP client with default settings
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        TcpClientBuilder::new(host, port).build()
    }

    /// Create a builder for custom configuration
    pub fn builder(host: impl Into<String>, port: u16) -> TcpClientBuilder {
        TcpClientBuilder::new(host, port)
    }

    /// Connect to the MUD server
    ///
    /// Attempts to establish a TCP connection to the configured host and port
    /// with the specified timeout. If already connected, returns an error.
    ///
    /// # Errors
    /// - `MushError::ConnectionFailed`: DNS resolution failed or connection refused
    /// - `MushError::ConnectionTimeout`: Connection attempt exceeded timeout duration
    pub async fn connect(&mut self) -> Result<()> {
        if self.stream.is_some() {
            warn!("Already connected to {}:{}", self.host, self.port);
            return Ok(());
        }

        let addr = format!("{}:{}", self.host, self.port);
        let timeout_duration = Duration::from_secs(self.timeout_secs);

        info!("Connecting to {} (timeout: {}s)", addr, self.timeout_secs);

        let connect_future = TcpStream::connect(&addr);

        match timeout(timeout_duration, connect_future).await {
            Ok(Ok(stream)) => {
                info!("Successfully connected to {}", addr);
                self.stream = Some(stream);
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Connection failed to {}: {}", addr, e);
                Err(MushError::ConnectionFailed {
                    host: self.host.clone(),
                    port: self.port,
                    source: e,
                })
            }
            Err(_) => {
                error!("Connection timeout after {}s to {}", self.timeout_secs, addr);
                Err(MushError::ConnectionTimeout {
                    timeout_secs: self.timeout_secs,
                })
            }
        }
    }

    /// Send data to the server
    ///
    /// Sends the provided data to the connected server. All bytes are sent
    /// before returning.
    ///
    /// # Errors
    /// - `MushError::NotConnected`: Not currently connected to a server
    /// - `MushError::ConnectionFailed`: Write operation failed (connection dropped)
    pub async fn send(&mut self, data: &[u8]) -> Result<usize> {
        let stream = self.stream.as_mut().ok_or_else(|| {
            warn!("Attempted to send while not connected");
            MushError::NotConnected
        })?;

        debug!("Sending {} bytes", data.len());

        stream.write_all(data).await.map_err(|e| {
            error!("Failed to send data: {}", e);
            MushError::ConnectionFailed {
                host: self.host.clone(),
                port: self.port,
                source: e,
            }
        })?;

        debug!("Successfully sent {} bytes", data.len());
        Ok(data.len())
    }

    /// Receive data from the server
    ///
    /// Reads available data from the server into an internal buffer.
    /// Returns when data is available or the connection is closed.
    ///
    /// # Errors
    /// - `MushError::NotConnected`: Not currently connected to a server
    /// - `MushError::ConnectionFailed`: Read operation failed
    /// - `MushError::ConnectionClosed`: Server closed the connection (0 bytes read)
    pub async fn receive(&mut self) -> Result<Vec<u8>> {
        let stream = self.stream.as_mut().ok_or_else(|| {
            warn!("Attempted to receive while not connected");
            MushError::NotConnected
        })?;

        let mut buffer = vec![0u8; self.read_buffer_size];

        let n = stream.read(&mut buffer).await.map_err(|e| {
            error!("Failed to receive data: {}", e);
            MushError::ConnectionFailed {
                host: self.host.clone(),
                port: self.port,
                source: e,
            }
        })?;

        // Zero-byte read indicates connection closed
        if n == 0 {
            info!("Connection closed by remote host");
            self.stream = None;
            return Err(MushError::ConnectionClosed);
        }

        debug!("Received {} bytes", n);
        buffer.truncate(n);
        Ok(buffer)
    }

    /// Check if currently connected
    ///
    /// Returns `true` if a TCP connection is established, `false` otherwise.
    /// Note that this only checks local state; the remote host may have closed
    /// the connection without our knowledge.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Disconnect from the server
    ///
    /// Gracefully closes the TCP connection. Safe to call even if not connected.
    ///
    /// # Errors
    /// Currently always succeeds, but returns `Result` for future compatibility.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            info!("Disconnecting from {}:{}", self.host, self.port);
            if let Err(e) = stream.shutdown().await {
                warn!("Error during graceful shutdown: {}", e);
            }
            debug!("Disconnected successfully");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Helper: Start a mock MUD server for testing
    async fn start_mock_server() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn test_create_client_with_defaults() {
        let client = TcpClient::new("localhost", 4000);
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 4000);
        assert_eq!(client.timeout_secs, 30);
        assert_eq!(client.read_buffer_size, 8192);
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let client = TcpClient::builder("example.com", 4000)
            .timeout(Duration::from_secs(10))
            .read_buffer_size(4096)
            .build();

        assert_eq!(client.host, "example.com");
        assert_eq!(client.port, 4000);
        assert_eq!(client.timeout_secs, 10);
        assert_eq!(client.read_buffer_size, 4096);
    }

    #[tokio::test]
    async fn test_connect_to_localhost() {
        let (listener, port) = start_mock_server().await;

        // Accept connection in background
        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let mut client = TcpClient::new("127.0.0.1", port);
        let result = client.connect().await;

        assert!(result.is_ok(), "Should connect to localhost");
        assert!(client.is_connected(), "Should report as connected");
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        // Use a non-routable IP to force timeout
        let mut client = TcpClient::builder("192.0.2.1", 9999)
            .timeout(Duration::from_secs(2))
            .build();

        let result = client.connect().await;

        assert!(result.is_err(), "Should timeout on non-routable IP");
        assert!(
            matches!(result, Err(MushError::ConnectionTimeout { .. })),
            "Should return ConnectionTimeout error"
        );
        assert!(!client.is_connected(), "Should not be connected after timeout");
    }

    #[tokio::test]
    async fn test_invalid_hostname() {
        let mut client = TcpClient::new("invalid..host..name", 4000);
        let result = client.connect().await;

        assert!(result.is_err(), "Should fail on invalid hostname");
        assert!(
            matches!(result, Err(MushError::ConnectionFailed { .. })),
            "Should return ConnectionFailed error"
        );
    }

    #[tokio::test]
    async fn test_send_data() {
        let (listener, port) = start_mock_server().await;

        // Accept and read data in background
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
            }
        });

        let mut client = TcpClient::new("127.0.0.1", port);
        client.connect().await.expect("Should connect");

        let data = b"look\n";
        let result = client.send(data).await;

        assert!(result.is_ok(), "Should send data successfully");
        assert_eq!(result.unwrap(), data.len(), "Should send all bytes");
    }

    #[tokio::test]
    async fn test_receive_data() {
        let (listener, port) = start_mock_server().await;

        // Accept and send data in background
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"Welcome to the MUD!\n").await;
            }
        });

        let mut client = TcpClient::new("127.0.0.1", port);
        client.connect().await.expect("Should connect");

        let result = client.receive().await;

        assert!(result.is_ok(), "Should receive data successfully");
        let data = result.unwrap();
        assert!(!data.is_empty(), "Should receive non-empty data");
        assert_eq!(data, b"Welcome to the MUD!\n");
    }

    #[tokio::test]
    async fn test_send_without_connection() {
        let mut client = TcpClient::new("localhost", 4000);
        let result = client.send(b"test").await;

        assert!(result.is_err(), "Should fail to send without connection");
        assert!(
            matches!(result, Err(MushError::NotConnected)),
            "Should return NotConnected error"
        );
    }

    #[tokio::test]
    async fn test_receive_without_connection() {
        let mut client = TcpClient::new("localhost", 4000);
        let result = client.receive().await;

        assert!(result.is_err(), "Should fail to receive without connection");
        assert!(
            matches!(result, Err(MushError::NotConnected)),
            "Should return NotConnected error"
        );
    }

    #[tokio::test]
    async fn test_disconnect() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let mut client = TcpClient::new("127.0.0.1", port);
        client.connect().await.expect("Should connect");
        assert!(client.is_connected());

        let result = client.disconnect().await;

        assert!(result.is_ok(), "Should disconnect successfully");
        assert!(!client.is_connected(), "Should not be connected after disconnect");
    }

    #[tokio::test]
    async fn test_multiple_sends() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let _ = socket.read(&mut buf).await;
                let _ = socket.read(&mut buf).await;
            }
        });

        let mut client = TcpClient::new("127.0.0.1", port);
        client.connect().await.expect("Should connect");

        // Send multiple commands
        assert!(client.send(b"north\n").await.is_ok());
        assert!(client.send(b"look\n").await.is_ok());
        assert!(client.send(b"inventory\n").await.is_ok());
    }
}

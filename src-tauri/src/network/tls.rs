/// TLS client for secure MUD connections
///
/// Provides async TLS connection with:
/// - System certificate store integration
/// - Rustls for modern TLS support
/// - Fallback to plain TCP
/// - Certificate validation

use crate::error::{MushError, Result};
use rustls::pki_types::ServerName;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::{TlsConnector, client::TlsStream};
use tracing::{debug, error, info, warn};

/// Stream type enum to handle both plain and TLS connections
pub enum MudStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl MudStream {
    /// Read data from the stream
    pub async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MudStream::Plain(stream) => stream.read(buf).await,
            MudStream::Tls(stream) => stream.read(buf).await,
        }
    }

    /// Write data to the stream
    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            MudStream::Plain(stream) => stream.write_all(buf).await,
            MudStream::Tls(stream) => stream.write_all(buf).await,
        }
    }

    /// Shutdown the stream
    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        match self {
            MudStream::Plain(stream) => stream.shutdown().await,
            MudStream::Tls(stream) => stream.shutdown().await,
        }
    }
}

/// TLS client configuration builder
#[derive(Debug, Clone)]
pub struct TlsClientBuilder {
    host: String,
    port: u16,
    use_tls: bool,
    timeout_secs: u64,
    read_buffer_size: usize,
    verify_certificates: bool,
}

impl TlsClientBuilder {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            use_tls: false,
            timeout_secs: 30,
            read_buffer_size: 8192,
            verify_certificates: true,
        }
    }

    pub fn use_tls(mut self, enabled: bool) -> Self {
        self.use_tls = enabled;
        self
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout_secs = duration.as_secs();
        self
    }

    pub fn read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size;
        self
    }

    pub fn verify_certificates(mut self, enabled: bool) -> Self {
        self.verify_certificates = enabled;
        self
    }

    pub fn build(self) -> TlsClient {
        TlsClient {
            host: self.host,
            port: self.port,
            use_tls: self.use_tls,
            timeout_secs: self.timeout_secs,
            read_buffer_size: self.read_buffer_size,
            verify_certificates: self.verify_certificates,
            stream: None,
        }
    }
}

/// TLS/TCP client for MUD connections
pub struct TlsClient {
    host: String,
    port: u16,
    use_tls: bool,
    timeout_secs: u64,
    read_buffer_size: usize,
    verify_certificates: bool,
    stream: Option<MudStream>,
}

impl TlsClient {
    /// Create a new client with default settings (plain TCP)
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        TlsClientBuilder::new(host, port).build()
    }

    /// Create a builder for custom configuration
    pub fn builder(host: impl Into<String>, port: u16) -> TlsClientBuilder {
        TlsClientBuilder::new(host, port)
    }

    /// Create TLS connector with system certificates
    fn create_tls_connector(&self) -> Result<TlsConnector> {
        let mut root_store = rustls::RootCertStore::empty();

        // Load system certificates
        let cert_result = rustls_native_certs::load_native_certs();

        // Add all certificates to the root store
        for cert in cert_result.certs {
            root_store.add(cert).map_err(|e| MushError::TlsError(
                format!("Failed to add certificate: {:?}", e)
            ))?;
        }

        // Log any errors encountered while loading certs (but don't fail)
        for err in cert_result.errors {
            warn!("Certificate loading error: {}", err);
        }

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    }

    /// Connect to the MUD server (plain or TLS)
    ///
    /// # Errors
    /// - `MushError::ConnectionFailed`: DNS resolution failed or connection refused
    /// - `MushError::ConnectionTimeout`: Connection attempt exceeded timeout duration
    /// - `MushError::TlsError`: TLS handshake failed
    pub async fn connect(&mut self) -> Result<()> {
        if self.stream.is_some() {
            warn!("Already connected to {}:{}", self.host, self.port);
            return Ok(());
        }

        let addr = format!("{}:{}", self.host, self.port);
        let timeout_duration = Duration::from_secs(self.timeout_secs);

        info!(
            "Connecting to {} (timeout: {}s, TLS: {})",
            addr, self.timeout_secs, self.use_tls
        );

        // Establish TCP connection
        let tcp_stream = match timeout(timeout_duration, TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => {
                debug!("TCP connection established to {}", addr);
                stream
            }
            Ok(Err(e)) => {
                error!("Connection failed to {}: {}", addr, e);
                return Err(MushError::ConnectionFailed {
                    host: self.host.clone(),
                    port: self.port,
                    source: e,
                });
            }
            Err(_) => {
                error!("Connection timeout after {}s to {}", self.timeout_secs, addr);
                return Err(MushError::ConnectionTimeout {
                    timeout_secs: self.timeout_secs,
                });
            }
        };

        // If TLS enabled, perform TLS handshake
        if self.use_tls {
            let connector = self.create_tls_connector()?;

            let server_name = ServerName::try_from(self.host.clone())
                .map_err(|e| MushError::TlsError(
                    format!("Invalid hostname for TLS: {}", e)
                ))?;

            match timeout(timeout_duration, connector.connect(server_name, tcp_stream)).await {
                Ok(Ok(tls_stream)) => {
                    info!("TLS handshake successful with {}", addr);
                    self.stream = Some(MudStream::Tls(tls_stream));
                    Ok(())
                }
                Ok(Err(e)) => {
                    error!("TLS handshake failed with {}: {}", addr, e);
                    Err(MushError::TlsError(
                        format!("TLS handshake failed: {}", e)
                    ))
                }
                Err(_) => {
                    error!("TLS handshake timeout after {}s to {}", self.timeout_secs, addr);
                    Err(MushError::ConnectionTimeout {
                        timeout_secs: self.timeout_secs,
                    })
                }
            }
        } else {
            info!("Plain TCP connection established to {}", addr);
            self.stream = Some(MudStream::Plain(tcp_stream));
            Ok(())
        }
    }

    /// Send data to the server
    ///
    /// # Errors
    /// - `MushError::NotConnected`: Not currently connected to a server
    /// - `MushError::ConnectionFailed`: Write operation failed
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
    /// # Errors
    /// - `MushError::NotConnected`: Not currently connected to a server
    /// - `MushError::ConnectionFailed`: Read operation failed
    /// - `MushError::ConnectionClosed`: Server closed the connection
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
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Check if using TLS
    pub fn is_tls(&self) -> bool {
        matches!(self.stream, Some(MudStream::Tls(_)))
    }

    /// Disconnect from the server
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

    async fn start_mock_server() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn test_plain_tcp_connection() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let mut client = TlsClient::builder("127.0.0.1", port)
            .use_tls(false)
            .build();

        let result = client.connect().await;
        assert!(result.is_ok(), "Should connect with plain TCP");
        assert!(client.is_connected());
        assert!(!client.is_tls());
    }

    #[tokio::test]
    async fn test_send_receive_plain() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"Welcome!\n").await;
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
            }
        });

        let mut client = TlsClient::new("127.0.0.1", port);
        client.connect().await.expect("Should connect");

        let received = client.receive().await.expect("Should receive");
        assert_eq!(received, b"Welcome!\n");

        let sent = client.send(b"test\n").await.expect("Should send");
        assert_eq!(sent, 5);
    }
}

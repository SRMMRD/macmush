/// Network layer: TCP, TLS, and MUD protocols
///
/// This module handles all network communication:
/// - TCP client with async I/O
/// - TLS wrapper for secure connections
/// - MUD protocol support (Telnet, MCCP, MXP, GMCP)

pub mod tcp;
pub mod tls;
pub mod codec;
pub mod mccp;
pub mod mxp;

// Re-export commonly used types
pub use tcp::TcpClient;
pub use tls::{TlsClient, TlsClientBuilder, MudStream};
pub use codec::MudCodec;
pub use mccp::{MccpHandler, CompressionStats};
pub use mxp::{MxpParser, MxpMode, MxpTag, MxpElement};

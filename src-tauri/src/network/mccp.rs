/// MCCP (Mud Client Compression Protocol) Support
///
/// Implements MCCP2 (option 86) for server-to-client compression
/// and MCCP3 (option 87) for client-to-server compression.
///
/// References:
/// - https://tintin.mudhalla.net/protocols/mccp/
/// - https://www.gammon.com.au/mccp/protocol.html

use crate::error::{MushError, Result};
use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use std::io::Write;
use tracing::{debug, info, warn};

/// MCCP telnet option numbers
pub const TELOPT_COMPRESS2: u8 = 86; // MCCP2 (server-to-client)
pub const TELOPT_COMPRESS3: u8 = 87; // MCCP3 (client-to-server)

/// MCCP compression state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionState {
    /// No compression active
    None,
    /// Compression negotiated, waiting for subnegotiation
    Negotiated,
    /// Compression active
    Active,
}

/// MCCP handler for compression/decompression
pub struct MccpHandler {
    /// Server-to-client compression state (MCCP2)
    rx_state: CompressionState,
    /// Client-to-server compression state (MCCP3)
    tx_state: CompressionState,
    /// Zlib decompressor for incoming data
    decompressor: Option<ZlibDecoder<Vec<u8>>>,
    /// Zlib compressor for outgoing data
    compressor: Option<ZlibEncoder<Vec<u8>>>,
    /// Buffer for decompressed data
    decompress_buffer: Vec<u8>,
}

impl MccpHandler {
    /// Create a new MCCP handler
    pub fn new() -> Self {
        Self {
            rx_state: CompressionState::None,
            tx_state: CompressionState::None,
            decompressor: None,
            compressor: None,
            decompress_buffer: Vec::new(),
        }
    }

    /// Handle MCCP2 (server-to-client) negotiation
    pub fn handle_mccp2_will(&mut self) -> Result<Vec<u8>> {
        info!("Server offered MCCP2 compression");
        self.rx_state = CompressionState::Negotiated;

        // Respond with IAC DO COMPRESS2
        Ok(vec![255, 253, TELOPT_COMPRESS2]) // IAC DO COMPRESS2
    }

    /// Handle MCCP2 subnegotiation - compression starts immediately after
    pub fn handle_mccp2_subnegotiation(&mut self) -> Result<()> {
        info!("MCCP2 compression starting");
        self.rx_state = CompressionState::Active;

        // Initialize zlib decompressor
        self.decompressor = Some(ZlibDecoder::new(Vec::new()));
        self.decompress_buffer.clear();

        Ok(())
    }

    /// Handle MCCP3 (client-to-server) negotiation
    pub fn handle_mccp3_will(&mut self) -> Result<Vec<u8>> {
        info!("Server offered MCCP3 compression");
        self.tx_state = CompressionState::Negotiated;

        // Respond with IAC DO COMPRESS3 IAC SB COMPRESS3 IAC SE
        Ok(vec![
            255, 253, TELOPT_COMPRESS3, // IAC DO COMPRESS3
            255, 250, TELOPT_COMPRESS3, // IAC SB COMPRESS3
            255, 240,                    // IAC SE
        ])
    }

    /// Start client-to-server compression after negotiation
    pub fn start_mccp3(&mut self) -> Result<()> {
        info!("MCCP3 compression starting");
        self.tx_state = CompressionState::Active;

        // Initialize zlib compressor
        self.compressor = Some(ZlibEncoder::new(Vec::new(), Compression::default()));

        Ok(())
    }

    /// Check if server-to-client compression is active
    pub fn is_receiving_compressed(&self) -> bool {
        self.rx_state == CompressionState::Active
    }

    /// Check if client-to-server compression is active
    pub fn is_sending_compressed(&self) -> bool {
        self.tx_state == CompressionState::Active
    }

    /// Decompress incoming data (MCCP2)
    pub fn decompress(&mut self, compressed_data: &[u8]) -> Result<Vec<u8>> {
        if self.rx_state != CompressionState::Active {
            return Ok(compressed_data.to_vec());
        }

        let decompressor = self.decompressor.as_mut()
            .ok_or_else(|| MushError::CompressionError("No decompressor initialized".to_string()))?;

        // Write compressed data to decompressor
        decompressor.write_all(compressed_data)
            .map_err(|e| MushError::CompressionError(format!("Decompression write failed: {}", e)))?;

        // Get decompressed output
        let output = decompressor.get_ref().clone();
        decompressor.get_mut().clear();

        if !output.is_empty() {
            debug!("Decompressed {} bytes to {} bytes", compressed_data.len(), output.len());
        }

        Ok(output)
    }

    /// Compress outgoing data (MCCP3)
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if self.tx_state != CompressionState::Active {
            return Ok(data.to_vec());
        }

        let compressor = self.compressor.as_mut()
            .ok_or_else(|| MushError::CompressionError("No compressor initialized".to_string()))?;

        // Compress data
        compressor.write_all(data)
            .map_err(|e| MushError::CompressionError(format!("Compression failed: {}", e)))?;

        compressor.flush()
            .map_err(|e| MushError::CompressionError(format!("Compression flush failed: {}", e)))?;

        // Get compressed output
        let compressed = compressor.get_ref().clone();
        debug!("Compressed {} bytes to {} bytes", data.len(), compressed.len());

        Ok(compressed)
    }

    /// Disable server-to-client compression
    pub fn disable_mccp2(&mut self) -> Result<Vec<u8>> {
        info!("Disabling MCCP2 compression");
        self.rx_state = CompressionState::None;
        self.decompressor = None;
        self.decompress_buffer.clear();

        // Send IAC DONT COMPRESS2
        Ok(vec![255, 254, TELOPT_COMPRESS2]) // IAC DONT COMPRESS2
    }

    /// Disable client-to-server compression
    pub fn disable_mccp3(&mut self) -> Result<Vec<u8>> {
        info!("Disabling MCCP3 compression");
        self.tx_state = CompressionState::None;
        self.compressor = None;

        // Send IAC DONT COMPRESS3
        Ok(vec![255, 254, TELOPT_COMPRESS3]) // IAC DONT COMPRESS3
    }

    /// Get compression statistics
    pub fn get_stats(&self) -> CompressionStats {
        CompressionStats {
            mccp2_active: self.is_receiving_compressed(),
            mccp3_active: self.is_sending_compressed(),
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Copy)]
pub struct CompressionStats {
    pub mccp2_active: bool,
    pub mccp3_active: bool,
}

impl Default for MccpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mccp_handler_creation() {
        let handler = MccpHandler::new();
        assert_eq!(handler.rx_state, CompressionState::None);
        assert_eq!(handler.tx_state, CompressionState::None);
        assert!(!handler.is_receiving_compressed());
        assert!(!handler.is_sending_compressed());
    }

    #[test]
    fn test_mccp2_negotiation() {
        let mut handler = MccpHandler::new();

        // Handle WILL COMPRESS2
        let response = handler.handle_mccp2_will().unwrap();
        assert_eq!(response, vec![255, 253, TELOPT_COMPRESS2]); // IAC DO COMPRESS2
        assert_eq!(handler.rx_state, CompressionState::Negotiated);

        // Handle subnegotiation
        handler.handle_mccp2_subnegotiation().unwrap();
        assert_eq!(handler.rx_state, CompressionState::Active);
        assert!(handler.is_receiving_compressed());
    }

    #[test]
    fn test_mccp3_negotiation() {
        let mut handler = MccpHandler::new();

        // Handle WILL COMPRESS3
        let response = handler.handle_mccp3_will().unwrap();
        assert!(response.contains(&TELOPT_COMPRESS3));
        assert_eq!(handler.tx_state, CompressionState::Negotiated);

        // Start compression
        handler.start_mccp3().unwrap();
        assert_eq!(handler.tx_state, CompressionState::Active);
        assert!(handler.is_sending_compressed());
    }

    #[test]
    fn test_compression_stats() {
        let mut handler = MccpHandler::new();

        let stats = handler.get_stats();
        assert!(!stats.mccp2_active);
        assert!(!stats.mccp3_active);

        handler.handle_mccp2_will().unwrap();
        handler.handle_mccp2_subnegotiation().unwrap();

        let stats = handler.get_stats();
        assert!(stats.mccp2_active);
        assert!(!stats.mccp3_active);
    }
}

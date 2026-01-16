/// Comprehensive error types for MACMush
///
/// This module provides typed error handling throughout the application,
/// eliminating the use of generic String errors and unwrap() calls.

use thiserror::Error;

/// Main error type for MACMush operations
#[derive(Error, Debug)]
pub enum MushError {
    // ========================================
    // Connection Errors
    // ========================================

    #[error("Failed to connect to {host}:{port} - {source}")]
    ConnectionFailed {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("Connection timeout after {timeout_secs} seconds")]
    ConnectionTimeout {
        timeout_secs: u64,
    },

    #[error("Connection closed by remote host")]
    ConnectionClosed,

    #[error("Not connected to server")]
    NotConnected,

    #[error("TLS handshake failed: {0}")]
    TlsError(String),

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("MXP parse error: {0}")]
    MxpError(String),

    #[error("Lua script error: {0}")]
    LuaError(String),

    // ========================================
    // Automation Errors
    // ========================================

    #[error("Invalid regex pattern '{pattern}': {source}")]
    InvalidRegex {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    #[error("Trigger '{name}' not found")]
    TriggerNotFound {
        name: String,
    },

    #[error("Alias '{name}' not found")]
    AliasNotFound {
        name: String,
    },

    #[error("Timer '{name}' not found")]
    TimerNotFound {
        name: String,
    },

    #[error("Variable '{name}' not found")]
    VariableNotFound {
        name: String,
    },

    // ========================================
    // Persistence Errors
    // ========================================

    #[error("Failed to parse world file: {0}")]
    WorldFileParseError(String),

    #[error("Failed to serialize world file: {0}")]
    WorldFileSerializeError(String),

    #[error("World '{id}' not found")]
    WorldNotFound {
        id: String,
    },

    // ========================================
    // Validation Errors
    // ========================================

    #[error("Invalid input for field '{field}': {reason}")]
    ValidationError {
        field: String,
        reason: String,
    },

    #[error("Invalid hostname: {0}")]
    InvalidHostname(String),

    #[error("Invalid port: {0} (must be 1-65535)")]
    InvalidPort(u16),

    #[error("Command too long: {length} bytes (max: {max})")]
    CommandTooLong {
        length: usize,
        max: usize,
    },

    // ========================================
    // IO Errors
    // ========================================

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    // ========================================
    // Serialization Errors
    // ========================================

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("XML error: {0}")]
    XmlError(String),

    // ========================================
    // Generic Errors
    // ========================================

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for MACMush operations
pub type Result<T> = std::result::Result<T, MushError>;

// Conversion from mlua::Error
impl From<mlua::Error> for MushError {
    fn from(err: mlua::Error) -> Self {
        MushError::LuaError(err.to_string())
    }
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_connection_failed_error() {
        let io_err = IoError::new(ErrorKind::ConnectionRefused, "refused");
        let err = MushError::ConnectionFailed {
            host: "localhost".to_string(),
            port: 4000,
            source: io_err,
        };

        assert!(err.to_string().contains("localhost:4000"));
        assert!(err.to_string().contains("refused"));
    }

    #[test]
    fn test_connection_timeout_error() {
        let err = MushError::ConnectionTimeout {
            timeout_secs: 30,
        };

        assert!(err.to_string().contains("30 seconds"));
    }

    #[test]
    fn test_invalid_regex_error() {
        let regex_err = regex::Regex::new("[").unwrap_err();
        let err = MushError::InvalidRegex {
            pattern: "[".to_string(),
            source: regex_err,
        };

        assert!(err.to_string().contains("Invalid regex pattern"));
        assert!(err.to_string().contains("["));
    }

    #[test]
    fn test_validation_error() {
        let err = MushError::ValidationError {
            field: "hostname".to_string(),
            reason: "contains invalid characters".to_string(),
        };

        assert!(err.to_string().contains("hostname"));
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_command_too_long_error() {
        let err = MushError::CommandTooLong {
            length: 2000,
            max: 1024,
        };

        assert!(err.to_string().contains("2000"));
        assert!(err.to_string().contains("1024"));
    }

    #[test]
    fn test_error_source_chain() {
        let io_err = IoError::new(ErrorKind::NotFound, "not found");
        let err = MushError::ConnectionFailed {
            host: "test.mud".to_string(),
            port: 4000,
            source: io_err,
        };

        // Verify error source chain works
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn test_result_type_usage() {
        fn example_function() -> Result<String> {
            Err(MushError::WorldNotFound {
                id: "test-world".to_string(),
            })
        }

        match example_function() {
            Ok(_) => panic!("Should have returned error"),
            Err(e) => assert!(matches!(e, MushError::WorldNotFound { .. })),
        }
    }
}

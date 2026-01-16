/// World configuration and management
///
/// Represents a MUD/MUSH world connection profile with all settings
/// including connection details, automation, and preferences.

use crate::error::{MushError, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// World configuration with connection and automation settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct World {
    /// Unique identifier
    pub id: Uuid,

    /// World name (user-friendly)
    pub name: String,

    /// Host address
    pub host: String,

    /// Port number
    pub port: u16,

    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Auto-connect on startup
    #[serde(default)]
    pub auto_connect: bool,

    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Enable TLS/SSL
    #[serde(default)]
    pub use_tls: bool,
}

fn default_timeout() -> u64 {
    30
}

impl World {
    /// Create a new World with required fields
    ///
    /// Creates a new world configuration with sensible defaults:
    /// - Unique UUID generated automatically
    /// - 30-second connection timeout
    /// - No auto-connect
    /// - No TLS/SSL encryption
    ///
    /// # Arguments
    /// * `name` - User-friendly world name (cannot be empty/whitespace)
    /// * `host` - Hostname or IP address (cannot be empty, basic validation applied)
    /// * `port` - Port number (1-65535, cannot be 0)
    ///
    /// # Errors
    /// - `MushError::ValidationError`: Name or host is empty/whitespace only
    /// - `MushError::InvalidHostname`: Host contains invalid patterns (consecutive dots, leading/trailing dots)
    /// - `MushError::InvalidPort`: Port is 0
    ///
    /// # Examples
    /// ```ignore
    /// let world = World::new("Test MUD", "mud.example.com", 4000)?;
    /// let world = World::new("Local Test", "localhost", 4000)?;
    /// let world = World::new("IP Test", "192.168.1.100", 4000)?;
    /// ```
    pub fn new(name: impl Into<String>, host: impl Into<String>, port: u16) -> Result<Self> {
        let name = name.into();
        let host = host.into();

        debug!("Creating new world: name='{}', host='{}', port={}", name, host, port);

        let world = World {
            id: Uuid::new_v4(),
            name,
            host,
            port,
            description: None,
            auto_connect: false,
            timeout_secs: 30,
            use_tls: false,
        };

        world.validate()?;

        info!("Created world '{}' with ID {}", world.name, world.id);
        Ok(world)
    }

    /// Create a builder for custom configuration
    ///
    /// Use this when you need to configure optional fields like description,
    /// auto-connect, timeout, or TLS settings.
    ///
    /// # Examples
    /// ```ignore
    /// let world = World::builder("Test MUD", "mud.example.com", 4000)
    ///     .description("My favorite MUD")
    ///     .auto_connect(true)
    ///     .timeout_secs(60)
    ///     .use_tls(true)
    ///     .build()?;
    /// ```
    pub fn builder(name: impl Into<String>, host: impl Into<String>, port: u16) -> WorldBuilder {
        WorldBuilder::new(name, host, port)
    }

    /// Validate world configuration
    ///
    /// Performs comprehensive validation of all world settings:
    /// - Name must not be empty or whitespace-only
    /// - Host must not be empty and must not contain invalid patterns
    /// - Port must be in range 1-65535 (cannot be 0)
    ///
    /// # Errors
    /// Returns the first validation error encountered.
    pub fn validate(&self) -> Result<()> {
        // Validate name
        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            warn!("Validation failed: empty name");
            return Err(MushError::ValidationError {
                field: "name".to_string(),
                reason: "Name cannot be empty or whitespace only".to_string(),
            });
        }

        // Validate host
        let trimmed_host = self.host.trim();
        if trimmed_host.is_empty() {
            warn!("Validation failed: empty host");
            return Err(MushError::ValidationError {
                field: "host".to_string(),
                reason: "Host cannot be empty".to_string(),
            });
        }

        // Basic hostname validation - reject obvious invalid patterns
        if trimmed_host.contains("..") || trimmed_host.starts_with('.') || trimmed_host.ends_with('.') {
            warn!("Validation failed: invalid hostname pattern '{}'", self.host);
            return Err(MushError::InvalidHostname(self.host.clone()));
        }

        // Validate port
        if self.port == 0 {
            warn!("Validation failed: port cannot be 0");
            return Err(MushError::InvalidPort(self.port));
        }

        debug!("Validation passed for world '{}'", self.name);
        Ok(())
    }

    /// Serialize to XML format (MUSHClient compatible)
    ///
    /// Serializes the world configuration to XML format compatible with
    /// the classic MUSHClient .mush world file format.
    ///
    /// # Errors
    /// - `MushError::XmlError`: Serialization failed
    ///
    /// # Examples
    /// ```ignore
    /// let world = World::new("Test", "localhost", 4000)?;
    /// let xml = world.to_xml()?;
    /// ```
    pub fn to_xml(&self) -> Result<String> {
        use quick_xml::se::to_string;

        debug!("Serializing world '{}' to XML", self.name);

        to_string(&self).map_err(|e| {
            let err_msg = format!("Failed to serialize world to XML: {}", e);
            warn!("{}", err_msg);
            MushError::XmlError(err_msg)
        })
    }

    /// Deserialize from XML format
    ///
    /// Parses XML into a World configuration and validates the result.
    /// Compatible with MUSHClient .mush world file format.
    ///
    /// # Errors
    /// - `MushError::WorldFileParseError`: XML parsing failed
    /// - Any validation errors from `validate()`
    ///
    /// # Examples
    /// ```ignore
    /// let xml = r#"<World><name>Test</name>...</World>"#;
    /// let world = World::from_xml(xml)?;
    /// ```
    pub fn from_xml(xml: &str) -> Result<Self> {
        use quick_xml::de::from_str;

        debug!("Deserializing world from XML ({} bytes)", xml.len());

        let world: World = from_str(xml).map_err(|e| {
            let err_msg = format!("Failed to parse world XML: {}", e);
            warn!("{}", err_msg);
            MushError::WorldFileParseError(err_msg)
        })?;

        // Validate after deserialization to ensure data integrity
        world.validate()?;

        info!("Successfully loaded world '{}' from XML", world.name);
        Ok(world)
    }
}

/// Builder for World configuration
#[derive(Debug)]
pub struct WorldBuilder {
    id: Uuid,
    name: String,
    host: String,
    port: u16,
    description: Option<String>,
    auto_connect: bool,
    timeout_secs: u64,
    use_tls: bool,
}

impl WorldBuilder {
    pub fn new(name: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            host: host.into(),
            port,
            description: None,
            auto_connect: false,
            timeout_secs: 30,
            use_tls: false,
        }
    }

    pub fn id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn auto_connect(mut self, enabled: bool) -> Self {
        self.auto_connect = enabled;
        self
    }

    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn use_tls(mut self, enabled: bool) -> Self {
        self.use_tls = enabled;
        self
    }

    pub fn build(self) -> Result<World> {
        let world = World {
            id: self.id,
            name: self.name,
            host: self.host,
            port: self.port,
            description: self.description,
            auto_connect: self.auto_connect,
            timeout_secs: self.timeout_secs,
            use_tls: self.use_tls,
        };

        world.validate()?;
        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_world_with_valid_config() {
        let world = World::new("Test MUD", "mud.example.com", 4000);

        assert!(world.is_ok(), "Should create world with valid config");
        let world = world.unwrap();
        assert_eq!(world.name, "Test MUD");
        assert_eq!(world.host, "mud.example.com");
        assert_eq!(world.port, 4000);
        assert_eq!(world.timeout_secs, 30);
        assert!(!world.auto_connect);
        assert!(!world.use_tls);
    }

    #[test]
    fn test_validate_empty_name() {
        let result = World::new("", "mud.example.com", 4000);

        assert!(result.is_err(), "Should reject empty name");
        assert!(
            matches!(result, Err(MushError::ValidationError { .. })),
            "Should return ValidationError"
        );
    }

    #[test]
    fn test_validate_whitespace_only_name() {
        let result = World::new("   ", "mud.example.com", 4000);

        assert!(result.is_err(), "Should reject whitespace-only name");
    }

    #[test]
    fn test_validate_empty_host() {
        let result = World::new("Test MUD", "", 4000);

        assert!(result.is_err(), "Should reject empty host");
        assert!(
            matches!(result, Err(MushError::ValidationError { .. })),
            "Should return ValidationError"
        );
    }

    #[test]
    fn test_validate_invalid_hostname() {
        let result = World::new("Test MUD", "invalid..host..name", 4000);

        assert!(result.is_err(), "Should reject invalid hostname");
    }

    #[test]
    fn test_validate_port_zero() {
        let result = World::new("Test MUD", "mud.example.com", 0);

        assert!(result.is_err(), "Should reject port 0");
        assert!(
            matches!(result, Err(MushError::InvalidPort(_))),
            "Should return InvalidPort error"
        );
    }

    #[test]
    fn test_builder_pattern() {
        let world = World::builder("Test MUD", "mud.example.com", 4000)
            .description("A test MUD world")
            .auto_connect(true)
            .timeout_secs(60)
            .use_tls(true)
            .build();

        assert!(world.is_ok(), "Builder should create valid world");
        let world = world.unwrap();
        assert_eq!(world.name, "Test MUD");
        assert_eq!(world.description, Some("A test MUD world".to_string()));
        assert!(world.auto_connect);
        assert_eq!(world.timeout_secs, 60);
        assert!(world.use_tls);
    }

    #[test]
    fn test_builder_with_custom_id() {
        let id = Uuid::new_v4();
        let world = World::builder("Test MUD", "mud.example.com", 4000)
            .id(id)
            .build()
            .unwrap();

        assert_eq!(world.id, id);
    }

    #[test]
    fn test_json_serialization() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let json = serde_json::to_string(&world);

        assert!(json.is_ok(), "Should serialize to JSON");
        let json = json.unwrap();
        assert!(json.contains("Test MUD"));
        assert!(json.contains("mud.example.com"));
        assert!(json.contains("4000"));
    }

    #[test]
    fn test_json_deserialization() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let json = serde_json::to_string(&world).unwrap();

        let deserialized = serde_json::from_str::<World>(&json);
        assert!(deserialized.is_ok(), "Should deserialize from JSON");

        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.name, world.name);
        assert_eq!(deserialized.host, world.host);
        assert_eq!(deserialized.port, world.port);
    }

    #[test]
    fn test_xml_serialization() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let xml = world.to_xml();

        assert!(xml.is_ok(), "Should serialize to XML");
        let xml = xml.unwrap();
        assert!(xml.contains("<world>") || xml.contains("<World>"));
        assert!(xml.contains("Test MUD"));
        assert!(xml.contains("mud.example.com"));
    }

    #[test]
    fn test_xml_deserialization() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let xml = world.to_xml().unwrap();

        let deserialized = World::from_xml(&xml);
        assert!(deserialized.is_ok(), "Should deserialize from XML");

        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.name, world.name);
        assert_eq!(deserialized.host, world.host);
        assert_eq!(deserialized.port, world.port);
    }

    #[test]
    fn test_xml_roundtrip_with_optional_fields() {
        let world = World::builder("Test MUD", "mud.example.com", 4000)
            .description("Test description")
            .auto_connect(true)
            .use_tls(true)
            .build()
            .unwrap();

        let xml = world.to_xml().unwrap();
        let deserialized = World::from_xml(&xml).unwrap();

        assert_eq!(deserialized.name, world.name);
        assert_eq!(deserialized.description, world.description);
        assert_eq!(deserialized.auto_connect, world.auto_connect);
        assert_eq!(deserialized.use_tls, world.use_tls);
    }

    #[test]
    fn test_validate_ipv4_address() {
        let world = World::new("Test MUD", "192.168.1.100", 4000);
        assert!(world.is_ok(), "Should accept IPv4 addresses");
    }

    #[test]
    fn test_validate_localhost() {
        let world = World::new("Test MUD", "localhost", 4000);
        assert!(world.is_ok(), "Should accept localhost");
    }

    #[test]
    fn test_validate_high_port() {
        let world = World::new("Test MUD", "mud.example.com", 65535);
        assert!(world.is_ok(), "Should accept port 65535");
    }

    #[test]
    fn test_clone_world() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let cloned = world.clone();

        assert_eq!(cloned.id, world.id);
        assert_eq!(cloned.name, world.name);
        assert_eq!(cloned.host, world.host);
        assert_eq!(cloned.port, world.port);
    }
}

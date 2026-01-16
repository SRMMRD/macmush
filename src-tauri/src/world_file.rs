/// MUSHclient XML World File Import/Export
///
/// This module handles reading and writing MUSHclient XML world files (.mcl)
/// for compatibility with the original Windows MUSHclient.
///
/// XML Format Reference:
/// - http://gammon.com.au/forum/?id=1214
/// - http://www.gammon.com.au/scripts/options.php

use crate::error::{MushError, Result};
use serde::{Deserialize, Serialize};

/// Root element for MUSHclient world file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "muclient")]
pub struct WorldFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<WorldConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggers: Option<TriggersList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliases: Option<AliasesList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timers: Option<TimersList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub macros: Option<MacrosList>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<VariablesList>,
}

/// World configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    // Connection settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub player: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_mxp: Option<bool>,

    // Feature toggles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_triggers: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_aliases: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_timers: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_scripts: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_speed_walk: Option<bool>,

    // Script settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_language: Option<String>,

    // Logging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file_name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_log: Option<bool>,
}

/// List of triggers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggersList {
    #[serde(rename = "trigger", default)]
    pub items: Vec<Trigger>,
}

/// Individual trigger definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    #[serde(rename = "@match")]
    pub match_text: String,

    #[serde(rename = "@enabled", default = "default_true")]
    pub enabled: bool,

    #[serde(rename = "@regexp", default)]
    pub regexp: bool,

    #[serde(rename = "@ignore_case", default)]
    pub ignore_case: bool,

    #[serde(rename = "@group", skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    #[serde(rename = "@sequence", default = "default_sequence")]
    pub sequence: u32,

    #[serde(rename = "$value", skip_serializing_if = "Option::is_none")]
    pub send: Option<String>,

    #[serde(rename = "@script", skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

/// List of aliases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasesList {
    #[serde(rename = "alias", default)]
    pub items: Vec<Alias>,
}

/// Individual alias definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    #[serde(rename = "@match")]
    pub match_text: String,

    #[serde(rename = "@enabled", default = "default_true")]
    pub enabled: bool,

    #[serde(rename = "@regexp", default)]
    pub regexp: bool,

    #[serde(rename = "@ignore_case", default = "default_true")]
    pub ignore_case: bool,

    #[serde(rename = "@group", skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    #[serde(rename = "@sequence", default = "default_sequence")]
    pub sequence: u32,

    #[serde(rename = "$value", skip_serializing_if = "Option::is_none")]
    pub send: Option<String>,
}

/// List of timers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimersList {
    #[serde(rename = "timer", default)]
    pub items: Vec<Timer>,
}

/// Individual timer definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@enabled", default = "default_true")]
    pub enabled: bool,

    #[serde(rename = "@at_time", skip_serializing_if = "Option::is_none")]
    pub at_time: Option<String>,

    #[serde(rename = "@second", skip_serializing_if = "Option::is_none")]
    pub second: Option<u32>,

    #[serde(rename = "@minute", skip_serializing_if = "Option::is_none")]
    pub minute: Option<u32>,

    #[serde(rename = "@hour", skip_serializing_if = "Option::is_none")]
    pub hour: Option<u32>,

    #[serde(rename = "@interval_seconds", skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u32>,

    #[serde(rename = "@active_closed", default)]
    pub active_closed: bool,

    #[serde(rename = "$value", skip_serializing_if = "Option::is_none")]
    pub send: Option<String>,

    #[serde(rename = "@script", skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

/// List of macros
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacrosList {
    #[serde(rename = "macro", default)]
    pub items: Vec<Macro>,
}

/// Individual macro definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    #[serde(rename = "@key")]
    pub key: String,

    #[serde(rename = "$value")]
    pub send: String,
}

/// List of variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariablesList {
    #[serde(rename = "variable", default)]
    pub items: Vec<Variable>,
}

/// Individual variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "$value")]
    pub value: String,
}

// Default helper functions
fn default_true() -> bool {
    true
}

fn default_sequence() -> u32 {
    100
}

impl WorldFile {
    /// Create a new empty world file
    pub fn new() -> Self {
        Self {
            world: None,
            triggers: None,
            aliases: None,
            timers: None,
            macros: None,
            variables: None,
        }
    }

    /// Load world file from XML string
    pub fn from_xml(xml: &str) -> Result<Self> {
        quick_xml::de::from_str(xml).map_err(|e| MushError::WorldFileParseError(e.to_string()))
    }

    /// Serialize world file to XML string
    pub fn to_xml(&self) -> Result<String> {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        xml.push_str("<!DOCTYPE muclient>\n");
        xml.push_str(&format!("<!-- Saved by MACMush v{} -->\n", env!("CARGO_PKG_VERSION")));
        xml.push_str("<!-- https://github.com/macmush/macmush -->\n\n");

        let serialized = quick_xml::se::to_string(self)
            .map_err(|e| MushError::WorldFileSerializeError(e.to_string()))?;

        xml.push_str(&serialized);
        Ok(xml)
    }
}

impl Default for WorldFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_world_file() {
        let world_file = WorldFile::new();
        assert!(world_file.world.is_none());
        assert!(world_file.triggers.is_none());
    }

    #[test]
    fn test_world_file_serialization() {
        let mut world_file = WorldFile::new();
        world_file.world = Some(WorldConfig {
            site: Some("mud.example.com".to_string()),
            port: Some(4000),
            name: Some("Test MUD".to_string()),
            player: None,
            password: None,
            use_mxp: Some(false),
            enable_triggers: Some(true),
            enable_aliases: Some(true),
            enable_timers: Some(true),
            enable_scripts: Some(false),
            enable_speed_walk: Some(true),
            script_language: None,
            log_file_name: None,
            auto_log: Some(false),
        });

        let xml = world_file.to_xml().unwrap();
        assert!(xml.contains("<?xml version"));
        assert!(xml.contains("mud.example.com"));
    }

    #[test]
    fn test_trigger_serialization() {
        let trigger = Trigger {
            match_text: "You have (\\d+) gold".to_string(),
            enabled: true,
            regexp: true,
            ignore_case: false,
            group: Some("Gold".to_string()),
            sequence: 100,
            send: Some("say I have gold!".to_string()),
            script: None,
        };

        let triggers_list = TriggersList {
            items: vec![trigger],
        };

        let mut world_file = WorldFile::new();
        world_file.triggers = Some(triggers_list);

        let xml = world_file.to_xml().unwrap();
        assert!(xml.contains("You have"));
    }
}

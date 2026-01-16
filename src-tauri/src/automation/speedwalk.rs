/// Speed-walking system for efficient MUD navigation
///
/// Allows commands like "4n 5w" to be expanded into individual direction commands:
/// north, north, north, north, west, west, west, west, west
///
/// Supports standard MUD directions and optional separator characters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Configuration for speed-walking behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedwalkConfig {
    /// Enabled or disabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Direction mappings (short form -> full command)
    #[serde(default = "default_direction_map")]
    pub direction_map: HashMap<String, String>,
}

fn default_enabled() -> bool {
    true
}

fn default_direction_map() -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Cardinal directions
    map.insert("n".to_string(), "north".to_string());
    map.insert("s".to_string(), "south".to_string());
    map.insert("e".to_string(), "east".to_string());
    map.insert("w".to_string(), "west".to_string());

    // Diagonal directions
    map.insert("ne".to_string(), "northeast".to_string());
    map.insert("nw".to_string(), "northwest".to_string());
    map.insert("se".to_string(), "southeast".to_string());
    map.insert("sw".to_string(), "southwest".to_string());

    // Vertical directions
    map.insert("u".to_string(), "up".to_string());
    map.insert("d".to_string(), "down".to_string());

    map
}

impl Default for SpeedwalkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            direction_map: default_direction_map(),
        }
    }
}

/// Speed-walking parser and expander
#[derive(Debug, Clone)]
pub struct Speedwalk {
    config: SpeedwalkConfig,
}

impl Speedwalk {
    /// Create a new speedwalk instance with default configuration
    pub fn new() -> Self {
        Self {
            config: SpeedwalkConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: SpeedwalkConfig) -> Self {
        Self { config }
    }

    /// Check if speedwalking is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable or disable speedwalking
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Add or update a direction mapping
    pub fn add_direction(&mut self, short: impl Into<String>, full: impl Into<String>) {
        self.config.direction_map.insert(short.into(), full.into());
    }

    /// Remove a direction mapping
    pub fn remove_direction(&mut self, short: &str) -> bool {
        self.config.direction_map.remove(short).is_some()
    }

    /// Get all direction mappings
    pub fn get_directions(&self) -> &HashMap<String, String> {
        &self.config.direction_map
    }

    /// Try to expand a command as a speedwalk sequence
    ///
    /// Returns Some(Vec<String>) if the command is a valid speedwalk sequence,
    /// None if it should be treated as a regular command.
    ///
    /// Examples:
    /// - "4n" -> Some(["north", "north", "north", "north"])
    /// - "3n 2e" -> Some(["north", "north", "north", "east", "east"])
    /// - "look" -> None (not a speedwalk command)
    pub fn try_expand(&self, command: &str) -> Option<Vec<String>> {
        if !self.config.enabled {
            return None;
        }

        let trimmed = command.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Try to parse the entire command as speedwalk
        let mut expanded = Vec::new();
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();

        for token in tokens {
            match self.parse_token(token) {
                Some(commands) => expanded.extend(commands),
                None => {
                    // If any token fails to parse as speedwalk, the entire
                    // command is not a speedwalk sequence
                    debug!("Token '{}' is not a speedwalk pattern", token);
                    return None;
                }
            }
        }

        if expanded.is_empty() {
            None
        } else {
            debug!("Expanded speedwalk '{}' into {} commands", command, expanded.len());
            Some(expanded)
        }
    }

    /// Parse a single token as a speedwalk pattern
    ///
    /// Patterns:
    /// - "4n" -> Some(["north", "north", "north", "north"])
    /// - "n" -> Some(["north"])
    /// - "northeast" -> Some(["northeast"])
    /// - "look" -> None
    fn parse_token(&self, token: &str) -> Option<Vec<String>> {
        // Check if it's a full direction name
        if self.config.direction_map.values().any(|v| v == token) {
            return Some(vec![token.to_string()]);
        }

        // Check if it's a simple direction (no count)
        if let Some(full_dir) = self.config.direction_map.get(token) {
            return Some(vec![full_dir.clone()]);
        }

        // Try to parse as count+direction (e.g., "4n")
        if let Some((count, direction)) = self.parse_counted_direction(token) {
            if let Some(full_dir) = self.config.direction_map.get(direction) {
                return Some(vec![full_dir.clone(); count]);
            }
        }

        None
    }

    /// Parse a token like "4n" into (count, direction)
    fn parse_counted_direction<'a>(&self, token: &'a str) -> Option<(usize, &'a str)> {
        // Find where digits end
        let digit_end = token.chars().take_while(|c| c.is_ascii_digit()).count();

        if digit_end == 0 || digit_end == token.len() {
            // No digits at start, or only digits
            return None;
        }

        let count_str = &token[..digit_end];
        let direction = &token[digit_end..];

        // Parse count
        let count = count_str.parse::<usize>().ok()?;

        // Validate count is reasonable (prevent abuse)
        if count == 0 || count > 1000 {
            return None;
        }

        Some((count, direction))
    }
}

impl Default for Speedwalk {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_speedwalk() {
        let sw = Speedwalk::new();
        assert!(sw.is_enabled());
        assert!(!sw.get_directions().is_empty());
    }

    #[test]
    fn test_simple_direction() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("n");
        assert_eq!(result, Some(vec!["north".to_string()]));

        let result = sw.try_expand("s");
        assert_eq!(result, Some(vec!["south".to_string()]));

        let result = sw.try_expand("ne");
        assert_eq!(result, Some(vec!["northeast".to_string()]));
    }

    #[test]
    fn test_full_direction_name() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("north");
        assert_eq!(result, Some(vec!["north".to_string()]));

        let result = sw.try_expand("southeast");
        assert_eq!(result, Some(vec!["southeast".to_string()]));
    }

    #[test]
    fn test_counted_direction() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("4n");
        assert_eq!(
            result,
            Some(vec![
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "north".to_string()
            ])
        );

        let result = sw.try_expand("3e");
        assert_eq!(
            result,
            Some(vec![
                "east".to_string(),
                "east".to_string(),
                "east".to_string()
            ])
        );
    }

    #[test]
    fn test_multiple_directions() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("4n 5w");
        assert_eq!(
            result,
            Some(vec![
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string()
            ])
        );
    }

    #[test]
    fn test_mixed_formats() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("3n 2ne u");
        assert_eq!(
            result,
            Some(vec![
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "northeast".to_string(),
                "northeast".to_string(),
                "up".to_string()
            ])
        );
    }

    #[test]
    fn test_non_speedwalk_command() {
        let sw = Speedwalk::new();

        assert_eq!(sw.try_expand("look"), None);
        assert_eq!(sw.try_expand("say hello"), None);
        assert_eq!(sw.try_expand("kill orc"), None);
        assert_eq!(sw.try_expand("get sword"), None);
    }

    #[test]
    fn test_invalid_patterns() {
        let sw = Speedwalk::new();

        // Invalid direction
        assert_eq!(sw.try_expand("4x"), None);

        // Zero count
        assert_eq!(sw.try_expand("0n"), None);

        // Empty
        assert_eq!(sw.try_expand(""), None);
    }

    #[test]
    fn test_large_count() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("50n");
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 50);
    }

    #[test]
    fn test_excessive_count() {
        let sw = Speedwalk::new();

        // Over 1000 should be rejected
        assert_eq!(sw.try_expand("1001n"), None);
    }

    #[test]
    fn test_disabled_speedwalk() {
        let mut sw = Speedwalk::new();
        sw.set_enabled(false);

        assert_eq!(sw.try_expand("4n"), None);
        assert!(!sw.is_enabled());
    }

    #[test]
    fn test_custom_direction() {
        let mut sw = Speedwalk::new();

        // Add custom direction
        sw.add_direction("in", "enter");

        let result = sw.try_expand("3in");
        assert_eq!(
            result,
            Some(vec![
                "enter".to_string(),
                "enter".to_string(),
                "enter".to_string()
            ])
        );
    }

    #[test]
    fn test_remove_direction() {
        let mut sw = Speedwalk::new();

        // Remove north
        assert!(sw.remove_direction("n"));

        // Should no longer work
        assert_eq!(sw.try_expand("n"), None);
        assert_eq!(sw.try_expand("4n"), None);
    }

    #[test]
    fn test_vertical_directions() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("3u 2d");
        assert_eq!(
            result,
            Some(vec![
                "up".to_string(),
                "up".to_string(),
                "up".to_string(),
                "down".to_string(),
                "down".to_string()
            ])
        );
    }

    #[test]
    fn test_single_digit_no_direction() {
        let sw = Speedwalk::new();

        // Just a number is not a speedwalk command
        assert_eq!(sw.try_expand("4"), None);
        assert_eq!(sw.try_expand("123"), None);
    }

    #[test]
    fn test_whitespace_handling() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("  4n  5w  ");
        assert_eq!(
            result,
            Some(vec![
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "north".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string(),
                "west".to_string()
            ])
        );
    }

    #[test]
    fn test_complex_path() {
        let sw = Speedwalk::new();

        let result = sw.try_expand("2n 3e 1s 2w u");
        assert!(result.is_some());
        let commands = result.unwrap();
        assert_eq!(commands.len(), 2 + 3 + 1 + 2 + 1);
        assert_eq!(commands[0], "north");
        assert_eq!(commands[1], "north");
        assert_eq!(commands[2], "east");
        assert_eq!(commands[8], "up");
    }

    #[test]
    fn test_serialization() {
        let config = SpeedwalkConfig::default();
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
    }

    #[test]
    fn test_deserialization() {
        let json = r#"{"enabled":true,"direction_map":{"n":"north","s":"south"}}"#;
        let config: SpeedwalkConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.direction_map.len(), 2);
    }
}

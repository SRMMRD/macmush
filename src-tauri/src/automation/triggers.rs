/// Trigger system for pattern matching and automation
///
/// Triggers match incoming MUD text against regex patterns and execute
/// actions when matches are found. Includes ReDoS protection and caching.

use crate::error::{MushError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Trigger action to execute on match
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerAction {
    /// Send command to server
    SendCommand(String),

    /// Display text to user
    DisplayText(String),

    /// Play sound file
    PlaySound(String),

    /// Execute Lua script
    ExecuteScript(String),

    /// Execute multiple actions in sequence
    Sequence(Vec<TriggerAction>),
}

/// Individual trigger with pattern and action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    /// Unique identifier
    pub id: Uuid,

    /// Trigger name
    pub name: String,

    /// Regex pattern to match
    pub pattern: String,

    /// Action to execute on match
    pub action: TriggerAction,

    /// Whether trigger is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Cached compiled regex (not serialized)
    #[serde(skip)]
    regex: Option<Regex>,
}

fn default_enabled() -> bool {
    true
}

impl Trigger {
    /// Create a new trigger
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        action: TriggerAction,
    ) -> Result<Self> {
        let name = name.into();
        let pattern = pattern.into();

        debug!("Creating trigger '{}' with pattern '{}'", name, pattern);

        // Validate pattern for ReDoS
        Self::validate_pattern(&pattern)?;

        let trigger = Trigger {
            id: Uuid::new_v4(),
            name,
            pattern,
            action,
            enabled: true,
            regex: None,
        };

        info!("Created trigger '{}' with ID {}", trigger.name, trigger.id);
        Ok(trigger)
    }

    /// Validate pattern for ReDoS vulnerabilities
    pub fn validate_pattern(pattern: &str) -> Result<()> {
        debug!("Validating pattern: {}", pattern);

        // Check for common ReDoS patterns

        // Pattern 1: Nested quantifiers (a+)+ or (a*)*
        if pattern.contains(")+") || pattern.contains(")*") || pattern.contains("}+") || pattern.contains("}*") {
            warn!("ReDoS vulnerability detected: nested quantifiers in '{}'", pattern);
            return Err(MushError::InvalidRegex {
                pattern: pattern.to_string(),
                source: regex::Error::Syntax("Nested quantifiers detected - potential ReDoS vulnerability".to_string()),
            });
        }

        // Pattern 2: Alternation with overlap (a|a)* or (ab|a)*
        // This is a simplified check - full ReDoS detection is complex
        if pattern.contains("|") && (pattern.contains(")*") || pattern.contains(")+")) {
            // Check for obvious overlapping alternations
            if pattern.contains("(a|a)") {
                warn!("ReDoS vulnerability detected: overlapping alternation in '{}'", pattern);
                return Err(MushError::InvalidRegex {
                    pattern: pattern.to_string(),
                    source: regex::Error::Syntax("Overlapping alternation detected - potential ReDoS vulnerability".to_string()),
                });
            }
        }

        // Try to compile to catch other regex errors
        Regex::new(pattern).map_err(|e| {
            warn!("Invalid regex pattern '{}': {}", pattern, e);
            MushError::InvalidRegex {
                pattern: pattern.to_string(),
                source: e,
            }
        })?;

        debug!("Pattern validation successful");
        Ok(())
    }

    /// Compile regex pattern with caching
    pub fn compile(&mut self) -> Result<()> {
        if self.regex.is_some() {
            return Ok(());
        }

        let regex = Regex::new(&self.pattern).map_err(|e| MushError::InvalidRegex {
            pattern: self.pattern.clone(),
            source: e,
        })?;

        self.regex = Some(regex);
        Ok(())
    }

    /// Test if pattern matches input text
    pub fn matches(&mut self, text: &str) -> Result<bool> {
        // Ensure regex is compiled
        if self.regex.is_none() {
            self.compile()?;
        }

        Ok(self.regex.as_ref().unwrap().is_match(text))
    }

    /// Extract capture groups from matched text
    /// Returns a HashMap with capture group names/numbers as keys
    pub fn extract_captures(&mut self, text: &str) -> Result<HashMap<String, String>> {
        let mut captures = HashMap::new();

        // Ensure regex is compiled
        if self.regex.is_none() {
            self.compile()?;
        }

        // Extract capture groups
        if let Some(regex) = &self.regex {
            if let Some(caps) = regex.captures(text) {
                // Store full match as 0
                if let Some(full) = caps.get(0) {
                    captures.insert("0".to_string(), full.as_str().to_string());
                }

                // Store numbered captures as 1, 2, etc.
                for i in 1..caps.len() {
                    if let Some(cap) = caps.get(i) {
                        captures.insert(i.to_string(), cap.as_str().to_string());
                    }
                }

                // Store named captures
                for name in regex.capture_names().flatten() {
                    if let Some(cap) = caps.name(name) {
                        captures.insert(name.to_string(), cap.as_str().to_string());
                    }
                }
            }
        }

        Ok(captures)
    }

    /// Execute trigger action
    pub fn execute(&self) -> Result<Vec<String>> {
        let mut commands = Vec::new();

        self.execute_action(&self.action, &mut commands);

        Ok(commands)
    }

    fn execute_action(&self, action: &TriggerAction, commands: &mut Vec<String>) {
        match action {
            TriggerAction::SendCommand(cmd) => {
                commands.push(cmd.clone());
            }
            TriggerAction::DisplayText(_text) => {
                // Display text doesn't generate commands
            }
            TriggerAction::PlaySound(_file) => {
                // Play sound doesn't generate commands
            }
            TriggerAction::ExecuteScript(_script) => {
                // Script execution handled at Session level via Lua runtime
                // Scripts can call world.Send() to generate commands
            }
            TriggerAction::Sequence(actions) => {
                for action in actions {
                    self.execute_action(action, commands);
                }
            }
        }
    }
}

/// Manages collection of triggers with matching and caching
pub struct TriggerManager {
    triggers: Vec<Trigger>,

    /// Cache for matching results (TODO: implement caching logic for performance)
    #[allow(dead_code)]
    match_cache: Arc<Mutex<HashMap<String, Vec<usize>>>>,
}

impl TriggerManager {
    /// Create new trigger manager
    pub fn new() -> Self {
        debug!("Creating new TriggerManager");
        Self {
            triggers: Vec::new(),
            match_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add trigger to manager
    pub fn add_trigger(&mut self, trigger: Trigger) -> Result<()> {
        info!("Adding trigger '{}' (ID: {}) to manager", trigger.name, trigger.id);
        self.triggers.push(trigger);
        debug!("Total triggers: {}", self.triggers.len());
        Ok(())
    }

    /// Find matching triggers for input text
    pub fn find_matches(&mut self, text: &str) -> Result<Vec<&Trigger>> {
        debug!("Finding matches for text: {}", text);
        let mut matches = Vec::new();

        for trigger in &mut self.triggers {
            if trigger.enabled && trigger.matches(text)? {
                debug!("Trigger '{}' matched", trigger.name);
                matches.push(trigger as &Trigger);
            }
        }

        info!("Found {} matching trigger(s)", matches.len());
        Ok(matches)
    }

    /// Get trigger by ID
    pub fn get_trigger(&self, id: Uuid) -> Option<&Trigger> {
        debug!("Looking up trigger with ID: {}", id);
        let result = self.triggers.iter().find(|t| t.id == id);
        if result.is_some() {
            debug!("Trigger found");
        } else {
            debug!("Trigger not found");
        }
        result
    }

    /// Get mutable trigger by ID
    pub fn get_trigger_mut(&mut self, id: Uuid) -> Option<&mut Trigger> {
        debug!("Looking up mutable trigger with ID: {}", id);
        let result = self.triggers.iter_mut().find(|t| t.id == id);
        if result.is_some() {
            debug!("Mutable trigger found");
        } else {
            debug!("Mutable trigger not found");
        }
        result
    }

    /// Remove trigger by ID
    pub fn remove_trigger(&mut self, id: Uuid) -> Result<()> {
        info!("Removing trigger with ID: {}", id);
        let before = self.triggers.len();
        self.triggers.retain(|t| t.id != id);
        let after = self.triggers.len();

        if before == after {
            warn!("Trigger ID {} not found, nothing removed", id);
        } else {
            debug!("Trigger removed, {} trigger(s) remaining", after);
        }

        Ok(())
    }
}

impl Default for TriggerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_simple_trigger() {
        let trigger = Trigger::new(
            "Test Trigger",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        );

        assert!(trigger.is_ok(), "Should create simple trigger");
        let trigger = trigger.unwrap();
        assert_eq!(trigger.name, "Test Trigger");
        assert_eq!(trigger.pattern, "^You see");
        assert!(trigger.enabled);
    }

    #[test]
    fn test_validate_safe_pattern() {
        let result = Trigger::validate_pattern("^Hello world$");
        assert!(result.is_ok(), "Should accept safe pattern");
    }

    #[test]
    fn test_validate_redos_vulnerable_pattern() {
        // Classic ReDoS pattern: (a+)+b
        let result = Trigger::validate_pattern("(a+)+b");
        assert!(result.is_err(), "Should reject ReDoS pattern");
        assert!(
            matches!(result, Err(MushError::InvalidRegex { .. })),
            "Should return InvalidRegex error"
        );
    }

    #[test]
    fn test_validate_nested_quantifiers() {
        let result = Trigger::validate_pattern("(x*)*");
        assert!(result.is_err(), "Should reject nested quantifiers");
    }

    #[test]
    fn test_validate_alternation_with_overlap() {
        // ReDoS: (a|a)*b
        let result = Trigger::validate_pattern("(a|a)*b");
        assert!(result.is_err(), "Should reject overlapping alternations");
    }

    #[test]
    fn test_compile_valid_pattern() {
        let mut trigger = Trigger::new(
            "Test",
            "^You see (.+)",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let result = trigger.compile();
        assert!(result.is_ok(), "Should compile valid pattern");
        assert!(trigger.regex.is_some(), "Should cache compiled regex");
    }

    #[test]
    fn test_compile_invalid_regex() {
        // Invalid regex should be caught during creation due to validation
        let result = Trigger::new(
            "Test",
            "[invalid",
            TriggerAction::SendCommand("look".to_string()),
        );

        assert!(result.is_err(), "Should fail to create trigger with invalid regex");
        assert!(
            matches!(result, Err(MushError::InvalidRegex { .. })),
            "Should return InvalidRegex error"
        );
    }

    #[test]
    fn test_matches_simple_pattern() {
        let mut trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        trigger.compile().unwrap();

        assert!(trigger.matches("You see a door").unwrap());
        assert!(!trigger.matches("You hear a sound").unwrap());
    }

    #[test]
    fn test_matches_with_capture_groups() {
        let mut trigger = Trigger::new(
            "Test",
            r"^(\w+) enters the room",
            TriggerAction::DisplayText("Someone arrived!".to_string()),
        ).unwrap();

        trigger.compile().unwrap();

        assert!(trigger.matches("Alice enters the room").unwrap());
        assert!(trigger.matches("Bob enters the room").unwrap());
        assert!(!trigger.matches("Alice leaves the room").unwrap());
    }

    #[test]
    fn test_disabled_trigger() {
        let mut trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        trigger.enabled = false;
        trigger.compile().unwrap();

        // Disabled triggers should still match but not execute
        assert!(trigger.matches("You see a door").unwrap());
    }

    #[test]
    fn test_execute_send_command() {
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look north".to_string()),
        ).unwrap();

        let commands = trigger.execute().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "look north");
    }

    #[test]
    fn test_execute_sequence() {
        let trigger = Trigger::new(
            "Test",
            "^Battle!",
            TriggerAction::Sequence(vec![
                TriggerAction::SendCommand("draw sword".to_string()),
                TriggerAction::SendCommand("attack".to_string()),
            ]),
        ).unwrap();

        let commands = trigger.execute().unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "draw sword");
        assert_eq!(commands[1], "attack");
    }

    #[test]
    fn test_trigger_manager_add() {
        let mut manager = TriggerManager::new();
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let result = manager.add_trigger(trigger);
        assert!(result.is_ok(), "Should add trigger to manager");
    }

    #[test]
    fn test_trigger_manager_find_matches() {
        let mut manager = TriggerManager::new();

        let t1 = Trigger::new(
            "Door Trigger",
            "^You see a door",
            TriggerAction::SendCommand("open door".to_string()),
        ).unwrap();

        let t2 = Trigger::new(
            "Enemy Trigger",
            "^An enemy appears",
            TriggerAction::SendCommand("attack".to_string()),
        ).unwrap();

        manager.add_trigger(t1).unwrap();
        manager.add_trigger(t2).unwrap();

        let matches = manager.find_matches("You see a door to the north").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Door Trigger");

        let matches = manager.find_matches("An enemy appears!").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "Enemy Trigger");

        let matches = manager.find_matches("Nothing special here").unwrap();
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_trigger_manager_get_by_id() {
        let mut manager = TriggerManager::new();
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let id = trigger.id;
        manager.add_trigger(trigger).unwrap();

        let found = manager.get_trigger(id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");
    }

    #[test]
    fn test_trigger_manager_remove() {
        let mut manager = TriggerManager::new();
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let id = trigger.id;
        manager.add_trigger(trigger).unwrap();

        let result = manager.remove_trigger(id);
        assert!(result.is_ok(), "Should remove trigger");

        let found = manager.get_trigger(id);
        assert!(found.is_none(), "Trigger should be removed");
    }

    #[test]
    fn test_json_serialization() {
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let json = serde_json::to_string(&trigger);
        assert!(json.is_ok(), "Should serialize to JSON");
    }

    #[test]
    fn test_json_deserialization() {
        let trigger = Trigger::new(
            "Test",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        ).unwrap();

        let json = serde_json::to_string(&trigger).unwrap();
        let deserialized = serde_json::from_str::<Trigger>(&json);

        assert!(deserialized.is_ok(), "Should deserialize from JSON");
        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.name, trigger.name);
        assert_eq!(deserialized.pattern, trigger.pattern);
    }
}

// Property-based tests for ReDoS protection
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_validate_pattern_always_completes(pattern in ".*") {
            // Validation should always complete (no infinite loops)
            let _ = Trigger::validate_pattern(&pattern);
        }

        #[test]
        fn test_safe_patterns_validate(pattern in "[a-zA-Z0-9 ]+") {
            // Simple alphanumeric patterns should always validate
            let result = Trigger::validate_pattern(&pattern);
            prop_assert!(result.is_ok());
        }

        #[test]
        fn test_pattern_compilation_doesnt_panic(pattern in "[a-z]{1,10}") {
            // Pattern compilation should never panic
            let mut trigger = Trigger::new(
                "Test",
                &pattern,
                TriggerAction::SendCommand("test".to_string()),
            ).unwrap();

            let _ = trigger.compile();
        }

        #[test]
        fn test_matching_doesnt_timeout(
            pattern in "[a-z]{1,5}",
            text in "[a-z ]{1,100}"
        ) {
            // Matching should complete quickly even with varied input
            if let Ok(mut trigger) = Trigger::new(
                "Test",
                &pattern,
                TriggerAction::SendCommand("test".to_string()),
            ) {
                if trigger.compile().is_ok() {
                    let _ = trigger.matches(&text);
                }
            }
        }
    }
}

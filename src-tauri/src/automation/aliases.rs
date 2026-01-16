/// Alias system for command shortcuts and input automation
///
/// Aliases match user input against regex patterns and execute actions,
/// providing command shortcuts, macro expansion, and Lua script execution.

use crate::error::{MushError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Alias action to execute on match
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AliasAction {
    /// Send command to server
    SendCommand(String),

    /// Send multiple commands in sequence
    SendCommands(Vec<String>),

    /// Execute Lua script
    ExecuteScript(String),

    /// Execute multiple actions in sequence
    Sequence(Vec<AliasAction>),
}

/// Individual alias with pattern and action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    /// Unique identifier
    pub id: Uuid,

    /// Alias name
    pub name: String,

    /// Regex pattern to match (supports capture groups)
    pub pattern: String,

    /// Action to execute on match
    pub action: AliasAction,

    /// Whether alias is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Cached compiled regex (not serialized)
    #[serde(skip)]
    regex: Option<Regex>,
}

fn default_enabled() -> bool {
    true
}

impl Alias {
    /// Create a new alias
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        action: AliasAction,
    ) -> Result<Self> {
        let name = name.into();
        let pattern = pattern.into();

        debug!("Creating alias '{}' with pattern '{}'", name, pattern);

        // Validate pattern
        Self::validate_pattern(&pattern)?;

        let alias = Alias {
            id: Uuid::new_v4(),
            name,
            pattern,
            action,
            enabled: true,
            regex: None,
        };

        info!("Created alias '{}' with ID {}", alias.name, alias.id);
        Ok(alias)
    }

    /// Validate pattern for basic regex correctness
    pub fn validate_pattern(pattern: &str) -> Result<()> {
        debug!("Validating pattern: {}", pattern);

        // Try to compile to catch regex errors
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

    /// Execute alias action and return commands to send
    /// Also returns capture groups for Lua script access
    pub fn execute(&mut self, input: &str) -> Result<(Vec<String>, HashMap<String, String>)> {
        let mut commands = Vec::new();
        let mut captures = HashMap::new();

        // Ensure regex is compiled
        if self.regex.is_none() {
            self.compile()?;
        }

        // Extract capture groups
        if let Some(regex) = &self.regex {
            if let Some(caps) = regex.captures(input) {
                // Store full match as %0
                if let Some(full) = caps.get(0) {
                    captures.insert("0".to_string(), full.as_str().to_string());
                }

                // Store numbered captures as %1, %2, etc.
                for i in 1..caps.len() {
                    if let Some(cap) = caps.get(i) {
                        captures.insert(i.to_string(), cap.as_str().to_string());
                    }
                }
            }
        }

        self.execute_action(&self.action.clone(), &mut commands, &captures);

        Ok((commands, captures))
    }

    fn execute_action(
        &self,
        action: &AliasAction,
        commands: &mut Vec<String>,
        captures: &HashMap<String, String>,
    ) {
        match action {
            AliasAction::SendCommand(cmd) => {
                // Replace %1, %2, etc. with capture groups
                let expanded = self.expand_wildcards(cmd, captures);
                commands.push(expanded);
            }
            AliasAction::SendCommands(cmds) => {
                for cmd in cmds {
                    let expanded = self.expand_wildcards(cmd, captures);
                    commands.push(expanded);
                }
            }
            AliasAction::ExecuteScript(_script) => {
                // Script execution handled at Session level via Lua runtime
                // Scripts can access captures via world.GetVariable()
            }
            AliasAction::Sequence(actions) => {
                for action in actions {
                    self.execute_action(action, commands, captures);
                }
            }
        }
    }

    /// Expand wildcard placeholders (%1, %2, etc.) with capture groups
    fn expand_wildcards(&self, text: &str, captures: &HashMap<String, String>) -> String {
        let mut result = text.to_string();

        // Replace %0 through %9 with corresponding captures
        for i in 0..=9 {
            let placeholder = format!("%{}", i);
            if let Some(value) = captures.get(&i.to_string()) {
                result = result.replace(&placeholder, value);
            }
        }

        result
    }
}

/// Manages collection of aliases with matching and caching
pub struct AliasManager {
    aliases: Vec<Alias>,

    /// Cache for matching results (TODO: implement caching logic for performance)
    #[allow(dead_code)]
    match_cache: Arc<Mutex<HashMap<String, Vec<usize>>>>,
}

impl AliasManager {
    /// Create new alias manager
    pub fn new() -> Self {
        debug!("Creating new AliasManager");
        Self {
            aliases: Vec::new(),
            match_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add alias to manager
    pub fn add_alias(&mut self, alias: Alias) -> Result<()> {
        info!("Adding alias '{}' (ID: {}) to manager", alias.name, alias.id);
        self.aliases.push(alias);
        debug!("Total aliases: {}", self.aliases.len());
        Ok(())
    }

    /// Find matching alias for input text (returns first match only)
    pub fn find_match(&mut self, text: &str) -> Result<Option<&mut Alias>> {
        debug!("Finding match for input: {}", text);

        for alias in &mut self.aliases {
            if alias.enabled && alias.matches(text)? {
                debug!("Alias '{}' matched", alias.name);
                return Ok(Some(alias));
            }
        }

        debug!("No matching alias found");
        Ok(None)
    }

    /// Get alias by ID
    pub fn get_alias(&self, id: Uuid) -> Option<&Alias> {
        debug!("Looking up alias with ID: {}", id);
        let result = self.aliases.iter().find(|a| a.id == id);
        if result.is_some() {
            debug!("Alias found");
        } else {
            debug!("Alias not found");
        }
        result
    }

    /// Remove alias by ID
    pub fn remove_alias(&mut self, id: Uuid) -> Result<()> {
        info!("Removing alias with ID: {}", id);
        let before = self.aliases.len();
        self.aliases.retain(|a| a.id != id);
        let after = self.aliases.len();

        if before == after {
            warn!("Alias ID {} not found, nothing removed", id);
        } else {
            debug!("Alias removed, {} alias(es) remaining", after);
        }

        Ok(())
    }
}

impl Default for AliasManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_simple_alias() {
        let alias = Alias::new(
            "Get Gold",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        );

        assert!(alias.is_ok(), "Should create simple alias");
        let alias = alias.unwrap();
        assert_eq!(alias.name, "Get Gold");
        assert_eq!(alias.pattern, "^gg$");
        assert!(alias.enabled);
    }

    #[test]
    fn test_validate_safe_pattern() {
        let result = Alias::validate_pattern("^test$");
        assert!(result.is_ok(), "Should accept safe pattern");
    }

    #[test]
    fn test_validate_invalid_regex() {
        let result = Alias::validate_pattern("[invalid");
        assert!(result.is_err(), "Should reject invalid regex");
    }

    #[test]
    fn test_compile_valid_pattern() {
        let mut alias = Alias::new(
            "Test",
            "^get (.+)",
            AliasAction::SendCommand("take %1".to_string()),
        )
        .unwrap();

        let result = alias.compile();
        assert!(result.is_ok(), "Should compile valid pattern");
        assert!(alias.regex.is_some(), "Should cache compiled regex");
    }

    #[test]
    fn test_matches_simple_pattern() {
        let mut alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        alias.compile().unwrap();

        assert!(alias.matches("gg").unwrap());
        assert!(!alias.matches("get gold").unwrap());
    }

    #[test]
    fn test_matches_with_wildcard() {
        let mut alias = Alias::new(
            "Test",
            r"^get (.+)",
            AliasAction::SendCommand("take %1".to_string()),
        )
        .unwrap();

        alias.compile().unwrap();

        assert!(alias.matches("get sword").unwrap());
        assert!(alias.matches("get all").unwrap());
        assert!(!alias.matches("take sword").unwrap());
    }

    #[test]
    fn test_execute_simple_command() {
        let mut alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        let (commands, _) = alias.execute("gg").unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "get gold");
    }

    #[test]
    fn test_execute_with_wildcard_substitution() {
        let mut alias = Alias::new(
            "Test",
            r"^get (.+)",
            AliasAction::SendCommand("take %1".to_string()),
        )
        .unwrap();

        let (commands, captures) = alias.execute("get sword").unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "take sword");
        assert_eq!(captures.get("1"), Some(&"sword".to_string()));
    }

    #[test]
    fn test_execute_multiple_captures() {
        let mut alias = Alias::new(
            "Test",
            r"^give (\w+) to (\w+)",
            AliasAction::SendCommand("give %2 %1".to_string()),
        )
        .unwrap();

        let (commands, captures) = alias.execute("give sword to guard").unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "give guard sword");
        assert_eq!(captures.get("1"), Some(&"sword".to_string()));
        assert_eq!(captures.get("2"), Some(&"guard".to_string()));
    }

    #[test]
    fn test_execute_multiple_commands() {
        let mut alias = Alias::new(
            "Test",
            "^loot$",
            AliasAction::SendCommands(vec![
                "get gold".to_string(),
                "get silver".to_string(),
            ]),
        )
        .unwrap();

        let (commands, _) = alias.execute("loot").unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "get gold");
        assert_eq!(commands[1], "get silver");
    }

    #[test]
    fn test_alias_manager_add() {
        let mut manager = AliasManager::new();
        let alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        let result = manager.add_alias(alias);
        assert!(result.is_ok(), "Should add alias to manager");
    }

    #[test]
    fn test_alias_manager_find_match() {
        let mut manager = AliasManager::new();

        let a1 = Alias::new(
            "Get Gold",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        let a2 = Alias::new(
            "Get All",
            "^ga$",
            AliasAction::SendCommand("get all".to_string()),
        )
        .unwrap();

        manager.add_alias(a1).unwrap();
        manager.add_alias(a2).unwrap();

        let matched = manager.find_match("gg").unwrap();
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "Get Gold");

        let no_match = manager.find_match("xyz").unwrap();
        assert!(no_match.is_none());
    }

    #[test]
    fn test_alias_manager_get_by_id() {
        let mut manager = AliasManager::new();
        let alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        let id = alias.id;
        manager.add_alias(alias).unwrap();

        let found = manager.get_alias(id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");
    }

    #[test]
    fn test_alias_manager_remove() {
        let mut manager = AliasManager::new();
        let alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        let id = alias.id;
        manager.add_alias(alias).unwrap();

        let result = manager.remove_alias(id);
        assert!(result.is_ok(), "Should remove alias");

        let found = manager.get_alias(id);
        assert!(found.is_none(), "Alias should be removed");
    }

    #[test]
    fn test_disabled_alias() {
        let mut alias = Alias::new(
            "Test",
            "^gg$",
            AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();

        alias.enabled = false;
        alias.compile().unwrap();

        // Disabled aliases still match but won't execute
        assert!(alias.matches("gg").unwrap());
    }
}

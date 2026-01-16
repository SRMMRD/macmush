/// Variable storage system for game state and automation
///
/// Variables provide persistent storage for game state, trigger captures,
/// and script data, enabling complex automation and state tracking.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Individual variable with name and value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variable {
    /// Variable name (case-insensitive)
    pub name: String,

    /// Variable value (stored as string)
    pub value: String,
}

impl Variable {
    /// Create a new variable
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        let value = value.into();

        debug!("Creating variable '{}' = '{}'", name, value);

        Self { name, value }
    }

    /// Get the variable name in lowercase for case-insensitive comparison
    pub fn normalized_name(&self) -> String {
        self.name.to_lowercase()
    }
}

/// Manages collection of variables with case-insensitive lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableManager {
    /// Variables stored with lowercase keys for case-insensitive access
    #[serde(default)]
    variables: HashMap<String, Variable>,
}

impl VariableManager {
    /// Create a new variable manager
    pub fn new() -> Self {
        debug!("Creating new VariableManager");
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set a variable (creates if doesn't exist, updates if exists)
    pub fn set_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        let key = name.to_lowercase();

        let variable = Variable::new(name.clone(), value);

        if self.variables.contains_key(&key) {
            debug!("Updating variable '{}'", name);
        } else {
            debug!("Creating variable '{}'", name);
        }

        self.variables.insert(key, variable);
        info!("Variable '{}' set", name);
    }

    /// Get a variable value (case-insensitive lookup)
    pub fn get_variable(&self, name: impl AsRef<str>) -> Option<String> {
        let key = name.as_ref().to_lowercase();

        if let Some(var) = self.variables.get(&key) {
            debug!("Retrieved variable '{}' = '{}'", name.as_ref(), var.value);
            Some(var.value.clone())
        } else {
            debug!("Variable '{}' not found", name.as_ref());
            None
        }
    }

    /// Delete a variable (case-insensitive)
    pub fn delete_variable(&mut self, name: impl AsRef<str>) -> bool {
        let key = name.as_ref().to_lowercase();

        if self.variables.remove(&key).is_some() {
            info!("Deleted variable '{}'", name.as_ref());
            true
        } else {
            debug!("Variable '{}' not found for deletion", name.as_ref());
            false
        }
    }

    /// Check if a variable exists (case-insensitive)
    pub fn has_variable(&self, name: impl AsRef<str>) -> bool {
        let key = name.as_ref().to_lowercase();
        self.variables.contains_key(&key)
    }

    /// Get list of all variable names
    pub fn get_variable_list(&self) -> Vec<String> {
        let names: Vec<String> = self.variables.values()
            .map(|v| v.name.clone())
            .collect();

        debug!("Returning {} variable names", names.len());
        names
    }

    /// Get all variables as a map
    pub fn get_all_variables(&self) -> HashMap<String, String> {
        self.variables.iter()
            .map(|(_, v)| (v.name.clone(), v.value.clone()))
            .collect()
    }

    /// Clear all variables
    pub fn clear_all(&mut self) {
        let count = self.variables.len();
        self.variables.clear();
        info!("Cleared {} variables", count);
    }

    /// Get number of variables
    pub fn count(&self) -> usize {
        self.variables.len()
    }

    /// Set multiple variables at once (useful for trigger captures)
    pub fn set_variables(&mut self, vars: HashMap<String, String>) {
        for (name, value) in vars {
            self.set_variable(name, value);
        }
    }
}

impl Default for VariableManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_variable() {
        let var = Variable::new("health", "100");
        assert_eq!(var.name, "health");
        assert_eq!(var.value, "100");
    }

    #[test]
    fn test_variable_normalized_name() {
        let var1 = Variable::new("Health", "100");
        let var2 = Variable::new("HEALTH", "100");
        let var3 = Variable::new("health", "100");

        assert_eq!(var1.normalized_name(), "health");
        assert_eq!(var2.normalized_name(), "health");
        assert_eq!(var3.normalized_name(), "health");
    }

    #[test]
    fn test_set_and_get_variable() {
        let mut manager = VariableManager::new();

        manager.set_variable("player_name", "Alice");

        let value = manager.get_variable("player_name");
        assert_eq!(value, Some("Alice".to_string()));
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let mut manager = VariableManager::new();

        manager.set_variable("PlayerName", "Bob");

        // All these should retrieve the same variable
        assert_eq!(manager.get_variable("playername"), Some("Bob".to_string()));
        assert_eq!(manager.get_variable("PLAYERNAME"), Some("Bob".to_string()));
        assert_eq!(manager.get_variable("PlayerName"), Some("Bob".to_string()));
    }

    #[test]
    fn test_update_existing_variable() {
        let mut manager = VariableManager::new();

        manager.set_variable("score", "100");
        assert_eq!(manager.get_variable("score"), Some("100".to_string()));

        manager.set_variable("score", "200");
        assert_eq!(manager.get_variable("score"), Some("200".to_string()));

        // Should only have one variable
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_delete_variable() {
        let mut manager = VariableManager::new();

        manager.set_variable("temp", "value");
        assert!(manager.has_variable("temp"));

        let deleted = manager.delete_variable("temp");
        assert!(deleted, "Should successfully delete variable");
        assert!(!manager.has_variable("temp"));
        assert_eq!(manager.get_variable("temp"), None);
    }

    #[test]
    fn test_delete_nonexistent_variable() {
        let mut manager = VariableManager::new();

        let deleted = manager.delete_variable("nonexistent");
        assert!(!deleted, "Should return false for nonexistent variable");
    }

    #[test]
    fn test_has_variable() {
        let mut manager = VariableManager::new();

        assert!(!manager.has_variable("test"));

        manager.set_variable("test", "value");
        assert!(manager.has_variable("test"));
        assert!(manager.has_variable("TEST")); // Case-insensitive
    }

    #[test]
    fn test_get_variable_list() {
        let mut manager = VariableManager::new();

        assert_eq!(manager.get_variable_list().len(), 0);

        manager.set_variable("var1", "value1");
        manager.set_variable("var2", "value2");
        manager.set_variable("var3", "value3");

        let list = manager.get_variable_list();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&"var1".to_string()));
        assert!(list.contains(&"var2".to_string()));
        assert!(list.contains(&"var3".to_string()));
    }

    #[test]
    fn test_get_all_variables() {
        let mut manager = VariableManager::new();

        manager.set_variable("name", "Alice");
        manager.set_variable("age", "25");
        manager.set_variable("city", "Portland");

        let all = manager.get_all_variables();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("name"), Some(&"Alice".to_string()));
        assert_eq!(all.get("age"), Some(&"25".to_string()));
        assert_eq!(all.get("city"), Some(&"Portland".to_string()));
    }

    #[test]
    fn test_clear_all() {
        let mut manager = VariableManager::new();

        manager.set_variable("var1", "value1");
        manager.set_variable("var2", "value2");
        manager.set_variable("var3", "value3");

        assert_eq!(manager.count(), 3);

        manager.clear_all();

        assert_eq!(manager.count(), 0);
        assert_eq!(manager.get_variable_list().len(), 0);
    }

    #[test]
    fn test_count() {
        let mut manager = VariableManager::new();

        assert_eq!(manager.count(), 0);

        manager.set_variable("var1", "value1");
        assert_eq!(manager.count(), 1);

        manager.set_variable("var2", "value2");
        assert_eq!(manager.count(), 2);

        manager.delete_variable("var1");
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_set_variables_batch() {
        let mut manager = VariableManager::new();

        let mut vars = HashMap::new();
        vars.insert("hp".to_string(), "100".to_string());
        vars.insert("mp".to_string(), "50".to_string());
        vars.insert("level".to_string(), "5".to_string());

        manager.set_variables(vars);

        assert_eq!(manager.count(), 3);
        assert_eq!(manager.get_variable("hp"), Some("100".to_string()));
        assert_eq!(manager.get_variable("mp"), Some("50".to_string()));
        assert_eq!(manager.get_variable("level"), Some("5".to_string()));
    }

    #[test]
    fn test_json_serialization() {
        let mut manager = VariableManager::new();
        manager.set_variable("test", "value");

        let json = serde_json::to_string(&manager);
        assert!(json.is_ok(), "Should serialize to JSON");
    }

    #[test]
    fn test_json_deserialization() {
        let mut manager = VariableManager::new();
        manager.set_variable("name", "Alice");
        manager.set_variable("score", "100");

        let json = serde_json::to_string(&manager).unwrap();
        let deserialized: VariableManager = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.count(), 2);
        assert_eq!(deserialized.get_variable("name"), Some("Alice".to_string()));
        assert_eq!(deserialized.get_variable("score"), Some("100".to_string()));
    }

    #[test]
    fn test_empty_variable_value() {
        let mut manager = VariableManager::new();

        manager.set_variable("empty", "");

        assert!(manager.has_variable("empty"));
        assert_eq!(manager.get_variable("empty"), Some("".to_string()));
    }

    #[test]
    fn test_numeric_string_values() {
        let mut manager = VariableManager::new();

        manager.set_variable("integer", "42");
        manager.set_variable("float", "3.14");
        manager.set_variable("negative", "-10");

        assert_eq!(manager.get_variable("integer"), Some("42".to_string()));
        assert_eq!(manager.get_variable("float"), Some("3.14".to_string()));
        assert_eq!(manager.get_variable("negative"), Some("-10".to_string()));
    }

    #[test]
    fn test_special_characters_in_values() {
        let mut manager = VariableManager::new();

        manager.set_variable("path", "/usr/local/bin");
        manager.set_variable("email", "user@example.com");
        manager.set_variable("formula", "x + y = z");

        assert_eq!(manager.get_variable("path"), Some("/usr/local/bin".to_string()));
        assert_eq!(manager.get_variable("email"), Some("user@example.com".to_string()));
        assert_eq!(manager.get_variable("formula"), Some("x + y = z".to_string()));
    }
}

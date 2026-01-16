/// Command history system for recalling previous commands
///
/// Provides navigation through previously entered commands with up/down keys,
/// similar to shell history. Supports configurable history size and duplicate
/// handling.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, info};

/// Default maximum number of commands to store in history
const DEFAULT_MAX_HISTORY: usize = 1000;

/// Command history with navigation support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    /// Commands stored in chronological order (oldest first)
    #[serde(default)]
    commands: VecDeque<String>,

    /// Maximum number of commands to store
    #[serde(default = "default_max_history")]
    max_size: usize,

    /// Current position in history during navigation (None = at prompt)
    #[serde(skip)]
    current_position: Option<usize>,

    /// Temporary command being edited when navigating history
    #[serde(skip)]
    temp_command: Option<String>,
}

fn default_max_history() -> usize {
    DEFAULT_MAX_HISTORY
}

impl CommandHistory {
    /// Create a new command history with default size
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_HISTORY)
    }

    /// Create a new command history with specified maximum size
    pub fn with_max_size(max_size: usize) -> Self {
        debug!("Creating CommandHistory with max size {}", max_size);
        Self {
            commands: VecDeque::new(),
            max_size,
            current_position: None,
            temp_command: None,
        }
    }

    /// Add a command to history
    ///
    /// Empty commands are ignored. Duplicate consecutive commands are also ignored
    /// to prevent cluttering history with repeated commands.
    pub fn add_command(&mut self, command: impl Into<String>) {
        let command = command.into();

        // Ignore empty commands
        if command.trim().is_empty() {
            return;
        }

        // Ignore duplicate consecutive commands
        if let Some(last) = self.commands.back() {
            if last == &command {
                debug!("Ignoring duplicate command: {}", command);
                return;
            }
        }

        // Add to end of history
        self.commands.push_back(command.clone());

        // Trim if exceeds max size
        while self.commands.len() > self.max_size {
            self.commands.pop_front();
        }

        // Reset navigation position
        self.current_position = None;
        self.temp_command = None;

        debug!("Added command to history (total: {})", self.commands.len());
    }

    /// Get previous command (navigate up in history)
    ///
    /// If current_input is provided and we're not already navigating,
    /// it will be saved so it can be restored when pressing down.
    pub fn get_previous(&mut self, current_input: Option<String>) -> Option<String> {
        if self.commands.is_empty() {
            return None;
        }

        // If not currently navigating, save current input
        if self.current_position.is_none() {
            self.temp_command = current_input;
        }

        // Calculate new position (move backward/up in history)
        let new_position = match self.current_position {
            None => self.commands.len() - 1,
            Some(0) => 0, // Already at oldest command
            Some(pos) => pos - 1,
        };

        self.current_position = Some(new_position);
        let command = self.commands.get(new_position).cloned();

        if let Some(ref cmd) = command {
            debug!("Retrieved previous command at position {}: {}", new_position, cmd);
        }

        command
    }

    /// Get next command (navigate down in history)
    ///
    /// Returns None when reaching the end of history (back to current prompt).
    pub fn get_next(&mut self) -> Option<String> {
        match self.current_position {
            None => None, // Not navigating
            Some(pos) => {
                if pos >= self.commands.len() - 1 {
                    // Reached end of history, return to current prompt
                    self.current_position = None;
                    let temp = self.temp_command.take();
                    debug!("Reached end of history, returning to prompt");
                    temp
                } else {
                    // Move forward in history
                    let new_position = pos + 1;
                    self.current_position = Some(new_position);
                    let command = self.commands.get(new_position).cloned();

                    if let Some(ref cmd) = command {
                        debug!("Retrieved next command at position {}: {}", new_position, cmd);
                    }

                    command
                }
            }
        }
    }

    /// Reset navigation position (return to prompt)
    pub fn reset_position(&mut self) {
        self.current_position = None;
        self.temp_command = None;
        debug!("Reset history navigation position");
    }

    /// Get command at a specific index (0 = oldest)
    pub fn get_command(&self, index: usize) -> Option<&String> {
        self.commands.get(index)
    }

    /// Get the most recent command
    pub fn get_last_command(&self) -> Option<&String> {
        self.commands.back()
    }

    /// Get all commands in chronological order (oldest first)
    pub fn get_all_commands(&self) -> Vec<String> {
        self.commands.iter().cloned().collect()
    }

    /// Get number of commands in history
    pub fn count(&self) -> usize {
        self.commands.len()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        let count = self.commands.len();
        self.commands.clear();
        self.current_position = None;
        self.temp_command = None;
        info!("Cleared {} commands from history", count);
    }

    /// Set maximum history size
    ///
    /// If new size is smaller than current count, oldest commands are removed.
    pub fn set_max_size(&mut self, max_size: usize) {
        self.max_size = max_size;

        // Trim if necessary
        while self.commands.len() > self.max_size {
            self.commands.pop_front();
        }

        info!("Set history max size to {}", max_size);
    }

    /// Get maximum history size
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Check if currently navigating history
    pub fn is_navigating(&self) -> bool {
        self.current_position.is_some()
    }

    /// Get current navigation position (None = at prompt)
    pub fn current_position(&self) -> Option<usize> {
        self.current_position
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_command_history() {
        let history = CommandHistory::new();
        assert_eq!(history.count(), 0);
        assert_eq!(history.max_size(), DEFAULT_MAX_HISTORY);
    }

    #[test]
    fn test_create_with_custom_size() {
        let history = CommandHistory::with_max_size(100);
        assert_eq!(history.max_size(), 100);
    }

    #[test]
    fn test_add_command() {
        let mut history = CommandHistory::new();

        history.add_command("look");
        assert_eq!(history.count(), 1);
        assert_eq!(history.get_last_command(), Some(&"look".to_string()));
    }

    #[test]
    fn test_add_multiple_commands() {
        let mut history = CommandHistory::new();

        history.add_command("look");
        history.add_command("north");
        history.add_command("get sword");

        assert_eq!(history.count(), 3);
        assert_eq!(history.get_command(0), Some(&"look".to_string()));
        assert_eq!(history.get_command(1), Some(&"north".to_string()));
        assert_eq!(history.get_command(2), Some(&"get sword".to_string()));
    }

    #[test]
    fn test_ignore_empty_commands() {
        let mut history = CommandHistory::new();

        history.add_command("");
        history.add_command("   ");
        history.add_command("\t");

        assert_eq!(history.count(), 0);
    }

    #[test]
    fn test_ignore_duplicate_consecutive_commands() {
        let mut history = CommandHistory::new();

        history.add_command("look");
        history.add_command("look");
        history.add_command("look");

        assert_eq!(history.count(), 1);
    }

    #[test]
    fn test_allow_non_consecutive_duplicates() {
        let mut history = CommandHistory::new();

        history.add_command("look");
        history.add_command("north");
        history.add_command("look");

        assert_eq!(history.count(), 3);
    }

    #[test]
    fn test_navigate_previous() {
        let mut history = CommandHistory::new();

        history.add_command("command1");
        history.add_command("command2");
        history.add_command("command3");

        // First up arrow - get most recent
        assert_eq!(history.get_previous(None), Some("command3".to_string()));

        // Second up arrow - go back one
        assert_eq!(history.get_previous(None), Some("command2".to_string()));

        // Third up arrow - go back one more
        assert_eq!(history.get_previous(None), Some("command1".to_string()));

        // Fourth up arrow - stay at oldest
        assert_eq!(history.get_previous(None), Some("command1".to_string()));
    }

    #[test]
    fn test_navigate_next() {
        let mut history = CommandHistory::new();

        history.add_command("command1");
        history.add_command("command2");
        history.add_command("command3");

        // Navigate to oldest
        history.get_previous(None);
        history.get_previous(None);
        history.get_previous(None);

        // Navigate forward
        assert_eq!(history.get_next(), Some("command2".to_string()));
        assert_eq!(history.get_next(), Some("command3".to_string()));

        // Reached end of history
        assert_eq!(history.get_next(), None);
    }

    #[test]
    fn test_save_and_restore_current_input() {
        let mut history = CommandHistory::new();

        history.add_command("command1");
        history.add_command("command2");

        // Start typing something
        let current_input = "partial command".to_string();

        // Navigate up (should save current input)
        assert_eq!(history.get_previous(Some(current_input.clone())), Some("command2".to_string()));

        // Navigate down past end (should restore saved input)
        let restored = history.get_next();
        assert_eq!(restored, Some(current_input));
    }

    #[test]
    fn test_reset_position_after_add() {
        let mut history = CommandHistory::new();

        history.add_command("command1");
        history.get_previous(None); // Start navigating

        assert!(history.is_navigating());

        history.add_command("command2"); // Should reset position

        assert!(!history.is_navigating());
    }

    #[test]
    fn test_reset_position() {
        let mut history = CommandHistory::new();

        history.add_command("command1");
        history.get_previous(None);

        assert!(history.is_navigating());

        history.reset_position();

        assert!(!history.is_navigating());
    }

    #[test]
    fn test_max_size_limit() {
        let mut history = CommandHistory::with_max_size(3);

        history.add_command("cmd1");
        history.add_command("cmd2");
        history.add_command("cmd3");
        history.add_command("cmd4");

        assert_eq!(history.count(), 3);
        assert_eq!(history.get_command(0), Some(&"cmd2".to_string()));
        assert_eq!(history.get_command(1), Some(&"cmd3".to_string()));
        assert_eq!(history.get_command(2), Some(&"cmd4".to_string()));
    }

    #[test]
    fn test_set_max_size() {
        let mut history = CommandHistory::with_max_size(10);

        for i in 0..10 {
            history.add_command(format!("cmd{}", i));
        }

        assert_eq!(history.count(), 10);

        history.set_max_size(5);

        assert_eq!(history.count(), 5);
        assert_eq!(history.get_command(0), Some(&"cmd5".to_string()));
    }

    #[test]
    fn test_clear() {
        let mut history = CommandHistory::new();

        history.add_command("cmd1");
        history.add_command("cmd2");
        history.add_command("cmd3");

        assert_eq!(history.count(), 3);

        history.clear();

        assert_eq!(history.count(), 0);
        assert_eq!(history.get_last_command(), None);
    }

    #[test]
    fn test_get_all_commands() {
        let mut history = CommandHistory::new();

        history.add_command("cmd1");
        history.add_command("cmd2");
        history.add_command("cmd3");

        let all = history.get_all_commands();

        assert_eq!(all.len(), 3);
        assert_eq!(all[0], "cmd1");
        assert_eq!(all[1], "cmd2");
        assert_eq!(all[2], "cmd3");
    }

    #[test]
    fn test_is_navigating() {
        let mut history = CommandHistory::new();

        history.add_command("cmd1");

        assert!(!history.is_navigating());

        history.get_previous(None);
        assert!(history.is_navigating());

        history.reset_position();
        assert!(!history.is_navigating());
    }

    #[test]
    fn test_current_position() {
        let mut history = CommandHistory::new();

        history.add_command("cmd1");
        history.add_command("cmd2");
        history.add_command("cmd3");

        assert_eq!(history.current_position(), None);

        history.get_previous(None);
        assert_eq!(history.current_position(), Some(2));

        history.get_previous(None);
        assert_eq!(history.current_position(), Some(1));
    }

    #[test]
    fn test_json_serialization() {
        let mut history = CommandHistory::new();
        history.add_command("cmd1");
        history.add_command("cmd2");

        let json = serde_json::to_string(&history);
        assert!(json.is_ok(), "Should serialize to JSON");
    }

    #[test]
    fn test_json_deserialization() {
        let mut history = CommandHistory::new();
        history.add_command("cmd1");
        history.add_command("cmd2");

        let json = serde_json::to_string(&history).unwrap();
        let deserialized: CommandHistory = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.count(), 2);
        assert_eq!(deserialized.get_command(0), Some(&"cmd1".to_string()));
        assert_eq!(deserialized.get_command(1), Some(&"cmd2".to_string()));
    }

    #[test]
    fn test_navigation_at_empty_history() {
        let mut history = CommandHistory::new();

        assert_eq!(history.get_previous(None), None);
        assert_eq!(history.get_next(), None);
    }

    #[test]
    fn test_complex_navigation_scenario() {
        let mut history = CommandHistory::new();

        history.add_command("north");
        history.add_command("east");
        history.add_command("south");

        // User types something
        let typing = Some("west".to_string());

        // Up arrow - get "south"
        assert_eq!(history.get_previous(typing.clone()), Some("south".to_string()));

        // Up arrow - get "east"
        assert_eq!(history.get_previous(None), Some("east".to_string()));

        // Down arrow - get "south" again
        assert_eq!(history.get_next(), Some("south".to_string()));

        // Down arrow - back to typing
        assert_eq!(history.get_next(), Some("west".to_string()));

        // Down arrow - nothing (already at prompt)
        assert_eq!(history.get_next(), None);
    }
}

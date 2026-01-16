/// Keypad navigation system for MUD movement and commands
///
/// Maps numeric keypad keys to customizable commands, typically used for
/// directional navigation in MUDs. Supports standard keys (0-9, operators)
/// and Ctrl-modified variants.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::Result;

/// Keypad key identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeypadKey {
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    Dot,      // .
    Slash,    // /
    Star,     // *
    Minus,    // -
    Plus,     // +
    Enter,
}

impl KeypadKey {
    /// Parse keypad key from string identifier
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "0" | "num0" => Some(Self::Num0),
            "1" | "num1" => Some(Self::Num1),
            "2" | "num2" => Some(Self::Num2),
            "3" | "num3" => Some(Self::Num3),
            "4" | "num4" => Some(Self::Num4),
            "5" | "num5" => Some(Self::Num5),
            "6" | "num6" => Some(Self::Num6),
            "7" | "num7" => Some(Self::Num7),
            "8" | "num8" => Some(Self::Num8),
            "9" | "num9" => Some(Self::Num9),
            "." | "dot" | "decimal" => Some(Self::Dot),
            "/" | "slash" | "divide" => Some(Self::Slash),
            "*" | "star" | "multiply" => Some(Self::Star),
            "-" | "minus" | "subtract" => Some(Self::Minus),
            "+" | "plus" | "add" => Some(Self::Plus),
            "enter" | "return" => Some(Self::Enter),
            _ => None,
        }
    }

    /// Get string representation for display
    pub fn to_string(&self) -> &'static str {
        match self {
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::Dot => ".",
            Self::Slash => "/",
            Self::Star => "*",
            Self::Minus => "-",
            Self::Plus => "+",
            Self::Enter => "Enter",
        }
    }
}

/// Keypad modifier state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeypadModifier {
    None,
    Ctrl,
}

/// Complete keypad mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeypadMapping {
    /// Normal (unmodified) key mappings
    normal: HashMap<KeypadKey, String>,
    /// Ctrl-modified key mappings
    ctrl: HashMap<KeypadKey, String>,
}

impl KeypadMapping {
    /// Create new keypad mapping with default MUD navigation commands
    pub fn new() -> Self {
        let mut normal = HashMap::new();
        let mut ctrl = HashMap::new();

        // Standard MUD directional navigation (compass layout on keypad)
        // 7 NW    8 N     9 NE
        // 4 W     5 look  6 E
        // 1 SW    2 S     3 SE
        normal.insert(KeypadKey::Num8, "north".to_string());
        normal.insert(KeypadKey::Num2, "south".to_string());
        normal.insert(KeypadKey::Num4, "west".to_string());
        normal.insert(KeypadKey::Num6, "east".to_string());
        normal.insert(KeypadKey::Num7, "northwest".to_string());
        normal.insert(KeypadKey::Num9, "northeast".to_string());
        normal.insert(KeypadKey::Num1, "southwest".to_string());
        normal.insert(KeypadKey::Num3, "southeast".to_string());

        // Center key for common action
        normal.insert(KeypadKey::Num5, "look".to_string());

        // Zero for examine/look
        normal.insert(KeypadKey::Num0, "examine".to_string());

        // Vertical movement
        normal.insert(KeypadKey::Plus, "up".to_string());
        normal.insert(KeypadKey::Dot, "down".to_string());

        // Other useful defaults
        normal.insert(KeypadKey::Minus, "inventory".to_string());
        normal.insert(KeypadKey::Star, "score".to_string());
        normal.insert(KeypadKey::Slash, "who".to_string());
        normal.insert(KeypadKey::Enter, "".to_string()); // Empty = send current input

        // Ctrl variants - quick combat/utility commands
        ctrl.insert(KeypadKey::Num8, "flee".to_string());
        ctrl.insert(KeypadKey::Num5, "rest".to_string());
        ctrl.insert(KeypadKey::Num0, "get all".to_string());
        ctrl.insert(KeypadKey::Plus, "climb up".to_string());
        ctrl.insert(KeypadKey::Dot, "climb down".to_string());
        ctrl.insert(KeypadKey::Minus, "equipment".to_string());
        ctrl.insert(KeypadKey::Star, "status".to_string());

        Self { normal, ctrl }
    }

    /// Get command for keypad key press
    pub fn get_command(&self, key: KeypadKey, modifier: KeypadModifier) -> Option<&str> {
        let map = match modifier {
            KeypadModifier::None => &self.normal,
            KeypadModifier::Ctrl => &self.ctrl,
        };

        map.get(&key).map(|s| s.as_str())
    }

    /// Set command for keypad key
    pub fn set_command(&mut self, key: KeypadKey, modifier: KeypadModifier, command: String) {
        let map = match modifier {
            KeypadModifier::None => &mut self.normal,
            KeypadModifier::Ctrl => &mut self.ctrl,
        };

        map.insert(key, command);
    }

    /// Remove command mapping for keypad key
    pub fn remove_command(&mut self, key: KeypadKey, modifier: KeypadModifier) {
        let map = match modifier {
            KeypadModifier::None => &mut self.normal,
            KeypadModifier::Ctrl => &mut self.ctrl,
        };

        map.remove(&key);
    }

    /// Get all normal key mappings
    pub fn get_normal_mappings(&self) -> &HashMap<KeypadKey, String> {
        &self.normal
    }

    /// Get all Ctrl key mappings
    pub fn get_ctrl_mappings(&self) -> &HashMap<KeypadKey, String> {
        &self.ctrl
    }

    /// Check if a key has a mapping
    pub fn has_mapping(&self, key: KeypadKey, modifier: KeypadModifier) -> bool {
        let map = match modifier {
            KeypadModifier::None => &self.normal,
            KeypadModifier::Ctrl => &self.ctrl,
        };

        map.contains_key(&key)
    }
}

impl Default for KeypadMapping {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypad_key_from_str() {
        assert_eq!(KeypadKey::from_str("0"), Some(KeypadKey::Num0));
        assert_eq!(KeypadKey::from_str("num5"), Some(KeypadKey::Num5));
        assert_eq!(KeypadKey::from_str("."), Some(KeypadKey::Dot));
        assert_eq!(KeypadKey::from_str("slash"), Some(KeypadKey::Slash));
        assert_eq!(KeypadKey::from_str("Enter"), Some(KeypadKey::Enter));
        assert_eq!(KeypadKey::from_str("invalid"), None);
    }

    #[test]
    fn test_default_mappings() {
        let mapping = KeypadMapping::new();

        // Test directional commands
        assert_eq!(mapping.get_command(KeypadKey::Num8, KeypadModifier::None), Some("north"));
        assert_eq!(mapping.get_command(KeypadKey::Num2, KeypadModifier::None), Some("south"));
        assert_eq!(mapping.get_command(KeypadKey::Num4, KeypadModifier::None), Some("west"));
        assert_eq!(mapping.get_command(KeypadKey::Num6, KeypadModifier::None), Some("east"));

        // Test diagonal commands
        assert_eq!(mapping.get_command(KeypadKey::Num7, KeypadModifier::None), Some("northwest"));
        assert_eq!(mapping.get_command(KeypadKey::Num9, KeypadModifier::None), Some("northeast"));
        assert_eq!(mapping.get_command(KeypadKey::Num1, KeypadModifier::None), Some("southwest"));
        assert_eq!(mapping.get_command(KeypadKey::Num3, KeypadModifier::None), Some("southeast"));

        // Test utility commands
        assert_eq!(mapping.get_command(KeypadKey::Num5, KeypadModifier::None), Some("look"));
        assert_eq!(mapping.get_command(KeypadKey::Num0, KeypadModifier::None), Some("examine"));
        assert_eq!(mapping.get_command(KeypadKey::Plus, KeypadModifier::None), Some("up"));
        assert_eq!(mapping.get_command(KeypadKey::Dot, KeypadModifier::None), Some("down"));
    }

    #[test]
    fn test_ctrl_mappings() {
        let mapping = KeypadMapping::new();

        assert_eq!(mapping.get_command(KeypadKey::Num8, KeypadModifier::Ctrl), Some("flee"));
        assert_eq!(mapping.get_command(KeypadKey::Num5, KeypadModifier::Ctrl), Some("rest"));
        assert_eq!(mapping.get_command(KeypadKey::Plus, KeypadModifier::Ctrl), Some("climb up"));
    }

    #[test]
    fn test_set_command() {
        let mut mapping = KeypadMapping::new();

        mapping.set_command(KeypadKey::Num8, KeypadModifier::None, "n".to_string());
        assert_eq!(mapping.get_command(KeypadKey::Num8, KeypadModifier::None), Some("n"));

        mapping.set_command(KeypadKey::Num0, KeypadModifier::Ctrl, "search".to_string());
        assert_eq!(mapping.get_command(KeypadKey::Num0, KeypadModifier::Ctrl), Some("search"));
    }

    #[test]
    fn test_remove_command() {
        let mut mapping = KeypadMapping::new();

        assert!(mapping.has_mapping(KeypadKey::Num8, KeypadModifier::None));
        mapping.remove_command(KeypadKey::Num8, KeypadModifier::None);
        assert!(!mapping.has_mapping(KeypadKey::Num8, KeypadModifier::None));
        assert_eq!(mapping.get_command(KeypadKey::Num8, KeypadModifier::None), None);
    }

    #[test]
    fn test_has_mapping() {
        let mapping = KeypadMapping::new();

        assert!(mapping.has_mapping(KeypadKey::Num8, KeypadModifier::None));
        assert!(mapping.has_mapping(KeypadKey::Num8, KeypadModifier::Ctrl));

        // Key that has normal but not ctrl mapping
        assert!(mapping.has_mapping(KeypadKey::Num4, KeypadModifier::None));
        assert!(!mapping.has_mapping(KeypadKey::Num4, KeypadModifier::Ctrl));
    }

    #[test]
    fn test_get_mappings() {
        let mapping = KeypadMapping::new();

        let normal = mapping.get_normal_mappings();
        assert!(normal.len() > 0);
        assert!(normal.contains_key(&KeypadKey::Num8));

        let ctrl = mapping.get_ctrl_mappings();
        assert!(ctrl.len() > 0);
        assert!(ctrl.contains_key(&KeypadKey::Num8));
    }
}

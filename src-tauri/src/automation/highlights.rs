/// Highlight system for text coloring and formatting
///
/// Highlights match incoming MUD text against regex patterns and apply
/// styling (color, bold, italic, underline). Multiple highlights can match
/// the same text, allowing for overlay styling.

use crate::error::{MushError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Style information for matched text
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HighlightStyle {
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// Individual highlight with pattern and styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    /// Unique identifier
    pub id: Uuid,

    /// Highlight name
    pub name: String,

    /// Regex pattern to match
    pub pattern: String,

    /// Text color (hex format: #RRGGBB)
    pub color: String,

    /// Bold formatting
    #[serde(default)]
    pub bold: bool,

    /// Italic formatting
    #[serde(default)]
    pub italic: bool,

    /// Underline formatting
    #[serde(default)]
    pub underline: bool,

    /// Variable names to extract from captures
    #[serde(default)]
    pub variables: Vec<String>,

    /// Whether highlight is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Cached compiled regex (not serialized)
    #[serde(skip)]
    regex: Option<Regex>,
}

fn default_enabled() -> bool {
    true
}

impl Highlight {
    /// Create a new highlight
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        color: impl Into<String>,
    ) -> Result<Self> {
        let name = name.into();
        let pattern = pattern.into();
        let color = color.into();

        debug!("Creating highlight '{}' with pattern '{}'", name, pattern);

        // Validate pattern for ReDoS
        Self::validate_pattern(&pattern)?;

        // Validate color format
        Self::validate_color(&color)?;

        let highlight = Highlight {
            id: Uuid::new_v4(),
            name,
            pattern,
            color,
            bold: false,
            italic: false,
            underline: false,
            variables: Vec::new(),
            enabled: true,
            regex: None,
        };

        info!("Created highlight '{}' with ID {}", highlight.name, highlight.id);
        Ok(highlight)
    }

    /// Validate pattern for ReDoS vulnerabilities
    pub fn validate_pattern(pattern: &str) -> Result<()> {
        debug!("Validating pattern: {}", pattern);

        // Check for common ReDoS patterns (same as Trigger validation)
        if pattern.contains(")+") || pattern.contains(")*") || pattern.contains("}+") || pattern.contains("}*") {
            warn!("ReDoS vulnerability detected: nested quantifiers in '{}'", pattern);
            return Err(MushError::InvalidRegex {
                pattern: pattern.to_string(),
                source: regex::Error::Syntax("Nested quantifiers detected - potential ReDoS vulnerability".to_string()),
            });
        }

        // Try to compile regex to catch syntax errors
        Regex::new(pattern).map_err(|e| MushError::InvalidRegex {
            pattern: pattern.to_string(),
            source: e,
        })?;

        debug!("Pattern validation successful");
        Ok(())
    }

    /// Validate color format (#RRGGBB)
    pub fn validate_color(color: &str) -> Result<()> {
        if !color.starts_with('#') || color.len() != 7 {
            return Err(MushError::ValidationError {
                field: "color".to_string(),
                reason: format!("Invalid color format: '{}'. Expected #RRGGBB", color),
            });
        }

        // Check if all characters after # are hex digits
        if !color[1..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(MushError::ValidationError {
                field: "color".to_string(),
                reason: format!("Invalid color format: '{}'. Expected hex digits", color),
            });
        }

        Ok(())
    }

    /// Get or compile regex for matching
    fn get_regex(&mut self) -> Result<&Regex> {
        if self.regex.is_none() {
            debug!("Compiling regex for highlight '{}'", self.name);
            let re = Regex::new(&self.pattern).map_err(|e| MushError::InvalidRegex {
                pattern: self.pattern.clone(),
                source: e,
            })?;
            self.regex = Some(re);
        }
        Ok(self.regex.as_ref().unwrap())
    }

    /// Check if highlight matches the input text
    pub fn matches(&mut self, text: &str) -> Result<bool> {
        let regex = self.get_regex()?;
        Ok(regex.is_match(text))
    }

    /// Get style information
    pub fn get_style(&self) -> HighlightStyle {
        HighlightStyle {
            color: self.color.clone(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
        }
    }

    /// Extract capture groups as variables
    pub fn extract_captures(&mut self, text: &str) -> Result<HashMap<String, String>> {
        let regex = self.get_regex()?;
        let mut captures_map = HashMap::new();

        if let Some(caps) = regex.captures(text) {
            // Add full match as %0
            if let Some(full_match) = caps.get(0) {
                captures_map.insert("0".to_string(), full_match.as_str().to_string());
            }

            // Add numbered captures
            for i in 1..caps.len() {
                if let Some(capture) = caps.get(i) {
                    captures_map.insert(i.to_string(), capture.as_str().to_string());
                }
            }

            // Map to variable names if provided
            for (idx, var_name) in self.variables.iter().enumerate() {
                if let Some(capture) = caps.get(idx + 1) {
                    captures_map.insert(var_name.clone(), capture.as_str().to_string());
                }
            }
        }

        Ok(captures_map)
    }

    /// Find all matches in text with their positions
    pub fn find_all_matches(&mut self, text: &str) -> Result<Vec<(usize, usize, HighlightStyle)>> {
        // Get style first (immutable borrow)
        let style = self.get_style();

        // Then get regex (mutable borrow) - this is safe because get_style() is done
        let regex = self.get_regex()?;
        let mut matches = Vec::new();

        for mat in regex.find_iter(text) {
            matches.push((mat.start(), mat.end(), style.clone()));
        }

        Ok(matches)
    }
}

/// Manages collection of highlights with matching
pub struct HighlightManager {
    highlights: Vec<Highlight>,

    /// Cache for matching results
    #[allow(dead_code)]
    match_cache: Arc<Mutex<HashMap<String, Vec<usize>>>>,
}

impl HighlightManager {
    /// Create new highlight manager
    pub fn new() -> Self {
        debug!("Creating new HighlightManager");
        Self {
            highlights: Vec::new(),
            match_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add highlight to manager
    pub fn add_highlight(&mut self, highlight: Highlight) -> Result<()> {
        info!("Adding highlight '{}' (ID: {}) to manager", highlight.name, highlight.id);
        self.highlights.push(highlight);
        debug!("Total highlights: {}", self.highlights.len());
        Ok(())
    }

    /// Find all highlights that match the input text
    pub fn find_matches(&mut self, text: &str) -> Result<Vec<&mut Highlight>> {
        debug!("Finding highlight matches for text: {}", text);
        let mut matches = Vec::new();

        for highlight in &mut self.highlights {
            if highlight.enabled && highlight.matches(text)? {
                debug!("Highlight '{}' matched", highlight.name);
                matches.push(highlight);
            }
        }

        info!("Found {} matching highlight(s)", matches.len());
        Ok(matches)
    }

    /// Get all match positions with styles for rendering
    pub fn get_all_matches(&mut self, text: &str) -> Result<Vec<(usize, usize, HighlightStyle)>> {
        debug!("Getting all highlight matches for rendering");
        let mut all_matches = Vec::new();

        for highlight in &mut self.highlights {
            if highlight.enabled {
                let matches = highlight.find_all_matches(text)?;
                all_matches.extend(matches);
            }
        }

        // Sort by start position
        all_matches.sort_by_key(|(start, _, _)| *start);

        debug!("Found {} total match positions", all_matches.len());
        Ok(all_matches)
    }

    /// Get highlight by ID
    pub fn get_highlight(&self, id: Uuid) -> Option<&Highlight> {
        debug!("Looking up highlight with ID: {}", id);
        let result = self.highlights.iter().find(|h| h.id == id);
        if result.is_some() {
            debug!("Highlight found");
        } else {
            debug!("Highlight not found");
        }
        result
    }

    /// Get mutable highlight by ID
    pub fn get_highlight_mut(&mut self, id: Uuid) -> Option<&mut Highlight> {
        debug!("Looking up mutable highlight with ID: {}", id);
        let result = self.highlights.iter_mut().find(|h| h.id == id);
        if result.is_some() {
            debug!("Mutable highlight found");
        } else {
            debug!("Mutable highlight not found");
        }
        result
    }

    /// Remove highlight by ID
    pub fn remove_highlight(&mut self, id: Uuid) -> Result<()> {
        info!("Removing highlight with ID: {}", id);
        let before = self.highlights.len();
        self.highlights.retain(|h| h.id != id);
        let after = self.highlights.len();

        if before == after {
            warn!("Highlight with ID {} not found", id);
            return Err(MushError::ValidationError {
                field: "id".to_string(),
                reason: format!("Highlight with ID {} not found", id),
            });
        }

        info!("Highlight removed successfully. Total highlights: {}", after);
        Ok(())
    }

    /// Get count of highlights
    pub fn count(&self) -> usize {
        self.highlights.len()
    }

    /// Get count of enabled highlights
    pub fn enabled_count(&self) -> usize {
        self.highlights.iter().filter(|h| h.enabled).count()
    }

    /// Clear all highlights
    pub fn clear(&mut self) {
        info!("Clearing all highlights");
        self.highlights.clear();
    }
}

impl Default for HighlightManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_highlight() {
        let highlight = Highlight::new("Test", "hello", "#FF0000").unwrap();
        assert_eq!(highlight.name, "Test");
        assert_eq!(highlight.pattern, "hello");
        assert_eq!(highlight.color, "#FF0000");
        assert!(highlight.enabled);
    }

    #[test]
    fn test_validate_color() {
        assert!(Highlight::validate_color("#FF0000").is_ok());
        assert!(Highlight::validate_color("#00FF00").is_ok());
        assert!(Highlight::validate_color("#0000FF").is_ok());
        assert!(Highlight::validate_color("FF0000").is_err()); // Missing #
        assert!(Highlight::validate_color("#FF00").is_err()); // Too short
        assert!(Highlight::validate_color("#GG0000").is_err()); // Invalid hex
    }

    #[test]
    fn test_highlight_matches() {
        let mut highlight = Highlight::new("Test", "hello", "#FF0000").unwrap();
        assert!(highlight.matches("hello world").unwrap());
        assert!(!highlight.matches("goodbye world").unwrap());
    }

    #[test]
    fn test_highlight_style() {
        let mut highlight = Highlight::new("Test", "hello", "#FF0000").unwrap();
        highlight.bold = true;
        highlight.italic = true;

        let style = highlight.get_style();
        assert_eq!(style.color, "#FF0000");
        assert!(style.bold);
        assert!(style.italic);
        assert!(!style.underline);
    }

    #[test]
    fn test_highlight_manager() {
        let mut manager = HighlightManager::new();

        let h1 = Highlight::new("Red", "hello", "#FF0000").unwrap();
        let h2 = Highlight::new("Blue", "world", "#0000FF").unwrap();

        manager.add_highlight(h1).unwrap();
        manager.add_highlight(h2).unwrap();

        assert_eq!(manager.count(), 2);
        assert_eq!(manager.enabled_count(), 2);
    }

    #[test]
    fn test_find_all_matches() {
        let mut highlight = Highlight::new("Test", r"\d+", "#FF0000").unwrap();
        let matches = highlight.find_all_matches("HP: 100/150 MP: 50/75").unwrap();

        assert_eq!(matches.len(), 4); // Should match 100, 150, 50, 75
    }
}

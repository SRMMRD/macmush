/// Tab-completion from recent output
///
/// Provides word completion based on text that has appeared in recent MUD output.
/// Users can press TAB to auto-complete partial words.

use std::collections::{HashMap, VecDeque};
use tracing::{debug, trace};

/// Maximum number of recent words to track
const MAX_WORD_HISTORY: usize = 1000;

/// Minimum word length to track
const MIN_WORD_LENGTH: usize = 3;

/// Tab-completion engine
#[derive(Debug, Clone)]
pub struct TabCompletion {
    /// Recent words with frequency counts
    word_frequency: HashMap<String, usize>,
    /// Recent words in order (FIFO queue)
    word_history: VecDeque<String>,
    /// Maximum words to track
    max_history: usize,
    /// Case-sensitive matching
    case_sensitive: bool,
}

impl TabCompletion {
    /// Create a new tab-completion engine
    pub fn new() -> Self {
        Self {
            word_frequency: HashMap::new(),
            word_history: VecDeque::new(),
            max_history: MAX_WORD_HISTORY,
            case_sensitive: false,
        }
    }

    /// Create with custom configuration
    pub fn with_config(max_history: usize, case_sensitive: bool) -> Self {
        Self {
            word_frequency: HashMap::new(),
            word_history: VecDeque::new(),
            max_history,
            case_sensitive,
        }
    }

    /// Add text from MUD output to word history
    ///
    /// Extracts all words and adds them to the completion database.
    pub fn add_output(&mut self, text: &str) {
        let words = Self::extract_words(text);

        for word in words {
            if word.len() >= MIN_WORD_LENGTH {
                self.add_word(word);
            }
        }
    }

    /// Add a single word to history
    fn add_word(&mut self, word: String) {
        // Normalize case if case-insensitive
        let key = if self.case_sensitive {
            word.clone()
        } else {
            word.to_lowercase()
        };

        // Update frequency
        *self.word_frequency.entry(key.clone()).or_insert(0) += 1;

        // Add to history if not already recent
        if !self.word_history.iter().rev().take(10).any(|w| w == &key) {
            self.word_history.push_back(key);

            // Remove oldest if over limit
            if self.word_history.len() > self.max_history {
                if let Some(old_word) = self.word_history.pop_front() {
                    // Decrement frequency
                    if let Some(count) = self.word_frequency.get_mut(&old_word) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            self.word_frequency.remove(&old_word);
                        }
                    }
                }
            }
        }
    }

    /// Extract words from text
    ///
    /// Splits on whitespace and non-alphanumeric characters, preserving apostrophes.
    fn extract_words(text: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current_word = String::new();

        for ch in text.chars() {
            if ch.is_alphanumeric() || ch == '\'' || ch == '-' {
                current_word.push(ch);
            } else {
                if !current_word.is_empty() {
                    words.push(current_word.clone());
                    current_word.clear();
                }
            }
        }

        // Don't forget the last word
        if !current_word.is_empty() {
            words.push(current_word);
        }

        words
    }

    /// Get completion matches for a partial word
    ///
    /// Returns all words that start with the given prefix, sorted by frequency.
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        if partial.is_empty() {
            return Vec::new();
        }

        let prefix = if self.case_sensitive {
            partial.to_string()
        } else {
            partial.to_lowercase()
        };

        // Find all matching words
        let mut matches: Vec<(String, usize)> = self
            .word_frequency
            .iter()
            .filter(|(word, _)| word.starts_with(&prefix))
            .map(|(word, &freq)| (word.clone(), freq))
            .collect();

        // Sort by frequency (descending) then alphabetically
        matches.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        // Return just the words
        matches.into_iter().map(|(word, _)| word).collect()
    }

    /// Get the best completion match
    ///
    /// Returns the most frequent match, or None if no matches.
    pub fn get_best_match(&self, partial: &str) -> Option<String> {
        let completions = self.get_completions(partial);
        completions.first().cloned()
    }

    /// Get completion with cycling support
    ///
    /// Returns the next match after the current completion.
    /// If current is None, returns the first match.
    pub fn cycle_completion(&self, partial: &str, current: Option<&str>) -> Option<String> {
        let completions = self.get_completions(partial);

        if completions.is_empty() {
            return None;
        }

        if let Some(current_word) = current {
            // Find current position and return next
            if let Some(pos) = completions.iter().position(|w| w == current_word) {
                let next_pos = (pos + 1) % completions.len();
                return Some(completions[next_pos].clone());
            }
        }

        // Return first match
        completions.first().cloned()
    }

    /// Clear all completion history
    pub fn clear(&mut self) {
        self.word_frequency.clear();
        self.word_history.clear();
        debug!("Cleared tab-completion history");
    }

    /// Get statistics
    pub fn stats(&self) -> CompletionStats {
        CompletionStats {
            total_words: self.word_frequency.len(),
            history_size: self.word_history.len(),
            most_frequent: self.get_most_frequent_words(5),
        }
    }

    /// Get most frequent words
    fn get_most_frequent_words(&self, limit: usize) -> Vec<(String, usize)> {
        let mut words: Vec<_> = self.word_frequency.iter()
            .map(|(word, &freq)| (word.clone(), freq))
            .collect();

        words.sort_by(|a, b| b.1.cmp(&a.1));
        words.truncate(limit);
        words
    }

    /// Enable/disable case-sensitive matching
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive != case_sensitive {
            self.case_sensitive = case_sensitive;
            // Rebuild word map with new case settings
            let words: Vec<_> = self.word_history.iter().cloned().collect();
            self.clear();
            for word in words {
                self.add_word(word);
            }
        }
    }
}

impl Default for TabCompletion {
    fn default() -> Self {
        Self::new()
    }
}

/// Tab-completion statistics
#[derive(Debug, Clone)]
pub struct CompletionStats {
    pub total_words: usize,
    pub history_size: usize,
    pub most_frequent: Vec<(String, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_words() {
        let text = "You see a goblin and an orc-warrior here.";
        let words = TabCompletion::extract_words(text);

        assert_eq!(words, vec!["You", "see", "a", "goblin", "and", "an", "orc-warrior", "here"]);
    }

    #[test]
    fn test_add_output() {
        let mut completion = TabCompletion::new();
        completion.add_output("attack goblin with sword");

        let matches = completion.get_completions("att");
        assert!(matches.contains(&"attack".to_string()));
    }

    #[test]
    fn test_completion_matching() {
        let mut completion = TabCompletion::new();

        completion.add_output("attack the goblin");
        completion.add_output("attack the orc");
        completion.add_output("defend yourself");

        let matches = completion.get_completions("att");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "attack");

        let matches = completion.get_completions("def");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "defend");
    }

    #[test]
    fn test_frequency_sorting() {
        let mut completion = TabCompletion::new();

        // Add "attack" multiple times
        completion.add_output("attack");
        completion.add_output("attack");
        completion.add_output("attack");
        completion.add_output("attempt");

        let matches = completion.get_completions("att");
        assert_eq!(matches[0], "attack"); // Most frequent should be first
        assert_eq!(matches[1], "attempt");
    }

    #[test]
    fn test_case_insensitive() {
        let mut completion = TabCompletion::new();
        completion.add_output("Attack the Goblin");

        let matches = completion.get_completions("att");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "attack");

        let matches = completion.get_completions("gob");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "goblin");
    }

    #[test]
    fn test_case_sensitive() {
        let mut completion = TabCompletion::with_config(1000, true);
        completion.add_output("Attack the Goblin");

        let matches = completion.get_completions("Att");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], "Attack");

        let matches = completion.get_completions("att");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_best_match() {
        let mut completion = TabCompletion::new();
        completion.add_output("attack goblin defend yourself");

        let best = completion.get_best_match("att");
        assert_eq!(best, Some("attack".to_string()));

        let best = completion.get_best_match("xyz");
        assert_eq!(best, None);
    }

    #[test]
    fn test_cycle_completion() {
        let mut completion = TabCompletion::new();
        completion.add_output("attack attempt attract");

        // First cycle
        let first = completion.cycle_completion("att", None);
        assert!(first.is_some());

        // Second cycle
        let second = completion.cycle_completion("att", first.as_deref());
        assert!(second.is_some());
        assert_ne!(first, second);

        // Should wrap around
        let matches = completion.get_completions("att");
        let wrapped = completion.cycle_completion("att", matches.last().map(|s| s.as_str()));
        assert_eq!(wrapped, first);
    }

    #[test]
    fn test_min_word_length() {
        let mut completion = TabCompletion::new();
        completion.add_output("a an the it");

        // Short words shouldn't be added
        let matches = completion.get_completions("a");
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_history_limit() {
        let mut completion = TabCompletion::with_config(5, false);

        for i in 0..10 {
            completion.add_output(&format!("word{}", i));
        }

        assert!(completion.word_history.len() <= 5);
    }

    #[test]
    fn test_clear() {
        let mut completion = TabCompletion::new();
        completion.add_output("attack defend");

        assert!(completion.word_frequency.len() > 0);

        completion.clear();

        assert_eq!(completion.word_frequency.len(), 0);
        assert_eq!(completion.word_history.len(), 0);
    }

    #[test]
    fn test_stats() {
        let mut completion = TabCompletion::new();
        completion.add_output("attack attack defend");

        let stats = completion.stats();
        assert_eq!(stats.total_words, 2); // attack, defend
        assert!(stats.most_frequent.len() > 0);
        assert_eq!(stats.most_frequent[0].0, "attack"); // Most frequent
    }
}

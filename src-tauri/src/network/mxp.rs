/// MXP (MUD eXtension Protocol) implementation
///
/// Provides parsing and handling for MXP tags, which enable MUDs to send
/// formatted text with clickable links, colors, fonts, and other rich content.
///
/// Reference: https://www.zuggsoft.com/zmud/mxp.htm

use crate::error::{MushError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// MXP telnet option number
pub const TELOPT_MXP: u8 = 91;

/// MXP line modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MxpMode {
    /// Open mode - only open tags allowed, resets at newline
    Open = 0,
    /// Secure mode - all tags permitted, resets at newline
    Secure = 1,
    /// Locked mode - no parsing, verbatim text output
    Locked = 2,
    /// Reset mode - close tags, restore defaults
    Reset = 3,
    /// Locked open mode - persists until changed
    LockedOpen = 5,
    /// Locked secure mode - persists until changed
    LockedSecure = 6,
    /// Locked locked mode - persists until changed
    LockedLocked = 7,
}

impl MxpMode {
    /// Parse MXP mode from number
    pub fn from_u8(mode: u8) -> Option<Self> {
        match mode {
            0 => Some(Self::Open),
            1 => Some(Self::Secure),
            2 => Some(Self::Locked),
            3 => Some(Self::Reset),
            5 => Some(Self::LockedOpen),
            6 => Some(Self::LockedSecure),
            7 => Some(Self::LockedLocked),
            _ => None,
        }
    }
}

/// MXP tag types
#[derive(Debug, Clone, PartialEq)]
pub enum MxpTag {
    /// Bold text
    Bold,
    /// Italic text
    Italic,
    /// Underline text
    Underline,
    /// Strikethrough text
    Strikethrough,
    /// Color specification
    Color { fore: Option<String>, back: Option<String> },
    /// Font specification
    Font { face: Option<String>, size: Option<String>, color: Option<String> },
    /// Hyperlink (web, telnet, mailto)
    Anchor { href: String, hint: Option<String> },
    /// Send command to MUD
    Send { href: String, hint: Option<String>, prompt: Option<String> },
    /// Line break
    Br,
    /// Paragraph break
    P,
    /// Expire/remove previous link
    Expire { name: String },
    /// Version query
    Version,
    /// Support query
    Support { tag: String },
    /// Custom element
    Custom { name: String, attrs: HashMap<String, String> },
}

/// Parsed MXP element with content
#[derive(Debug, Clone)]
pub struct MxpElement {
    pub tag: MxpTag,
    pub content: String,
    pub line_number: usize,
}

/// MXP parser state
#[derive(Debug)]
pub struct MxpParser {
    mode: MxpMode,
    enabled: bool,
    elements: HashMap<String, String>,  // Custom element definitions
    entities: HashMap<String, String>,   // Custom entity definitions
}

impl MxpParser {
    /// Create new MXP parser
    pub fn new() -> Self {
        Self {
            mode: MxpMode::Open,
            enabled: false,
            elements: HashMap::new(),
            entities: HashMap::new(),
        }
    }

    /// Enable MXP processing
    pub fn enable(&mut self) {
        self.enabled = true;
        info!("MXP enabled");
    }

    /// Disable MXP processing
    pub fn disable(&mut self) {
        self.enabled = false;
        debug!("MXP disabled");
    }

    /// Check if MXP is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get current mode
    pub fn mode(&self) -> MxpMode {
        self.mode
    }

    /// Set MXP mode
    pub fn set_mode(&mut self, mode: MxpMode) {
        debug!("MXP mode changed: {:?} -> {:?}", self.mode, mode);
        self.mode = mode;

        // Reset mode clears state
        if mode == MxpMode::Reset {
            self.mode = MxpMode::Open;
        }
    }

    /// Parse MXP mode sequence: ESC[#z
    pub fn parse_mode_sequence(&mut self, text: &str) -> Option<usize> {
        let re = Regex::new(r"\x1B\[(\d+)z").ok()?;
        if let Some(cap) = re.captures(text) {
            if let Ok(mode_num) = cap[1].parse::<u8>() {
                if let Some(mode) = MxpMode::from_u8(mode_num) {
                    self.set_mode(mode);
                    return Some(cap.get(0)?.end());
                }
            }
        }
        None
    }

    /// Parse MXP tags from text
    pub fn parse(&mut self, text: &str) -> Result<Vec<MxpElement>> {
        if !self.enabled || self.mode == MxpMode::Locked || self.mode == MxpMode::LockedLocked {
            // No parsing in locked modes
            return Ok(vec![]);
        }

        let mut elements = Vec::new();
        let mut pos = 0;
        let line_number = 0; // TODO: track actual line numbers

        while pos < text.len() {
            // Check for mode sequence
            if let Some(end) = self.parse_mode_sequence(&text[pos..]) {
                pos += end;
                continue;
            }

            // Look for opening tag
            if let Some(tag_start) = text[pos..].find('<') {
                let tag_pos = pos + tag_start;

                // Find closing >
                if let Some(tag_end_rel) = text[tag_pos + 1..].find('>') {
                    let tag_end = tag_pos + 1 + tag_end_rel;
                    let tag_text = &text[tag_pos + 1..tag_end];

                    // Parse the tag
                    if let Some(element) = self.parse_tag(tag_text, line_number) {
                        // Extract content between tags (for closing tags)
                        let content = String::new(); // TODO: extract content between open/close

                        elements.push(MxpElement {
                            tag: element,
                            content,
                            line_number,
                        });
                    }

                    pos = tag_end + 1;
                } else {
                    pos = tag_pos + 1;
                }
            } else {
                break;
            }
        }

        Ok(elements)
    }

    /// Parse individual MXP tag
    fn parse_tag(&self, tag_text: &str, _line: usize) -> Option<MxpTag> {
        let tag_lower = tag_text.to_lowercase();
        let parts: Vec<&str> = tag_lower.split_whitespace().collect();

        if parts.is_empty() {
            return None;
        }

        let tag_name = parts[0];

        match tag_name {
            "b" | "bold" => Some(MxpTag::Bold),
            "i" | "italic" => Some(MxpTag::Italic),
            "u" | "underline" => Some(MxpTag::Underline),
            "s" | "strikeout" => Some(MxpTag::Strikethrough),
            "br" => Some(MxpTag::Br),
            "p" => Some(MxpTag::P),

            "color" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                Some(MxpTag::Color {
                    fore: attrs.get("fore").or_else(|| attrs.get("color")).cloned(),
                    back: attrs.get("back").cloned(),
                })
            }

            "font" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                Some(MxpTag::Font {
                    face: attrs.get("face").cloned(),
                    size: attrs.get("size").cloned(),
                    color: attrs.get("color").cloned(),
                })
            }

            "a" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                attrs.get("href").map(|href| MxpTag::Anchor {
                    href: href.clone(),
                    hint: attrs.get("hint").cloned(),
                })
            }

            "send" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                attrs.get("href").map(|href| MxpTag::Send {
                    href: href.clone(),
                    hint: attrs.get("hint").cloned(),
                    prompt: attrs.get("prompt").cloned(),
                })
            }

            "expire" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                attrs.get("name").map(|name| MxpTag::Expire {
                    name: name.clone(),
                })
            }

            "version" => Some(MxpTag::Version),

            "support" => {
                let attrs = Self::parse_attributes(&parts[1..]);
                if let Some(tag) = attrs.get("tag") {
                    Some(MxpTag::Support {
                        tag: tag.clone(),
                    })
                } else if let Some(tag_str) = parts.get(1) {
                    Some(MxpTag::Support {
                        tag: tag_str.to_string(),
                    })
                } else {
                    None
                }
            }

            _ => {
                // Custom element
                let attrs = Self::parse_attributes(&parts[1..]);
                Some(MxpTag::Custom {
                    name: tag_name.to_string(),
                    attrs,
                })
            }
        }
    }

    /// Parse tag attributes from parts
    fn parse_attributes(parts: &[&str]) -> HashMap<String, String> {
        let mut attrs = HashMap::new();

        for part in parts {
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].to_lowercase();
                let value = part[eq_pos + 1..].trim_matches(|c| c == '"' || c == '\'');
                attrs.insert(key, value.to_string());
            }
        }

        attrs
    }

    /// Strip all MXP tags from text (for display without MXP support)
    pub fn strip_tags(text: &str) -> String {
        let re = Regex::new(r"<[^>]+>").unwrap();
        re.replace_all(text, "").to_string()
    }

    /// Handle Telnet IAC WILL MXP negotiation
    pub fn handle_will_mxp(&mut self) -> Result<Vec<u8>> {
        info!("Server offered MXP support");
        self.enable();

        // Send IAC DO MXP
        Ok(vec![255, 253, TELOPT_MXP])
    }

    /// Handle Telnet IAC DO MXP negotiation
    pub fn handle_do_mxp(&mut self) -> Result<Vec<u8>> {
        debug!("Client received DO MXP");
        // Server is confirming our MXP support
        Ok(Vec::new())
    }
}

impl Default for MxpParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mxp_mode_parsing() {
        let mut parser = MxpParser::new();

        // Test open mode
        let result = parser.parse_mode_sequence("\x1B[0z");
        assert!(result.is_some());
        assert_eq!(parser.mode(), MxpMode::Open);

        // Test secure mode
        let result = parser.parse_mode_sequence("\x1B[1z");
        assert!(result.is_some());
        assert_eq!(parser.mode(), MxpMode::Secure);
    }

    #[test]
    fn test_parse_bold_tag() {
        let mut parser = MxpParser::new();
        parser.enable();

        let result = parser.parse("<b>text</b>").unwrap();
        assert!(!result.is_empty());
        assert!(matches!(result[0].tag, MxpTag::Bold));
    }

    #[test]
    fn test_parse_color_tag() {
        let mut parser = MxpParser::new();
        parser.enable();

        let result = parser.parse("<color fore=red>").unwrap();
        assert!(!result.is_empty());

        if let MxpTag::Color { fore, .. } = &result[0].tag {
            assert_eq!(fore.as_deref(), Some("red"));
        } else {
            panic!("Expected Color tag");
        }
    }

    #[test]
    fn test_parse_send_tag() {
        let mut parser = MxpParser::new();
        parser.enable();

        let result = parser.parse("<send href='north'>Go North</send>").unwrap();
        assert!(!result.is_empty());

        if let MxpTag::Send { href, .. } = &result[0].tag {
            assert_eq!(href, "north");
        } else {
            panic!("Expected Send tag");
        }
    }

    #[test]
    fn test_strip_tags() {
        let text = "<b>Bold</b> <i>italic</i> normal";
        let stripped = MxpParser::strip_tags(text);
        assert_eq!(stripped, "Bold italic normal");
    }

    #[test]
    fn test_locked_mode_no_parsing() {
        let mut parser = MxpParser::new();
        parser.enable();
        parser.set_mode(MxpMode::Locked);

        let result = parser.parse("<b>text</b>").unwrap();
        assert!(result.is_empty(), "Locked mode should not parse tags");
    }

    #[test]
    fn test_disabled_no_parsing() {
        let mut parser = MxpParser::new();
        // Parser starts disabled

        let result = parser.parse("<b>text</b>").unwrap();
        assert!(result.is_empty(), "Disabled parser should not parse tags");
    }
}

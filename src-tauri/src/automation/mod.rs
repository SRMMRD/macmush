/// Automation features: triggers, aliases, timers, variables
///
/// This module implements MUSHclient's automation capabilities:
/// - Triggers: Pattern matching on incoming text
/// - Aliases: Command shortcuts and expansion
/// - Timers: Scheduled command execution
/// - Variables: Session-persistent data storage
/// - Speedwalk: Quick navigation with commands like "4n 5w"

pub mod triggers;
pub mod aliases;
pub mod timers;
pub mod highlights;
pub mod variables;
pub mod tab_completion;
pub mod keypad;
pub mod command_history;
pub mod speedwalk;

// Re-export commonly used types
pub use triggers::{Trigger, TriggerManager};
pub use aliases::{Alias, AliasManager};
pub use timers::{Timer, TimerManager};
pub use highlights::{Highlight, HighlightManager, HighlightStyle};
pub use variables::{Variable, VariableManager};
pub use tab_completion::{TabCompletion, CompletionStats};
pub use keypad::{KeypadKey, KeypadModifier, KeypadMapping};
pub use command_history::CommandHistory;
pub use speedwalk::{Speedwalk, SpeedwalkConfig};

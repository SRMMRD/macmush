/// Timer system for scheduled command execution
///
/// Timers execute actions at specific intervals or after delays,
/// providing scheduled automation and recurring commands.

use crate::error::{MushError, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Timer action to execute when timer fires
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TimerAction {
    /// Send command to server
    SendCommand(String),

    /// Send multiple commands in sequence
    SendCommands(Vec<String>),

    /// Execute Lua script
    ExecuteScript(String),

    /// Execute multiple actions in sequence
    Sequence(Vec<TimerAction>),
}

/// Timer type - one-shot or repeating
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TimerType {
    /// Execute once after delay
    OneShot,

    /// Execute repeatedly at interval
    Repeating,
}

/// Individual timer with scheduling and action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timer {
    /// Unique identifier
    pub id: Uuid,

    /// Timer name
    pub name: String,

    /// Timer type (one-shot or repeating)
    pub timer_type: TimerType,

    /// Interval in seconds
    pub interval: f64,

    /// Action to execute when timer fires
    pub action: TimerAction,

    /// Whether timer is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Next fire time (not serialized)
    #[serde(skip)]
    next_fire: Option<Instant>,

    /// Whether timer has fired (for one-shot timers, not serialized)
    #[serde(skip)]
    has_fired: bool,
}

fn default_enabled() -> bool {
    true
}

impl Timer {
    /// Create a new timer
    pub fn new(
        name: impl Into<String>,
        timer_type: TimerType,
        interval: f64,
        action: TimerAction,
    ) -> Result<Self> {
        let name = name.into();

        if interval <= 0.0 {
            return Err(MushError::ValidationError {
                field: "interval".to_string(),
                reason: format!("must be positive, got {}", interval),
            });
        }

        debug!(
            "Creating timer '{}' type {:?} interval {}s",
            name, timer_type, interval
        );

        let timer = Timer {
            id: Uuid::new_v4(),
            name,
            timer_type,
            interval,
            action,
            enabled: true,
            next_fire: None,
            has_fired: false,
        };

        info!("Created timer '{}' with ID {}", timer.name, timer.id);
        Ok(timer)
    }

    /// Start the timer (set initial fire time)
    pub fn start(&mut self) {
        let delay = Duration::from_secs_f64(self.interval);
        self.next_fire = Some(Instant::now() + delay);
        self.has_fired = false;
        debug!("Timer '{}' started, fires in {}s", self.name, self.interval);
    }

    /// Stop the timer
    pub fn stop(&mut self) {
        self.next_fire = None;
        debug!("Timer '{}' stopped", self.name);
    }

    /// Reset the timer (for repeating timers)
    pub fn reset(&mut self) {
        if self.timer_type == TimerType::Repeating {
            let delay = Duration::from_secs_f64(self.interval);
            self.next_fire = Some(Instant::now() + delay);
            debug!("Timer '{}' reset, next fire in {}s", self.name, self.interval);
        } else {
            self.has_fired = true;
            self.next_fire = None;
            debug!("One-shot timer '{}' completed", self.name);
        }
    }

    /// Check if timer should fire
    pub fn should_fire(&self) -> bool {
        if !self.enabled {
            return false;
        }

        if self.timer_type == TimerType::OneShot && self.has_fired {
            return false;
        }

        if let Some(next_fire) = self.next_fire {
            Instant::now() >= next_fire
        } else {
            false
        }
    }

    /// Execute timer action and return commands to send
    pub fn execute(&mut self) -> Result<Vec<String>> {
        let mut commands = Vec::new();
        self.execute_action(&self.action.clone(), &mut commands);

        // Reset timer for next execution
        self.reset();

        Ok(commands)
    }

    fn execute_action(&self, action: &TimerAction, commands: &mut Vec<String>) {
        match action {
            TimerAction::SendCommand(cmd) => {
                commands.push(cmd.clone());
            }
            TimerAction::SendCommands(cmds) => {
                for cmd in cmds {
                    commands.push(cmd.clone());
                }
            }
            TimerAction::ExecuteScript(_script) => {
                // Script execution handled at Session level via Lua runtime
            }
            TimerAction::Sequence(actions) => {
                for action in actions {
                    self.execute_action(action, commands);
                }
            }
        }
    }

    /// Get time until next fire
    pub fn time_until_fire(&self) -> Option<Duration> {
        if !self.enabled {
            return None;
        }

        if let Some(next_fire) = self.next_fire {
            let now = Instant::now();
            if next_fire > now {
                Some(next_fire - now)
            } else {
                Some(Duration::from_secs(0))
            }
        } else {
            None
        }
    }
}

/// Manages collection of timers with scheduling and execution
pub struct TimerManager {
    timers: Vec<Timer>,
}

impl TimerManager {
    /// Create new timer manager
    pub fn new() -> Self {
        debug!("Creating new TimerManager");
        Self {
            timers: Vec::new(),
        }
    }

    /// Add timer to manager and start it
    pub fn add_timer(&mut self, mut timer: Timer) -> Result<()> {
        info!(
            "Adding timer '{}' (ID: {}) to manager",
            timer.name, timer.id
        );
        timer.start();
        self.timers.push(timer);
        debug!("Total timers: {}", self.timers.len());
        Ok(())
    }

    /// Get timers ready to fire
    pub fn get_ready_timers(&mut self) -> Vec<&mut Timer> {
        self.timers
            .iter_mut()
            .filter(|t| t.should_fire())
            .collect()
    }

    /// Get timer by ID
    pub fn get_timer(&self, id: Uuid) -> Option<&Timer> {
        debug!("Looking up timer with ID: {}", id);
        let result = self.timers.iter().find(|t| t.id == id);
        if result.is_some() {
            debug!("Timer found");
        } else {
            debug!("Timer not found");
        }
        result
    }

    /// Get mutable timer by ID
    pub fn get_timer_mut(&mut self, id: Uuid) -> Option<&mut Timer> {
        self.timers.iter_mut().find(|t| t.id == id)
    }

    /// Remove timer by ID
    pub fn remove_timer(&mut self, id: Uuid) -> Result<()> {
        info!("Removing timer with ID: {}", id);
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id);
        let after = self.timers.len();

        if before == after {
            warn!("Timer ID {} not found, nothing removed", id);
        } else {
            debug!("Timer removed, {} timer(s) remaining", after);
        }

        Ok(())
    }

    /// Get all timers
    pub fn timers(&self) -> &[Timer] {
        &self.timers
    }

    /// Start all timers
    pub fn start_all(&mut self) {
        for timer in &mut self.timers {
            timer.start();
        }
        info!("Started {} timer(s)", self.timers.len());
    }

    /// Stop all timers
    pub fn stop_all(&mut self) {
        for timer in &mut self.timers {
            timer.stop();
        }
        info!("Stopped {} timer(s)", self.timers.len());
    }

    /// Get next timer fire time
    pub fn next_fire_time(&self) -> Option<Duration> {
        self.timers
            .iter()
            .filter_map(|t| t.time_until_fire())
            .min()
    }
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_create_oneshot_timer() {
        let timer = Timer::new(
            "Test Timer",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        );

        assert!(timer.is_ok(), "Should create one-shot timer");
        let timer = timer.unwrap();
        assert_eq!(timer.name, "Test Timer");
        assert_eq!(timer.timer_type, TimerType::OneShot);
        assert_eq!(timer.interval, 1.0);
        assert!(timer.enabled);
    }

    #[test]
    fn test_create_repeating_timer() {
        let timer = Timer::new(
            "Repeating",
            TimerType::Repeating,
            5.0,
            TimerAction::SendCommand("check hp".to_string()),
        );

        assert!(timer.is_ok(), "Should create repeating timer");
        let timer = timer.unwrap();
        assert_eq!(timer.timer_type, TimerType::Repeating);
    }

    #[test]
    fn test_invalid_interval() {
        let result = Timer::new(
            "Bad Timer",
            TimerType::OneShot,
            -1.0,
            TimerAction::SendCommand("test".to_string()),
        );

        assert!(result.is_err(), "Should reject negative interval");
    }

    #[test]
    fn test_timer_start() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        assert!(timer.next_fire.is_none(), "Should not have fire time initially");

        timer.start();

        assert!(timer.next_fire.is_some(), "Should have fire time after start");
    }

    #[test]
    fn test_timer_should_fire() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        timer.start();

        assert!(!timer.should_fire(), "Should not fire immediately");

        // Wait for timer to be ready
        thread::sleep(Duration::from_millis(150));

        assert!(timer.should_fire(), "Should fire after interval");
    }

    #[test]
    fn test_disabled_timer_no_fire() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        timer.start();
        timer.enabled = false;

        thread::sleep(Duration::from_millis(150));

        assert!(!timer.should_fire(), "Disabled timer should not fire");
    }

    #[test]
    fn test_oneshot_fires_once() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        timer.start();
        thread::sleep(Duration::from_millis(150));

        assert!(timer.should_fire(), "Should fire first time");

        // Execute timer
        let _ = timer.execute();

        assert!(!timer.should_fire(), "One-shot should not fire again");
    }

    #[test]
    fn test_repeating_resets() {
        let mut timer = Timer::new(
            "Test",
            TimerType::Repeating,
            0.1,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        timer.start();
        thread::sleep(Duration::from_millis(150));

        assert!(timer.should_fire(), "Should fire first time");

        // Execute timer
        let _ = timer.execute();

        assert!(
            timer.next_fire.is_some(),
            "Repeating timer should reset fire time"
        );
    }

    #[test]
    fn test_execute_send_command() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("check hp".to_string()),
        )
        .unwrap();

        timer.start();

        let commands = timer.execute().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "check hp");
    }

    #[test]
    fn test_execute_multiple_commands() {
        let mut timer = Timer::new(
            "Test",
            TimerType::Repeating,
            5.0,
            TimerAction::SendCommands(vec!["look".to_string(), "inventory".to_string()]),
        )
        .unwrap();

        timer.start();

        let commands = timer.execute().unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], "look");
        assert_eq!(commands[1], "inventory");
    }

    #[test]
    fn test_timer_manager_add() {
        let mut manager = TimerManager::new();
        let timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let result = manager.add_timer(timer);
        assert!(result.is_ok(), "Should add timer to manager");
        assert_eq!(manager.timers.len(), 1);
    }

    #[test]
    fn test_timer_manager_get_ready() {
        let mut manager = TimerManager::new();

        let timer = Timer::new(
            "Quick",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        manager.add_timer(timer).unwrap();

        // No timers ready immediately
        let ready = manager.get_ready_timers();
        assert_eq!(ready.len(), 0);

        // Wait for timer
        thread::sleep(Duration::from_millis(150));

        let ready = manager.get_ready_timers();
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_timer_manager_get_by_id() {
        let mut manager = TimerManager::new();
        let timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let id = timer.id;
        manager.add_timer(timer).unwrap();

        let found = manager.get_timer(id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");
    }

    #[test]
    fn test_timer_manager_remove() {
        let mut manager = TimerManager::new();
        let timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let id = timer.id;
        manager.add_timer(timer).unwrap();

        let result = manager.remove_timer(id);
        assert!(result.is_ok(), "Should remove timer");

        let found = manager.get_timer(id);
        assert!(found.is_none(), "Timer should be removed");
    }

    #[test]
    fn test_time_until_fire() {
        let mut timer = Timer::new(
            "Test",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        assert!(
            timer.time_until_fire().is_none(),
            "Should have no fire time before start"
        );

        timer.start();

        let time_left = timer.time_until_fire();
        assert!(time_left.is_some());
        assert!(time_left.unwrap() <= Duration::from_secs(1));
    }

    #[test]
    fn test_next_fire_time() {
        let mut manager = TimerManager::new();

        let t1 = Timer::new(
            "Fast",
            TimerType::OneShot,
            0.5,
            TimerAction::SendCommand("fast".to_string()),
        )
        .unwrap();

        let t2 = Timer::new(
            "Slow",
            TimerType::OneShot,
            2.0,
            TimerAction::SendCommand("slow".to_string()),
        )
        .unwrap();

        manager.add_timer(t1).unwrap();
        manager.add_timer(t2).unwrap();

        let next = manager.next_fire_time();
        assert!(next.is_some());
        assert!(next.unwrap() <= Duration::from_millis(500));
    }

    #[test]
    fn test_json_serialization() {
        let timer = Timer::new(
            "Test",
            TimerType::Repeating,
            5.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let json = serde_json::to_string(&timer);
        assert!(json.is_ok(), "Should serialize to JSON");
    }

    #[test]
    fn test_json_deserialization() {
        let timer = Timer::new(
            "Test",
            TimerType::Repeating,
            5.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let json = serde_json::to_string(&timer).unwrap();
        let deserialized = serde_json::from_str::<Timer>(&json);

        assert!(deserialized.is_ok(), "Should deserialize from JSON");
        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.name, timer.name);
        assert_eq!(deserialized.interval, timer.interval);
    }
}

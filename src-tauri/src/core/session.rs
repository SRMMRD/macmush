/// Session management and orchestration
///
/// Coordinates Connection, TriggerManager, and EventBus to provide a complete
/// MUD client session with automatic trigger processing and event generation.

use crate::automation::triggers::{Trigger, TriggerManager, TriggerAction};
use crate::automation::{Alias, AliasManager, Timer, TimerManager, Highlight, HighlightManager, VariableManager, CommandHistory, TabCompletion, KeypadMapping, Speedwalk};
use crate::core::{Connection, EventBus, MudEvent, World};
use crate::error::Result;
use crate::scripting::{LuaRuntime, WorldApi};
use std::sync::Arc;
use tracing::{debug, error, info};

/// MUD session orchestrator
pub struct Session {
    connection: Connection,
    trigger_manager: TriggerManager,
    alias_manager: AliasManager,
    timer_manager: TimerManager,
    highlight_manager: HighlightManager,
    variable_manager: VariableManager,
    command_history: CommandHistory,
    tab_completion: TabCompletion,
    keypad_mapping: KeypadMapping,
    speedwalk: Speedwalk,
    event_bus: Arc<EventBus>,
    lua_runtime: LuaRuntime,
    world_api: WorldApi,
}

impl Session {
    /// Create a new session for a world
    pub fn new(world: World, event_bus: Arc<EventBus>) -> Result<Self> {
        debug!("Creating session for world '{}'", world.name);

        let world_id = world.id.to_string();
        let connection = Connection::new(world, event_bus.clone());
        let trigger_manager = TriggerManager::new();
        let alias_manager = AliasManager::new();
        let timer_manager = TimerManager::new();
        let highlight_manager = HighlightManager::new();
        let variable_manager = VariableManager::new();
        let command_history = CommandHistory::new();
        let tab_completion = TabCompletion::new();
        let keypad_mapping = KeypadMapping::new();
        let speedwalk = Speedwalk::new();

        // Initialize Lua runtime and World API
        let lua_runtime = LuaRuntime::new(&world_id)?;
        let world_api = WorldApi::new(&world_id);
        world_api.register_functions(lua_runtime.lua())?;

        Ok(Self {
            connection,
            trigger_manager,
            alias_manager,
            timer_manager,
            highlight_manager,
            variable_manager,
            command_history,
            tab_completion,
            keypad_mapping,
            speedwalk,
            event_bus,
            lua_runtime,
            world_api,
        })
    }

    /// Start the session (connect to MUD)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting session for world '{}'", self.connection.world().name);
        self.connection.connect().await
    }

    /// Stop the session (disconnect from MUD)
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping session for world '{}'", self.connection.world().name);

        // Stop all timers
        self.timer_manager.stop_all();

        self.connection.disconnect().await
    }

    /// Send command to MUD server (processes speedwalk and aliases)
    pub async fn send_command(&mut self, command: impl AsRef<str>) -> Result<()> {
        let input = command.as_ref();

        // Add to command history
        self.command_history.add_command(input);

        // Try speedwalk expansion first
        if let Some(expanded_commands) = self.speedwalk.try_expand(input) {
            debug!("Speedwalk expanded '{}' into {} commands", input, expanded_commands.len());

            // Send each expanded command (they can trigger aliases)
            for cmd in expanded_commands {
                // Recursively process each command (without adding to history)
                // This allows speedwalk commands to trigger aliases
                self.process_command_internal(&cmd).await?;
            }
            return Ok(());
        }

        // Process the actual command (may trigger alias or send directly)
        self.process_command_internal(input).await?;

        Ok(())
    }

    /// Internal command processing (without history addition)
    async fn process_command_internal(&mut self, input: &str) -> Result<()> {
        // Check if input matches an alias and extract all needed data
        let alias_data = if let Some(alias) = self.alias_manager.find_match(input)? {
            debug!("Alias '{}' matched for input: {}", alias.name, input);

            // Execute alias and get commands + captures
            let (commands, captures) = alias.execute(input)?;

            // Extract all data we need from alias before releasing the borrow
            let alias_id = alias.id;
            let alias_name = alias.name.clone();

            // Check if alias has ExecuteScript action
            let script_opt = match &alias.action {
                crate::automation::aliases::AliasAction::ExecuteScript(script) => Some(script.clone()),
                crate::automation::aliases::AliasAction::Sequence(actions) => {
                    actions.iter().find_map(|action| {
                        if let crate::automation::aliases::AliasAction::ExecuteScript(script) = action {
                            Some(script.clone())
                        } else {
                            None
                        }
                    })
                }
                _ => None,
            };

            Some((alias_id, alias_name, commands, captures, script_opt))
        } else {
            None
        };

        // Process alias if matched
        if let Some((alias_id, alias_name, commands, captures, script_opt)) = alias_data {
            // Publish AliasMatched event
            self.event_bus.publish(MudEvent::AliasMatched {
                world_id: self.connection.world().id,
                alias_id,
                alias_name: alias_name.clone(),
                matched_text: input.to_string(),
            })?;

            // Execute Lua script if present
            if let Some(script) = script_opt {
                // Set captures as variables for script access
                for (key, value) in &captures {
                    self.world_api.set_variable(key, value);
                }

                match self.lua_runtime.execute(&script) {
                    Ok(result) => {
                        debug!("Alias script executed successfully: {:?}", result);

                        // Send any commands queued by world.Send()
                        let queued_commands = self.world_api.drain_command_queue();
                        for cmd in queued_commands {
                            debug!("Sending queued command from alias script: {}", cmd);
                            Box::pin(self.process_command_internal(&cmd)).await?;
                        }
                    }
                    Err(e) => {
                        error!("Alias script execution failed: {}", e);
                        self.event_bus.publish(MudEvent::AliasError {
                            world_id: self.connection.world().id,
                            alias_id,
                            error: format!("Script error: {}", e),
                        })?;
                    }
                }
            }

            // Send each command from alias
            if !commands.is_empty() {
                // Publish AliasExecuted event
                self.event_bus.publish(MudEvent::AliasExecuted {
                    world_id: self.connection.world().id,
                    alias_id,
                    commands: commands.clone(),
                })?;

                for cmd in commands {
                    if let Err(e) = self.connection.send_command(&cmd).await {
                        error!("Failed to send alias command '{}': {}", cmd, e);
                        self.event_bus.publish(MudEvent::AliasError {
                            world_id: self.connection.world().id,
                            alias_id,
                            error: e.to_string(),
                        })?;
                    }
                }
            }
        } else {
            // No alias matched, send command directly
            self.connection.send_command(input).await?;
        }

        Ok(())
    }

    /// Add trigger to session
    pub fn add_trigger(&mut self, trigger: Trigger) -> Result<()> {
        debug!("Adding trigger '{}' to session", trigger.name);
        self.trigger_manager.add_trigger(trigger)
    }

    /// Add alias to session
    pub fn add_alias(&mut self, alias: Alias) -> Result<()> {
        debug!("Adding alias '{}' to session", alias.name);
        self.alias_manager.add_alias(alias)
    }

    /// Add timer to session
    pub fn add_timer(&mut self, timer: Timer) -> Result<()> {
        debug!("Adding timer '{}' to session", timer.name);
        self.timer_manager.add_timer(timer)
    }

    /// Add highlight to session
    pub fn add_highlight(&mut self, highlight: Highlight) -> Result<()> {
        debug!("Adding highlight '{}' to session", highlight.name);
        self.highlight_manager.add_highlight(highlight)
    }

    /// Remove trigger from session by ID
    pub fn remove_trigger(&mut self, id: uuid::Uuid) -> Result<()> {
        debug!("Removing trigger {} from session", id);
        self.trigger_manager.remove_trigger(id)
    }

    /// Remove alias from session by ID
    pub fn remove_alias(&mut self, id: uuid::Uuid) -> Result<()> {
        debug!("Removing alias {} from session", id);
        self.alias_manager.remove_alias(id)
    }

    /// Remove timer from session by ID
    pub fn remove_timer(&mut self, id: uuid::Uuid) -> Result<()> {
        debug!("Removing timer {} from session", id);
        self.timer_manager.remove_timer(id)
    }

    /// Remove highlight from session by ID
    pub fn remove_highlight(&mut self, id: uuid::Uuid) -> Result<()> {
        debug!("Removing highlight {} from session", id);
        self.highlight_manager.remove_highlight(id)
    }

    /// Update trigger in session
    pub fn update_trigger(&mut self, trigger: Trigger) -> Result<()> {
        debug!("Updating trigger '{}' in session", trigger.name);
        // Remove old trigger and add updated one
        self.trigger_manager.remove_trigger(trigger.id)?;
        self.trigger_manager.add_trigger(trigger)
    }

    /// Update alias in session
    pub fn update_alias(&mut self, alias: Alias) -> Result<()> {
        debug!("Updating alias '{}' in session", alias.name);
        // Remove old alias and add updated one
        self.alias_manager.remove_alias(alias.id)?;
        self.alias_manager.add_alias(alias)
    }

    /// Update timer in session
    pub fn update_timer(&mut self, timer: Timer) -> Result<()> {
        debug!("Updating timer '{}' in session", timer.name);
        // Remove old timer and add updated one
        self.timer_manager.remove_timer(timer.id)?;
        self.timer_manager.add_timer(timer)
    }

    /// Update highlight in session
    pub fn update_highlight(&mut self, highlight: Highlight) -> Result<()> {
        debug!("Updating highlight '{}' in session", highlight.name);
        // Remove old highlight and add updated one
        self.highlight_manager.remove_highlight(highlight.id)?;
        self.highlight_manager.add_highlight(highlight)
    }

    /// Get trigger by ID
    pub fn get_trigger(&self, id: uuid::Uuid) -> Option<&Trigger> {
        self.trigger_manager.get_trigger(id)
    }

    /// Get alias by ID
    pub fn get_alias(&self, id: uuid::Uuid) -> Option<&Alias> {
        self.alias_manager.get_alias(id)
    }

    /// Get timer by ID
    pub fn get_timer(&self, id: uuid::Uuid) -> Option<&Timer> {
        self.timer_manager.get_timer(id)
    }

    /// Get highlight by ID
    pub fn get_highlight(&self, id: uuid::Uuid) -> Option<&Highlight> {
        self.highlight_manager.get_highlight(id)
    }

    /// Process timers: check for ready timers and execute them
    pub async fn process_timers(&mut self) -> Result<()> {
        let ready_timers = self.timer_manager.get_ready_timers();

        // Extract all timer data before processing to avoid borrow checker issues
        let timer_data: Vec<_> = ready_timers
            .into_iter()
            .map(|timer| {
                // Extract script option
                let script_opt = match &timer.action {
                    crate::automation::timers::TimerAction::ExecuteScript(script) => Some(script.clone()),
                    crate::automation::timers::TimerAction::Sequence(actions) => {
                        actions.iter().find_map(|action| {
                            if let crate::automation::timers::TimerAction::ExecuteScript(script) = action {
                                Some(script.clone())
                            } else {
                                None
                            }
                        })
                    }
                    _ => None,
                };

                // Execute timer to get commands
                let commands = timer.execute().unwrap_or_default();

                (timer.id, timer.name.clone(), script_opt, commands)
            })
            .collect();

        // Now process each timer with all data extracted
        for (timer_id, timer_name, script_opt, commands) in timer_data {
            debug!("Timer '{}' is ready to fire", timer_name);

            // Execute Lua script if present
            if let Some(script) = script_opt {
                match self.lua_runtime.execute(&script) {
                    Ok(result) => {
                        debug!("Timer script executed successfully: {:?}", result);

                        // Send any commands queued by world.Send()
                        let queued_commands = self.world_api.drain_command_queue();
                        for cmd in queued_commands {
                            debug!("Sending queued command from timer script: {}", cmd);
                            Box::pin(self.process_command_internal(&cmd)).await?;
                        }
                    }
                    Err(e) => {
                        error!("Timer '{}' script execution failed: {}", timer_name, e);
                        self.event_bus.publish(MudEvent::TimerError {
                            world_id: self.connection.world().id,
                            timer_id,
                            error: format!("Script error: {}", e),
                        })?;
                    }
                }
            }

            // Send timer commands
            if !commands.is_empty() {
                // Publish TimerExecuted event
                self.event_bus.publish(MudEvent::TimerExecuted {
                    world_id: self.connection.world().id,
                    timer_id,
                    commands: commands.clone(),
                })?;

                // Send each command
                for command in commands {
                    if let Err(e) = self.connection.send_command(&command).await {
                        error!("Failed to send timer command '{}': {}", command, e);
                        self.event_bus.publish(MudEvent::TimerError {
                            world_id: self.connection.world().id,
                            timer_id,
                            error: e.to_string(),
                        })?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Process incoming data: receive, match triggers, execute actions
    pub async fn process_incoming_data(&mut self) -> Result<()> {
        // Receive data from connection
        let data = self.connection.receive().await?;
        let text = String::from_utf8_lossy(&data).to_string();

        debug!("Processing {} bytes of data", data.len());

        // Feed text to tab-completion system
        self.tab_completion.add_output(&text);

        // Process highlights and get styled text segments
        let highlight_matches = self.highlight_manager.get_all_matches(&text)?;

        // Publish DataReceived event for frontend display (with highlight info)
        self.event_bus.publish(MudEvent::DataReceived {
            world_id: self.connection.world().id,
            data: data.clone(),
            text: text.clone(),
        })?;

        // Publish highlights if any matched
        if !highlight_matches.is_empty() {
            debug!("Publishing {} highlight matches", highlight_matches.len());
            self.event_bus.publish(MudEvent::HighlightMatched {
                world_id: self.connection.world().id,
                matches: highlight_matches,
            })?;
        }

        // Find matching triggers
        let matches = self.trigger_manager.find_matches(&text)?;

        if !matches.is_empty() {
            info!("Found {} matching trigger(s)", matches.len());
        }

        // Execute each matching trigger
        // Clone trigger IDs to avoid borrow checker issues when extracting captures
        let trigger_ids: Vec<uuid::Uuid> = matches.iter().map(|t| t.id).collect();

        for trigger_id in trigger_ids {
            // Get mutable reference to extract captures
            let (trigger_name, script_opt, captures) = {
                if let Some(trigger) = self.trigger_manager.get_trigger_mut(trigger_id) {
                    debug!("Executing trigger '{}'", trigger.name);

                    // Extract capture groups for script access
                    let captures = trigger.extract_captures(&text)?;

                    // Check if trigger has ExecuteScript action
                    let script_opt = match &trigger.action {
                        TriggerAction::ExecuteScript(script) => Some(script.clone()),
                        TriggerAction::Sequence(actions) => {
                            actions.iter().find_map(|action| {
                                if let TriggerAction::ExecuteScript(script) = action {
                                    Some(script.clone())
                                } else {
                                    None
                                }
                            })
                        }
                        _ => None,
                    };

                    (trigger.name.clone(), script_opt, captures)
                } else {
                    continue;
                }
            };

            // Publish TriggerMatched event
            self.event_bus.publish(MudEvent::TriggerMatched {
                world_id: self.connection.world().id,
                trigger_id,
                trigger_name: trigger_name.clone(),
                matched_text: text.to_string(),
            })?;

            if let Some(script) = script_opt {
                // Set captures as variables for script access
                for (key, value) in &captures {
                    self.world_api.set_variable(key, value);
                }

                // Execute Lua script
                match self.lua_runtime.execute(&script) {
                    Ok(result) => {
                        debug!("Trigger script executed successfully: {:?}", result);

                        // Send any commands queued by world.Send()
                        let queued_commands = self.world_api.drain_command_queue();
                        for cmd in queued_commands {
                            debug!("Sending queued command from trigger script: {}", cmd);
                            Box::pin(self.process_command_internal(&cmd)).await?;
                        }
                    }
                    Err(e) => {
                        error!("Trigger script execution failed: {}", e);
                        self.event_bus.publish(MudEvent::TriggerError {
                            world_id: self.connection.world().id,
                            trigger_id,
                            error: format!("Script error: {}", e),
                        })?;
                    }
                }
            }

            // Execute trigger and get commands (get trigger reference again)
            if let Some(trigger) = self.trigger_manager.get_trigger(trigger_id) {
                match trigger.execute() {
                    Ok(commands) => {
                        if !commands.is_empty() {
                            // Publish TriggerExecuted event
                            self.event_bus.publish(MudEvent::TriggerExecuted {
                                world_id: self.connection.world().id,
                                trigger_id: trigger.id,
                                commands: commands.clone(),
                            })?;

                            // Send each command
                            for command in commands {
                                if let Err(e) = self.connection.send_command(&command).await {
                                    error!("Failed to send trigger command '{}': {}", command, e);
                                    // Publish TriggerError event
                                    self.event_bus.publish(MudEvent::TriggerError {
                                        world_id: self.connection.world().id,
                                        trigger_id: trigger.id,
                                        error: e.to_string(),
                                    })?;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Trigger '{}' execution failed: {}", trigger.name, e);
                        // Publish TriggerError event
                        self.event_bus.publish(MudEvent::TriggerError {
                            world_id: self.connection.world().id,
                            trigger_id: trigger.id,
                            error: e.to_string(),
                        })?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if session is connected
    pub fn is_connected(&self) -> bool {
        self.connection.is_connected()
    }

    /// Get reference to the world
    pub fn world(&self) -> &World {
        self.connection.world()
    }

    /// Get tab-completion matches for a partial word
    pub fn get_completions(&self, partial: &str) -> Vec<String> {
        self.tab_completion.get_completions(partial)
    }

    /// Get the best tab-completion match
    pub fn get_best_completion(&self, partial: &str) -> Option<String> {
        self.tab_completion.get_best_match(partial)
    }

    /// Cycle through tab-completion matches
    pub fn cycle_completion(&self, partial: &str, current: Option<&str>) -> Option<String> {
        self.tab_completion.cycle_completion(partial, current)
    }

    /// Get keypad mapping reference
    pub fn keypad_mapping(&self) -> &KeypadMapping {
        &self.keypad_mapping
    }

    /// Get mutable keypad mapping reference
    pub fn keypad_mapping_mut(&mut self) -> &mut KeypadMapping {
        &mut self.keypad_mapping
    }

    /// Execute keypad key press
    pub async fn execute_keypad_key(
        &mut self,
        key: crate::automation::KeypadKey,
        modifier: crate::automation::KeypadModifier,
    ) -> Result<()> {
        // Clone the command to avoid borrow checker issues
        let command_opt = self.keypad_mapping.get_command(key, modifier).map(|s| s.to_string());

        if let Some(command) = command_opt {
            if !command.is_empty() {
                debug!("Executing keypad command: {}", command);
                self.send_command(&command).await?;
            }
        }
        Ok(())
    }

    /// Set a variable value
    pub fn set_variable(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.variable_manager.set_variable(name, value);
    }

    /// Get a variable value
    pub fn get_variable(&self, name: impl AsRef<str>) -> Option<String> {
        self.variable_manager.get_variable(name)
    }

    /// Delete a variable
    pub fn delete_variable(&mut self, name: impl AsRef<str>) -> bool {
        self.variable_manager.delete_variable(name)
    }

    /// Check if a variable exists
    pub fn has_variable(&self, name: impl AsRef<str>) -> bool {
        self.variable_manager.has_variable(name)
    }

    /// Get all variable names
    pub fn get_variable_list(&self) -> Vec<String> {
        self.variable_manager.get_variable_list()
    }

    /// Get all variables as a map
    pub fn get_all_variables(&self) -> std::collections::HashMap<String, String> {
        self.variable_manager.get_all_variables()
    }

    /// Clear all variables
    pub fn clear_variables(&mut self) {
        self.variable_manager.clear_all();
    }

    /// Get reference to variable manager
    pub fn variable_manager(&self) -> &VariableManager {
        &self.variable_manager
    }

    /// Get mutable reference to variable manager
    pub fn variable_manager_mut(&mut self) -> &mut VariableManager {
        &mut self.variable_manager
    }

    /// Get previous command from history (navigate up)
    pub fn get_previous_command(&mut self, current_input: Option<String>) -> Option<String> {
        self.command_history.get_previous(current_input)
    }

    /// Get next command from history (navigate down)
    pub fn get_next_command(&mut self) -> Option<String> {
        self.command_history.get_next()
    }

    /// Reset command history navigation
    pub fn reset_command_history_position(&mut self) {
        self.command_history.reset_position();
    }

    /// Get all commands from history
    pub fn get_command_history(&self) -> Vec<String> {
        self.command_history.get_all_commands()
    }

    /// Clear command history
    pub fn clear_command_history(&mut self) {
        self.command_history.clear();
    }

    /// Get command history count
    pub fn command_history_count(&self) -> usize {
        self.command_history.count()
    }

    /// Set command history max size
    pub fn set_command_history_max_size(&mut self, max_size: usize) {
        self.command_history.set_max_size(max_size);
    }

    /// Get reference to command history
    pub fn command_history(&self) -> &CommandHistory {
        &self.command_history
    }

    /// Get mutable reference to command history
    pub fn command_history_mut(&mut self) -> &mut CommandHistory {
        &mut self.command_history
    }

    // Speedwalk convenience methods

    /// Check if speedwalking is enabled
    pub fn is_speedwalk_enabled(&self) -> bool {
        self.speedwalk.is_enabled()
    }

    /// Enable or disable speedwalking
    pub fn set_speedwalk_enabled(&mut self, enabled: bool) {
        self.speedwalk.set_enabled(enabled);
    }

    /// Add or update a speedwalk direction mapping
    pub fn add_speedwalk_direction(&mut self, short: impl Into<String>, full: impl Into<String>) {
        self.speedwalk.add_direction(short, full);
    }

    /// Remove a speedwalk direction mapping
    pub fn remove_speedwalk_direction(&mut self, short: &str) -> bool {
        self.speedwalk.remove_direction(short)
    }

    /// Get reference to speedwalk
    pub fn speedwalk(&self) -> &Speedwalk {
        &self.speedwalk
    }

    /// Get mutable reference to speedwalk
    pub fn speedwalk_mut(&mut self) -> &mut Speedwalk {
        &mut self.speedwalk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::triggers::TriggerAction;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Helper: Start a mock MUD server
    async fn start_mock_server() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn test_create_session() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let session = Session::new(world.clone(), event_bus).unwrap();

        assert_eq!(session.world().name, "Test MUD");
        assert_eq!(session.world().host, "mud.example.com");
        assert_eq!(session.world().port, 4000);
        assert!(!session.is_connected());
    }

    #[tokio::test]
    async fn test_session_start_stop() {
        let (listener, port) = start_mock_server().await;

        // Accept connection in background
        tokio::spawn(async move {
            let _accept = listener.accept().await;
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        // Start session
        let result = session.start().await;
        assert!(result.is_ok(), "Should start session successfully");
        assert!(session.is_connected(), "Session should be connected");

        // Stop session
        let result = session.stop().await;
        assert!(result.is_ok(), "Should stop session successfully");
        assert!(!session.is_connected(), "Session should be disconnected");
    }

    #[tokio::test]
    async fn test_session_send_command() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send command
        let result = session.send_command("look").await;
        assert!(result.is_ok(), "Should send command successfully");
    }

    #[tokio::test]
    async fn test_session_add_trigger() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        let trigger = Trigger::new(
            "Test Trigger",
            "^You see",
            TriggerAction::SendCommand("look".to_string()),
        )
        .unwrap();

        let result = session.add_trigger(trigger);
        assert!(result.is_ok(), "Should add trigger successfully");
    }

    #[tokio::test]
    async fn test_session_process_incoming_data_with_trigger() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Send data that will match trigger
                let _ = socket.write_all(b"You see a door\n").await;

                // Read command sent by trigger
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add trigger that matches "You see"
        let trigger = Trigger::new(
            "Door Trigger",
            "^You see",
            TriggerAction::SendCommand("open door".to_string()),
        )
        .unwrap();
        session.add_trigger(trigger).unwrap();

        // Process incoming data
        let result = session.process_incoming_data().await;
        assert!(result.is_ok(), "Should process data successfully");

        // Verify events were published
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: Connected, DataReceived, TriggerMatched, TriggerExecuted, CommandSent
        assert!(
            events.len() >= 4,
            "Should publish at least 4 events (found {})",
            events.len()
        );

        // Verify TriggerMatched event
        let trigger_matched = events
            .iter()
            .any(|e| matches!(e, MudEvent::TriggerMatched { .. }));
        assert!(trigger_matched, "Should publish TriggerMatched event");

        // Verify TriggerExecuted event
        let trigger_executed = events
            .iter()
            .any(|e| matches!(e, MudEvent::TriggerExecuted { .. }));
        assert!(trigger_executed, "Should publish TriggerExecuted event");
    }

    #[tokio::test]
    async fn test_session_process_data_without_trigger_match() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"Nothing special here\n").await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add trigger that won't match
        let trigger = Trigger::new(
            "Door Trigger",
            "^You see a door",
            TriggerAction::SendCommand("open door".to_string()),
        )
        .unwrap();
        session.add_trigger(trigger).unwrap();

        // Process incoming data
        let result = session.process_incoming_data().await;
        assert!(result.is_ok(), "Should process data successfully");

        // Verify no trigger events were published
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        let trigger_matched = events
            .iter()
            .any(|e| matches!(e, MudEvent::TriggerMatched { .. }));
        assert!(!trigger_matched, "Should not publish TriggerMatched event");
    }

    #[tokio::test]
    async fn test_session_alias_matching() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Read command sent by alias
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add alias that matches "gg" and expands to "get gold"
        let alias = Alias::new(
            "Get Gold",
            "^gg$",
            crate::automation::aliases::AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();
        session.add_alias(alias).unwrap();

        // Send user input that matches alias
        let result = session.send_command("gg").await;
        assert!(result.is_ok(), "Should process alias successfully");

        // Verify events were published
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: Connected, AliasMatched, AliasExecuted, CommandSent
        let alias_matched = events
            .iter()
            .any(|e| matches!(e, MudEvent::AliasMatched { .. }));
        assert!(alias_matched, "Should publish AliasMatched event");

        let alias_executed = events
            .iter()
            .any(|e| matches!(e, MudEvent::AliasExecuted { .. }));
        assert!(alias_executed, "Should publish AliasExecuted event");
    }

    #[tokio::test]
    async fn test_session_alias_with_wildcards() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add alias with wildcard that transforms "get X" to "take X"
        let alias = Alias::new(
            "Get Transform",
            r"^get (.+)",
            crate::automation::aliases::AliasAction::SendCommand("take %1".to_string()),
        )
        .unwrap();
        session.add_alias(alias).unwrap();

        // Send command that should match and transform
        let result = session.send_command("get sword").await;
        assert!(result.is_ok(), "Should process alias with wildcards");
    }

    #[tokio::test]
    async fn test_session_no_alias_match() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add alias that won't match
        let alias = Alias::new(
            "Get Gold",
            "^gg$",
            crate::automation::aliases::AliasAction::SendCommand("get gold".to_string()),
        )
        .unwrap();
        session.add_alias(alias).unwrap();

        // Send command that doesn't match alias
        let result = session.send_command("look").await;
        assert!(result.is_ok(), "Should send command directly when no alias matches");

        // Verify no alias events were published
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        let alias_matched = events
            .iter()
            .any(|e| matches!(e, MudEvent::AliasMatched { .. }));
        assert!(!alias_matched, "Should not publish AliasMatched event");
    }

    #[tokio::test]
    async fn test_session_multiple_triggers() {
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"HP: 100/100\n").await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add multiple triggers
        let t1 = Trigger::new(
            "HP Trigger",
            r"^HP: (\d+)/(\d+)",
            TriggerAction::DisplayText("HP updated".to_string()),
        )
        .unwrap();

        let t2 = Trigger::new(
            "HP Low",
            r"^HP: ([0-9]|[1-4][0-9])/",
            TriggerAction::SendCommand("heal".to_string()),
        )
        .unwrap();

        session.add_trigger(t1).unwrap();
        session.add_trigger(t2).unwrap();

        // Process incoming data
        let result = session.process_incoming_data().await;
        assert!(result.is_ok(), "Should process data successfully");

        // Collect events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should match first trigger (HP pattern)
        let trigger_count = events
            .iter()
            .filter(|e| matches!(e, MudEvent::TriggerMatched { .. }))
            .count();
        assert!(trigger_count >= 1, "Should match at least one trigger");
    }

    #[tokio::test]
    async fn test_session_add_timer() {
        use crate::automation::timers::{Timer, TimerType, TimerAction};

        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        let timer = Timer::new(
            "Test Timer",
            TimerType::OneShot,
            1.0,
            TimerAction::SendCommand("test".to_string()),
        )
        .unwrap();

        let result = session.add_timer(timer);
        assert!(result.is_ok(), "Should add timer successfully");
    }

    #[tokio::test]
    async fn test_session_process_timer_send_command() {
        use crate::automation::timers::{Timer, TimerType, TimerAction};
        use std::time::Duration;
        use tokio::time::sleep;

        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Read command sent by timer
                let mut buf = [0u8; 256];
                let _ = socket.read(&mut buf).await;
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add timer that fires after 100ms
        let timer = Timer::new(
            "Quick Timer",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommand("test command".to_string()),
        )
        .unwrap();
        session.add_timer(timer).unwrap();

        // Wait for timer to be ready
        sleep(Duration::from_millis(150)).await;

        // Process timers
        let result = session.process_timers().await;
        assert!(result.is_ok(), "Should process timers successfully");

        // Verify events were published
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have TimerExecuted and CommandSent events
        let timer_executed = events
            .iter()
            .any(|e| matches!(e, MudEvent::TimerExecuted { .. }));
        assert!(timer_executed, "Should publish TimerExecuted event");
    }

    #[tokio::test]
    async fn test_session_process_repeating_timer() {
        use crate::automation::timers::{Timer, TimerType, TimerAction};
        use std::time::Duration;
        use tokio::time::sleep;

        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // Keep reading commands from repeating timer
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add repeating timer that fires every 100ms
        let timer = Timer::new(
            "Repeating Timer",
            TimerType::Repeating,
            0.1,
            TimerAction::SendCommand("heartbeat".to_string()),
        )
        .unwrap();
        session.add_timer(timer).unwrap();

        // Wait and process multiple times
        for _ in 0..3 {
            sleep(Duration::from_millis(120)).await;
            let _ = session.process_timers().await;
        }

        // Verify multiple TimerExecuted events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        let timer_exec_count = events
            .iter()
            .filter(|e| matches!(e, MudEvent::TimerExecuted { .. }))
            .count();
        assert!(
            timer_exec_count >= 2,
            "Repeating timer should fire multiple times (found {})",
            timer_exec_count
        );
    }

    #[tokio::test]
    async fn test_session_timer_multiple_commands() {
        use crate::automation::timers::{Timer, TimerType, TimerAction};
        use std::time::Duration;
        use tokio::time::sleep;

        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut rx = event_bus.subscribe();

        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add timer with multiple commands
        let timer = Timer::new(
            "Multi Command Timer",
            TimerType::OneShot,
            0.1,
            TimerAction::SendCommands(vec![
                "look".to_string(),
                "inventory".to_string(),
                "score".to_string(),
            ]),
        )
        .unwrap();
        session.add_timer(timer).unwrap();

        // Wait and process
        sleep(Duration::from_millis(150)).await;
        let result = session.process_timers().await;
        assert!(result.is_ok(), "Should process timer with multiple commands");

        // Verify TimerExecuted event with multiple commands
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        let timer_event = events
            .iter()
            .find(|e| matches!(e, MudEvent::TimerExecuted { .. }));
        assert!(timer_event.is_some(), "Should publish TimerExecuted event");

        if let Some(MudEvent::TimerExecuted { commands, .. }) = timer_event {
            assert_eq!(commands.len(), 3, "Should have 3 commands");
        }
    }

    #[tokio::test]
    async fn test_session_set_and_get_variable() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        session.set_variable("player_name", "Alice");
        assert_eq!(session.get_variable("player_name"), Some("Alice".to_string()));
    }

    #[tokio::test]
    async fn test_session_variable_case_insensitive() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        session.set_variable("PlayerName", "Bob");

        // Case-insensitive retrieval
        assert_eq!(session.get_variable("playername"), Some("Bob".to_string()));
        assert_eq!(session.get_variable("PLAYERNAME"), Some("Bob".to_string()));
        assert_eq!(session.get_variable("PlayerName"), Some("Bob".to_string()));
    }

    #[tokio::test]
    async fn test_session_delete_variable() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        session.set_variable("temp", "value");
        assert!(session.has_variable("temp"));

        let deleted = session.delete_variable("temp");
        assert!(deleted, "Should successfully delete");
        assert!(!session.has_variable("temp"));
    }

    #[tokio::test]
    async fn test_session_get_all_variables() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        session.set_variable("hp", "100");
        session.set_variable("mp", "50");
        session.set_variable("level", "5");

        let all = session.get_all_variables();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("hp"), Some(&"100".to_string()));
        assert_eq!(all.get("mp"), Some(&"50".to_string()));
        assert_eq!(all.get("level"), Some(&"5".to_string()));
    }

    #[tokio::test]
    async fn test_session_clear_variables() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());

        let mut session = Session::new(world, event_bus).unwrap();

        session.set_variable("var1", "value1");
        session.set_variable("var2", "value2");
        session.set_variable("var3", "value3");

        let list = session.get_variable_list();
        assert_eq!(list.len(), 3);

        session.clear_variables();

        let list = session.get_variable_list();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_session_command_history_basic() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send some commands
        session.send_command("look").await.unwrap();
        session.send_command("north").await.unwrap();
        session.send_command("get sword").await.unwrap();

        // Verify history
        assert_eq!(session.command_history_count(), 3);

        let history = session.get_command_history();
        assert_eq!(history[0], "look");
        assert_eq!(history[1], "north");
        assert_eq!(history[2], "get sword");
    }

    #[tokio::test]
    async fn test_session_command_history_navigation() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send commands
        session.send_command("cmd1").await.unwrap();
        session.send_command("cmd2").await.unwrap();
        session.send_command("cmd3").await.unwrap();

        // Navigate up
        assert_eq!(session.get_previous_command(None), Some("cmd3".to_string()));
        assert_eq!(session.get_previous_command(None), Some("cmd2".to_string()));

        // Navigate down
        assert_eq!(session.get_next_command(), Some("cmd3".to_string()));
        assert_eq!(session.get_next_command(), None);
    }

    #[tokio::test]
    async fn test_session_command_history_clear() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send commands
        session.send_command("cmd1").await.unwrap();
        session.send_command("cmd2").await.unwrap();

        assert_eq!(session.command_history_count(), 2);

        // Clear history
        session.clear_command_history();

        assert_eq!(session.command_history_count(), 0);
    }

    #[tokio::test]
    async fn test_session_command_history_ignores_empty() {
        let world = World::new("Test MUD", "mud.example.com", 4000).unwrap();
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send empty and whitespace commands
        session.send_command("").await.unwrap();
        session.send_command("   ").await.unwrap();
        session.send_command("look").await.unwrap();

        // Should only have "look" in history
        assert_eq!(session.command_history_count(), 1);
        assert_eq!(session.get_command_history()[0], "look");
    }

    // Speedwalk integration tests

    #[tokio::test]
    async fn test_session_speedwalk_simple() {
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        let received_data = Arc::new(tokio::sync::Mutex::new(String::new()));
        let received_data_clone = Arc::clone(&received_data);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                if let Ok(n) = socket.read(&mut buf).await {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    *received_data_clone.lock().await = data;
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send a simple speedwalk command
        session.send_command("4n").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let data = received_data.lock().await;
        let commands: Vec<&str> = data.lines().collect();
        assert_eq!(commands.len(), 4);
        assert!(commands.iter().all(|c| c.contains("north")));
    }

    #[tokio::test]
    async fn test_session_speedwalk_multiple_directions() {
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        let received_data = Arc::new(tokio::sync::Mutex::new(String::new()));
        let received_data_clone = Arc::clone(&received_data);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                if let Ok(n) = socket.read(&mut buf).await {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    *received_data_clone.lock().await = data;
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send speedwalk with multiple directions
        session.send_command("2n 3e").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let data = received_data.lock().await;
        let commands: Vec<&str> = data.lines().collect();
        assert_eq!(commands.len(), 5);
        assert!(commands[0].contains("north"));
        assert!(commands[1].contains("north"));
        assert!(commands[2].contains("east"));
        assert!(commands[3].contains("east"));
        assert!(commands[4].contains("east"));
    }

    #[tokio::test]
    async fn test_session_speedwalk_disabled() {
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        let received_commands = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let received_commands_clone = Arc::clone(&received_commands);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    match socket.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let cmd = String::from_utf8_lossy(&buf[..n]).to_string();
                            received_commands_clone.lock().await.push(cmd);
                        }
                        Err(_) => break,
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Disable speedwalk
        session.set_speedwalk_enabled(false);

        // Send speedwalk command - should be sent as-is
        session.send_command("4n").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let commands = received_commands.lock().await;
        // Should be sent as single "4n" command, not expanded
        assert_eq!(commands.len(), 1);
        assert!(commands[0].contains("4n"));
    }

    #[tokio::test]
    async fn test_session_speedwalk_custom_direction() {
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        let received_data = Arc::new(tokio::sync::Mutex::new(String::new()));
        let received_data_clone = Arc::clone(&received_data);

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                if let Ok(n) = socket.read(&mut buf).await {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    *received_data_clone.lock().await = data;
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Add custom direction
        session.add_speedwalk_direction("in", "enter");

        // Use custom direction
        session.send_command("3in").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let data = received_data.lock().await;
        let commands: Vec<&str> = data.lines().collect();
        assert_eq!(commands.len(), 3);
        assert!(commands.iter().all(|c| c.contains("enter")));
    }

    #[tokio::test]
    async fn test_session_speedwalk_in_history() {
        let event_bus = Arc::new(EventBus::new());
        let (listener, port) = start_mock_server().await;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                loop {
                    let mut buf = [0u8; 256];
                    if socket.read(&mut buf).await.is_err() {
                        break;
                    }
                }
            }
        });

        let world = World::new("Test MUD", "127.0.0.1", port).unwrap();
        let mut session = Session::new(world, event_bus).unwrap();
        session.start().await.unwrap();

        // Send speedwalk command
        session.send_command("4n 2e").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Original speedwalk command should be in history, not expanded versions
        let history = session.get_command_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], "4n 2e");
    }
}

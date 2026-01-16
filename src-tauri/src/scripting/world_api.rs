/// MUSHclient World Object API Implementation
///
/// Implements the MUSHclient scripting API that scripts can call via the `world` object.
/// Reference: https://www.gammon.com.au/scripts/doc.php?general=lua

use crate::error::Result;
use mlua::{Lua, Table};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// World API function registry
pub struct WorldApi {
    world_id: String,
    variables: Arc<Mutex<HashMap<String, String>>>,
    /// Command queue for world.Send() calls
    command_queue: Arc<Mutex<Vec<String>>>,
}

impl WorldApi {
    /// Create new World API instance
    pub fn new(world_id: impl Into<String>) -> Self {
        Self {
            world_id: world_id.into(),
            variables: Arc::new(Mutex::new(HashMap::new())),
            command_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register all world API functions with Lua runtime
    pub fn register_functions(&self, lua: &Lua) -> Result<()> {
        info!("Registering World API functions for world '{}'", self.world_id);

        let world_table = lua.create_table()?;

        // Register world.Note(message)
        self.register_note(lua, &world_table)?;

        // Register world.Send(command)
        self.register_send(lua, &world_table)?;

        // Register world.GetVariable(name)
        self.register_get_variable(lua, &world_table)?;

        // Register world.SetVariable(name, value)
        self.register_set_variable(lua, &world_table)?;

        // Register world.DeleteVariable(name)
        self.register_delete_variable(lua, &world_table)?;

        // Register world.GetVariableList()
        self.register_get_variable_list(lua, &world_table)?;

        // Register world.GetInfo(info_type)
        self.register_get_info(lua, &world_table)?;

        // Set world table as global
        lua.globals().set("world", world_table)?;

        info!("World API functions registered successfully");
        Ok(())
    }

    /// Register world.Note(message) - Display message to user
    fn register_note(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let note_fn = lua.create_function(|_lua, message: String| {
            info!("[Lua Note] {}", message);
            Ok(())
        })?;

        world_table.set("Note", note_fn)?;
        debug!("Registered world.Note()");
        Ok(())
    }

    /// Register world.Send(command) - Send command to MUD
    fn register_send(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let command_queue = Arc::clone(&self.command_queue);

        let send_fn = lua.create_function(move |_lua, command: String| {
            info!("[Lua Send] {}", command);
            let mut queue = command_queue.lock().unwrap();
            queue.push(command);
            Ok(())
        })?;

        world_table.set("Send", send_fn)?;
        debug!("Registered world.Send()");
        Ok(())
    }

    /// Register world.GetVariable(name) - Get script variable
    fn register_get_variable(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let variables = Arc::clone(&self.variables);

        let get_var_fn = lua.create_function(move |_lua, name: String| {
            let variables = variables.lock().unwrap();
            Ok(variables.get(&name).cloned())
        })?;

        world_table.set("GetVariable", get_var_fn)?;
        debug!("Registered world.GetVariable()");
        Ok(())
    }

    /// Register world.SetVariable(name, value) - Set script variable
    fn register_set_variable(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let variables = Arc::clone(&self.variables);

        let set_var_fn = lua.create_function(move |_lua, (name, value): (String, String)| {
            let mut variables = variables.lock().unwrap();
            variables.insert(name.clone(), value.clone());
            debug!("Set variable '{}' = '{}'", name, value);
            Ok(())
        })?;

        world_table.set("SetVariable", set_var_fn)?;
        debug!("Registered world.SetVariable()");
        Ok(())
    }

    /// Register world.DeleteVariable(name) - Delete script variable
    fn register_delete_variable(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let variables = Arc::clone(&self.variables);

        let delete_var_fn = lua.create_function(move |_lua, name: String| {
            let mut variables = variables.lock().unwrap();
            variables.remove(&name);
            debug!("Deleted variable '{}'", name);
            Ok(())
        })?;

        world_table.set("DeleteVariable", delete_var_fn)?;
        debug!("Registered world.DeleteVariable()");
        Ok(())
    }

    /// Register world.GetVariableList() - Get all variable names
    fn register_get_variable_list(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let variables = Arc::clone(&self.variables);

        let get_list_fn = lua.create_function(move |lua, ()| {
            let variables = variables.lock().unwrap();
            let table = lua.create_table()?;

            for (i, key) in variables.keys().enumerate() {
                table.set(i + 1, key.clone())?;
            }

            Ok(table)
        })?;

        world_table.set("GetVariableList", get_list_fn)?;
        debug!("Registered world.GetVariableList()");
        Ok(())
    }

    /// Register world.GetInfo(info_type) - Get world information
    fn register_get_info(&self, lua: &Lua, world_table: &Table) -> Result<()> {
        let world_id = self.world_id.clone();

        let get_info_fn = lua.create_function(move |_lua, info_type: i64| {
            // MUSHclient GetInfo constants
            let result = match info_type {
                1 => Some(world_id.clone()),                  // World name
                2 => Some("127.0.0.1".to_string()),           // Host
                3 => Some("4000".to_string()),                // Port
                20 => Some("MACMush".to_string()),   // Client name
                21 => Some("0.1.0".to_string()),              // Client version
                _ => None,
            };

            Ok(result)
        })?;

        world_table.set("GetInfo", get_info_fn)?;
        debug!("Registered world.GetInfo()");
        Ok(())
    }

    /// Get variable value (for Rust code access)
    pub fn get_variable(&self, name: &str) -> Option<String> {
        self.variables.lock().unwrap().get(name).cloned()
    }

    /// Set variable value (for Rust code access)
    pub fn set_variable(&self, name: impl Into<String>, value: impl Into<String>) {
        self.variables.lock().unwrap().insert(name.into(), value.into());
    }

    /// Delete variable (for Rust code access)
    pub fn delete_variable(&self, name: &str) -> Option<String> {
        self.variables.lock().unwrap().remove(name)
    }

    /// Get all variables (for Rust code access)
    pub fn get_all_variables(&self) -> HashMap<String, String> {
        self.variables.lock().unwrap().clone()
    }

    /// Get and clear all queued commands from world.Send() calls
    pub fn drain_command_queue(&self) -> Vec<String> {
        let mut queue = self.command_queue.lock().unwrap();
        std::mem::take(&mut *queue)
    }

    /// Get queued commands without clearing them (for testing)
    pub fn get_command_queue(&self) -> Vec<String> {
        self.command_queue.lock().unwrap().clone()
    }

    /// Clear the command queue
    pub fn clear_command_queue(&self) {
        self.command_queue.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_api_creation() {
        let api = WorldApi::new("test-world");
        assert_eq!(api.world_id, "test-world");
    }

    #[test]
    fn test_variable_operations() {
        let api = WorldApi::new("test-world");

        // Set variable
        api.set_variable("test_key", "test_value");

        // Get variable
        let value = api.get_variable("test_key");
        assert_eq!(value, Some("test_value".to_string()));

        // Delete variable
        let deleted = api.delete_variable("test_key");
        assert_eq!(deleted, Some("test_value".to_string()));

        // Verify deleted
        let value = api.get_variable("test_key");
        assert_eq!(value, None);
    }

    #[test]
    fn test_get_all_variables() {
        let api = WorldApi::new("test-world");

        api.set_variable("key1", "value1");
        api.set_variable("key2", "value2");
        api.set_variable("key3", "value3");

        let all_vars = api.get_all_variables();
        assert_eq!(all_vars.len(), 3);
        assert_eq!(all_vars.get("key1"), Some(&"value1".to_string()));
        assert_eq!(all_vars.get("key2"), Some(&"value2".to_string()));
        assert_eq!(all_vars.get("key3"), Some(&"value3".to_string()));
    }

    #[test]
    fn test_register_functions() {
        let api = WorldApi::new("test-world");
        let lua = Lua::new();

        let result = api.register_functions(&lua);
        assert!(result.is_ok(), "Failed to register functions");

        // Verify world table exists
        let world: Table = lua.globals().get("world").unwrap();

        // Verify Note function is registered
        assert!(!world.get::<mlua::Value>("Note").unwrap().is_nil());

        // Verify Send function is registered
        assert!(!world.get::<mlua::Value>("Send").unwrap().is_nil());

        // Verify GetVariable function is registered
        assert!(!world.get::<mlua::Value>("GetVariable").unwrap().is_nil());

        // Verify SetVariable function is registered
        assert!(!world.get::<mlua::Value>("SetVariable").unwrap().is_nil());
    }

    #[test]
    fn test_lua_note_function() {
        let api = WorldApi::new("test-world");
        let lua = Lua::new();

        api.register_functions(&lua).unwrap();

        // Call world.Note()
        let _result: () = lua
            .load(r#"world.Note("Test message")"#)
            .eval()
            .unwrap();

        // Should not panic or error
    }

    #[test]
    fn test_lua_get_info() {
        let api = WorldApi::new("test-world");
        let lua = Lua::new();

        api.register_functions(&lua).unwrap();

        // Get world name (info_type = 1)
        let result: Option<String> = lua
            .load(r#"return world.GetInfo(1)"#)
            .eval()
            .unwrap();

        assert_eq!(result, Some("test-world".to_string()));

        // Get client name (info_type = 20)
        let result: Option<String> = lua
            .load(r#"return world.GetInfo(20)"#)
            .eval()
            .unwrap();

        assert_eq!(result, Some("MACMush".to_string()));
    }

    #[test]
    fn test_lua_send_queue() {
        let api = WorldApi::new("test-world");
        let lua = Lua::new();

        api.register_functions(&lua).unwrap();

        // Send multiple commands via Lua
        lua.load(r#"
            world.Send("north")
            world.Send("east")
            world.Send("look")
        "#)
        .exec()
        .unwrap();

        // Get queued commands
        let commands = api.get_command_queue();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0], "north");
        assert_eq!(commands[1], "east");
        assert_eq!(commands[2], "look");

        // Drain and verify
        let drained = api.drain_command_queue();
        assert_eq!(drained, commands);

        // Queue should be empty after drain
        assert_eq!(api.get_command_queue().len(), 0);
    }

    #[test]
    fn test_lua_send_and_variables() {
        let api = WorldApi::new("test-world");
        let lua = Lua::new();

        api.register_functions(&lua).unwrap();

        // Set variable and use it in command
        lua.load(r#"
            world.SetVariable("target", "orc")
            local target = world.GetVariable("target")
            world.Send("kill " .. target)
        "#)
        .exec()
        .unwrap();

        // Verify command was queued with variable value
        let commands = api.get_command_queue();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], "kill orc");
    }
}

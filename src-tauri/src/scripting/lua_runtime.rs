/// Lua runtime integration for MUSHClient
///
/// Provides a safe Rust wrapper around mlua for executing Lua scripts
/// with the MUSHclient world object API.

use crate::error::{MushError, Result};
use mlua::{Lua, Table, Value};
use tracing::{debug, error, info};

/// Lua script execution context
pub struct LuaRuntime {
    lua: Lua,
    world_id: String,
}

impl LuaRuntime {
    /// Create a new Lua runtime for a world
    pub fn new(world_id: impl Into<String>) -> Result<Self> {
        let world_id = world_id.into();
        info!("Creating Lua runtime for world '{}'", world_id);

        let lua = Lua::new();

        // Create the world object table
        let world_table = lua.create_table()?;

        // Store world_id in the Lua registry for access by API functions
        lua.set_named_registry_value("__mushclient_world_id", world_id.clone())?;
        lua.set_named_registry_value("__mushclient_world_table", world_table)?;

        debug!("Lua runtime initialized for world '{}'", world_id);

        Ok(Self { lua, world_id })
    }

    /// Get the Lua VM reference
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Get the world ID
    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    /// Execute Lua code and return result
    pub fn execute(&self, code: &str) -> Result<Option<String>> {
        debug!("Executing Lua code for world '{}'", self.world_id);

        match self.lua.load(code).eval::<Value>() {
            Ok(value) => {
                // Convert Lua value to string representation
                let result = match value {
                    Value::Nil => None,
                    Value::Boolean(b) => Some(b.to_string()),
                    Value::Integer(i) => Some(i.to_string()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::String(s) => Some(s.to_str()?.to_string()),
                    Value::Table(_) => Some("[table]".to_string()),
                    Value::Function(_) => Some("[function]".to_string()),
                    Value::Thread(_) => Some("[thread]".to_string()),
                    Value::UserData(_) => Some("[userdata]".to_string()),
                    Value::LightUserData(_) => Some("[lightuserdata]".to_string()),
                    Value::Error(e) => return Err(MushError::Internal(e.to_string())),
                    _ => Some("[other]".to_string()),
                };

                debug!("Lua execution completed: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                error!("Lua execution error: {}", e);
                Err(MushError::Internal(format!("Lua error: {}", e)))
            }
        }
    }

    /// Execute Lua code without returning result
    pub fn execute_no_result(&self, code: &str) -> Result<()> {
        self.execute(code)?;
        Ok(())
    }

    /// Register a Rust function as a Lua global (simple string->string functions)
    pub fn register_function<F>(&self, name: &str, func: F) -> Result<()>
    where
        F: Fn(&Lua, String) -> mlua::Result<String> + Send + 'static,
    {
        let lua_func = self.lua.create_function(move |lua, input: String| {
            func(lua, input)
        })?;
        self.lua.globals().set(name, lua_func)?;
        debug!("Registered Lua function '{}'", name);
        Ok(())
    }

    /// Get the world table
    pub fn world_table(&self) -> Result<Table> {
        self.lua
            .named_registry_value::<Table>("__mushclient_world_table")
            .map_err(|e| MushError::Internal(format!("Failed to get world table: {}", e)))
    }

    /// Set a value in the world table (string values)
    pub fn set_world_value<K: Into<String>, V: Into<String>>(
        &self,
        key: K,
        value: V,
    ) -> Result<()> {
        let world_table = self.world_table()?;
        let key_str = key.into();
        let value_str = value.into();

        world_table.set(key_str.clone(), value_str)?;
        debug!("Set world.{} value", key_str);
        Ok(())
    }

    /// Get a value from the world table
    pub fn get_world_value<K: Into<String>>(&self, key: K) -> Result<Option<String>> {
        let world_table = self.world_table()?;
        let key_str = key.into();

        match world_table.get::<Value>(key_str)? {
            Value::Nil => Ok(None),
            Value::Boolean(b) => Ok(Some(b.to_string())),
            Value::Integer(i) => Ok(Some(i.to_string())),
            Value::Number(n) => Ok(Some(n.to_string())),
            Value::String(s) => Ok(Some(s.to_str()?.to_string())),
            _ => Ok(Some("[complex value]".to_string())),
        }
    }

    /// Register the world table as a global
    pub fn register_world_global(&self) -> Result<()> {
        let world_table = self.world_table()?;
        self.lua.globals().set("world", world_table)?;
        debug!("Registered 'world' global object");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_runtime() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        assert_eq!(runtime.world_id(), "test-world");
    }

    #[test]
    fn test_execute_simple_expression() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        let result = runtime.execute("return 2 + 2").unwrap();
        assert_eq!(result, Some("4".to_string()));
    }

    #[test]
    fn test_execute_string() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        let result = runtime.execute("return 'hello'").unwrap();
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn test_execute_nil() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        let result = runtime.execute("return nil").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_execute_error() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        let result = runtime.execute("return invalid_function()");
        assert!(result.is_err());
    }

    #[test]
    fn test_world_table_access() {
        let runtime = LuaRuntime::new("test-world").unwrap();
        runtime.register_world_global().unwrap();

        // Set a value through Rust
        runtime.set_world_value("test_key", "test_value").unwrap();

        // Get it back through Lua
        let result = runtime.execute("return world.test_key").unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[test]
    fn test_register_function() {
        let runtime = LuaRuntime::new("test-world").unwrap();

        // Register a simple function
        runtime.register_function("greet", |_lua, name: String| {
            Ok(format!("Hello, {}!", name))
        }).unwrap();

        let result = runtime.execute("return greet('World')").unwrap();
        assert_eq!(result, Some("Hello, World!".to_string()));
    }

    #[test]
    fn test_lua_variables() {
        let runtime = LuaRuntime::new("test-world").unwrap();

        // Set variable
        runtime.execute("test_var = 42").unwrap();

        // Get variable
        let result = runtime.execute("return test_var").unwrap();
        assert_eq!(result, Some("42".to_string()));
    }

    #[test]
    fn test_lua_functions() {
        let runtime = LuaRuntime::new("test-world").unwrap();

        // Define function
        runtime.execute("function double(x) return x * 2 end").unwrap();

        // Call function
        let result = runtime.execute("return double(21)").unwrap();
        assert_eq!(result, Some("42".to_string()));
    }
}

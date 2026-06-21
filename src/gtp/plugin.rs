//! Plugin manager for GTP plugins
//!
//! This module will implement plugin lifecycle management, including:
//! - Starting plugin processes
//! - Handshake protocol (hello/ready)
//! - Method invocation (call/result)
//! - Event handling
//!
//! TODO: Full implementation in Phase 3

use crate::gtp::frame::{Frame, GtpError, Value};
use crate::gtp::transport::Transport;
use std::collections::HashMap;
use std::io;

/// Plugin manager - manages multiple plugin instances
pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
}

/// A single plugin instance
pub struct Plugin {
    pub name: String,
    pub capabilities: Vec<String>,
    pub modules: HashMap<String, Vec<String>>, // module -> methods
                                               // TODO: Add transport and process handle
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Load plugins from a configuration file
    pub fn load_from_config(&mut self, _config_path: &str) -> io::Result<()> {
        // TODO: Implement in Phase 3
        Ok(())
    }

    /// Spawn a single plugin
    pub fn spawn_plugin(
        &mut self,
        _name: &str,
        _command: &str,
        _args: &[String],
    ) -> io::Result<()> {
        // TODO: Implement in Phase 3
        Ok(())
    }

    /// Call a plugin method
    pub fn call(&mut self, _module: &str, _method: &str, _args: Vec<Value>) -> io::Result<Value> {
        // TODO: Implement in Phase 3
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Plugin manager not yet implemented",
        ))
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

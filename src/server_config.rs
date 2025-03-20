use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use uuid::Uuid;

/// Type alias for convenience when accessing global state
#[allow(non_upper_case_globals)]
pub const g: fn() -> () = || ();

/// Configuration for an MCP server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub description: Option<String>,
    pub is_default: bool,
}

/// Collection of server configurations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfigs {
    pub servers: Vec<ServerConfig>,
}

impl ServerConfig {
    /// Create a new server configuration
    pub fn new(
        name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        description: Option<String>,
        is_default: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            command,
            args,
            env,
            description,
            is_default,
        }
    }

    /// Create a default filesystem server configuration
    pub fn default_filesystem() -> Self {
        let mut env = HashMap::new();
        
        Self {
            id: "filesystem".to_string(),
            name: "Filesystem MCP".to_string(),
            command: "docker".to_string(),
            args: vec![
                "run".to_string(),
                "-i".to_string(),
                "--rm".to_string(),
                "--mount".to_string(),
                "type=bind,src=/Users/martinmaurer/Desktop,dst=/Users/martinmaurer/Desktop".to_string(),
                "--mount".to_string(),
                "type=bind,src=/Users/martinmaurer/Projects,dst=/Users/martinmaurer/Projects".to_string(),
                "mcp/filesystem".to_string(),
                "/Users/martinmaurer/Desktop".to_string(),
                "/Users/martinmaurer/Projects".to_string(),
            ],
            env,
            description: Some("Default filesystem MCP provider".to_string()),
            is_default: true,
        }
    }
}

impl Default for ServerConfigs {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
        }
    }
}

impl ServerConfigs {
    /// Initialize with default server configurations
    pub fn initialize_default() -> Self {
        Self {
            servers: vec![ServerConfig::default_filesystem()],
        }
    }

    /// Load server configurations from a file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let configs: ServerConfigs = serde_json::from_str(&contents)?;
        Ok(configs)
    }

    /// Save server configurations to a file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Get a server configuration by ID
    pub fn get_by_id(&self, id: &str) -> Option<&ServerConfig> {
        self.servers.iter().find(|s| s.id == id)
    }

    /// Get the default server configuration
    pub fn get_default(&self) -> Option<&ServerConfig> {
        self.servers.iter().find(|s| s.is_default)
    }

    /// Add a new server configuration
    pub fn add_server(&mut self, mut server: ServerConfig) {
        // If this is set as default, unset any existing default
        if server.is_default {
            for existing in &mut self.servers {
                existing.is_default = false;
            }
        }
        
        // If this is the first server, make it default
        if self.servers.is_empty() {
            server.is_default = true;
        }
        
        self.servers.push(server);
    }

    /// Update an existing server configuration
    pub fn update_server(&mut self, server: ServerConfig) {
        // If this is set as default, unset any existing default
        if server.is_default {
            for existing in &mut self.servers {
                if existing.id != server.id {
                    existing.is_default = false;
                }
            }
        }
        
        // Find and update
        if let Some(index) = self.servers.iter().position(|s| s.id == server.id) {
            self.servers[index] = server;
        }
    }

    /// Remove a server configuration
    pub fn remove_server(&mut self, id: &str) -> bool {
        // Don't allow removing the default server
        if let Some(server) = self.get_by_id(id) {
            if server.is_default {
                return false;
            }
        } else {
            return false;
        }
        
        // Find and remove
        if let Some(index) = self.servers.iter().position(|s| s.id == id) {
            self.servers.remove(index);
            return true;
        }
        
        false
    }

    /// Ensure at least one server is marked as default
    pub fn ensure_default_exists(&mut self) {
        if self.servers.is_empty() {
            return;
        }
        
        // Check if any server is marked as default
        if !self.servers.iter().any(|s| s.is_default) {
            // If not, mark the first one as default
            self.servers[0].is_default = true;
        }
    }
}
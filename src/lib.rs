// Re-export modules for testing purposes

pub mod components;
pub mod openrouter;
pub mod server_config;

// Re-export common types and structures
pub use crate::components::*;

// Re-export McpState and ServerStatus from main.rs
use dioxus::prelude::*;
use mcp_client::{
    ClientCapabilities, ClientInfo, McpClient, McpClientTrait, McpService,
    transport::stdio::{StdioTransport, StdioTransportHandle},
    Transport,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tower::timeout::Timeout;

// Define McpState here for testing purposes
#[derive(Clone)]
pub struct McpState {
    pub client: Option<Arc<Mutex<McpClient<Timeout<McpService<StdioTransportHandle>>>>>>,
    pub selected_server: Option<server_config::ServerConfig>,
    pub active_clients: HashMap<String, Arc<Mutex<McpClient<Timeout<McpService<StdioTransportHandle>>>>>>,
    // Track the status of each server (id -> status)
    pub server_status: HashMap<String, ServerStatus>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            client: None,
            selected_server: None,
            active_clients: HashMap::new(),
            server_status: HashMap::new(),
        }
    }
}

// Status of each server
#[derive(Clone, Debug, PartialEq)]
pub enum ServerStatus {
    Running,
    Failed(String),
    Stopped,
    Starting,
} 
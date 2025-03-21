use dioxus::prelude::*;
use mcp_client::{
    ClientCapabilities, ClientInfo, Error as McpError, McpClient, McpClientTrait, McpService,
    transport::stdio::{StdioTransport, StdioTransportHandle},
    Transport,
};
use mcp_core::{protocol::JsonRpcMessage, Resource as McpResource, Tool};
use std::{collections::HashMap, sync::Arc, time::Duration, env};
use tokio::sync::Mutex;
use tower::{timeout::Timeout, ServiceExt};
use serde_json::Value;
use dotenv::dotenv;

mod components;
mod openrouter;
mod server_config;

use components::ChatTab;
use components::server_manager::ServerManager;
use server_config::ServerConfig;

// Load environment variables from .env file if it exists
fn load_env() {
    match dotenv() {
        Ok(_) => eprintln!("Loaded environment from .env file"),
        Err(_) => eprintln!("No .env file found, using default environment"),
    }
}

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    McpDemo {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const MDESK_CSS: Asset = asset!("/assets/mdesk.css");

fn main() {
    // Load environment variables
    load_env();
    
    // Launch the app
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        document::Link { rel: "stylesheet", href: MDESK_CSS }
        Router::<Route> {}
    }
}

#[derive(Clone)]
struct McpState {
    client: Option<Arc<Mutex<McpClient<Timeout<McpService<StdioTransportHandle>>>>>>,
    selected_server: Option<ServerConfig>,
    active_clients: HashMap<String, Arc<Mutex<McpClient<Timeout<McpService<StdioTransportHandle>>>>>>,
    // Track the status of each server (id -> status)
    server_status: HashMap<String, ServerStatus>,
}

// Status of each server
#[derive(Clone, Debug, PartialEq)]
enum ServerStatus {
    Running,
    Failed(String),
    Stopped,
    Starting,
}

/// MCP Demo page with real client implementation
#[component]
fn McpDemo() -> Element {
    let mut client_status = use_signal(|| "Not initialized".to_string());
    let mut error_message = use_signal(|| None::<String>);
    let mut show_resources = use_signal(|| false);
    let mut show_tools = use_signal(|| false);
    let mut resources = use_signal(Vec::<McpResource>::new);
    let mut tools = use_signal(Vec::<Tool>::new);
    let mut active_section = use_signal(|| "chat");
    
    let mut mcp_state = use_signal(|| McpState { 
        client: None,
        selected_server: None,
        active_clients: HashMap::new(),
        server_status: HashMap::new(),
    });
    
    // Get OpenRouter API key from environment variables
    let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
    
    // Server action handles both start and stop
    let server_action = move |_| {
        let has_client = mcp_state.read().client.is_some();
        
        if has_client {
            // Shutdown case
            client_status.set("Shutting down...".to_string());
            error_message.set(None);
            show_resources.set(false);
            show_tools.set(false);
            
            // Clear all clients and mark all servers as stopped
            let mut state = mcp_state.write();
            state.client = None;
            
            // First collect all server IDs to avoid the mutable/immutable borrow conflict
            let server_ids: Vec<String> = state.active_clients.keys().cloned().collect();
            
            // Update all server statuses to Stopped
            for id in server_ids {
                state.server_status.insert(id, ServerStatus::Stopped);
            }
            
            state.active_clients.clear();
            client_status.set("Not initialized".to_string());
            return;
        }

        // Start case - load all server configurations
        client_status.set("Initializing...".to_string());
        error_message.set(None);
        show_resources.set(false);
        show_tools.set(false);
        
        spawn({
            to_owned![mcp_state, client_status, error_message];
            async move {
                // Load server configurations from the file
                let configs = match server_config::ServerConfigs::load_from_file("servers.json") {
                    Ok(configs) => configs,
                    Err(e) => {
                        // If there's an error (likely file not found), create default configs
                        eprintln!("Error loading server configurations: {}", e);
                        server_config::ServerConfigs::initialize_default()
                    }
                };
                
                // Log the number of servers to start
                eprintln!("Starting {} MCP servers", configs.servers.len());
                
                // Create a hashmap to store all active clients
                let mut active_clients = HashMap::new();
                let mut default_server = None;
                let mut server_status = HashMap::new();
                
                // Start each server configuration - use & to borrow instead of moving
                for server_config in &configs.servers {
                    // Clone the server config to avoid borrowing issues
                    let server_config = server_config.clone();
                    let server_id = server_config.id.clone();
                    
                    // Mark this server as starting
                    server_status.insert(server_id.clone(), ServerStatus::Starting);
                    
                    // Log which server we're connecting to
                    eprintln!("Connecting to MCP server: {}", server_config.name);
                    
                    // Create transport with the server's configuration
                    let env_vars = server_config.env.clone();
                    
                    let transport = StdioTransport::new(
                        &server_config.command,
                        server_config.args.clone(),
                        env_vars
                    );
                    
                    match transport.start().await {
                        Ok(handle) => {
                            let service = McpService::with_timeout(handle, Duration::from_secs(30));
                            let mut client = McpClient::new(service);
                            
                            match client.initialize(
                                ClientInfo {
                                    name: "mDesk".to_string(),
                                    version: "0.1.0".to_string(),
                                },
                                ClientCapabilities::default()
                            ).await {
                                Ok(_) => {
                                    // Successfully started the server
                                    client_status.set(format!("Connected to {} (MCP v1.0)", server_config.name));
                                    
                                    // Store the client in our HashMap
                                    let client_arc = Arc::new(Mutex::new(client));
                                    active_clients.insert(server_id.clone(), client_arc.clone());
                                    
                                    // Update server status to Running
                                    server_status.insert(server_id.clone(), ServerStatus::Running);
                                    
                                    // Remember default server
                                    if server_config.is_default {
                                        default_server = Some((server_config, client_arc));
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to initialize: {}", e);
                                    eprintln!("Failed to initialize client for server {}: {}", server_config.name, e);
                                    
                                    // Update server status to Failed
                                    server_status.insert(server_id, ServerStatus::Failed(error_msg));
                                    
                                    if configs.servers.len() == 1 {
                                        // Only show error in UI if this is the only server
                                        client_status.set("Error".to_string());
                                        error_message.set(Some(format!("Failed to initialize client: {}", e)));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to start: {}", e);
                            eprintln!("Failed to start transport for server {}: {}", server_config.name, e);
                            
                            // Update server status to Failed
                            server_status.insert(server_id, ServerStatus::Failed(error_msg));
                            
                            if configs.servers.len() == 1 {
                                // Only show error in UI if this is the only server
                                client_status.set("Error".to_string());
                                error_message.set(Some(format!("Failed to start transport: {}", e)));
                            }
                        }
                    }
                }
                
                // After the loop, update the mcp_state 
                {
                    let mut state = mcp_state.write();
                    state.active_clients = active_clients;
                    state.server_status = server_status;
                    
                    // Select the default server if available, otherwise select the first one
                    if let Some((server, client)) = default_server {
                        state.client = Some(client);
                        state.selected_server = Some(server);
                    } else {
                        // Find the first available client
                        if let Some((id, client)) = state.active_clients.iter().next() {
                            let id = id.clone();
                            // Get the server config for this client
                            if let Ok(configs) = server_config::ServerConfigs::load_from_file("servers.json") {
                                if let Some(server) = configs.get_by_id(&id) {
                                    state.client = Some(client.clone());
                                    state.selected_server = Some(server.clone());
                                }
                            }
                        }
                    }
                }
                
                // Final check - if no clients were successfully started, show an error
                if mcp_state.read().client.is_none() {
                    client_status.set("Error".to_string());
                    error_message.set(Some("Failed to start any MCP servers".to_string()));
                }
            }
        });
    };
    
    // List resources using real client
    let mut list_resources = move |_| {
        if let Some(client) = &mcp_state.read().client {
            client_status.set("Fetching resources...".to_string());
            error_message.set(None);
            show_resources.set(true);
            show_tools.set(false);
            active_section.set("resources");
            
            // Debug log the tools before passing them to the chat
            eprintln!("Tools available in main.rs before passing to ChatTab: {}", tools.read().len());
            for tool in tools.read().iter() {
                eprintln!("  - Available Tool: {} ({})", tool.name, tool.description);
            }
            
            spawn({
                to_owned![client, client_status, error_message, resources];
                async move {
                    let client = client.lock().await;
                    match client.list_resources(None).await {
                        Ok(result) => {
                            resources.set(result.resources);
                            client_status.set("Connected to MCP Server v1.0".to_string());
                        }
                        Err(e) => {
                            client_status.set("Error".to_string());
                            error_message.set(Some(format!("Failed to list resources: {}", e)));
                        }
                    }
                }
            });
        } else {
            error_message.set(Some("Client not initialized".to_string()));
        }
    };
    
    // Create a new function to load tools that can be called from multiple places
    let mut fetch_tools = {
        to_owned![mcp_state, client_status, error_message, tools, show_tools, show_resources];
        move || {
            if mcp_state.read().client.is_none() {
                error_message.set(Some("Client not initialized".to_string()));
                return;
            }
            
            client_status.set("Fetching tools...".to_string());
            error_message.set(None);
            show_tools.set(true);
            show_resources.set(false);
            
            spawn({
                to_owned![mcp_state, client_status, error_message, tools];
                async move {
                    if let Some(ref client) = mcp_state.read().client {
                        let client_lock = client.lock().await;
                        
                        match client_lock.list_tools(None).await {
                            Ok(result) => {
                                tools.set(result.tools);
                                client_status.set("Connected to MCP Server v1.0".to_string());
                            }
                            Err(e) => {
                                client_status.set("Error".to_string());
                                error_message.set(Some(format!("Failed to list tools: {}", e)));
                            }
                        }
                    }
                }
            });
        }
    };
    
    // Wrapper function for use in UI events
    let list_tools = {
        let mut fetch_tools = fetch_tools.clone();
        move |_| {
            fetch_tools();
        }
    };
    
    // Handle server selection
    let select_server = move |server: ServerConfig| {
        // Get a copy of server name for error messages
        let server_name = server.name.clone();
        
        // Check if we have any active clients
        if mcp_state.read().active_clients.is_empty() {
            // No active clients yet, just set the selection
            mcp_state.write().selected_server = Some(server);
            return;
        }
        
        // Check if we have a client for this server
        let client_opt = mcp_state.read().active_clients.get(&server.id).cloned();
        
        if let Some(client) = client_opt {
            // We found a client, update the state with it
            let mut state = mcp_state.write();
            state.client = Some(client);
            state.selected_server = Some(server);
            
            // Update status message
            client_status.set(format!("Connected to {} (MCP v1.0)", server_name));
        } else {
            // This server is not running yet
            error_message.set(Some(format!("Server {} is not running. Start the server first.", server_name)));
        }
    };

    // Set active section
    let set_section = |section: &'static str| {
        let mut fetch_tools = fetch_tools.clone();
        
        move |_| {
            active_section.set(section);
            
            if section == "resources" {
                list_resources(());
            } else if section == "tools" {
                fetch_tools();
            } else if section == "servers" {
                // No need to list resources or tools for server settings
            } else {
                show_resources.set(false);
                show_tools.set(false);
            }
        }
    };
    rsx! {
        div { class: "app-wrapper",
            // Sidebar
            aside { class: "sidebar",
                div { class: "sidebar-header",
                    svg {
                        class: "app-logo",
                        width: "32",
                        height: "32",
                        view_box: "0 0 24 24",
                        fill: "none",
                        xmlns: "http://www.w3.org/2000/svg",
                        path {
                            d: "M10 4H14C18.4183 4 22 7.58172 22 12C22 16.4183 18.4183 20 14 20H4V12C4 7.58172 7.58172 4 12 4",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round"
                        }
                    }
                    div { class: "app-title", "mDesk" }
                }

                div { class: "sidebar-section",
                    div { class: "section-header", "Navigation" }
                    
                    button {
                        class: if *active_section.read() == "chat" { "nav-item active" } else { "nav-item" },
                        onclick: set_section("chat"),
                        svg {
                            class: "nav-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path {
                                d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"
                            }
                        }
                        span { "Chat" }
                    }

                    /*button {
                        class: if *active_section.read() == "home" { "nav-item active" } else { "nav-item" },
                        onclick: set_section("home"),
                        svg {
                            class: "nav-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path {
                                d: "M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
                            }
                        }
                        span { "Home" }
                    }*/
                    
                    button {
                        class: if *active_section.read() == "resources" { "nav-item active" } else { "nav-item" },
                        onclick: set_section("resources"),
                        disabled: mcp_state.read().client.is_none(),
                        svg {
                            class: "nav-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path {
                                d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
                            }
                            polyline {
                                points: "7 10 12 15 17 10"
                            }
                            line {
                                x1: "12",
                                y1: "15",
                                x2: "12",
                                y2: "3"
                            }
                        }
                        span { "Resources" }
                    }
                    
                    button {
                        class: if *active_section.read() == "tools" { "nav-item active" } else { "nav-item" },
                        onclick: set_section("tools"),
                        disabled: mcp_state.read().client.is_none(),
                        svg {
                            class: "nav-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            path {
                                d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                            }
                        }
                        span { "Tools" }
                    }
                    
                    button {
                        class: if *active_section.read() == "servers" { "nav-item active" } else { "nav-item" },
                        onclick: set_section("servers"),
                        svg {
                            class: "nav-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            rect { 
                                x: "2", 
                                y: "2", 
                                width: "20", 
                                height: "8", 
                                rx: "2", 
                                ry: "2" 
                            }
                            rect { 
                                x: "2", 
                                y: "14", 
                                width: "20", 
                                height: "8", 
                                rx: "2", 
                                ry: "2" 
                            }
                            line { 
                                x1: "6", 
                                y1: "6", 
                                x2: "6.01", 
                                y2: "6" 
                            }
                            line { 
                                x1: "6", 
                                y1: "18", 
                                x2: "6.01", 
                                y2: "18" 
                            }
                        }
                        span { "Server Settings" }
                    }
                    
                }

                div { class: "sidebar-section",
                    div { class: "section-header", "Server Status" }
                    
                    div { class: "status-card",
                        div { 
                            class: {
                                match client_status.read().as_str() {
                                    "Not initialized" => "status-dot offline",
                                    "Error" => "status-dot error",
                                    _ => "status-dot online"
                                }
                            }
                        }
                        div { class: "status-info",
                            div { class: "status-label", "Status" }
                            div { class: "status-value", "{client_status}" }
                        }
                    }

                    // Show a list of individual server statuses when at least one server has been configured
                    if !mcp_state.read().server_status.is_empty() {
                        div { class: "server-status-list",
                            div { class: "server-status-heading", "Individual Servers" }
                            
                            // Load all server configs to get names
                            {
                                let server_statuses = mcp_state.read().server_status.clone();
                                let configs_result = server_config::ServerConfigs::load_from_file("servers.json");
                                
                                if let Ok(configs) = configs_result {
                                    // First show any running servers
                                    for server in &configs.servers {
                                        if let Some(status) = server_statuses.get(&server.id) {
                                            let status_class = match status {
                                                ServerStatus::Running => "server-status-item running",
                                                ServerStatus::Failed(_) => "server-status-item failed",
                                                ServerStatus::Stopped => "server-status-item stopped",
                                                ServerStatus::Starting => "server-status-item starting",
                                            };
                                            
                                            let status_text = match status {
                                                ServerStatus::Running => "Running",
                                                ServerStatus::Failed(_) => "Failed",
                                                ServerStatus::Stopped => "Stopped",
                                                ServerStatus::Starting => "Starting",
                                            };
                                            
                                            let error_icon = if let ServerStatus::Failed(error) = status {
                                                Some(error.clone())
                                            } else {
                                                None
                                            };
                                            
                                            rsx! {
                                                div { 
                                                    key: "{server.id}",
                                                    class: status_class,
                                                    div {
                                                        class: "server-status-name",
                                                        "{server.name}"
                                                    }
                                                    div {
                                                        class: "server-status-value",
                                                        "{status_text}"
                                                        if let Some(error_msg) = error_icon {
                                                            span {
                                                                class: "server-status-error",
                                                                title: "{error_msg}",
                                                                "!"
                                                            }
                                                        }
                                                    }
                                                }
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }

                    button {
                        class: if mcp_state.read().client.is_some() {
                            "action-button stop"
                        } else {
                            "action-button start"
                        },
                        disabled: client_status.read().to_string() == "Shutting down..." || client_status.read().to_string() == "Initializing...",
                        onclick: server_action,
                        
                        svg {
                            class: "button-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            if mcp_state.read().client.is_some() {
                                rect {
                                    x: "6",
                                    y: "6",
                                    width: "12",
                                    height: "12",
                                    rx: "2",
                                    ry: "2"
                                }
                            } else {
                                polygon {
                                    points: "5 3 19 12 5 21 5 3"
                                }
                            }
                        }
                        
                        if mcp_state.read().client.is_some() {
                            "Stop Server"
                        } else {
                            "Start Server"
                        }
                    }
                }

                // Version info
                div { class: "sidebar-footer",
                    div { class: "version-info", "mDesk v0.1.0" }
                }
            }

            // Main content
            main { class: "main-content",
                if let Some(ref error) = *error_message.read() {
                    div { class: "error-alert",
                        svg {
                            class: "error-icon",
                            xmlns: "http://www.w3.org/2000/svg",
                            width: "20",
                            height: "20",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            circle {
                                cx: "12",
                                cy: "12",
                                r: "10"
                            }
                            line {
                                x1: "12", 
                                y1: "8", 
                                x2: "12", 
                                y2: "12"
                            }
                            line {
                                x1: "12", 
                                y1: "16", 
                                x2: "12", 
                                y2: "16"
                            }
                        }
                        div { class: "error-content",
                            div { class: "error-title", "Error" }
                            div { class: "error-message", "{error}" }
                        }
                    }
                }

                // Home section
                div { class: if *active_section.read() == "home" { "content-section active" } else { "content-section" },
                    div { class: "welcome-header",
                        h1 { class: "welcome-title", "Welcome to mDesk" }
                        p { class: "welcome-subtitle", "A native desktop application for managing MCP tools with OpenRouter LLM access" }
                    }

                    div { class: "panel getting-started",
                        h2 { class: "panel-title", 
                            svg {
                                class: "panel-icon",
                                xmlns: "http://www.w3.org/2000/svg",
                                width: "18",
                                height: "18",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "currentColor",
                                stroke_width: "2",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                circle {
                                    cx: "12",
                                    cy: "12",
                                    r: "10"
                                }
                                line {
                                    x1: "12", 
                                    y1: "16", 
                                    x2: "12", 
                                    y2: "12"
                                }
                                line {
                                    x1: "12", 
                                    y1: "8", 
                                    x2: "12", 
                                    y2: "8"
                                }
                            }
                            "Getting Started"
                        }

                        div { class: "panel-content",
                            p { class: "panel-text", "Follow these steps to start using mDesk:" }
                            
                            ol { class: "steps-list",
                                li { 
                                    span { class: "step-number", "1" }
                                    div { class: "step-content",
                                        div { class: "step-title", "Start the MCP Server" }
                                        div { class: "step-description", "Click the 'Start Server' button in the sidebar to initialize the MCP service." }
                                    }
                                }
                                li { 
                                    span { class: "step-number", "2" }
                                    div { class: "step-content",
                                        div { class: "step-title", "Explore Available Resources" }
                                        div { class: "step-description", "Navigate to the Resources tab to view all available MCP resources." }
                                    }
                                }
                                li { 
                                    span { class: "step-number", "3" }
                                    div { class: "step-content",
                                        div { class: "step-title", "Discover Available Tools" }
                                        div { class: "step-description", "Check the Tools tab to see what MCP tools are at your disposal." }
                                    }
                                }
                                li { 
                                    span { class: "step-number", "4" }
                                    div { class: "step-content",
                                        div { class: "step-title", "Chat with AI Models" }
                                        div { class: "step-description", "Use the Chat tab to interact with AI models via OpenRouter." }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "features-grid",
                        div { class: "feature-card",
                            div { class: "feature-icon",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "24",
                                    height: "24",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { 
                                        d: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"
                                    }
                                    polyline { 
                                        points: "7.5 4.21 12 6.81 16.5 4.21" 
                                    }
                                    polyline { 
                                        points: "7.5 19.79 7.5 14.6 3 12" 
                                    }
                                    polyline { 
                                        points: "21 12 16.5 14.6 16.5 19.79" 
                                    }
                                    polyline { 
                                        points: "3.27 6.96 12 12.01 20.73 6.96" 
                                    }
                                    line { 
                                        x1: "12", 
                                        y1: "22.08", 
                                        x2: "12", 
                                        y2: "12" 
                                    }
                                }
                            }
                            div { class: "feature-title", "Native Integration" }
                            div { class: "feature-description", "Seamlessly integrates with the Model Context Protocol (MCP) Rust SDK" }
                        }

                        div { class: "feature-card",
                            div { class: "feature-icon",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "24",
                                    height: "24",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    circle { 
                                        cx: "12", 
                                        cy: "12", 
                                        r: "10" 
                                    }
                                    circle { 
                                        cx: "12", 
                                        cy: "12", 
                                        r: "4" 
                                    }
                                    line { 
                                        x1: "21.17", 
                                        y1: "8", 
                                        x2: "12", 
                                        y2: "8" 
                                    }
                                    line { 
                                        x1: "3.95", 
                                        y1: "6.06", 
                                        x2: "8.54", 
                                        y2: "14" 
                                    }
                                    line { 
                                        x1: "10.88", 
                                        y1: "21.94", 
                                        x2: "15.46", 
                                        y2: "14" 
                                    }
                                }
                            }
                            div { class: "feature-title", "OpenRouter Access" }
                            div { class: "feature-description", "Connect to multiple LLMs through the OpenRouter service" }
                        }

                        div { class: "feature-card",
                            div { class: "feature-icon",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "24",
                                    height: "24",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    rect { 
                                        x: "3", 
                                        y: "3", 
                                        width: "18", 
                                        height: "18", 
                                        rx: "2", 
                                        ry: "2" 
                                    }
                                    line { 
                                        x1: "3", 
                                        y1: "9", 
                                        x2: "21", 
                                        y2: "9" 
                                    }
                                    line { 
                                        x1: "9", 
                                        y1: "21", 
                                        x2: "9", 
                                        y2: "9" 
                                    }
                                }
                            }
                            div { class: "feature-title", "Modern Interface" }
                            div { class: "feature-description", "Clean, intuitive UI designed for productivity" }
                        }
                    }
                }                
                
                // Resources section
                div { class: if *active_section.read() == "resources" { "content-section active" } else { "content-section" },
                    div { class: "section-header",
                        h1 { class: "section-title", "MCP Resources" }
                        p { class: "section-description", "Explore available resources in the MCP server" }
                    }

                    div { class: "resource-container",
                        if resources.read().is_empty() {
                            div { class: "empty-state",
                                svg {
                                    class: "empty-icon",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "48",
                                    height: "48",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path {
                                        d: "M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"
                                    }
                                    polyline {
                                        points: "13 2 13 9 20 9"
                                    }
                                }
                                div { class: "empty-title", "No Resources Found" }
                                div { class: "empty-message", "There are currently no resources available in the MCP server." }
                                button {
                                    class: "reload-button",
                                    onclick: move |_| list_resources(()),
                                    svg {
                                        class: "button-icon",
                                        xmlns: "http://www.w3.org/2000/svg",
                                        width: "16",
                                        height: "16",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path {
                                            d: "M23 4v6h-6"
                                        }
                                        path {
                                            d: "M1 20v-6h6"
                                        }
                                        path {
                                            d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
                                        }
                                    }
                                    "Reload Resources"
                                }
                            }
                        } else {
                            div { class: "resource-grid",
                                for resource in resources.read().iter() {
                                    div {
                                        key: format!("resource-{}", &resource.name),
                                        class: "resource-card",
                                        div { class: "resource-header",
                                            div { class: "resource-icon",
                                                svg {
                                                    xmlns: "http://www.w3.org/2000/svg",
                                                    width: "20",
                                                    height: "20",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    path {
                                                        d: "M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"
                                                    }
                                                    polyline {
                                                        points: "13 2 13 9 20 9"
                                                    }
                                                }
                                            }
                                            h3 { class: "resource-name", "{resource.name}" }
                                        }
                                        if let Some(desc) = &resource.description {
                                            p { class: "resource-description", "{desc}" }
                                        }
                                        if let Some(annotations) = &resource.annotations {
                                            div { class: "resource-annotations",
                                                h4 { class: "annotations-title", "Annotations" }
                                                pre { class: "annotations-content", 
                                                    "{annotations:?}" 
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Tools section
                div { class: if *active_section.read() == "tools" { "content-section active" } else { "content-section" },
                    div { class: "section-header",
                        h1 { class: "section-title", "MCP Tools" }
                        p { class: "section-description", "Discover available tools in the MCP server" }
                    }

                    div { class: "tools-container",
                        if tools.read().is_empty() {
                            div { class: "empty-state",
                                svg {
                                    class: "empty-icon",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "48",
                                    height: "48",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "1",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path {
                                        d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                                    }
                                }
                                div { class: "empty-title", "No Tools Found" }
                                div { class: "empty-message", "There are currently no tools available in the MCP server." }
                                button {
                                    class: "reload-button",
                                    onclick: move |_| {
                                        let mut list_tools_clone = list_tools.clone();
                                        list_tools_clone(());
                                    },
                                    svg {
                                        class: "button-icon",
                                        xmlns: "http://www.w3.org/2000/svg",
                                        width: "16",
                                        height: "16",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path {
                                            d: "M23 4v6h-6"
                                        }
                                        path {
                                            d: "M1 20v-6h6"
                                        }
                                        path {
                                            d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
                                        }
                                    }
                                    "Reload Tools"
                                }
                            }
                        } else {
                            div { class: "tools-grid",
                                for tool in tools.read().iter() {
                                    div {
                                        key: format!("tool-{}", &tool.name),
                                        class: "tool-card",
                                        div { class: "tool-header",
                                            div { class: "tool-icon",
                                                svg {
                                                    xmlns: "http://www.w3.org/2000/svg",
                                                    width: "20",
                                                    height: "20",
                                                    view_box: "0 0 24 24",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "2",
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    path {
                                                        d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                                                    }
                                                }
                                            }
                                            h3 { class: "tool-name", "{tool.name}" }
                                        }
                                        p { class: "tool-description", "{tool.description}" }
                                        div { class: "tool-schema",
                                            h4 { class: "schema-title", "Parameters" }
                                            pre { class: "schema-content", 
                                                "{tool.input_schema}" 
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Server Settings section
                div { class: if *active_section.read() == "servers" { "content-section active" } else { "content-section" },
                    div { class: "section-header",
                        h1 { class: "section-title", "Server Settings" }
                        p { class: "section-description", "Configure and manage MCP server connections" }
                    }

                    ServerManager {
                        on_select_server: select_server,
                        selected_id: mcp_state.read().selected_server.as_ref().map(|s| s.id.clone()),
                    }
                }

                // Chat section
                div { class: if *active_section.read() == "chat" { "content-section active" } else { "content-section" },
                    div { class: "section-header",
                        h1 { class: "section-title", "MCP Chat" }
                        p { class: "section-description", "Interact with AI models using MCP tools" }
                    }
                    
                    // Load tools if needed and show chat component
                    {
                        // Ensure we fetch tools before rendering the chat tab if needed
                        if tools.read().is_empty() && mcp_state.read().client.is_some() {
                            eprintln!("Tools not loaded yet, fetching them for chat");
                            
                            // Clone and call fetch_tools
                            let mut fetch_tools_clone = fetch_tools.clone();
                            fetch_tools_clone();
                            
                            // Give a short wait for tools to update
                            eprintln!("Waiting for tools to load...");
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                        
                        // Debug logs
                        eprintln!("Sending {} tools to ChatTab", tools.read().len());
                        for tool in tools.read().iter() {
                            eprintln!("  - Sending Tool to ChatTab: {} ({})", tool.name, tool.description);
                        }
                        
                        // Render ChatTab component
                        rsx! {
                            ChatTab {
                                mcp_tools: tools.read().to_vec(),
                                api_key: openrouter_api_key.clone(),
                                mcp_state: mcp_state.clone(),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    // ...existing fields...
}

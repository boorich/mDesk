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

use components::ChatTab;

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
    
    let mut mcp_state = use_signal(|| McpState { client: None });
    
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
            
            // Take the client out of the state
            mcp_state.write().client = None;
            client_status.set("Not initialized".to_string());
            return;
        }

        // Start case
        client_status.set("Initializing...".to_string());
        error_message.set(None);
        show_resources.set(false);
        show_tools.set(false);
        
        spawn({
            to_owned![mcp_state, client_status, error_message];
            async move {
                let transport = StdioTransport::new(
                    "docker",
                    vec![
                        "run".to_string(),
                        "-i".to_string(),
                        "--rm".to_string(),
                        "--mount".to_string(),
                        "type=bind,src=/Users/martinmaurer/Desktop,dst=/Users/martinmaurer/Desktop".to_string(),
                        "--mount".to_string(),
                        "type=bind,src=/Users/martinmaurer/Projects,dst=/Users/martinmaurer/Projects".to_string(),
                        "mcp/filesystem".to_string(),
                        "/Users/martinmaurer/Desktop".to_string(),
                        "/Users/martinmaurer/Projects".to_string()
                    ],
                    HashMap::new()
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
                                client_status.set("Connected to MCP Server v1.0".to_string());
                                mcp_state.set(McpState {
                                    client: Some(Arc::new(Mutex::new(client)))
                                });
                            }
                            Err(e) => {
                                client_status.set("Error".to_string());
                                error_message.set(Some(format!("Failed to initialize client: {}", e)));
                            }
                        }
                    }
                    Err(e) => {
                        client_status.set("Error".to_string());
                        error_message.set(Some(format!("Failed to start transport: {}", e)));
                    }
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
    
    // List tools using real client
    let mut list_tools = move |_| {
        if let Some(client) = &mcp_state.read().client {
            client_status.set("Fetching tools...".to_string());
            error_message.set(None);
            show_resources.set(false);
            show_tools.set(true);
            active_section.set("tools");
            
            spawn({
                to_owned![client, client_status, error_message, tools];
                async move {
                    let client = client.lock().await;
                    match client.list_tools(None).await {
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
            });
        } else {
            error_message.set(Some("Client not initialized".to_string()));
        }
    };

    // Set active section
    let set_section = move |section: &'static str| {
        move |_| {
            active_section.set(section);
            if section == "resources" {
                list_resources(());
            } else if section == "tools" {
                list_tools(());
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
                                    onclick: move |_| list_tools(()),
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

                // Chat section
                div { class: if *active_section.read() == "chat" { "content-section active" } else { "content-section" },
                    div { class: "section-header",
                        h1 { class: "section-title", "Chat with AI" }
                        p { class: "section-description", "Interact with AI models via OpenRouter" }
                    }

                    // Load tools if needed and show chat component
                    {
                        // Ensure we fetch tools before rendering the chat tab if needed
                        if tools.read().is_empty() && mcp_state.read().client.is_some() {
                            eprintln!("Tools not loaded yet, fetching them for chat");
                            
                            // Load tools 
                            list_tools(());
                            
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

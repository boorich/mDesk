use dioxus::prelude::*;
use mcp_client::{
    ClientCapabilities, ClientInfo, Error as McpError, McpClient, McpClientTrait, McpService,
    transport::stdio::{StdioTransport, StdioTransportHandle},
    Transport,
};
use mcp_core::{protocol::JsonRpcMessage, Resource as McpResource, Tool};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tower::{timeout::Timeout, ServiceExt};
use serde_json::Value;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    McpDemo {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
const MCP_CLIENT_CSS: Asset = asset!("/assets/mcp-client.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        document::Link { rel: "stylesheet", href: MCP_CLIENT_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Hero() -> Element {
    rsx! {
        div { id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus",
                    "💫 VSCode Extension"
                }
                a { href: "https://discord.gg/XgGxMSkvUM", "👋 Community Discord" }
            }
        }
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        Hero {}
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
    
    let mut mcp_state = use_signal(|| McpState { client: None });
    
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
                                name: "dioxus-mcp-demo".to_string(),
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
    let list_resources = move |_| {
        if let Some(client) = &mcp_state.read().client {
            client_status.set("Fetching resources...".to_string());
            error_message.set(None);
            show_resources.set(true);
            show_tools.set(false);
            
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
    let list_tools = move |_| {
        if let Some(client) = &mcp_state.read().client {
            client_status.set("Fetching tools...".to_string());
            error_message.set(None);
            show_resources.set(false);
            show_tools.set(true);
            
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
    
    rsx! {
        div { class: "app-container",
            // Sidebar
            div { class: "sidebar",
                h1 { class: "app-title", "MCP Server Manager" }
                div { class: "server-status",
                    div { 
                        class: {
                            match client_status.read().as_str() {
                                "Not initialized" => "status-indicator offline",
                                "Error" => "status-indicator error",
                                _ => "status-indicator online"
                            }
                        }
                    }
                    span { class: "status-text", "{client_status}" }
                }

                if let Some(ref error) = *error_message.read() {
                    div { class: "error-message",
                        span { class: "error-icon", "⚠" }
                        span { class: "error-text", "{error}" }
                    }
                }

                // Main actions
                div { class: "server-controls",
                    button {
                        class: if mcp_state.read().client.is_some() {
                            "control-button stop"
                        } else {
                            "control-button start"
                        },
                        disabled: client_status.read().to_string() == "Shutting down...",
                        onclick: server_action,
                        if mcp_state.read().client.is_some() {
                            "Stop MCP Server"
                        } else {
                            "Start MCP Server"
                        }
                    }
                }
            }

            // Main content
            div { class: "main-content",
                div { class: "tools-panel",
                    h2 { "Server Tools" }
                    div { class: "tool-buttons",
                        button {
                            class: "tool-button",
                            disabled: mcp_state.read().client.is_none(),
                            onclick: list_resources,
                            span { class: "icon", "📁" }
                            span { "List Resources" }
                        }
                        button {
                            class: "tool-button",
                            disabled: mcp_state.read().client.is_none(),
                            onclick: list_tools,
                            span { class: "icon", "🔧" }
                            span { "List Tools" }
                        }
                    }
                }

                // Results section
                div { class: "results-section",
                    if *show_resources.read() || *show_tools.read() {
                        h2 { "Results" }
                    }
                    
                    if *show_resources.read() {
                        div { class: "results-container",
                            if resources.read().is_empty() {
                                div { class: "empty-state", "No resources found" }
                            } else {
                                div { class: "resource-grid",
                                    for resource in resources.read().iter() {
                                        div { 
                                            key: format!("resource-{}", &resource.name),
                                            class: "resource-card",
                                            h3 { class: "resource-name", "{resource.name}" }
                                            if let Some(desc) = &resource.description {
                                                p { class: "resource-description", "{desc}" }
                                            }
                                            if let Some(annotations) = &resource.annotations {
                                                div { class: "resource-annotations",
                                                    h4 { "Annotations" }
                                                    pre { "{annotations:?}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if *show_tools.read() {
                        div { class: "results-container",
                            if tools.read().is_empty() {
                                div { class: "empty-state", "No tools found" }
                            } else {
                                div { class: "tools-grid",
                                    for tool in tools.read().iter() {
                                        div { 
                                            key: format!("tool-{}", &tool.name),
                                            class: "tool-card",
                                            h3 { class: "tool-name", "{tool.name}" }
                                            p { class: "tool-description", "{tool.description}" }
                                            div { class: "tool-parameters",
                                                h4 { "Parameters" }
                                                pre { class: "schema",
                                                    "{tool.input_schema}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        style { {get_styles()} }
    }
}

// Add this function to provide the styles
fn get_styles() -> &'static str {
    r#"
    .app-container {
        display: flex;
        height: 100vh;
        background-color: #f8f9fa;
    }

    .sidebar {
        width: 280px;
        background-color: #2c3e50;
        color: white;
        padding: 24px;
        display: flex;
        flex-direction: column;
        gap: 24px;
    }

    .app-title {
        font-size: 24px;
        font-weight: 600;
        margin-bottom: 24px;
        color: #ecf0f1;
    }

    .server-status {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 12px;
        background-color: rgba(255, 255, 255, 0.1);
        border-radius: 8px;
    }

    .status-indicator {
        width: 12px;
        height: 12px;
        border-radius: 50%;
    }

    .status-indicator.online { background-color: #2ecc71; }
    .status-indicator.offline { background-color: #95a5a6; }
    .status-indicator.error { background-color: #e74c3c; }

    .server-controls {
        display: flex;
        flex-direction: column;
        gap: 12px;
    }

    .control-button {
        padding: 12px;
        border-radius: 8px;
        border: none;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s;
    }

    .control-button.start {
        background-color: #2ecc71;
        color: white;
    }

    .control-button.stop {
        background-color: #e74c3c;
        color: white;
    }

    .control-button:hover {
        transform: translateY(-1px);
        box-shadow: 0 2px 4px rgba(0,0,0,0.1);
    }

    .main-content {
        flex: 1;
        padding: 24px;
        overflow-y: auto;
    }

    .tools-panel {
        background-color: white;
        border-radius: 12px;
        padding: 24px;
        margin-bottom: 24px;
        box-shadow: 0 2px 4px rgba(0,0,0,0.05);
    }

    .tool-buttons {
        display: flex;
        gap: 12px;
        margin-top: 16px;
    }

    .tool-button {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 12px 16px;
        border-radius: 8px;
        border: 1px solid #e9ecef;
        background-color: white;
        color: #2c3e50;
        cursor: pointer;
        transition: all 0.2s;
    }

    .tool-button:hover:not(:disabled) {
        background-color: #f8f9fa;
        border-color: #dee2e6;
    }

    .tool-button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .results-section {
        background-color: white;
        border-radius: 12px;
        padding: 24px;
        box-shadow: 0 2px 4px rgba(0,0,0,0.05);
    }

    .resource-grid, .tools-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
        gap: 20px;
        margin-top: 20px;
    }

    .resource-card, .tool-card {
        background-color: #f8f9fa;
        border-radius: 8px;
        padding: 16px;
        border: 1px solid #e9ecef;
    }

    .resource-name, .tool-name {
        font-size: 18px;
        font-weight: 600;
        margin-bottom: 8px;
        color: #2c3e50;
    }

    .resource-description, .tool-description {
        color: #6c757d;
        margin-bottom: 16px;
        line-height: 1.5;
    }

    .error-message {
        background-color: rgba(231, 76, 60, 0.1);
        border-left: 4px solid #e74c3c;
        padding: 12px;
        border-radius: 4px;
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .error-icon {
        color: #e74c3c;
    }

    .empty-state {
        text-align: center;
        padding: 48px;
        color: #6c757d;
        font-style: italic;
    }

    pre {
        background-color: #f1f3f5;
        padding: 12px;
        border-radius: 4px;
        font-family: monospace;
        font-size: 14px;
        overflow-x: auto;
        margin-top: 8px;
    }

    h2 {
        font-size: 20px;
        font-weight: 600;
        color: #2c3e50;
        margin-bottom: 16px;
    }

    h4 {
        font-size: 16px;
        font-weight: 600;
        color: #2c3e50;
        margin-bottom: 8px;
    }
    "#
}
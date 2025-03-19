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
    
    let mcp_state = use_signal(|| McpState { client: None });
    
    // Initialize the MCP client with stdio transport
    let initialize_client = move |_| {
        client_status.set("Initializing...".to_string());
        error_message.set(None);
        show_resources.set(false);
        show_tools.set(false);
        
        spawn({
            to_owned![mcp_state, client_status, error_message];
            async move {
                // Create stdio transport
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
                        // Wrap the transport handle in McpService
                        let service = McpService::with_timeout(handle, Duration::from_secs(5));
                        let mut client = McpClient::new(service);
                        
                        // Initialize client with capabilities
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
        div { class: "mcp-demo",
            header { class: "mcp-header",
                h1 { "MCP Client Demo" }
                div { 
                    class: {
                        match client_status.read().as_str() {
                            "Not initialized" => "status-pill status-not-initialized",
                            "Error" => "status-pill status-error",
                            _ => "status-pill status-connected"
                        }
                    },
                    "{client_status}"
                }
            }

            if let Some(ref error) = *error_message.read() {
                div { class: "error-message",
                    span { class: "error-icon", "⚠" }
                    "{error}"
                }
            }

            div { class: "mcp-content",
                // Actions panel
                div { class: "actions-panel",
                    h2 { "Actions" }
                    button {
                        class: "action-button",
                        disabled: mcp_state.read().client.is_some(),
                        onclick: initialize_client,
                        "Initialize Client"
                    }
                    button {
                        class: "action-button",
                        disabled: mcp_state.read().client.is_none(),
                        onclick: list_resources,
                        "List Resources"
                    }
                    button {
                        class: "action-button",
                        disabled: mcp_state.read().client.is_none(),
                        onclick: list_tools,
                        "List Tools"
                    }
                }

                // Results panel
                div { class: "results-panel",
                    h2 { "Results" }
                    
                    if *show_resources.read() {
                        div { class: "results-list",
                            if resources.read().is_empty() {
                                p { class: "no-results", "No resources found" }
                            } else {
                                ul {
                                    for resource in resources.read().iter() {
                                        li { 
                                            key: format!("resource-{}", &resource.name),
                                            div { class: "result-item",
                                                h3 { "{resource.name}" }
                                                if let Some(desc) = &resource.description {
                                                    p { class: "result-description", "{desc}" }
                                                }
                                                if let Some(annotations) = &resource.annotations {
                                                    div { class: "annotations",
                                                        span { "Annotations:" }
                                                        ul {
                                                            li {
                                                                key: "annotations",
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
                        }
                    }

                    if *show_tools.read() {
                        div { class: "results-list",
                            if tools.read().is_empty() {
                                p { class: "no-results", "No tools found" }
                            } else {
                                ul {
                                    for tool in tools.read().iter() {
                                        li { 
                                            key: format!("tool-{}", &tool.name),
                                            div { class: "result-item",
                                                h3 { "{tool.name}" }
                                                p { class: "result-description", "{tool.description}" }
                                                div { class: "parameters",
                                                    span { "Parameters:" }
                                                    pre { class: "input-schema",
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

            footer { class: "mcp-footer",
                p { "Built with Dioxus and MCP" }
            }
        }
    }
}
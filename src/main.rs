use dioxus::prelude::*;
use mcp_client::{ClientCapabilities, ClientInfo, Error as McpError, McpClient, McpClientTrait};
// Import a mock transport since we don't know the exact transport structure
use std::sync::Arc;

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
        div {
            id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus", "💫 VSCode Extension" }
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


/// MCP Demo page with simulated client calls
#[component]
fn McpDemo() -> Element {
    let mut client_status = use_signal(|| "Not initialized".to_string());
    let mut error_message = use_signal(|| None::<String>);
    let mut show_resources = use_signal(|| false);
    let mut show_tools = use_signal(|| false);
    
    // Simulated action to initialize the MCP client
    let initialize_client = move |_| {
        client_status.set("Initializing...".to_string());
        error_message.set(None);
        show_resources.set(false);
        show_tools.set(false);
        
        spawn(async move {
            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            
            // Simulate successful connection
            client_status.set("Connected to MCP Server v1.0".to_string());
        });
    };
    
    // Simulated action to list resources
    let list_resources = move |_| {
        if *client_status.read() != "Connected to MCP Server v1.0" {
            error_message.set(Some("Client not initialized".to_string()));
            return;
        }
        
        client_status.set("Fetching resources...".to_string());
        error_message.set(None);
        show_resources.set(true);
        show_tools.set(false);
        
        spawn(async move {
            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            
            // Success - show resources
            client_status.set("Connected to MCP Server v1.0".to_string());
        });
    };
    
    // Simulated action to list tools
    let list_tools = move |_| {
        if *client_status.read() != "Connected to MCP Server v1.0" {
            error_message.set(Some("Client not initialized".to_string()));
            return;
        }
        
        client_status.set("Fetching tools...".to_string());
        error_message.set(None);
        show_resources.set(false);
        show_tools.set(true);
        
        spawn(async move {
            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            
            // Success - show tools
            client_status.set("Connected to MCP Server v1.0".to_string());
        });
    };
    
    // Mock resource data
    let resources = vec![
        ("Documentation", "resource:documentation"),
        ("User Guide", "resource:user-guide"),
        ("API Reference", "resource:api-reference"),
    ];
    
    // Mock tool data
    let tools = vec![
        ("calculator", "Performs mathematical calculations"),
        ("translator", "Translates text between languages"),
        ("weather", "Gets weather information for a location"),
    ];
    
    rsx! {
        div { class: "app-container",
            // Header
            header { class: "app-header",
                div { class: "app-title", "MCP Client Demo" }
                div { class: "status-indicator",
                    span { 
                        class: match &*client_status.read().as_str() {
                            "Not initialized" => "status not-initialized",
                            "Initializing..." => "status initializing",
                            "Connected to MCP Server v1.0" => "status connected",
                            "Fetching resources..." | "Fetching tools..." => "status fetching",
                            "Error" => "status error",
                            _ => "status not-initialized"
                        },
                        "{client_status}"
                    }
                }
            }
            
            // Main content area
            main { class: "app-content",
                // Error messages
                {match *error_message.read() {
                    Some(ref err) => rsx!(
                        div { class: "error-message",
                            span { class: "error-icon", "⚠" }
                            div { class: "error-text",
                                span { class: "error-title", "Error" }
                                span { class: "error-description", "{err}" }
                            }
                        }
                    ),
                    None => rsx!()
                }}
                
                // Two-column layout
                div { class: "content-columns",
                    // Sidebar
                    aside { class: "sidebar",
                        // Actions panel
                        section { class: "panel",
                            h2 { class: "panel-title", "Actions" }
                            div { class: "panel-content",
                                button {
                                    class: "btn primary",
                                    onclick: initialize_client,
                                    "Initialize Client"
                                }
                                
                                button {
                                    class: if *client_status.read() != "Connected to MCP Server v1.0" {
                                        "btn disabled"
                                    } else {
                                        "btn success"
                                    },
                                    disabled: *client_status.read() != "Connected to MCP Server v1.0",
                                    onclick: list_resources,
                                    "List Resources"
                                }
                                
                                button {
                                    class: if *client_status.read() != "Connected to MCP Server v1.0" {
                                        "btn disabled"
                                    } else {
                                        "btn secondary"
                                    },
                                    disabled: *client_status.read() != "Connected to MCP Server v1.0",
                                    onclick: list_tools,
                                    "List Tools"
                                }
                            }
                        }
                        
                        // Guide panel
                        section { class: "panel",
                            h2 { class: "panel-title", "Implementation Guide" }
                            div { class: "panel-content",
                                p { class: "guide-intro", 
                                    "To implement a real MCP client:" 
                                }
                                
                                ol { class: "guide-steps",
                                    li { "Add MCP dependencies to Cargo.toml" }
                                    li { "Create transport for your MCP server" }
                                    li { "Initialize client with capabilities" }
                                    li { "Call MCP operations as needed" }
                                }
                            }
                        }
                    }
                    
                    // Main panel
                    div { class: "main-panel",
                        section { class: "panel results-panel",
                            h2 { class: "panel-title", "Results" }
                            
                            // Results content
                            div { class: "panel-content",
                                // Empty state
                                {if !*show_resources.read() && !*show_tools.read() {
                                    rsx!(
                                        div { class: "empty-state",
                                            span { "Use the actions on the left to fetch data" }
                                        }
                                    )
                                } else {
                                    rsx!()
                                }}
                                
                                // Resources display
                                {match *show_resources.read() {
                                    true => rsx!(
                                        div { class: "result-section",
                                            h3 { class: "result-title", "Resources" }
                                            
                                            div { class: "resource-list",
                                                for (name, uri) in &resources {
                                                    div { class: "resource-item",
                                                        div { class: "resource-name", "{name}" }
                                                        div { class: "resource-uri", 
                                                            "URI: ",
                                                            code { "{uri}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    ),
                                    false => rsx!()
                                }}
                                
                                // Tools display
                                {match *show_tools.read() {
                                    true => rsx!(
                                        div { class: "result-section",
                                            h3 { class: "result-title", "Tools" }
                                            
                                            div { class: "tool-list",
                                                for (name, description) in &tools {
                                                    div { class: "tool-item",
                                                        div { class: "tool-name", "{name}" }
                                                        div { class: "tool-description", "{description}" }
                                                    }
                                                }
                                            }
                                        }
                                    ),
                                    false => rsx!()
                                }}
                            }
                        }
                    }
                }
            }
            
            // Footer
            footer { class: "app-footer",
                "MCP Client Demo • Built with Dioxus"
            }
        }
    }
}

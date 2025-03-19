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

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS } 
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
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
        div { class: "container mx-auto p-4",
            h1 { class: "text-2xl font-bold mb-4", "MCP Client Demo" }
            
            // Status and error messages
            div { class: "mb-4 p-4 bg-gray-100 rounded",
                p { 
                    span { class: "font-semibold", "Status: " }
                    span { "{client_status}" }
                }
                
                // Show error message if any
                {
                    match *error_message.read() {
                        Some(ref err) => {
                            rsx!(
                                div { 
                                    class: "mt-2 bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded",
                                    p { "{err}" }
                                }
                            )
                        },
                        None => rsx!()
                    }
                }
            }
            
            // Actions section
            div { class: "mb-4",
                h2 { class: "text-xl font-semibold mb-2", "MCP Client Actions" }
                
                div { class: "flex flex-wrap gap-2",
                    button {
                        class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                        onclick: initialize_client,
                        "Initialize Client"
                    }
                    
                    button {
                        class: "bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded",
                        disabled: *client_status.read() != "Connected to MCP Server v1.0",
                        onclick: list_resources,
                        "List Resources"
                    }
                    
                    button {
                        class: "bg-purple-500 hover:bg-purple-700 text-white font-bold py-2 px-4 rounded",
                        disabled: *client_status.read() != "Connected to MCP Server v1.0",
                        onclick: list_tools,
                        "List Tools"
                    }
                }
            }
            
            // Resources section
            {
                match *show_resources.read() {
                    true => {
                        rsx!(
                            div { 
                                class: "mb-4 p-4 border border-green-300 rounded",
                                h3 { class: "text-lg font-semibold mb-2", "Resources" }
                                
                                ul { 
                                    class: "list-disc pl-5",
                                    for (name, uri) in &resources {
                                        li { 
                                            span { class: "font-medium", "{name}" }
                                            " - URI: {uri}"
                                        }
                                    }
                                }
                            }
                        )
                    },
                    false => rsx!()
                }
            }
            
            // Tools section
            {
                match *show_tools.read() {
                    true => {
                        rsx!(
                            div { 
                                class: "mb-4 p-4 border border-purple-300 rounded",
                                h3 { class: "text-lg font-semibold mb-2", "Tools" }
                                
                                ul { 
                                    class: "list-disc pl-5",
                                    for (name, description) in &tools {
                                        li { 
                                            span { class: "font-medium", "{name}" }
                                            " - {description}"
                                        }
                                    }
                                }
                            }
                        )
                    },
                    false => rsx!()
                }
            }
            
            // Implementation guidance section
            div { class: "mt-8 p-4 border border-gray-300 rounded",
                h2 { class: "text-xl font-semibold mb-2", "Implementation Guidance" }
                
                p { class: "mb-2", 
                    "This is a simulated demo. To implement a real MCP client, you need to:" 
                }
                
                ol { class: "list-decimal pl-5 mb-4",
                    li { "Add the MCP dependencies to your Cargo.toml" }
                    li { "Create the appropriate transport for your MCP server" }
                    li { "Initialize the client with proper capabilities" }
                    li { "Call MCP operations as needed" }
                }
                
                p { 
                    "Explore the MCP client API to leverage all its capabilities once you have access to the real implementation."
                }
            }
        }
    }
}

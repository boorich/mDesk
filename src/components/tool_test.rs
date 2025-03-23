use dioxus::prelude::*;
use mcp_core::Tool;
use serde_json::{Value, json};
use crate::McpState;
use crate::components::tool_manager::ToolManager;

/// Props for ToolTestModal component
#[derive(Props, Clone, PartialEq)]
pub struct ToolTestModalProps {
    /// The tool to test
    pub tool: Tool,
    /// Event handler for when the dialog is closed
    pub on_close: EventHandler<()>,
    /// MCP state for accessing the client
    pub mcp_state: Signal<McpState>,
}

/// Modal component for testing a tool
#[component]
pub fn ToolTestModal(props: ToolTestModalProps) -> Element {
    let mut tool_params = use_signal(|| String::new());
    let mut is_executing = use_signal(|| false);
    let mut execution_result = use_signal(|| None::<Result<String, String>>);
    
    // Use reactive values for tool properties to avoid borrow checker issues
    let tool_name = use_memo(move || props.tool.name.clone());
    let tool_description = use_memo(move || props.tool.description.clone());
    
    // Pre-fill with empty JSON object
    use_effect(move || {
        // The input_schema is already a JSON value
        let schema = &props.tool.input_schema;
            
        if let Some(properties) = schema.get("properties") {
            if let Some(props_obj) = properties.as_object() {
                let mut template = json!({});
                
                // Include each property with appropriate default values by type
                for (key, value) in props_obj {
                    // Get the type of the property
                    let prop_type = value.get("type").and_then(|t| t.as_str()).unwrap_or("string");
                    
                    // Set appropriate defaults based on type
                    match prop_type {
                        "string" => { template[key] = json!(""); }
                        "number" => { template[key] = json!(0); }
                        "boolean" => { template[key] = json!(false); }
                        "array" => { template[key] = json!([]); }
                        "object" => { template[key] = json!({}); }
                        _ => { template[key] = json!(""); }
                    }
                }
                
                // Format with pretty-print and indentation
                tool_params.set(serde_json::to_string_pretty(&template).unwrap_or_else(|_| "{}".to_string()));
                return;
            }
        }
        
        // Fallback to empty object
        tool_params.set("{\n}".to_string());
    });
    
    // Handle test execution
    let execute_test = move |_| {
        // Parse parameters as JSON
        match serde_json::from_str::<Value>(&tool_params.read()) {
            Ok(params) => {
                is_executing.set(true);
                execution_result.set(None);
                
                // Clone values for the async block
                let tool_name_value = tool_name.to_string();
                let mcp_state = props.mcp_state.clone();
                
                // Execute the tool
                spawn({
                    to_owned![is_executing, execution_result];
                    async move {
                        match ToolManager::execute_tool(tool_name_value, params, &mcp_state.read()).await {
                            Ok(result) => {
                                let formatted = ToolManager::format_tool_result(&result);
                                execution_result.set(Some(Ok(formatted)));
                            }
                            Err(e) => {
                                execution_result.set(Some(Err(format!("Error: {}", e))));
                            }
                        }
                        is_executing.set(false);
                    }
                });
            }
            Err(e) => {
                execution_result.set(Some(Err(format!("Invalid JSON: {}", e))));
            }
        }
    };
    
    rsx! {
        div { class: "dialog-overlay",
            div { class: "tool-test-dialog",
                div { class: "dialog-header",
                    h2 { class: "dialog-title", "Test Tool: {tool_name}" }
                    button {
                        class: "dialog-close",
                        onclick: move |_| props.on_close.call(()),
                        "×"
                    }
                }
                
                div { class: "dialog-content",
                    p { class: "tool-description", "{tool_description}" }
                    
                    div { class: "form-group",
                        label { "Parameters (JSON):" }
                        textarea {
                            class: "form-control",
                            value: "{tool_params}",
                            oninput: move |evt| tool_params.set(evt.value().clone()),
                            placeholder: "Enter parameters as JSON",
                            disabled: *is_executing.read(),
                        }
                    }
                    
                    if let Some(result) = &*execution_result.read() {
                        match result {
                            Ok(success) => {
                                rsx! {
                                    div { class: "tool-result",
                                        h3 { "Result:" }
                                        pre { "{success}" }
                                    }
                                }
                            }
                            Err(error) => {
                                rsx! {
                                    div { class: "tool-result error",
                                        h3 { "Error:" }
                                        pre { "{error}" }
                                    }
                                }
                            }
                        }
                    }
                }
                
                div { class: "dialog-footer",
                    button {
                        class: "btn-cancel",
                        onclick: move |_| props.on_close.call(()),
                        "Close"
                    }
                    button {
                        class: "btn-submit",
                        onclick: execute_test,
                        disabled: *is_executing.read(),
                        if *is_executing.read() {
                            "Executing..."
                        } else {
                            "Execute Tool"
                        }
                    }
                }
            }
        }
    }
}

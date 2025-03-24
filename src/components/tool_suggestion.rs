use dioxus::prelude::*;
use mcp_core::Tool;
use serde_json::{Value, json};

#[derive(PartialEq, Props, Clone)]
pub struct ToolSuggestionProps {
    pub tool: Tool,
    pub suggested_args: Value,
    pub on_execute: EventHandler<(String, Value)>,
    pub on_cancel: EventHandler<()>,
}

/// Component for displaying a tool suggestion from the AI with execute/cancel buttons
#[component]
pub fn ToolSuggestion(props: ToolSuggestionProps) -> Element {
    let tool_name = props.tool.name.clone();
    let initial_args = serde_json::to_string_pretty(&props.suggested_args).unwrap_or_else(|_| "{}".to_string());
    let mut args_text = use_signal(|| initial_args);
    let mut parse_error = use_signal(|| None::<String>);
    
    let execute_with_args = move |_| {
        // Try to parse the current args text as JSON
        match serde_json::from_str::<Value>(&args_text.read()) {
            Ok(parsed_args) => {
                parse_error.set(None);
                props.on_execute.call((tool_name.clone(), parsed_args));
            },
            Err(err) => {
                parse_error.set(Some(format!("Invalid JSON: {}", err)));
            }
        }
    };
    
    rsx! {
        div { class: "tool-suggestion",
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
                h3 { class: "tool-name", "{props.tool.name}" }
            }
            p { class: "tool-description", "{props.tool.description}" }
            
            div { class: "tool-args",
                h4 { "Arguments (editable):" }
                textarea {
                    class: "args-editor",
                    value: "{args_text}",
                    oninput: move |evt| args_text.set(evt.value().clone()),
                    rows: "5",
                    style: "width: 100%; font-family: monospace; font-size: 13px;"
                }
                
                if let Some(error) = parse_error.read().as_ref() {
                    div { class: "parse-error", "{error}" }
                }
            }
            
            div { class: "tool-actions",
                button {
                    class: "btn-cancel",
                    onclick: move |_| props.on_cancel.call(()),
                    "Cancel"
                }
                button {
                    class: "btn-execute",
                    onclick: execute_with_args,
                    "Execute Tool"
                }
            }
        }
    }
}

/// Component for displaying tool execution status and results
#[derive(PartialEq, Props, Clone)]
pub struct ToolExecutionProps {
    pub tool_name: String,
    pub status: ToolExecutionStatus,
    pub result: Option<String>,
}

#[derive(PartialEq, Clone, Debug)]
pub enum ToolExecutionStatus {
    Running,
    Completed,
    Failed(String),
}

#[component]
pub fn ToolExecution(props: ToolExecutionProps) -> Element {
    rsx! {
        div { class: "tool-execution",
            div { class: "execution-header",
                match props.status {
                    ToolExecutionStatus::Running => {
                        rsx! {
                            div { class: "status running", 
                                div { class: "spinner" },
                                span { "Running {props.tool_name}..." }
                            }
                        }
                    }
                    ToolExecutionStatus::Completed => {
                        rsx! {
                            div { class: "status completed",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "16",
                                    height: "16",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M20 6L9 17l-5-5" }
                                }
                                span { "Completed {props.tool_name}" }
                            }
                        }
                    }
                    ToolExecutionStatus::Failed(ref error) => {
                        rsx! {
                            div { class: "status failed",
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "16",
                                    height: "16",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M18 6L6 18" },
                                    path { d: "M6 6l12 12" }
                                }
                                span { "Failed: {error}" }
                            }
                        }
                    }
                }
            }
            
            if let Some(result) = &props.result {
                div { class: "result-content",
                    pre { "{result}" }
                }
            }
        }
    }
} 
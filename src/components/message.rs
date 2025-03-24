use dioxus::prelude::*;
use crate::components::tool_suggestion::{ToolSuggestion, ToolSuggestionProps, ToolExecution, ToolExecutionProps, ToolExecutionStatus};
use crate::components::tool_manager::{ToolManager, ToolInteraction};
use mcp_core::Tool;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Thinking,
    Tool,  // New role for tool messages
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    // Optional tool interaction associated with this message
    pub tool_interaction: Option<ToolInteraction>,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: chrono::Utc::now(),
            tool_interaction: None,
        }
    }
    
    pub fn with_tool_interaction(mut self, interaction: ToolInteraction) -> Self {
        self.tool_interaction = Some(interaction);
        self
    }
    
    pub fn to_openrouter_format(&self) -> crate::openrouter::ChatMessage {
        let role = match self.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Thinking => "assistant", // We'll filter this out before sending
            MessageRole::Tool => "system", // Tool messages will be injected as system messages
        };
        
        crate::openrouter::ChatMessage {
            role: role.to_string(),
            content: self.content.clone(),
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct MessageViewProps {
    pub message: Message,
    pub tools: Vec<Tool>,
    pub on_tool_execute: EventHandler<(String, Value)>,
    pub on_tool_cancel: EventHandler<usize>,
}

#[component]
pub fn MessageView(props: MessageViewProps) -> Element {
    // Clone the message to own it fully
    let message = props.message.clone();
    
    let role_class = match message.role {
        MessageRole::User => "user-message",
        MessageRole::Assistant => "assistant-message",
        MessageRole::System => "system-message",
        MessageRole::Thinking => "thinking-message",
        MessageRole::Tool => "tool-message",
    };
    
    let avatar_icon = match message.role {
        MessageRole::User => rsx! {
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
                    cy: "8",
                    r: "5"
                }
                path {
                    d: "M20 21a8 8 0 1 0-16 0"
                }
            }
        },
        MessageRole::Assistant => rsx! {
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
                    width: "18",
                    height: "11",
                    x: "3",
                    y: "11",
                    rx: "2",
                    ry: "2"
                }
                path {
                    d: "M7 11V7a5 5 0 0 1 10 0v4"
                }
            }
        },
        MessageRole::System => rsx! {
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
                line {
                    x1: "12",
                    x2: "12",
                    y1: "8",
                    y2: "16"
                }
                line {
                    x1: "8",
                    x2: "16",
                    y1: "12",
                    y2: "12"
                }
            }
        },
        MessageRole::Thinking => rsx! {
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
                path {
                    d: "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"
                }
                line {
                    x1: "12",
                    x2: "12.01",
                    y1: "17",
                    y2: "17"
                }
            }
        },
        MessageRole::Tool => rsx! {
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
                    d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                }
            }
        },
    };
    
    let message_sender = match message.role {
        MessageRole::User => "You",
        MessageRole::Assistant => "Assistant",
        MessageRole::System => "System",
        MessageRole::Thinking => "Thinking...",
        MessageRole::Tool => "Tool",
    };
    
    let timestamp_str = message.timestamp.format("%H:%M").to_string();
    
    rsx! {
        div {
            id: "MessageView",
            class: match message.role {
                MessageRole::User => "message user-message",
                MessageRole::Assistant => "message assistant-message",
                MessageRole::System => "message system-message",
                MessageRole::Thinking => "message thinking-message",
                MessageRole::Tool => "message tool-message",
            },
            
            style {
                ".message-text {{
                    flex: 1;
                    word-break: break-word;
                    overflow-wrap: break-word;
                }}
                
                .system-message-content {{
                    padding: 8px 12px;
                    border-radius: 8px;
                    margin-bottom: 8px;
                }}
                
                .system-message-content.alternatives-message {{
                    background-color: rgba(255, 165, 0, 0.1);
                    border-left: 4px solid orange;
                }}
                
                .system-message-content.recovery-message {{
                    background-color: rgba(0, 128, 0, 0.1);
                    border-left: 4px solid green;
                }}
                
                .warning-container, .recovery-container {{
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    font-weight: 600;
                    margin-bottom: 8px;
                }}
                
                .warning-icon {{
                    color: orange;
                }}
                
                .recovery-icon {{
                    color: green;
                }}
                
                .error-details {{
                    margin-bottom: 12px;
                }}
                
                .alternative-tools-section h4 {{
                    font-size: 14px;
                    margin: 8px 0;
                }}
                
                .alternative-tool-item {{
                    padding: 4px 0;
                    font-family: monospace;
                }}
                
                .recovery-paragraph {{
                    margin: 4px 0;
                }}"
            },
            div {
                class: "message-avatar",
                div {
                    class: format!("avatar-icon {}-avatar", role_class),
                    {avatar_icon}
                }
            }
            div {
                class: "message-content",
                div {
                    class: "message-header",
                    div {
                        class: "message-sender",
                        {message_sender}
                    }
                    div {
                        class: "message-time",
                        {timestamp_str}
                    }
                }
                div {
                    class: "message-text",
                    if message.role == MessageRole::Thinking {
                        div {
                            class: "typing-indicator",
                            div { class: "dot" }
                            div { class: "dot" }
                            div { class: "dot" }
                        }
                    } else if message.role == MessageRole::System && message.content.contains("Alternative tools") {
                        // Special handling for system messages with alternative tools
                        div { 
                            class: "system-message-content alternatives-message",
                            div {
                                class: "warning-container",
                                svg {
                                    class: "warning-icon",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "20",
                                    height: "20",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" },
                                    line { x1: "12", y1: "9", x2: "12", y2: "13" },
                                    line { x1: "12", y1: "17", x2: "12.01", y2: "17" }
                                }
                                "Tool Validation Failed"
                            }
                            // Split the message to show error details and alternatives separately
                            {message.content.split("Alternative tools").enumerate().map(|(idx, part)| {
                                if idx == 0 {
                                    // First part is the error details
                                    rsx! {
                                        div { class: "error-details", {part} }
                                    }
                                } else {
                                    // Second part is the alternatives
                                    rsx! {
                                        div { 
                                            class: "alternative-tools-section",
                                            h4 { "Alternative tools you might want to use instead:" }
                                            {part.split("\n- ").skip(1).map(|tool_line| {
                                                rsx! {
                                                    div { class: "alternative-tool-item", "- {tool_line}" }
                                                }
                                            })}
                                        }
                                    }
                                }
                            })}
                        }
                    } else if message.role == MessageRole::System && message.content.contains("recovered") {
                        // Special handling for recovery messages
                        div {
                            class: "system-message-content recovery-message",
                            div {
                                class: "recovery-container",
                                svg {
                                    class: "recovery-icon",
                                    xmlns: "http://www.w3.org/2000/svg",
                                    width: "20",
                                    height: "20",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z" },
                                    path { d: "m9 12 2 2 4-4" }
                                }
                                "Input Recovery Applied"
                            }
                            // Split message to show what was recovered
                            {message.content.split("\n\n").map(|paragraph| {
                                if !paragraph.trim().is_empty() {
                                    rsx! {
                                        p { 
                                            class: "recovery-paragraph",
                                            {paragraph}
                                        }
                                    }
                                } else {
                                    rsx! {}
                                }
                            })}
                        }
                    } else {
                        // Default message rendering for other messages
                        // Split by newlines and render paragraphs
                        {message.content.split("\n\n").map(|paragraph| {
                            if !paragraph.trim().is_empty() {
                                rsx! {
                                    p { 
                                        class: "message-paragraph",
                                        {paragraph}
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        })}
                    }
                    
                    // Render tool interactions if present
                    if let Some(tool_interaction) = &message.tool_interaction {
                        match tool_interaction {
                            ToolInteraction::Suggestion { tool_name, suggested_args, message_idx } => {
                                if let Some(tool) = ToolManager::find_tool_by_name(tool_name, &props.tools) {
                                    // Clone the values we need for the closures
                                    let tool_name_clone = tool_name.clone();
                                    let args_clone = suggested_args.clone();
                                    let msg_idx = *message_idx;
                                    
                                    rsx! {
                                        ToolSuggestion {
                                            tool: tool.clone(),
                                            suggested_args: args_clone.clone(),
                                            on_execute: move |(name, args)| {
                                                props.on_tool_execute.call((name, args))
                                            },
                                            on_cancel: move |_| {
                                                props.on_tool_cancel.call(msg_idx)
                                            }
                                        }
                                    }
                                } else {
                                    rsx! {}
                                }
                            },
                            ToolInteraction::Execution { tool_name, arguments: _, status, result, message_idx: _ } => {
                                // Clone values for the ToolExecution component
                                let tool_name_clone = tool_name.clone();
                                let status_clone = status.clone();
                                let result_clone = result.clone();
                                
                                rsx! {
                                    ToolExecution {
                                        tool_name: tool_name_clone,
                                        status: status_clone,
                                        result: result_clone,
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
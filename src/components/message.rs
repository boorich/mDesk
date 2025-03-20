use dioxus::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Thinking,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content,
            timestamp: chrono::Utc::now(),
        }
    }
    
    pub fn to_openrouter_format(&self) -> crate::openrouter::ChatMessage {
        let role = match self.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Thinking => "assistant", // We'll filter this out before sending
        };
        
        crate::openrouter::ChatMessage {
            role: role.to_string(),
            content: self.content.clone(),
        }
    }
}

#[component]
pub fn MessageView(message: Message) -> Element {
    let role_class = match message.role {
        MessageRole::User => "user-message",
        MessageRole::Assistant => "assistant-message",
        MessageRole::System => "system-message",
        MessageRole::Thinking => "thinking-message",
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
    };
    
    rsx! {
        div {
            class: "message {role_class}",
            div {
                class: "message-avatar",
                div {
                    class: "avatar-icon {role_class}-avatar",
                    {avatar_icon}
                }
            }
            div {
                class: "message-content",
                div {
                    class: "message-header",
                    div {
                        class: "message-sender",
                        match message.role {
                            MessageRole::User => "You",
                            MessageRole::Assistant => "Assistant",
                            MessageRole::System => "System",
                            MessageRole::Thinking => "Thinking..."
                        }
                    }
                    div {
                        class: "message-time",
                        "{message.timestamp.format(\"%H:%M\")}"
                    }
                }
                div {
                    class: "message-text",
                    if message.role == MessageRole::Thinking {
                        rsx! {
                            div {
                                class: "typing-indicator",
                                div { class: "dot" }
                                div { class: "dot" }
                                div { class: "dot" }
                            }
                        }
                    } else {
                        // Split by newlines and render paragraphs
                        {message.content.split("\n\n").map(|paragraph| {
                            if !paragraph.trim().is_empty() {
                                rsx! {
                                    p { 
                                        class: "message-paragraph",
                                        "{paragraph}" 
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        })}
                    }
                }
            }
        }
    }
}

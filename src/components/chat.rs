use dioxus::prelude::*;
use crate::openrouter::{OpenRouterClient, ChatMessage, ModelInfo};
use crate::components::message::{Message, MessageRole, MessageView};
use std::env;
use mcp_core::Tool;

// Define a struct to hold OpenRouter models for the dropdown
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSelection {
    pub models: Vec<ModelInfo>,
    pub selected_model: String,
    pub loading: bool,
    pub error: Option<String>,
}

impl ModelSelection {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            selected_model: "anthropic/claude-3-opus".to_string(), // Default model
            loading: false,
            error: None,
        }
    }
}

#[component]
pub fn ChatTab(
    mcp_tools: Vec<Tool>,
    api_key: Option<String>
) -> Element {
    // Chat state
    let mut messages = use_signal(Vec::<Message>::new);
    let mut input = use_signal(String::new);
    let mut is_sending = use_signal(|| false);
    let mut model_selection = use_signal(ModelSelection::new);
    
    // OpenRouter client setup
    let openrouter_api_key = match api_key {
        Some(key) => key,
        None => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
    };
    
    let client = use_memo(|| {
        OpenRouterClient::new(openrouter_api_key.clone())
    });
    
    // Function to load available models
    let load_models = move |_| {
        if model_selection.read().loading {
            return;
        }
        
        model_selection.write().loading = true;
        model_selection.write().error = None;
        
        spawn({
            to_owned![client, model_selection];
            async move {
                match client.list_models().await {
                    Ok(models) => {
                        model_selection.write().models = models;
                        if model_selection.read().selected_model.is_empty() && !models.is_empty() {
                            model_selection.write().selected_model = models[0].id.clone();
                        }
                        model_selection.write().loading = false;
                    }
                    Err(e) => {
                        model_selection.write().error = Some(format!("Failed to load models: {}", e));
                        model_selection.write().loading = false;
                    }
                }
            }
        });
    };
    
    // Automatically load models on component mount
    if model_selection.read().models.is_empty() && !model_selection.read().loading {
        spawn({
            to_owned![load_models];
            async move {
                load_models(());
            }
        });
    }
    
    // Function to send message
    let send_message = move |_| {
        let user_input = input.get().trim().to_string();
        if user_input.is_empty() || is_sending.get() {
            return;
        }
        
        // Add user message
        messages.write().push(Message::new(MessageRole::User, user_input.clone()));
        input.set("".to_string());
        is_sending.set(true);
        
        // Add thinking message
        let thinking_id = messages.write().len();
        messages.write().push(Message::new(MessageRole::Thinking, "".to_string()));
        
        // Prepare chat history for API
        let chat_history: Vec<ChatMessage> = messages.read()
            .iter()
            .filter(|msg| msg.role != MessageRole::Thinking)
            .map(|msg| msg.to_openrouter_format())
            .collect();
        
        // Get selected model
        let selected_model = model_selection.read().selected_model.clone();
        
        spawn({
            to_owned![messages, client, selected_model, user_input, is_sending, mcp_tools];
            async move {
                // Check if we need to use any MCP tools
                // Simple approach: check message content for relevant keywords
                let mut tool_output = String::new();
                let potentially_relevant_tools = mcp_tools.iter()
                    .filter(|tool| {
                        user_input.to_lowercase().contains(&tool.name.to_lowercase()) ||
                        user_input.to_lowercase().contains(&tool.description.to_lowercase())
                    })
                    .collect::<Vec<_>>();
                
                if !potentially_relevant_tools.is_empty() {
                    tool_output.push_str("\n\nRelevant tools found:\n");
                    for tool in potentially_relevant_tools {
                        tool_output.push_str(&format!("- {}: {}\n", tool.name, tool.description));
                    }
                }
                
                // Create system message with context
                let system_message = if tool_output.is_empty() {
                    ChatMessage {
                        role: "system".to_string(),
                        content: "You are a helpful assistant in the mDesk application. Answer the user's questions directly and concisely.".to_string(),
                    }
                } else {
                    ChatMessage {
                        role: "system".to_string(),
                        content: format!(
                            "You are a helpful assistant in the mDesk application. The following tools are available for this query:{}\n\nAnswer the user's questions directly and concisely, mentioning the relevant tools where appropriate.", 
                            tool_output
                        ),
                    }
                };
                
                // Prepare final messages for API
                let mut api_messages = vec![system_message];
                api_messages.extend(chat_history);
                
                // Send to OpenRouter
                match client.chat_completion(
                    &selected_model,
                    api_messages,
                    Some(0.7), // temperature
                    None,      // max_tokens (use default)
                ).await {
                    Ok(response) => {
                        if let Some(assistant_message) = response.choices.first() {
                            // Replace thinking message with actual response
                            if thinking_id < messages.read().len() {
                                messages.write()[thinking_id] = Message::new(
                                    MessageRole::Assistant,
                                    assistant_message.message.content.clone()
                                );
                            } else {
                                messages.write().push(Message::new(
                                    MessageRole::Assistant,
                                    assistant_message.message.content.clone()
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        // Replace thinking message with error
                        if thinking_id < messages.read().len() {
                            messages.write()[thinking_id] = Message::new(
                                MessageRole::System,
                                format!("Error: {}", e)
                            );
                        } else {
                            messages.write().push(Message::new(
                                MessageRole::System,
                                format!("Error: {}", e)
                            ));
                        }
                    }
                }
                
                is_sending.set(false);
            }
        });
    };
    
    // Handle Enter key
    let handle_keydown = move |evt: KeyboardEvent| {
        if evt.key() == "Enter" && !evt.shift_key() {
            evt.prevent_default();
            send_message(());
        }
    };
    
    // UI Rendering
    rsx! {
        div { class: "chat-container",
            // Model selector section
            div { class: "model-selector",
                div { class: "model-selector-header",
                    h3 { class: "model-title", "Select AI Model" }
                    button {
                        class: "refresh-models-button",
                        disabled: model_selection.read().loading,
                        onclick: load_models,
                        svg {
                            class: "refresh-icon",
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
                        "Refresh"
                    }
                }
                
                if let Some(error) = &model_selection.read().error {
                    div { class: "model-error",
                        "{error}"
                    }
                }
                
                select {
                    class: "model-dropdown",
                    disabled: model_selection.read().loading || model_selection.read().models.is_empty(),
                    value: "{model_selection.read().selected_model}",
                    oninput: move |evt| {
                        model_selection.write().selected_model = evt.value.clone();
                    },
                    
                    if model_selection.read().models.is_empty() {
                        option { value: "", disabled: true,
                            if model_selection.read().loading {
                                "Loading models..."
                            } else {
                                "No models available"
                            }
                        }
                    } else {
                        for model in &model_selection.read().models {
                            option {
                                value: "{model.id}",
                                "{model.name}"
                            }
                        }
                    }
                }
            }
            
            // Messages area
            div { class: "chat-messages",
                if messages.read().is_empty() {
                    div { class: "empty-chat",
                        div { class: "empty-chat-icon",
                            svg {
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
                                    d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"
                                }
                            }
                        }
                        div { class: "empty-chat-title", "No messages yet" }
                        div { class: "empty-chat-subtitle", "Start a conversation with any of the available AI models" }
                    }
                } else {
                    for message in messages.read().iter() {
                        MessageView { message: message.clone() }
                    }
                }
            }
            
            // Input area
            div { class: "chat-input-container",
                textarea {
                    class: "chat-input",
                    placeholder: "Type your message...",
                    value: "{input}",
                    disabled: is_sending.get(),
                    oninput: move |evt| input.set(evt.value.clone()),
                    onkeydown: handle_keydown,
                }
                button {
                    class: "chat-send-button",
                    disabled: is_sending.get() || input.get().trim().is_empty(),
                    onclick: send_message,
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
                        line {
                            x1: "22", 
                            y1: "2", 
                            x2: "11", 
                            y2: "13"
                        }
                        polygon {
                            points: "22 2 15 22 11 13 2 9 22 2"
                        }
                    }
                }
            }
        }
    }
}

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
    
    let client = use_signal(|| OpenRouterClient::new(openrouter_api_key.clone()));
    
    // Use a static flag to ensure model loading only happens once
    static mut MODELS_LOADED: bool = false;
    
    // Function to load available models
    let mut load_models = move |_| {
        if model_selection.read().loading {
            return;
        }
        
        model_selection.write().loading = true;
        model_selection.write().error = None;
        
        let client_instance = client.read().clone();
        spawn({
            to_owned![model_selection];
            async move {
                match client_instance.list_models().await {
                    Ok(models) => {
                        let model_ids = models.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
                        model_selection.write().models = models;
                        
                        // Select first model if needed
                        if model_selection.read().selected_model.is_empty() && !model_ids.is_empty() {
                            model_selection.write().selected_model = model_ids[0].clone();
                        }
                        
                        model_selection.write().loading = false;
                    }
                    Err(e) => {
                        model_selection.write().error = Some(format!("Error fetching models: {}", e));
                        model_selection.write().loading = false;
                    }
                }
            }
        });
    };
    
    // Initialize models only once
    // We use unsafe to access the static flag, but it's safe because we're only loading models once
    let should_load_models = unsafe {
        if !MODELS_LOADED {
            MODELS_LOADED = true;
            true
        } else {
            false
        }
    };
    
    // Provide fallback models in case of API failure
    let fallback_models = || vec![
        ModelInfo {
            id: "anthropic/claude-3-opus".to_string(),
            name: "Claude 3 Opus".to_string(),
            description: Some("Anthropic's most capable model for highly complex tasks".to_string()),
            context_length: Some(200000),
            pricing: None,
        },
        ModelInfo {
            id: "anthropic/claude-3-sonnet".to_string(),
            name: "Claude 3 Sonnet".to_string(),
            description: Some("Anthropic's balanced model for most tasks".to_string()),
            context_length: Some(180000),
            pricing: None,
        },
        ModelInfo {
            id: "openai/gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            description: Some("OpenAI's latest multimodal model".to_string()),
            context_length: Some(128000),
            pricing: None,
        },
    ];
    
    if should_load_models {
        spawn({
            to_owned![load_models, model_selection, fallback_models];
            async move {
                // Try to load models from API
                load_models(());
                
                // Wait a short time to see if load fails
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                
                // If after 5 seconds we still have no models and there was an error, use fallback
                if model_selection.read().models.is_empty() && model_selection.read().error.is_some() {
                    eprintln!("Using fallback models list due to API error");
                    model_selection.write().models = fallback_models();
                    model_selection.write().loading = false;
                    // Keep the error message for debugging purposes
                }
            }
        });
    }
    
    // Add a retry button handler that forces model loading
    let mut retry_load_models = move |_| {
        // Clear previous errors
        model_selection.write().error = None;
        model_selection.write().loading = true;
        
        let client_instance = client.read().clone();
        let fallback = fallback_models();
        
        spawn({
            to_owned![model_selection];
            async move {
                match client_instance.list_models().await {
                    Ok(models) => {
                        let model_ids = models.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
                        model_selection.write().models = models;
                        
                        // Select first model if needed
                        if model_selection.read().selected_model.is_empty() && !model_ids.is_empty() {
                            model_selection.write().selected_model = model_ids[0].clone();
                        }
                        
                        model_selection.write().loading = false;
                    }
                    Err(e) => {
                        // Use fallback models but keep the error message
                        model_selection.write().error = Some(format!("Error: {}. Using fallback models.", e));
                        model_selection.write().models = fallback;
                        model_selection.write().loading = false;
                    }
                }
            }
        });
    };
    
    // Function to send message
    let mut send_message = move |_| {
        let user_input = input.read().trim().to_string();
        if user_input.is_empty() || *is_sending.read() {
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
        let client_instance = client.read().clone();
        let tools_clone = mcp_tools.clone();
        
        spawn({
            to_owned![messages, is_sending, user_input];
            async move {
                // Check if we need to use any MCP tools
                // Simple approach: check message content for relevant keywords
                let mut tool_output = String::new();
                let potentially_relevant_tools = tools_clone.iter()
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
                        content: "You are a helpful AI assistant.".to_string(),
                    }
                } else {
                    ChatMessage {
                        role: "system".to_string(),
                        content: format!("You are a helpful AI assistant. The user's message may require the use of MCP tools. Here are tools that might be relevant to the query: {}", tool_output),
                    }
                };
                
                // Add system message to beginning of chat history
                let mut final_messages = vec![system_message];
                final_messages.extend(chat_history);
                
                // Call OpenRouter API
                match client_instance.chat_completion(
                    &selected_model,
                    final_messages,
                    Some(0.7), // temperature
                    Some(1000), // max tokens
                ).await {
                    Ok(response) => {
                        // Remove thinking message
                        if thinking_id < messages.read().len() {
                            messages.write().remove(thinking_id);
                        }
                        
                        // Add assistant's response
                        if let Some(choice) = response.choices.first() {
                            messages.write().push(Message::new(
                                MessageRole::Assistant,
                                choice.message.content.clone(),
                            ));
                        }
                    }
                    Err(e) => {
                        // Replace thinking message with error
                        if thinking_id < messages.read().len() {
                            messages.write()[thinking_id] = Message::new(
                                MessageRole::System,
                                format!("Error: {}", e),
                            );
                        }
                    }
                }
                
                is_sending.set(false);
            }
        });
    };
    
    // Handle Enter key
    let mut send_message_ref = send_message.clone();
    let handle_keydown = move |evt: KeyboardEvent| {
        if evt.key().to_string() == "Enter" && !evt.modifiers().shift() {
            evt.prevent_default();
            send_message_ref(());
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
                        onclick: move |_| retry_load_models(()),
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
                            path { d: "M23 4v6h-6" }
                            path { d: "M1 20v-6h6" }
                            path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" }
                        }
                        "Retry"
                    }
                }
                if let Some(error) = &model_selection.read().error {
                    div { class: "model-error", 
                        // Show more user-friendly error message
                        if error.contains("Using fallback models") {
                            div {
                                span { 
                                    class: "warning-icon",
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
                                        path { d: "M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" },
                                        line { x1: "12", y1: "9", x2: "12", y2: "13" },
                                        line { x1: "12", y1: "17", x2: "12.01", y2: "17" }
                                    }
                                }
                                "API connection error. Using local model data."
                            }
                        } else {
                            "{error}"
                        }
                    }
                }
                select {
                    class: "model-dropdown",
                    disabled: model_selection.read().loading || model_selection.read().models.is_empty(),
                    value: "{model_selection.read().selected_model}",
                    onchange: move |evt| model_selection.write().selected_model = evt.value().clone(),
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
                            option { value: "{model.id}", "{model.name}" }
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
                                path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                            }
                        }
                        div { class: "empty-chat-title", "No messages yet" }
                        div { class: "empty-chat-subtitle",
                            "Start a conversation with any of the available AI models"
                        }
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
                    disabled: *is_sending.read(),
                    oninput: move |evt| input.set(evt.value().clone()),
                    onkeydown: handle_keydown,
                }
                button {
                    class: "chat-send-button",
                    disabled: *is_sending.read() || input.read().trim().is_empty(),
                    onclick: move |_| send_message(()),
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
                            y2: "13",
                        }
                        polygon { points: "22 2 15 22 11 13 2 9 22 2" }
                    }
                }
            }
        }
    }
}
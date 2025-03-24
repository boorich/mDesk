use dioxus::prelude::*;
use crate::openrouter::{OpenRouterClient, ChatMessage, ModelInfo};
use crate::components::message::{Message, MessageRole, MessageView};
use std::env;
use mcp_core::Tool;
use mcp_client::McpClientTrait;
use crate::components::tool_manager::{ToolManager, ToolInteraction};
use crate::components::tool_suggestion::ToolExecutionStatus;
use crate::McpState;
use serde_json::{Value, json};
use crate::components::validation_pipeline::{ValidationPipeline, ValidationState, RecoveryStrategy};
use crate::components::tool_selection_cache::ToolSelectionCache;
use crate::components::tool_selection::{LLMToolSelector, RankedToolSelection, ToolMatch, ValidationStatus};
use std::sync::Arc;
use anyhow::Result;
use tracing::{debug, info, warn, error};

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
    api_key: Option<String>,
    mcp_state: Signal<McpState>,
) -> Element {
    // Clone api_key to avoid ownership issues
    let api_key_ref = api_key.clone();
    
    // Chat state
    let mut messages = use_signal(Vec::<Message>::new);
    let mut input = use_signal(String::new);
    let mut is_sending = use_signal(|| false);
    let mut model_selection = use_signal(ModelSelection::new);
    let mut confidence_threshold = use_signal(|| 0.7); // New signal for confidence threshold
    
    // Store mcp_tools in a signal so it can be accessed from multiple closures
    let tools = use_signal(|| {
        // Fetch initial tools from props
        let mut all_tools = mcp_tools.clone();
        
        // Log the available tools for debugging
        eprintln!("Available tools for ChatTab component: {}", all_tools.len());
        for tool in &all_tools {
            eprintln!("  - Tool: {} ({})", tool.name, tool.description);
        }
        
        all_tools
    });
    
    // Function to fetch tools from all connected servers
    let mut fetch_all_servers_tools = {
        to_owned![tools, mcp_state];
        move || {
            // Use a static variable to track the last fetch time
            static mut LAST_FETCH_TIME: Option<std::time::Instant> = None;
            
            // Only fetch if it's been at least 5 seconds since the last fetch
            let should_fetch = unsafe {
                match LAST_FETCH_TIME {
                    Some(last_time) => {
                        if last_time.elapsed() >= std::time::Duration::from_secs(5) {
                            LAST_FETCH_TIME = Some(std::time::Instant::now());
                            true
                        } else {
                            false
                        }
                    },
                    None => {
                        // First time fetching
                        LAST_FETCH_TIME = Some(std::time::Instant::now());
                        true
                    }
                }
            };
            
            // Skip if we recently fetched tools
            if !should_fetch && !tools.read().is_empty() {
                eprintln!("Skipping tool fetch - last fetch was too recent");
                return;
            }
            
            eprintln!("Initiating tool fetch from all servers");
            spawn({
                to_owned![tools, mcp_state];
                async move {
                    // Get all active clients
                    let active_clients = mcp_state.read().active_clients.clone();
                    
                    // Skip if no clients
                    if active_clients.is_empty() {
                        return;
                    }
                    
                    // Collect tools from all clients
                    let mut all_tools = Vec::new();
                    
                    for (server_id, client_arc) in active_clients {
                        // Get a lock on the client
                        let client = client_arc.lock().await;
                        
                        // Try to fetch tools
                        match client.list_tools(None).await {
                            Ok(result) => {
                                eprintln!("Fetched {} tools from server {}", result.tools.len(), server_id);
                                
                                // Add these tools to our collection
                                all_tools.extend(result.tools);
                            }
                            Err(e) => {
                                eprintln!("Error fetching tools from server {}: {}", server_id, e);
                            }
                        }
                    }
                    
                    // Now update the tools signal with all tools from all servers
                    if !all_tools.is_empty() {
                        eprintln!("Updating tools with {} total tools from all servers", all_tools.len());
                        tools.set(all_tools);
                    }
                }
            });
        }
    };
    
    // Call the function to fetch tools during initialization
    // Only call this if we haven't preloaded tools yet
    if !unsafe { PRELOAD_CALLED } {
        fetch_all_servers_tools();
    }
    
    // Periodically refresh tools (every 2 minutes instead of 30 seconds)
    use_coroutine(move |_rx: dioxus::prelude::UnboundedReceiver<()>| {
        to_owned![fetch_all_servers_tools];
        async move {
            loop {
                // Wait 2 minutes (120 seconds)
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                
                // Fetch tools
                fetch_all_servers_tools();
            }
        }
    });
    
    // Tool validation pipeline - We don't use use_memo since we need to get the read value each time
    let validation_pipeline = ValidationPipeline::new()
        .with_max_depth(10)
        .with_max_string_length(1000)
        .with_auto_fix(true)
        .with_suggest_alternatives(true)
        .with_max_alternatives(3)
        .with_fallback("count", json!(5))
        .with_fallback("limit", json!(100))
        .with_available_tools(tools.read().clone());
    
    // Tool selection cache
    let cache_arc = Arc::new(ToolSelectionCache::new(
        std::time::Duration::from_secs(300), // 5 minute cache expiration
        100 // Max 100 entries
    ));
    
    // Store cache in a signal so it can be accessed from multiple closures
    let cache = use_signal(|| cache_arc.clone());
    
    // Tool selector using LLM
    let mut tool_selector = use_signal(|| {
        // Get the OpenRouter API key
        let api_key = match &api_key_ref {
            Some(key) => key.clone(),
            None => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
        };
        
        // Create the tool selector with the same model as the chat
        let model = model_selection.read().selected_model.clone();
        let selector = LLMToolSelector::new(api_key, model.clone())
            .with_cache(cache.read().clone())
            .with_max_prompt_tools(25); // Limit to 25 tools per prompt
            
        eprintln!("Created LLMToolSelector with model: {}", model);
        selector
    });
    
    // Debug MCP state and try to preload tools immediately if possible
    static mut CLIENT_STATE_LOGGED: bool = false;
    let should_log = unsafe {
        if CLIENT_STATE_LOGGED {
            false
        } else {
            CLIENT_STATE_LOGGED = true;
            true
        }
    };
    
    if should_log {
        eprintln!("ChatTab received MCP client state: {}", if mcp_state.read().client.is_some() { "Client available" } else { "No client available" });
    }
    
    // Only preload tools if we don't have any and fetch_all_servers_tools hasn't been called yet
    static mut PRELOAD_CALLED: bool = false;
    let should_preload = unsafe {
        if PRELOAD_CALLED {
            false
        } else {
            PRELOAD_CALLED = true;
            tools.read().is_empty() && mcp_state.read().client.is_some()
        }
    };
    
    if should_preload {
        eprintln!("Preloading tools during ChatTab initialization");
        let tools_clone = tools.clone();
        let mcp_clone = mcp_state.clone();
        
        spawn({
            to_owned![tools_clone, mcp_clone];
            async move {
                if let Some(client_arc) = &mcp_clone.read().client {
                    let client = client_arc.lock().await;
                    match client.list_tools(None).await {
                        Ok(result) => {
                            eprintln!("Successfully preloaded {} tools during initialization", result.tools.len());
                            tools_clone.set(result.tools);
                        }
                        Err(e) => {
                            eprintln!("Error preloading tools: {}", e);
                        }
                    }
                }
            }
        });
    }
    
    // OpenRouter client setup
    let openrouter_api_key = match &api_key_ref {
        Some(key) => key.clone(),
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
    
    // Add tool-related state
    let mut active_tool: Option<(String, Value)> = None;
    
    // Function to fetch MCP tools synchronously
    let fetch_tools_sync = move || {
        if mcp_state.read().client.is_some() {
            eprintln!("Fetching MCP tools synchronously for ChatTab");
            
            // We need to run this in a blocking context
            let mut tools_clone = tools.clone();
            let mcp_clone = mcp_state.clone();
            
            // Create a oneshot channel to get the result back
            let (tx, rx) = tokio::sync::oneshot::channel();
            
            spawn(async move {
                if let Some(client_arc) = &mcp_clone.read().client {
                    let client = client_arc.lock().await;
                    match client.list_tools(None).await {
                        Ok(result) => {
                            eprintln!("Successfully fetched {} tools", result.tools.len());
                            tools_clone.set(result.tools.clone());
                            let _ = tx.send(result.tools);
                        }
                        Err(e) => {
                            eprintln!("Error fetching tools: {}", e);
                            let _ = tx.send(Vec::new());
                        }
                    }
                } else {
                    let _ = tx.send(Vec::new());
                }
            });
            
            // Wait a short time for tools to load
            std::thread::sleep(std::time::Duration::from_millis(100));
            
            // Return whatever we got
            return tools.read().clone();
        }
        
        // Return empty list if no client
        return Vec::new();
    };
    
    // Modify send_message function to add tool suggestion detection
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
            .filter(|msg| msg.role != MessageRole::Thinking && msg.role != MessageRole::Tool)
            .map(|msg| msg.to_openrouter_format())
            .collect();
        
        // Get selected model
        let selected_model = model_selection.read().selected_model.clone();
        let client_instance = client.read().clone();
        
        // Check if we have tools available, if not try to fetch them
        let tools_clone = if tools.read().is_empty() && mcp_state.read().client.is_some() {
            eprintln!("No tools available, fetching them before processing message");
            fetch_tools_sync() // This will ensure we have tools before proceeding
        } else {
            tools.read().clone()
        };
        
        // Log the tools we have
        eprintln!("Processing message with {} tools available", tools_clone.len());
        
        // Get the current confidence threshold
        let conf_threshold = *confidence_threshold.read();
        
        // Get a reference to the tool selection cache
        let cache_ref = cache.read().clone();
        
        spawn({
            to_owned![messages, is_sending, user_input, mcp_state, conf_threshold];
            async move {
                // First, check the tool selection cache
                let cached_tool_suggestion = cache_ref.get(&user_input);
                
                if let Some((cached_tool_name, cached_confidence, cached_args)) = cached_tool_suggestion {
                    // Only use cached suggestion if confidence meets threshold
                    if cached_confidence >= conf_threshold {
                        eprintln!("Using cached tool suggestion: {} with confidence {}", cached_tool_name, cached_confidence);
                        
                        // Check if this tool exists
                        if let Some(tool) = tools_clone.iter().find(|t| t.name == cached_tool_name) {
                            // Validate the cached arguments against the tool schema
                            let pipeline = ValidationPipeline::new()
                                .with_max_depth(10)
                                .with_max_string_length(1000)
                                .with_auto_fix(true)
                                .with_suggest_alternatives(true)
                                .with_max_alternatives(3)
                                .with_fallback("count", json!(5))
                                .with_fallback("limit", json!(100))
                                .with_available_tools(tools_clone.clone());
                                
                            let validation_result = pipeline.validate_input(tool, cached_args.clone());
                            
                            match validation_result {
                                ValidationState::Valid(validated_args) => {
                                    // Cached arguments are valid, proceed with suggestion
                                    // Remove thinking message
                                    if thinking_id < messages.read().len() {
                                        messages.write().remove(thinking_id);
                                    }
                                    
                                    // Create a message that suggests using the cached tool
                                    let suggestion_message = format!(
                                        "I'll help you with that using the `{}` tool.\n\nParameters prepared based on your request.\n\nWould you like me to proceed?", 
                                        cached_tool_name
                                    );
                                    
                                    let message_id = messages.write().len();
                                    messages.write().push(
                                        Message::new(
                                            MessageRole::Assistant,
                                            suggestion_message
                                        ).with_tool_interaction(
                                            ToolInteraction::Suggestion {
                                                tool_name: cached_tool_name.clone(),
                                                suggested_args: validated_args.clone(),
                                                message_idx: message_id,
                                            }
                                        )
                                    );
                                    
                                    is_sending.set(false);
                                    return;
                                },
                                ValidationState::Sanitized { sanitized, changes, .. } => {
                                    // Cached arguments needed sanitization
                                    // Remove thinking message
                                    if thinking_id < messages.read().len() {
                                        messages.write().remove(thinking_id);
                                    }
                                    
                                    // Create a message that suggests using the cached tool with sanitized args
                                    let change_desc = if !changes.is_empty() {
                                        format!("\n\nNote: I've made minor adjustments to the parameters: {}", 
                                            changes.join(", "))
                                    } else {
                                        "".to_string()
                                    };
                                    
                                    let suggestion_message = format!(
                                        "I'll help you with that using the `{}` tool.{}\n\nWould you like me to proceed?", 
                                        cached_tool_name,
                                        change_desc
                                    );
                                    
                                    let message_id = messages.write().len();
                                    messages.write().push(
                                        Message::new(
                                            MessageRole::Assistant,
                                            suggestion_message
                                        ).with_tool_interaction(
                                            ToolInteraction::Suggestion {
                                                tool_name: cached_tool_name.clone(),
                                                suggested_args: sanitized.clone(),
                                                message_idx: message_id,
                                            }
                                        )
                                    );
                                    
                                    is_sending.set(false);
                                    return;
                                },
                                ValidationState::Recovered { recovered, strategies, .. } => {
                                    // Cached arguments needed recovery
                                    // Remove thinking message
                                    if thinking_id < messages.read().len() {
                                        messages.write().remove(thinking_id);
                                    }
                                    
                                    // Create a message that suggests using the cached tool with recovered args
                                    let recovery_desc = format!("\n\nNote: I've fixed some issues with the parameters:\n- {}", 
                                        strategies.iter()
                                            .map(|s| s.to_string())
                                            .collect::<Vec<String>>()
                                            .join("\n- ")
                                    );
                                    
                                    let suggestion_message = format!(
                                        "I'll help you with that using the `{}` tool.{}\n\nWould you like me to proceed?", 
                                        cached_tool_name,
                                        recovery_desc
                                    );
                                    
                                    let message_id = messages.write().len();
                                    messages.write().push(
                                        Message::new(
                                            MessageRole::Assistant,
                                            suggestion_message
                                        ).with_tool_interaction(
                                            ToolInteraction::Suggestion {
                                                tool_name: cached_tool_name.clone(),
                                                suggested_args: recovered.clone(),
                                                message_idx: message_id,
                                            }
                                        )
                                    );
                                    
                                    is_sending.set(false);
                                    return;
                                },
                                ValidationState::Invalid { errors, alternative_tools, .. } => {
                                    // Cached arguments are invalid, better use LLM to create new parameters
                                    eprintln!("Cached tool parameters are invalid: {}", errors.join(", "));
                                    // Fall through to normal flow to let LLM handle it
                                    
                                    // If we have alternative tools, add a system message about them
                                    if !alternative_tools.is_empty() {
                                        let mut alt_message = "Cache suggests these alternative tools:\n".to_string();
                                        for alt_tool in &alternative_tools {
                                            if let Some(tool) = tools_clone.iter().find(|t| &t.name == alt_tool) {
                                                alt_message.push_str(&format!("- {} ({})\n", tool.name, tool.description));
                                            }
                                        }
                                        eprintln!("{}", alt_message);
                                    }
                                }
                            }
                        }
                    } else {
                        eprintln!("Cached tool suggestion doesn't meet confidence threshold: {} < {}", 
                            cached_confidence, conf_threshold);
                    }
                }
                
                // Log cache statistics for debugging
                let stats = cache_ref.stats();
                if let Ok(stats_json) = serde_json::to_string(&stats) {
                    eprintln!("Tool selection cache stats: {}", stats_json);
                }
                
                // If no cached suggestion or it didn't meet the threshold, continue with normal flow
                // Create system message with context about available tools
                let mut system_message = String::from("You are a helpful AI assistant with access to special tools. ");
                
                // Provide tool information in the system message
                if !tools_clone.is_empty() {
                    system_message.push_str("IMPORTANT: You only have access to the following tools from the MCP server:\n\n");
                    for tool in &tools_clone {
                        system_message.push_str(&format!("- {}: {}\n", tool.name, tool.description));
                        system_message.push_str(&format!("  Parameters: {}\n\n", tool.input_schema));
                    }
                    
                    // Much more explicit instructions for the model
                    system_message.push_str("\nWhen you need to use one of these tools, simply mention the tool by name.\n");
                    system_message.push_str("For example, you can say something like:\n");
                    
                    if let Some(example_tool) = tools_clone.first() {
                        // Use a real tool as an example
                        system_message.push_str(&format!("\"I'll use the {} tool to help with that\"\n", example_tool.name));
                        system_message.push_str(&format!("\"Let me try the {} tool\"\n", example_tool.name));
                    } else {
                        // Fallback if no tools
                        system_message.push_str("\"I'll use the [tool_name] tool to help with that\"\n");
                        system_message.push_str("\"Let me try the [tool_name] tool\"\n");
                    }
                    
                    // Special hints for SQLite tools if present
                    let has_sqlite_tools = tools_clone.iter().any(|t| 
                        t.name == "execute_query" || 
                        t.name == "create_table" || 
                        t.name == "list_tables" || 
                        t.name.contains("sql"));
                        
                    if has_sqlite_tools {
                        system_message.push_str("\nIMPORTANT: I notice you have SQLite database tools available. You can use these to work with tables and data in a SQLite database. For example:\n");
                        system_message.push_str("- Use list_tables to see what tables are available\n");
                        system_message.push_str("- Use execute_query with a SQL query to run commands like SELECT, INSERT, UPDATE, etc.\n");
                        system_message.push_str("- Use create_table to create new database tables\n");
                    }
                    
                    system_message.push_str("You must only mention tools from the above list. Other standard AI tools like 'Web Search', 'Calculator', etc. are NOT available.\n");
                    system_message.push_str("The system will detect your desire to use the tool, and the user will approve or deny the tool usage.\n");
                    system_message.push_str("If a tool would be helpful, always suggest using one of the AVAILABLE tools listed above to help the user.");
                    
                    // Add confidence threshold guidance
                    system_message.push_str(&format!("\nIMPORTANT: The user's confidence threshold is set to {}. Only suggest tools when you are confident they will help address the user's query directly.", conf_threshold));
                } else {
                    system_message.push_str("IMPORTANT: No MCP tools are currently available. Please do not suggest using any tools as they cannot be executed.");
                }
                
                // Add system message to beginning of chat history
                let mut final_messages = vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: system_message,
                    }
                ];
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
                            let message_content = choice.message.content.clone();
                            let message_id = messages.write().len();
                            
                            // Use the new tool_selection algorithm first
                            let model = model_selection.read().selected_model.clone();
                            let use_new_selector = true; // Set to false to quickly revert if needed
                            
                            if use_new_selector {
                                let user_input_clone = user_input.clone();
                                
                                // Only try using the tool selector if we actually have tools
                                if tools_clone.is_empty() {
                                    // No tools available, just add the regular message
                                    messages.write().push(Message::new(
                                        MessageRole::Assistant,
                                        message_content,
                                    ));
                                    is_sending.set(false);
                                    return;
                                }
                                
                                // Spawn a task to select tools asynchronously
                                spawn({
                                    to_owned![messages, is_sending, tools_clone, cache_ref, conf_threshold, tool_selector];
                                    async move {
                                        // Use the tool selector to find appropriate tools
                                        let selection_result = match tool_selector.read().select_tools(&user_input_clone, tools_clone.clone()).await {
                                            Ok(result) => Ok(result),
                                            Err(e) => {
                                                error!("Tool selection failed: {}", e);
                                                
                                                // Try again with a better formed prompt or fallback
                                                if e.to_string().contains("parse LLM response as JSON") {
                                                    // This is likely a formatting issue with the LLM response
                                                    // Let's try legacy tool detection as a fallback
                                                    info!("Falling back to legacy tool detection due to JSON parsing error");
                                                    
                                                    if let Some((tool_name, suggested_args)) = 
                                                        ToolManager::detect_tool_suggestion(&message_content, &tools_clone) {
                                                        
                                                        // Check if this tool exists
                                                        if let Some(tool) = tools_clone.iter().find(|t| t.name == tool_name) {
                                                            let confidence = 0.8; // Assume reasonably high confidence
                                                            
                                                            info!("Legacy detection found tool: {}", tool_name);
                                                            
                                                            // Create a basic tool match
                                                            let matched_tool = ToolMatch {
                                                                tool: tool.clone(),
                                                                confidence,
                                                                suggested_parameters: Some(suggested_args.clone()),
                                                                reasoning: "Selected based on tool name mention in response".into(),
                                                                validation_status: ValidationStatus::Valid,
                                                            };
                                                            
                                                            Ok(RankedToolSelection::new(vec![matched_tool]))
                                                        } else {
                                                            Err(e)
                                                        }
                                                    } else {
                                                        Err(e)
                                                    }
                                                } else {
                                                    Err(e)
                                                }
                                            }
                                        };
                                        
                                        match selection_result {
                                            Ok(selection) => {
                                                info!("Tool selection complete: {}", selection.validation_summary());
                                                let valid_matches = selection.valid_matches(conf_threshold);
                                                
                                                if !valid_matches.is_empty() {
                                                    // Use the best match
                                                    if let Some(best_match) = selection.best_match() {
                                                        if best_match.confidence >= conf_threshold && best_match.is_valid() {
                                                            let tool_name = best_match.tool.name.clone();
                                                            let suggested_args = best_match.suggested_parameters.clone().unwrap_or(json!({}));
                                                            let reasoning = best_match.reasoning.clone();
                                                            
                                                            info!("Using tool: {} with confidence {}", tool_name, best_match.confidence);
                                                            
                                                            // Format a message that includes the reasoning from the tool selector
                                                            let suggestion_message = format!(
                                                                "I'll help you with that using the `{}` tool.\n\nParameters prepared based on your request.\n\nWould you like me to proceed?", 
                                                                tool_name
                                                            );
                                                            
                                                            let message_id = messages.read().len();
                                                            messages.write().push(
                                                                Message::new(
                                                                    MessageRole::Assistant,
                                                                    suggestion_message
                                                                ).with_tool_interaction(
                                                                    ToolInteraction::Suggestion {
                                                                        tool_name: tool_name.clone(),
                                                                        suggested_args: suggested_args.clone(),
                                                                        message_idx: message_id,
                                                                    }
                                                                )
                                                            );
                                                            
                                                            is_sending.set(false);
                                                            return;
                                                        }
                                                    }
                                                }
                                                
                                                // If we get here, no suitable tool was found, continue with regular message
                                                info!("No suitable tool found, using regular message");
                                                messages.write().push(Message::new(
                                                    MessageRole::Assistant,
                                                    message_content,
                                                ));
                                                is_sending.set(false);
                                            },
                                            Err(e) => {
                                                // Log the error and fall back to the regular message
                                                error!("Tool selection failed: {}", e);
                                                
                                                // Add a detailed message for debugging in dev mode
                                                if cfg!(debug_assertions) {
                                                    // Add the error as a system message when in debug mode
                                                    messages.write().push(Message::new(
                                                        MessageRole::System,
                                                        format!("Tool selection failed (debug info): {}", e)
                                                    ));
                                                }
                                                
                                                // Continue with the regular message
                                                messages.write().push(Message::new(
                                                    MessageRole::Assistant,
                                                    message_content,
                                                ));
                                                is_sending.set(false);
                                            }
                                        }
                                    }
                                });
                                
                                // We've handled the message in the async task, so return early
                                return;
                            }
                            
                            // Original tool suggestion detection (fallback approach)
                            // Check if the message contains a tool suggestion
                            eprintln!("Checking message for tool suggestions using legacy detection...");
                            
                            if let Some((tool_name, suggested_args)) = 
                                ToolManager::detect_tool_suggestion(&message_content, &tools_clone) {
                                
                                eprintln!("Found tool suggestion for '{}' in message!", tool_name);
                                
                                // Check if this tool exists in MCP
                                let tool_exists = tools_clone.iter().any(|t| t.name == tool_name);
                                
                                if tool_exists {
                                    // If tool suggestion is found, add the message with the suggestion
                                    let suggestion = ToolInteraction::Suggestion {
                                        tool_name: tool_name.clone(),
                                        suggested_args: suggested_args.clone(),
                                        message_idx: message_id,
                                    };
                                    
                                    eprintln!("Adding message with tool suggestion for '{}'", tool_name);
                                    
                                    messages.write().push(
                                        Message::new(
                                            MessageRole::Assistant,
                                            message_content
                                        )
                                        .with_tool_interaction(suggestion)
                                    );
                                    
                                    // Cache the tool suggestion with an estimated confidence of 0.9
                                    // In a real implementation, we'd get this from the LLM response
                                    cache_ref.add(&user_input, &tool_name, 0.9, &suggested_args);
                                } else {
                                    // Tool doesn't exist in MCP - add the message with an explanation
                                    eprintln!("Tool '{}' suggested but not available in MCP", tool_name);
                                    
                                    // Add the AI's message first
                                    messages.write().push(Message::new(
                                        MessageRole::Assistant,
                                        message_content
                                    ));
                                    
                                    // Then add a system message explaining why the tool can't be used
                                    messages.write().push(Message::new(
                                        MessageRole::System,
                                        format!("The '{}' tool was mentioned, but it's not available in the current MCP server.", tool_name)
                                    ));
                                }
                            } else {
                                eprintln!("No tool suggestion detected in message");
                                
                                // Regular message, no tool suggestion
                                messages.write().push(Message::new(
                                    MessageRole::Assistant,
                                    message_content,
                                ));
                            }
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
    
    // Modify execute_tool function to use validation pipeline
    let execute_tool = move |(tool_name, arguments): (String, Value)| {
        let message_id = messages.read().len();
        
        // Find the tool definition
        let tool_opt = tools.read().iter().find(|t| t.name == tool_name).cloned();
        
        if let Some(tool) = tool_opt {
            // Create a fresh ValidationPipeline with current tools
            let pipeline = ValidationPipeline::new()
                .with_max_depth(10)
                .with_max_string_length(1000)
                .with_auto_fix(true)
                .with_suggest_alternatives(true)
                .with_max_alternatives(3)
                .with_fallback("count", json!(5))
                .with_fallback("limit", json!(100))
                .with_available_tools(tools.read().clone());
                
            // Validate input with our pipeline
            let validation_result = pipeline.validate_input(&tool, arguments.clone());
            
            match validation_result {
                ValidationState::Valid(validated_args) | ValidationState::Sanitized { sanitized: validated_args, .. } => {
                    // Add a tool execution message
                    messages.write().push(
                        Message::new(
                            MessageRole::Tool,
                            format!("Executing tool: {}", tool_name)
                        ).with_tool_interaction(
                            ToolInteraction::Execution {
                                tool_name: tool_name.clone(),
                                arguments: validated_args.clone(),
                                status: ToolExecutionStatus::Running,
                                result: None,
                                message_idx: message_id,
                            }
                        )
                    );
                    
                    let mcp_state_clone = mcp_state.clone();
                    spawn({
                        to_owned![messages, message_id, tool_name, validated_args];
                        async move {
                            // Execute the tool
                            match ToolManager::execute_tool(tool_name.clone(), validated_args.clone(), &mcp_state_clone.read()).await {
                                Ok(result) => {
                                    // Format the result
                                    let result_text = ToolManager::format_tool_result(&result);
                                    
                                    // Update the message with the result
                                    if message_id < messages.read().len() {
                                        messages.write()[message_id] = Message::new(
                                            MessageRole::Tool,
                                            format!("Tool execution completed: {}", tool_name.clone())
                                        ).with_tool_interaction(
                                            ToolInteraction::Execution {
                                                tool_name: tool_name.clone(),
                                                arguments: validated_args,
                                                status: ToolExecutionStatus::Completed,
                                                result: Some(result_text.clone()),
                                                message_idx: message_id,
                                            }
                                        );
                                        
                                        // Also add the result to the chat history for the AI
                                        messages.write().push(
                                            Message::new(
                                                MessageRole::System,
                                                format!("Tool '{}' returned result:\n\n{}", tool_name, result_text)
                                            )
                                        );
                                    }
                                },
                                Err(e) => {
                                    // Update message with error
                                    if message_id < messages.read().len() {
                                        messages.write()[message_id] = Message::new(
                                            MessageRole::Tool,
                                            format!("Tool execution failed: {}", tool_name)
                                        ).with_tool_interaction(
                                            ToolInteraction::Execution {
                                                tool_name,
                                                arguments: validated_args,
                                                status: ToolExecutionStatus::Failed(format!("{}", e)),
                                                result: None,
                                                message_idx: message_id,
                                            }
                                        );
                                    }
                                }
                            }
                        }
                    });
                },
                ValidationState::Recovered { recovered, strategies, errors, .. } => {
                    // Add message about recovery
                    let recovery_description = strategies.iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>()
                        .join("\n");
                    
                    messages.write().push(
                        Message::new(
                            MessageRole::System,
                            format!("Input validation had issues but was recovered:\n{}\n\nProceeding with fixed input.", recovery_description)
                        )
                    );
                    
                    // Now execute with recovered args
                    messages.write().push(
                        Message::new(
                            MessageRole::Tool,
                            format!("Executing tool: {}", tool_name)
                        ).with_tool_interaction(
                            ToolInteraction::Execution {
                                tool_name: tool_name.clone(),
                                arguments: recovered.clone(),
                                status: ToolExecutionStatus::Running,
                                result: None,
                                message_idx: message_id + 1, // +1 because we added a message
                            }
                        )
                    );
                    
                    let message_idx = message_id + 1;
                    let mcp_state_clone = mcp_state.clone();
                    spawn({
                        to_owned![messages, message_idx, tool_name, recovered];
                        async move {
                            // Execute the tool
                            match ToolManager::execute_tool(tool_name.clone(), recovered.clone(), &mcp_state_clone.read()).await {
                                Ok(result) => {
                                    // Format the result
                                    let result_text = ToolManager::format_tool_result(&result);
                                    
                                    // Update the message with the result
                                    if message_idx < messages.read().len() {
                                        messages.write()[message_idx] = Message::new(
                                            MessageRole::Tool,
                                            format!("Tool execution completed: {}", tool_name.clone())
                                        ).with_tool_interaction(
                                            ToolInteraction::Execution {
                                                tool_name: tool_name.clone(),
                                                arguments: recovered,
                                                status: ToolExecutionStatus::Completed,
                                                result: Some(result_text.clone()),
                                                message_idx,
                                            }
                                        );
                                        
                                        // Also add the result to the chat history for the AI
                                        messages.write().push(
                                            Message::new(
                                                MessageRole::System,
                                                format!("Tool '{}' returned result:\n\n{}", tool_name, result_text)
                                            )
                                        );
                                    }
                                },
                                Err(e) => {
                                    // Update message with error
                                    if message_idx < messages.read().len() {
                                        messages.write()[message_idx] = Message::new(
                                            MessageRole::Tool,
                                            format!("Tool execution failed: {}", tool_name)
                                        ).with_tool_interaction(
                                            ToolInteraction::Execution {
                                                tool_name,
                                                arguments: recovered,
                                                status: ToolExecutionStatus::Failed(format!("{}", e)),
                                                result: None,
                                                message_idx,
                                            }
                                        );
                                    }
                                }
                            }
                        }
                    });
                },
                ValidationState::Invalid { errors, alternative_tools, .. } => {
                    // Add message about validation failure
                    let error_description = errors.join("\n");
                    
                    let mut message = format!(
                        "Tool validation failed for '{}':\n{}", 
                        tool_name, 
                        error_description
                    );
                    
                    // Add alternative tools if available
                    if !alternative_tools.is_empty() {
                        message.push_str("\n\nAlternative tools you might want to use instead:\n");
                        
                        // For each alternative tool, create options that the user can click
                        let alt_tool_descriptions: Vec<String> = alternative_tools.iter()
                            .filter_map(|alt_tool| {
                                tools.read().iter().find(|t| t.name == *alt_tool).map(|tool| {
                                    format!("- {} ({})", tool.name, tool.description)
                                })
                            })
                            .collect();
                        
                        // Add the descriptions to the message
                        message.push_str(&alt_tool_descriptions.join("\n"));
                        
                        // Create clickable options for users
                        for alt_tool in &alternative_tools {
                            if let Some(tool) = tools.read().iter().find(|t| t.name == *alt_tool) {
                                let suggest_message = format!("Try using {} instead", tool.name);
                                // Get the message index before the mutable borrow
                                let message_idx = messages.read().len();
                                messages.write().push(
                                    Message::new(
                                        MessageRole::System,
                                        suggest_message
                                    ).with_tool_interaction(
                                        ToolInteraction::Suggestion {
                                            tool_name: alt_tool.clone(),
                                            suggested_args: json!({}),  // Empty args to start
                                            message_idx,
                                        }
                                    )
                                );
                            }
                        }
                    }
                    
                    messages.write().push(
                        Message::new(
                            MessageRole::System,
                            message
                        )
                    );
                }
            }
        } else {
            // Tool not found
            messages.write().push(
                Message::new(
                    MessageRole::System,
                    format!("Tool '{}' not found in available tools.", tool_name)
                )
            );
        }
    };
    
    // Function to handle tool cancellation
    let cancel_tool = move |message_idx: usize| {
        if message_idx < messages.read().len() {
            // First read the information we need
            let tool_name_opt = messages.read().get(message_idx)
                .and_then(|msg| {
                    if let Some(ToolInteraction::Suggestion { tool_name, .. }) = &msg.tool_interaction {
                        Some(tool_name.clone())
                    } else {
                        None
                    }
                });
            
            // Now we can modify messages if we found a tool name
            if let Some(tool_name) = tool_name_opt {
                // Add a message indicating the tool was rejected
                messages.write().push(
                    Message::new(
                        MessageRole::System,
                        format!("Tool usage rejected: {}", tool_name)
                    )
                );
                
                // Remove the tool suggestion from the message
                if let Some(msg) = messages.write().get_mut(message_idx) {
                    msg.tool_interaction = None;
                }
            }
        }
    };
    
    // Handle Enter key
    let mut send_message_ref = send_message.clone();
    let handle_keydown = move |evt: KeyboardEvent| {
        if evt.key().to_string() == "Enter" && !evt.modifiers().shift() {
            evt.prevent_default();
            send_message_ref(());
        }
    };
    
    // Clone mcp_tools again for the UI to avoid move errors
    let tools_for_ui = tools.read().clone();

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
                
                // Add confidence threshold slider
                div { class: "confidence-controls",
                    label { for: "confidence-threshold", "Tool Confidence Threshold:" }
                    input {
                        id: "confidence-threshold",
                        class: "confidence-slider",
                        r#type: "range",
                        min: "0.1",
                        max: "1.0",
                        step: "0.1",
                        value: "{confidence_threshold}",
                        oninput: move |evt| {
                            if let Ok(val) = evt.value().parse::<f64>() {
                                confidence_threshold.set(val);
                            }
                        }
                    }
                    span { class: "confidence-value", "{confidence_threshold}" }
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
                    onchange: move |evt| {
                        let new_model = evt.value().clone();
                        model_selection.write().selected_model = new_model.clone();
                        
                        // Update the tool_selector with the new model
                        let api_key = match &api_key_ref {
                            Some(key) => key.clone(),
                            None => env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                        };
                        
                        // Create a new selector with the updated model
                        let new_selector = LLMToolSelector::new(api_key, new_model.clone())
                            .with_cache(cache.read().clone())
                            .with_max_prompt_tools(25); // Limit to 25 tools per prompt
                            
                        eprintln!("Updated LLMToolSelector to use model: {}", new_model);
                        tool_selector.set(new_selector);
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
                        MessageView { 
                            message: message.clone(),
                            tools: tools_for_ui.clone(),
                            on_tool_execute: execute_tool,
                            on_tool_cancel: cancel_tool,
                        }
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
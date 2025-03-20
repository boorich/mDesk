# mDesk Development ToDo List

## Overview
This document outlines the next steps for developing mDesk, a native desktop application for managing MCP tools with OpenRouter LLM access. The goal is to create a productive, focused day of development with clear tasks and milestones.

## High Priority Tasks

### 1. Dioxus AI Development Tools Integration
- [ ] Set up and use [dioxus-ai](https://github.com/DioxusLabs/dioxus-ai) developer tools to improve our workflow
  - [ ] Component Generation Tool
    - [ ] Use the tool to generate complex UI components for the chat interface
    - [ ] Generate reusable components for resource and tool cards
  - [ ] Automated QA
    - [ ] Set up automated testing for critical application paths
    - [ ] Create test scenarios for MCP server operations

### 2. Chat Interface & OpenRouter Integration
- [ ] Create a new tab in the UI for a Chat Interface
  - [ ] Design and implement a modern chat window UI
  - [ ] Add message history component with proper styling
  - [ ] Create input area with send button and keyboard shortcuts
- [ ] Implement OpenRouter API integration
  - [ ] Create API client for OpenRouter
  - [ ] Add model selection dropdown with available LLMs
  - [ ] Implement token tracking and usage monitoring
- [ ] Develop tool selection algorithm
  - [ ] Create logic to analyze user queries
  - [ ] Implement tool selection based on query intent
  - [ ] Add capability to chain multiple tools as needed

### 3. MCP Integration Improvements
- [ ] Enhance error handling for MCP client connections
- [ ] Implement proper loading states during MCP operations
- [ ] Add persistent storage for MCP connection settings
- [ ] Create a configuration panel for customizing MCP server settings

### 4. UI & UX Enhancements
- [ ] Add animations for state transitions
- [ ] Implement dark/light theme toggle
- [ ] Create a collapsible sidebar
- [ ] Add tooltips for better user guidance
- [ ] Implement keyboard shortcuts for common actions

## Medium Priority Tasks

### 5. Resource Management
- [ ] Implement resource creation/modification functionality
- [ ] Add filtering and searching capabilities for resources
- [ ] Create detailed resource view with additional metadata
- [ ] Implement resource export/import functionality

### 6. Tools Enhancement
- [ ] Create a detailed view for each tool
- [ ] Implement tool execution UI with parameter inputs
- [ ] Add result visualization for tool outputs
- [ ] Create tool favorites or recently used section

### 7. Performance Optimization
- [ ] Profile the application for performance bottlenecks
- [ ] Implement lazy loading for resources and tools lists
- [ ] Optimize SVG renders and DOM updates
- [ ] Add request caching for frequently accessed data

## Dioxus AI Development Tools Usage

### Component Generation
The Dioxus AI component generation tool can help us create complex UI components quickly.

1. Navigate to the component-generation directory:
   ```bash
   cd /Users/martinmaurer/Projects/dioxus-ai/component-generation
   ```

2. Run the tool with your component specification:
   ```bash
   cargo run -- "Create a chat message component with user and assistant variants that includes an avatar, message text, and timestamp"
   ```

3. Example use cases for mDesk:
   - Generate message bubbles for the chat interface
   - Create resource card components with complex layouts
   - Build tool parameter input forms

### Automated QA
The automated QA tools can help test the application functionality:

1. Navigate to the automated-qa directory:
   ```bash
   cd /Users/martinmaurer/Projects/dioxus-ai/automated-qa
   ```

2. Set up test scenarios:
   ```rust
   // Example test scenario
   "Test that the server connection succeeds and resources load correctly"
   ```

3. Focus areas for testing:
   - MCP server connection flow
   - Resource and tool listing
   - Chat interface interaction
   - Tool execution flow

## OpenRouter Integration Implementation

### 1. API Client
Create a Rust module for OpenRouter API communication:

```rust
// src/openrouter/mod.rs
use serde::{Deserialize, Serialize};
use reqwest::Client;

pub struct OpenRouterClient {
    api_key: String,
    client: Client,
    base_url: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    role: String,
    content: String,
}

impl OpenRouterClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
        }
    }
    
    pub async fn chat_completion(&self, model: &str, messages: Vec<ChatMessage>, stream: bool) -> Result<String, anyhow::Error> {
        // Implementation details...
    }
    
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error> {
        // Implementation details...
    }
}
```

### 2. Chat Interface Component
Create a new tab and chat interface:

```rust
#[component]
fn ChatTab() -> Element {
    let mut messages = use_signal(|| Vec::<Message>::new());
    let mut input = use_signal(String::new);
    let mut selected_model = use_signal(|| "anthropic/claude-3-opus".to_string());
    let mut is_loading = use_signal(|| false);
    
    // OpenRouter client setup
    let client = use_memo(|| {
        OpenRouterClient::new("YOUR_API_KEY".to_string())
    });
    
    // Handle message submission
    let send_message = move |_| {
        let user_input = input.get().trim().to_string();
        if user_input.is_empty() || is_loading.get() {
            return;
        }
        
        // Add user message
        messages.write().push(Message::User(user_input.clone()));
        input.set("".to_string());
        is_loading.set(true);
        
        // Show thinking indicator
        messages.write().push(Message::Assistant("...".to_string()));
        
        // Prepare MCP context for tools
        let mcp_tools = prepare_available_tools();
        
        spawn({
            to_owned![messages, client, selected_model, user_input, is_loading, mcp_tools];
            async move {
                // First determine if we need to use tools
                let tool_selector = ToolSelector::new();
                let selected_tools = tool_selector.select_tools(&user_input, mcp_tools).await;
                
                // Process with selected tools if needed
                let mut context = format!("User query: {}", user_input);
                if !selected_tools.is_empty() {
                    let tool_results = execute_tools(selected_tools).await;
                    context = format!("{}\n\nTool results: {}", context, tool_results);
                }
                
                // Send to LLM with context
                let chat_messages = vec![
                    ChatMessage { role: "system".to_string(), content: "You are a helpful assistant...".to_string() },
                    ChatMessage { role: "user".to_string(), content: context },
                ];
                
                let response = client.chat_completion(
                    &selected_model, 
                    chat_messages,
                    false
                ).await;
                
                // Update the assistant message with the response
                if let Ok(content) = response {
                    let idx = messages.read().len() - 1;
                    messages.write()[idx] = Message::Assistant(content);
                } else {
                    let idx = messages.read().len() - 1;
                    messages.write()[idx] = Message::Assistant("Sorry, I encountered an error processing your request.".to_string());
                }
                
                is_loading.set(false);
            }
        });
    };
    
    rsx! {
        div { class: "chat-tab",
            // Model selector
            div { class: "model-selector",
                label { "Model:" }
                select { 
                    oninput: move |evt| selected_model.set(evt.value.clone()),
                    option { value: "anthropic/claude-3-opus", "Claude 3 Opus" }
                    option { value: "anthropic/claude-3-sonnet", "Claude 3 Sonnet" }
                    option { value: "openai/gpt-4o", "GPT-4o" }
                    // Add more models...
                }
            }
            
            // Messages area
            div { class: "chat-messages",
                for (idx, msg) in messages.read().iter().enumerate() {
                    key: "{idx}",
                    match msg {
                        Message::User(content) => rsx! {
                            div { class: "message user-message",
                                div { class: "avatar user-avatar" }
                                div { class: "content",
                                    div { class: "sender", "You" }
                                    div { class: "message-text", "{content}" }
                                }
                            }
                        },
                        Message::Assistant(content) => rsx! {
                            div { class: "message assistant-message",
                                div { class: "avatar assistant-avatar" }
                                div { class: "content",
                                    div { class: "sender", "Assistant" }
                                    div { class: "message-text", 
                                        if content == "..." {
                                            rsx! { div { class: "typing-indicator", "..." } }
                                        } else {
                                            rsx! { "{content}" }
                                        }
                                    }
                                }
                            }
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
                    oninput: move |evt| input.set(evt.value.clone()),
                    onkeydown: move |evt| {
                        if evt.key() == "Enter" && !evt.shift_key() {
                            evt.prevent_default();
                            send_message(());
                        }
                    },
                    disabled: is_loading
                }
                button {
                    class: "send-button",
                    onclick: send_message,
                    disabled: is_loading || input.get().trim().is_empty(),
                    if is_loading {
                        "Sending..."
                    } else {
                        "Send"
                    }
                }
            }
        }
    }
}
```

### 3. Tool Selection Algorithm
Create logic to select appropriate MCP tools based on user queries:

```rust
struct ToolSelector {
    // Configuration, possibly embedding model, etc.
}

impl ToolSelector {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn select_tools(&self, query: &str, available_tools: Vec<Tool>) -> Vec<Tool> {
        // Simple keyword matching approach for initial implementation
        let query_lower = query.to_lowercase();
        let mut selected_tools = Vec::new();
        
        for tool in available_tools {
            // Check tool name and description for relevance
            if tool.name.to_lowercase().contains(&query_lower) || 
               tool.description.to_lowercase().contains(&query_lower) {
                selected_tools.push(tool);
            }
            
            // More sophisticated matching could include:
            // - Embedding similarity
            // - Intent classification
            // - Parameter matching
        }
        
        // Advanced version would leverage an LLM to analyze the query and select tools
        // that would be most appropriate for solving the user's request
        
        selected_tools
    }
}

async fn execute_tools(tools: Vec<Tool>) -> String {
    // Logic to execute selected tools and format their results
    // This would interact with the MCP client
    
    // For now, just return a placeholder
    format!("Executed {} tools", tools.len())
}
```

## Testing Milestones

### Morning Milestone
- [ ] Set up Dioxus AI tools and generate at least one complex component
- [ ] Create the basic structure for the Chat tab
- [ ] Begin implementing the OpenRouter API client

### Afternoon Milestone
- [ ] Complete the OpenRouter integration with model selection
- [ ] Implement the chat interface with proper styling
- [ ] Create the initial version of the tool selection algorithm

### End-of-Day Milestone
- [ ] Have a working chat interface that can communicate with OpenRouter
- [ ] Demonstrate basic tool selection and execution
- [ ] Document the implementation and next steps

## Resources
- [OpenRouter API Documentation](https://openrouter.ai/docs)
- [Dioxus Documentation](https://dioxuslabs.com/learn/0.6/)
- [MCP Protocol Documentation](https://github.com/microsoft/mcp-protocol)
- [Rust reqwest Library](https://docs.rs/reqwest/latest/reqwest/)

## Additional Notes
- Use environment variables for API keys rather than hardcoding them
- Consider implementing a streaming response mode for a better user experience
- Focus on modular development to make components reusable and testable
- Keep performance in mind, especially when processing large responses

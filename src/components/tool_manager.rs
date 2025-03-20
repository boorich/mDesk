use dioxus::prelude::*;
use mcp_client::{McpClientTrait, Error as McpError};
use mcp_core::{Tool, protocol::CallToolResult, content::Content};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;
use regex::Regex;
use crate::components::tool_suggestion::{ToolSuggestionProps, ToolExecutionProps, ToolExecutionStatus};
use crate::McpState;

/// Types of tool interactions detected in messages
#[derive(Debug, Clone, PartialEq)]
pub enum ToolInteraction {
    /// AI suggests using a tool
    Suggestion { 
        tool_name: String, 
        suggested_args: Value,
        message_idx: usize,
    },
    /// Tool has been executed
    Execution {
        tool_name: String,
        arguments: Value,
        status: ToolExecutionStatus,
        result: Option<String>,
        message_idx: usize,
    },
}

/// Component for managing tool interactions
pub struct ToolManager;

impl ToolManager {
    /// Detect potential tool operations in a message
    pub fn detect_tool_suggestion(message: &str, available_tools: &[Tool]) -> Option<(String, Value)> {
        eprintln!("Checking for tool suggestions in message: {}", message);
        
        // Look for tool suggestions in the message
        // More flexible pattern to catch various ways the model might express tool usage
        let tool_regex = Regex::new(r"(?:I need to use|I want to use|Let me use|I'll use|Using) the (?P<tool_name>[a-zA-Z0-9_]+) tool").ok()?;
        
        if let Some(captures) = tool_regex.captures(message) {
            eprintln!("Regex matched! Extracting tool name");
            let tool_name_opt = captures.name("tool_name");
            
            if let Some(tool_name_match) = tool_name_opt {
                let tool_name = tool_name_match.as_str().to_string();
                eprintln!("Extracted tool name: {}", tool_name);
                
                // Verify the tool exists
                if !available_tools.iter().any(|t| t.name == tool_name) {
                    eprintln!("Tool '{}' not found in available tools", tool_name);
                    return None;
                }
                
                eprintln!("Tool '{}' is valid", tool_name);
            } else {
                eprintln!("Failed to extract tool name from regex match");
                return None;
            }
            
            // Extract the tool_name for use below
            let tool_name = captures.name("tool_name").unwrap().as_str().to_string();
            
            // Look for JSON arguments
            let args_regex = Regex::new(r"\{[\s\S]*?\}").ok()?;
            eprintln!("Looking for JSON arguments");
            
            if let Some(args_match) = args_regex.find(message) {
                let args_str = args_match.as_str();
                eprintln!("Found potential JSON arguments: {}", args_str);
                
                // Try to parse as JSON
                match serde_json::from_str::<Value>(args_str) {
                    Ok(args) => {
                        eprintln!("Successfully parsed JSON arguments");
                        return Some((tool_name, args));
                    },
                    Err(e) => {
                        eprintln!("Failed to parse JSON arguments: {}", e);
                    }
                }
            } else {
                eprintln!("No JSON arguments found in message");
            }
            
            // If no valid JSON arguments found, return empty object
            eprintln!("Using empty JSON object as arguments");
            return Some((tool_name, json!({})));
        }
        
        None
    }
    
    /// Execute a tool with the given name and arguments
    pub async fn execute_tool(
        tool_name: String,
        arguments: Value,
        mcp_state: &McpState,
    ) -> Result<CallToolResult, McpError> {
        let client = mcp_state.client.as_ref()
            .ok_or(McpError::NotInitialized)?;
        
        let client = client.lock().await;
        client.call_tool(&tool_name, arguments).await
    }
    
    /// Process a tool result into readable text
    pub fn format_tool_result(result: &CallToolResult) -> String {
        let mut output = String::new();
        
        for content in &result.content {
            match content {
                Content::Text(text_content) => {
                    output.push_str(&text_content.text);
                    output.push('\n');
                },
                Content::Image(image_content) => {
                    output.push_str(&format!("[Image: {} ({})]", 
                        if image_content.data.len() > 20 {
                            format!("{}...", &image_content.data[..20])
                        } else {
                            image_content.data.clone()
                        },
                        image_content.mime_type
                    ));
                    output.push('\n');
                },
                Content::Resource(resource) => {
                    output.push_str(&resource.get_text());
                    output.push('\n');
                }
            }
        }
        
        output
    }
    
    /// Find a tool by name from a list of available tools
    pub fn find_tool_by_name<'a>(name: &str, tools: &'a [Tool]) -> Option<&'a Tool> {
        tools.iter().find(|t| t.name == name)
    }
} 
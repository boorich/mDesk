use dioxus::prelude::*;
use mcp_client::{McpClientTrait, Error as McpError};
use mcp_core::{Tool, protocol::CallToolResult, content::Content};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;
use regex::Regex;
use tracing::{debug, info, warn, error, trace};
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
        debug!("Checking for tool suggestions in message: {}", message);
        
        // Look for tool suggestions in the message
        // Log available tools with full details
        debug!("Available tools count: {}", available_tools.len());
        for tool in available_tools {
            trace!("  Tool in detect_tool_suggestion: {} ({})", tool.name, tool.description);
        }
        
        // Extract potential tool names directly - much more flexible approach
        let available_tool_names: Vec<&str> = available_tools.iter().map(|t| t.name.as_str()).collect();
        trace!("Available tool names: {:?}", available_tool_names);
        
        // NEW APPROACH: Look for ANY tool mention with a more generic regex
        let generic_tool_regex = Regex::new(r"([a-zA-Z0-9_]+)\s+tool").ok()?;
        
        debug!("Looking for ANY tool mention with generic pattern");
        
        for cap in generic_tool_regex.captures_iter(message) {
            let potential_tool = cap[1].to_string();
            debug!("Found potential tool mention: {}", potential_tool);
            
            // If available_tools is empty, we'll consider ANY tool valid for testing
            if available_tools.is_empty() || available_tool_names.contains(&potential_tool.as_str()) {
                info!("Accepting potential tool: {}", potential_tool);
                
                // Check if there are parameters mentioned
                let args_regex = Regex::new(r"\{[\s\S]*?\}").ok()?;
                if let Some(args_match) = args_regex.find(message) {
                    let args_str = args_match.as_str();
                    debug!("Found JSON parameters for {}: {}", potential_tool, args_str);
                    
                    match serde_json::from_str::<Value>(args_str) {
                        Ok(args) => return Some((potential_tool, args)),
                        Err(e) => warn!("Failed to parse JSON: {}", e)
                    }
                }
                
                // Return with empty parameters
                debug!("No parameters for {} tool, using empty object", potential_tool);
                return Some((potential_tool, json!({})));
            }
        }
        
        // Also try the original exact pattern approach
        let tool_regex = Regex::new(r"I need to use the (?P<tool_name>[a-zA-Z0-9_]+) tool").ok()?;
        if let Some(captures) = tool_regex.captures(message) {
            debug!("Regex matched! Extracting tool name");
            let tool_name_opt = captures.name("tool_name");
            
            if let Some(tool_name_match) = tool_name_opt {
                let tool_name = tool_name_match.as_str().to_string();
                debug!("Extracted tool name: {}", tool_name);
                
                // Verify the tool exists
                if !available_tools.iter().any(|t| t.name == tool_name) {
                    warn!("Tool '{}' not found in available tools", tool_name);
                    return None;
                }
                
                debug!("Tool '{}' is valid", tool_name);
            } else {
                warn!("Failed to extract tool name from regex match");
                return None;
            }
            
            // Extract the tool_name for use below
            let tool_name = captures.name("tool_name").unwrap().as_str().to_string();
            
            // Look for JSON arguments
            let args_regex = Regex::new(r"\{[\s\S]*?\}").ok()?;
            debug!("Looking for JSON arguments");
            
            if let Some(args_match) = args_regex.find(message) {
                let args_str = args_match.as_str();
                debug!("Found potential JSON arguments: {}", args_str);
                
                // Try to parse as JSON
                match serde_json::from_str::<Value>(args_str) {
                    Ok(args) => {
                        info!("Successfully parsed JSON arguments");
                        return Some((tool_name, args));
                    },
                    Err(e) => {
                        warn!("Failed to parse JSON arguments: {}", e);
                    }
                }
            } else {
                debug!("No JSON arguments found in message");
            }
            
            // If no valid JSON arguments found, return empty object
            debug!("Using empty JSON object as arguments");
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
        info!("Executing tool: {} with arguments: {}", tool_name, arguments);
        
        let client = mcp_state.client.as_ref()
            .ok_or_else(|| {
                error!("MCP client not initialized");
                McpError::NotInitialized
            })?;
        
        debug!("Got MCP client, acquiring lock");
        let client = client.lock().await;
        debug!("Lock acquired, calling tool");
        
        match client.call_tool(&tool_name, arguments).await {
            Ok(result) => {
                info!("Tool execution successful: {}", tool_name);
                Ok(result)
            }
            Err(e) => {
                error!("Tool execution failed: {} - Error: {}", tool_name, e);
                Err(e)
            }
        }
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
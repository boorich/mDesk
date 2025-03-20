// Helper function to determine if a tool is safe for auto-execution
// Add this inside the ChatTab component before other functions
fn is_safe_auto_executable_tool(tool_name: &str) -> bool {
    // Only auto-execute read-only tools that don't modify the file system
    match tool_name {
        "list_allowed_directories" | "list_directory" | "get_file_info" | 
        "search_files" | "directory_tree" | "read_multiple_files" => true,
        _ => false, // All other tools require manual confirmation
    }
}

// Modify this section in the send_message function where tool suggestions are added:
if tool_exists {
    // If tool suggestion is found, add the message with the suggestion
    let suggestion = ToolInteraction::Suggestion {
        tool_name: tool_name.clone(),
        suggested_args: suggested_args.clone(),
        message_idx: message_id,
    };
    
    // Add the message with tool suggestion
    eprintln!("Adding message with tool suggestion for '{}'", tool_name);
    messages.write().push(
        Message::new(MessageRole::Assistant, message_content)
            .with_tool_interaction(suggestion)
    );
    
    // For certain safe tools, auto-execute them
    if is_safe_auto_executable_tool(&tool_name) {
        eprintln!("Auto-executing safe tool: {}", tool_name);
        // Execute the tool automatically after a short delay
        let clone_tool_name = tool_name.clone();
        let clone_args = suggested_args.clone();
        spawn({
            to_owned![execute_tool];
            async move {
                // Small delay to show the UI first
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                execute_tool((clone_tool_name, clone_args));
            }
        });
    }
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
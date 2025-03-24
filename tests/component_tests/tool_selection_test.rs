#[cfg(test)]
mod tests {
    use mcp_core::Tool;
    use serde_json::json;
    use m_desk_new::components::tool_selection::{RankedToolSelection, ToolMatch, LLMToolSelector};
    use std::env;

    #[test]
    fn test_ranked_tool_selection_basic() {
        // Create a test tool
        let test_tool = Tool::new(
            "test_tool".to_string(),
            "A test tool".to_string(),
            json!({"type": "object"})
        );

        // Create a test match
        let tool_match = ToolMatch {
            tool: test_tool,
            confidence: 0.8,
            suggested_parameters: json!({}),
            specific_reasoning: "Test reasoning".to_string(),
        };

        // Create a selection with one match
        let selection = RankedToolSelection {
            selections: vec![tool_match],
            reasoning: "Test overall reasoning".to_string(),
        };

        // Test best_match
        assert!(selection.best_match(0.7).is_some());
        assert!(selection.best_match(0.9).is_none());

        // Test viable_matches
        assert_eq!(selection.viable_matches(0.7).len(), 1);
        assert_eq!(selection.viable_matches(0.9).len(), 0);
    }

    /// Tests the LLM-based tool selection with real API calls.
    /// 
    /// Note on model selection:
    /// - Default is claude-3-opus for highest accuracy in tool selection
    /// - For cost optimization, consider using:
    ///   - claude-3-sonnet: Good balance of performance/cost
    ///   - claude-2: Lower cost, still good performance
    ///   - gpt-3.5-turbo: Lowest cost, may need prompt tuning
    /// 
    /// Set MODEL_NAME env var to override the default model.
    #[tokio::test]
    async fn test_llm_tool_selection() {
        // Skip if no API key
        let api_key = match env::var("OPENROUTER_API_KEY") {
            Ok(key) => key,
            Err(_) => return, // Skip test if no API key
        };

        // Allow model override through environment variable
        let model = env::var("MODEL_NAME")
            .unwrap_or_else(|_| "anthropic/claude-3-opus".to_string());

        // Create test tools
        let tools = vec![
            Tool::new(
                "file_reader".to_string(),
                "Reads contents of a file".to_string(),
                json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    }
                })
            ),
            Tool::new(
                "web_search".to_string(),
                "Searches the web for information".to_string(),
                json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    }
                })
            ),
        ];

        let selector = LLMToolSelector::new(
            api_key,
            model,
        );

        // Test tool selection
        let result = selector.select_tools(
            "I need to search the web for information about Rust programming",
            tools
        ).await;

        assert!(result.is_ok());
        
        let selection = result.unwrap();
        assert!(!selection.selections.is_empty());
        
        // The web_search tool should be selected with high confidence
        if let Some(best_match) = selection.best_match(0.7) {
            assert_eq!(best_match.tool.name, "web_search");
            assert!(best_match.confidence > 0.7);
        } else {
            panic!("Expected web_search tool to be selected with high confidence");
        }
    }
} 
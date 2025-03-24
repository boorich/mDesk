#[cfg(test)]
mod tests {
    use mcp_core::Tool;
    use serde_json::json;
    use m_desk_new::components::tool_selection::{RankedToolSelection, ToolMatch, LLMToolSelector, ValidationStatus};
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
            suggested_parameters: Some(json!({})),
            reasoning: "Test reasoning".to_string(),
            validation_status: ValidationStatus::Valid,
        };

        // Create a selection with one match
        let selection = RankedToolSelection::new(vec![tool_match]);

        // Test best_match
        let best = selection.best_match();
        assert!(best.is_some());
        assert_eq!(best.unwrap().confidence, 0.8);

        // Test viable_matches
        assert_eq!(selection.viable_matches(0.7).len(), 1);
        assert_eq!(selection.viable_matches(0.9).len(), 0);

        // Test valid_matches
        assert_eq!(selection.valid_matches(0.7).len(), 1);
        assert_eq!(selection.valid_matches(0.9).len(), 0);
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
                    },
                    "required": ["path"]
                })
            ),
            Tool::new(
                "web_search".to_string(),
                "Searches the web for information".to_string(),
                json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
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
        let matches = selection.viable_matches(0.7);
        assert!(!matches.is_empty());
        
        // The web_search tool should be selected with high confidence
        if let Some(best_match) = selection.best_match() {
            assert_eq!(best_match.tool.name, "web_search");
            assert!(best_match.confidence > 0.7);
        } else {
            panic!("Expected web_search tool to be selected with high confidence");
        }
    }
} 
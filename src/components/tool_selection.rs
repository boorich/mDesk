use mcp_core::Tool;
use serde_json::Value;
use crate::openrouter::{OpenRouterClient, ChatMessage};
use anyhow::Result;

/// Represents a ranked match of a tool for a given user intent
#[derive(Debug, Clone)]
pub struct ToolMatch {
    pub tool: Tool,
    pub confidence: f32,
    pub suggested_parameters: Value,
    pub specific_reasoning: String,
}

/// Collection of ranked tool matches with overall reasoning
#[derive(Debug, Clone)]
pub struct RankedToolSelection {
    pub selections: Vec<ToolMatch>,
    pub reasoning: String,
}

impl RankedToolSelection {
    /// Returns the best match if it meets a minimum confidence threshold
    pub fn best_match(&self, min_confidence: f32) -> Option<&ToolMatch> {
        self.selections.first()
            .filter(|match_| match_.confidence >= min_confidence)
    }

    /// Returns all matches above a certain confidence threshold
    pub fn viable_matches(&self, min_confidence: f32) -> Vec<&ToolMatch> {
        self.selections.iter()
            .filter(|match_| match_.confidence >= min_confidence)
            .collect()
    }
}

/// LLM-based tool selector that ranks tools based on user intent
pub struct LLMToolSelector {
    client: OpenRouterClient,
    model: String,
}

impl LLMToolSelector {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: OpenRouterClient::new(api_key),
            model,
        }
    }

    /// Creates a system prompt for tool selection
    fn create_system_prompt(tools: &[Tool]) -> String {
        let tools_json = serde_json::to_string_pretty(tools).unwrap_or_default();
        format!(
            r#"You are a tool selection expert. Your task is to analyze the available tools and user's intent to select the most appropriate tools.

Available Tools:
{}

Analyze the user's intent and select up to 5 most relevant tools. For each tool provide:
1. Tool name
2. Confidence score (0-1)
3. Reasoning for selection
4. Suggested parameters based on the tool's schema

Format your response as JSON:
{{
    "selections": [
        {{
            "tool_name": "string",
            "confidence": 0.95,
            "reasoning": "string",
            "parameters": {{}}
        }}
    ],
    "overall_reasoning": "string"
}}
"#,
            tools_json
        )
    }

    /// Selects appropriate tools based on user intent
    pub async fn select_tools(&self, intent: &str, available_tools: Vec<Tool>) -> Result<RankedToolSelection> {
        let system_prompt = Self::create_system_prompt(&available_tools);
        
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            ChatMessage {
                role: "user".to_string(),
                content: intent.to_string(),
            },
        ];

        // Get LLM response
        let response = self.client.chat_completion(
            &self.model,
            messages,
            Some(0.7), // temperature - balance between creativity and precision
            Some(1000), // max tokens
        ).await?;

        // Parse LLM response into tool selections
        if let Some(choice) = response.choices.first() {
            let content = &choice.message.content;
            
            // Parse the JSON response
            let parsed: serde_json::Value = serde_json::from_str(content)?;
            
            // Convert to RankedToolSelection
            let mut selections = Vec::new();
            
            if let Some(tools_array) = parsed["selections"].as_array() {
                for tool_selection in tools_array {
                    let tool_name = tool_selection["tool_name"].as_str().unwrap_or_default();
                    
                    // Find the actual tool from available_tools
                    if let Some(tool) = available_tools.iter().find(|t| t.name == tool_name) {
                        selections.push(ToolMatch {
                            tool: tool.clone(),
                            confidence: tool_selection["confidence"].as_f64().unwrap_or(0.0) as f32,
                            suggested_parameters: tool_selection["parameters"].clone(),
                            specific_reasoning: tool_selection["reasoning"].as_str()
                                .unwrap_or_default()
                                .to_string(),
                        });
                    }
                }
            }

            Ok(RankedToolSelection {
                selections,
                reasoning: parsed["overall_reasoning"].as_str()
                    .unwrap_or_default()
                    .to_string(),
            })
        } else {
            anyhow::bail!("No response from LLM")
        }
    }
} 
use mcp_core::Tool;
use serde_json::Value;
use crate::openrouter::{OpenRouterClient, ChatMessage};
use crate::components::parameter_validation::ParameterValidator;
use anyhow::{Result, anyhow};
use tracing::{debug, error, info, warn, instrument};

#[derive(Debug, Clone)]
pub enum ValidationStatus {
    Valid,
    Fixed {
        original: Value,
        fixed: Value,
    },
    Failed {
        error: String,
    },
}

impl std::fmt::Display for ValidationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationStatus::Valid => write!(f, "valid"),
            ValidationStatus::Fixed { .. } => write!(f, "fixed"),
            ValidationStatus::Failed { error } => write!(f, "failed: {}", error),
        }
    }
}

/// Represents a ranked match of a tool for a given user intent
#[derive(Debug, Clone)]
pub struct ToolMatch {
    pub tool: Tool,
    pub confidence: f64,
    pub suggested_parameters: Option<Value>,
    pub reasoning: String,
    pub validation_status: ValidationStatus,
}

impl ToolMatch {
    pub fn is_valid(&self) -> bool {
        matches!(self.validation_status, ValidationStatus::Valid | ValidationStatus::Fixed { .. })
    }

    pub fn validation_error(&self) -> Option<&str> {
        if let ValidationStatus::Failed { error } = &self.validation_status {
            Some(error)
        } else {
            None
        }
    }
}

/// Collection of ranked tool matches with overall reasoning
#[derive(Debug, Clone)]
pub struct RankedToolSelection {
    matches: Vec<ToolMatch>,
}

impl RankedToolSelection {
    pub fn new(matches: Vec<ToolMatch>) -> Self {
        Self { matches }
    }

    /// Returns the best match if it meets a minimum confidence threshold
    pub fn best_match(&self) -> Option<&ToolMatch> {
        self.matches.iter().max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
    }

    /// Returns all matches above a certain confidence threshold
    pub fn viable_matches(&self, confidence_threshold: f64) -> Vec<&ToolMatch> {
        self.matches.iter()
            .filter(|m| m.confidence >= confidence_threshold)
            .collect()
    }

    pub fn valid_matches(&self, confidence_threshold: f64) -> Vec<&ToolMatch> {
        self.matches.iter()
            .filter(|m| {
                m.confidence >= confidence_threshold && m.is_valid()
            })
            .collect()
    }

    pub fn validation_summary(&self) -> String {
        let total = self.matches.len();
        let valid = self.matches.iter().filter(|m| m.is_valid()).count();
        let fixed = self.matches.iter()
            .filter(|m| matches!(m.validation_status, ValidationStatus::Fixed { .. }))
            .count();
        let failed = total - valid - fixed;

        format!(
            "Tool matches: {} total, {} valid, {} fixed, {} failed",
            total, valid, fixed, failed
        )
    }
}

/// LLM-based tool selector that ranks tools based on user intent
pub struct LLMToolSelector {
    client: OpenRouterClient,
    model: String,
}

impl LLMToolSelector {
    pub fn new(api_key: String, model: String) -> Self {
        info!("Creating new LLMToolSelector with model: {}", model);
        Self {
            client: OpenRouterClient::new(api_key),
            model,
        }
    }

    /// Creates a system prompt for tool selection
    #[instrument(skip(self, tools), fields(num_tools = tools.len()))]
    fn create_system_prompt(&self, tools: &[Tool], validation_feedback: Option<&str>) -> String {
        let tool_descriptions: Vec<String> = tools.iter()
            .map(|t| format!("- Name: {}\n  Description: {}\n  Schema: {}", 
                t.name, t.description, t.input_schema))
            .collect();

        let mut prompt = format!(
            "You are a tool selection expert. Given a user query and available tools, select the most appropriate tool(s) and suggest parameters.\n\n\
            Available tools:\n{}\n\n\
            Respond in JSON format:\n{{\n\
              \"selected_tools\": [\n\
                {{\n\
                  \"tool_name\": \"string\",\n\
                  \"confidence\": number, // 0.0 to 1.0\n\
                  \"parameters\": {{}}, // suggested parameters based on schema\n\
                  \"reasoning\": \"string\"\n\
                }}\n\
              ]\n\
            }}\n\n\
            Ensure parameters match the tool's schema exactly.",
            tool_descriptions.join("\n")
        );

        if let Some(feedback) = validation_feedback {
            debug!("Adding validation feedback to prompt: {}", feedback);
            prompt.push_str("\n\nPrevious attempt had validation issues:\n");
            prompt.push_str(feedback);
            prompt.push_str("\nPlease adjust parameters accordingly.");
        }

        prompt
    }

    #[instrument(skip(self, tool, parameters), fields(tool_name = %tool.name))]
    async fn try_fix_parameters(&self, tool: &Tool, parameters: Value, query: &str) -> Result<(Value, ValidationStatus)> {
        // First try validating as is
        if ParameterValidator::validate_parameters(tool, &parameters).is_ok() {
            debug!("Parameters valid without fixing");
            return Ok((parameters, ValidationStatus::Valid));
        }

        debug!("Attempting to fix invalid parameters");
        // Try fixing parameters
        match ParameterValidator::fix_parameters(tool, parameters.clone()) {
            Ok(fixed) => {
                // Validate the fixed parameters
                if ParameterValidator::validate_parameters(tool, &fixed).is_ok() {
                    info!("Successfully fixed parameters automatically");
                    Ok((fixed.clone(), ValidationStatus::Fixed { 
                        original: parameters, 
                        fixed 
                    }))
                } else {
                    debug!("Automatic fix failed, attempting LLM-based fix");
                    // If still invalid after fixing, try one more time with LLM
                    self.request_parameter_fix(tool, &parameters, query).await
                }
            }
            Err(e) => {
                warn!("Automatic parameter fixing failed: {}", e);
                // Try LLM-based fixing
                self.request_parameter_fix(tool, &parameters, query).await
            }
        }
    }

    #[instrument(skip(self, tool, invalid_params), fields(tool_name = %tool.name))]
    async fn request_parameter_fix(&self, tool: &Tool, invalid_params: &Value, query: &str) -> Result<(Value, ValidationStatus)> {
        debug!("Requesting LLM to fix parameters: {:?}", invalid_params);
        let prompt = format!(
            "Fix these invalid parameters for the tool:\n\
            Tool: {}\n\
            Schema: {}\n\
            Invalid parameters: {}\n\
            User query: {}\n\
            Return only the fixed parameters as valid JSON.",
            tool.name,
            tool.input_schema,
            invalid_params,
            query
        );

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a parameter fixing expert. Return only valid JSON matching the schema.".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ];

        let response = self.client.chat_completion(
            &self.model,
            messages,
            Some(0.7),
            Some(1000)
        ).await?;

        let content = response.choices.first()
            .ok_or_else(|| anyhow!("No response choices available"))?
            .message.content.clone();

        let fixed: Value = serde_json::from_str(&content)
            .map_err(|e| {
                error!("Failed to parse LLM response as JSON: {}", e);
                anyhow!("Failed to parse fixed parameters as JSON: {}", e)
            })?;

        // Validate the fixed parameters
        if ParameterValidator::validate_parameters(tool, &fixed).is_ok() {
            info!("LLM successfully fixed parameters");
            Ok((fixed.clone(), ValidationStatus::Fixed {
                original: invalid_params.clone(),
                fixed
            }))
        } else {
            error!("LLM failed to fix parameters");
            Err(anyhow!("Failed to fix parameters after LLM attempt"))
        }
    }

    /// Selects appropriate tools based on user intent
    #[instrument(skip(self, available_tools), fields(num_tools = available_tools.len()))]
    pub async fn select_tools(&self, query: &str, available_tools: Vec<Tool>) -> Result<RankedToolSelection> {
        let mut validation_feedback = None;
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 2;

        while attempts < MAX_ATTEMPTS {
            info!("Tool selection attempt {}/{}", attempts + 1, MAX_ATTEMPTS);
            let system_prompt = self.create_system_prompt(&available_tools, validation_feedback.as_deref());
            
            let messages = vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: query.to_string(),
                },
            ];

            let response = self.client.chat_completion(
                &self.model, 
                messages,
                Some(0.7),
                Some(1000)
            ).await?;
            
            let content = response.choices.first()
                .ok_or_else(|| anyhow!("No response choices available"))?
                .message.content.clone();

            let response_value: Value = serde_json::from_str(&content)
                .map_err(|e| {
                    error!("Failed to parse LLM response as JSON: {}", e);
                    anyhow!("Failed to parse LLM response as JSON: {}", e)
                })?;

            let selected_tools = response_value.get("selected_tools")
                .ok_or_else(|| anyhow!("Response missing selected_tools field"))?
                .as_array()
                .ok_or_else(|| anyhow!("selected_tools is not an array"))?;

            let mut matches = Vec::new();
            let mut validation_errors = Vec::new();

            for selection in selected_tools {
                let tool_name = selection.get("tool_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Tool selection missing tool_name"))?;

                let tool = available_tools.iter()
                    .find(|t| t.name == tool_name)
                    .ok_or_else(|| anyhow!("Selected tool {} not found in available tools", tool_name))?;

                let confidence = selection.get("confidence")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| anyhow!("Tool selection missing confidence"))?;

                let parameters = selection.get("parameters")
                    .ok_or_else(|| anyhow!("Tool selection missing parameters"))?
                    .clone();

                let reasoning = selection.get("reasoning")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No reasoning provided")
                    .to_string();

                debug!("Processing tool match: {}", tool_name);
                // Try to validate and fix parameters
                let (final_params, validation_status) = match self.try_fix_parameters(tool, parameters, query).await {
                    Ok((params, status)) => {
                        debug!("Parameter validation status: {}", status);
                        (Some(params), status)
                    },
                    Err(e) => {
                        error!("Parameter validation failed for {}: {}", tool_name, e);
                        validation_errors.push(format!("Tool '{}': {}", tool_name, e));
                        (None, ValidationStatus::Failed { error: e.to_string() })
                    }
                };

                matches.push(ToolMatch {
                    tool: tool.clone(),
                    confidence,
                    suggested_parameters: final_params,
                    reasoning,
                    validation_status,
                });
            }

            let selection = RankedToolSelection::new(matches);
            info!("{}", selection.validation_summary());

            // If we have any valid matches, return them
            if selection.matches.iter().any(|m| m.is_valid()) {
                return Ok(selection);
            }

            // If all matches failed validation and we haven't exceeded attempts, try again
            validation_feedback = Some(validation_errors.join("\n"));
            warn!("No valid matches found, attempting retry with feedback");
            attempts += 1;
        }

        // If we get here, we've exceeded attempts with no valid matches
        error!("Failed to get valid tool matches after {} attempts", MAX_ATTEMPTS);
        Err(anyhow!("Failed to get valid tool matches after {} attempts", MAX_ATTEMPTS))
    }
} 
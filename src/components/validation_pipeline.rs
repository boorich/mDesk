use serde_json::Value;
use anyhow::{Result, anyhow};
use tracing::{debug, error, info, warn, instrument};
use crate::components::parameter_validation::ParameterValidator;
use mcp_core::Tool;

/// Represents the state of input validation
#[derive(Debug, Clone)]
pub enum ValidationState {
    /// Input is valid and ready for processing
    Valid(Value),
    /// Input has been sanitized/modified but is valid
    Sanitized {
        original: Value,
        sanitized: Value,
        changes: Vec<String>,
    },
    /// Input is invalid and cannot be processed
    Invalid {
        input: Value,
        errors: Vec<String>,
    },
}

impl ValidationState {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationState::Valid(_) | ValidationState::Sanitized { .. })
    }

    pub fn get_value(&self) -> Option<&Value> {
        match self {
            ValidationState::Valid(v) => Some(v),
            ValidationState::Sanitized { sanitized, .. } => Some(sanitized),
            ValidationState::Invalid { .. } => None,
        }
    }

    pub fn get_changes(&self) -> Vec<String> {
        match self {
            ValidationState::Sanitized { changes, .. } => changes.clone(),
            _ => Vec::new(),
        }
    }

    pub fn get_errors(&self) -> Vec<String> {
        match self {
            ValidationState::Invalid { errors, .. } => errors.clone(),
            _ => Vec::new(),
        }
    }
}

/// Pipeline for validating and sanitizing tool inputs
pub struct ValidationPipeline {
    /// Maximum depth for nested JSON structures
    max_depth: usize,
    /// Maximum length for string values
    max_string_length: usize,
    /// Whether to attempt fixing common issues
    auto_fix: bool,
}

impl Default for ValidationPipeline {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_string_length: 1000,
            auto_fix: true,
        }
    }
}

impl ValidationPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_max_string_length(mut self, length: usize) -> Self {
        self.max_string_length = length;
        self
    }

    pub fn with_auto_fix(mut self, auto_fix: bool) -> Self {
        self.auto_fix = auto_fix;
        self
    }

    /// Validates and sanitizes input for a specific tool
    #[instrument(skip(self, input), fields(tool_name = %tool.name))]
    pub fn validate_input(&self, tool: &Tool, input: Value) -> ValidationState {
        let mut changes = Vec::new();
        let mut errors = Vec::new();

        // First pass: Check basic structure and sanitize
        let sanitized = match self.sanitize_value(input.clone(), 0, &mut changes, &mut errors) {
            Ok(v) => v,
            Err(e) => {
                error!("Critical validation error: {}", e);
                return ValidationState::Invalid {
                    input,
                    errors: vec![e.to_string()],
                };
            }
        };

        // If we have errors but auto_fix is disabled, return invalid state
        if !errors.is_empty() && !self.auto_fix {
            return ValidationState::Invalid {
                input,
                errors,
            };
        }

        // Second pass: Validate against tool schema
        match ParameterValidator::validate_parameters(tool, &sanitized) {
            Ok(_) => {
                if changes.is_empty() {
                    ValidationState::Valid(sanitized)
                } else {
                    ValidationState::Sanitized {
                        original: input,
                        sanitized,
                        changes,
                    }
                }
            }
            Err(e) => {
                // If auto_fix is enabled, try to fix the parameters
                if self.auto_fix {
                    match ParameterValidator::fix_parameters(tool, sanitized.clone()) {
                        Ok(fixed) => {
                            changes.push(format!("Parameters automatically fixed: {}", e));
                            ValidationState::Sanitized {
                                original: input,
                                sanitized: fixed,
                                changes,
                            }
                        }
                        Err(fix_err) => {
                            errors.push(e.to_string());
                            errors.push(format!("Auto-fix failed: {}", fix_err));
                            ValidationState::Invalid {
                                input,
                                errors,
                            }
                        }
                    }
                } else {
                    errors.push(e.to_string());
                    ValidationState::Invalid {
                        input,
                        errors,
                    }
                }
            }
        }
    }

    /// Sanitizes a JSON value, checking for security issues and malformed data
    fn sanitize_value(
        &self,
        value: Value,
        depth: usize,
        changes: &mut Vec<String>,
        errors: &mut Vec<String>,
    ) -> Result<Value> {
        // Check depth limit
        if depth > self.max_depth {
            return Err(anyhow!("Maximum nesting depth exceeded"));
        }

        match value {
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (key, val) in map {
                    // Sanitize key
                    let clean_key = self.sanitize_string(&key);
                    if clean_key != key {
                        changes.push(format!("Sanitized object key: {} -> {}", key, clean_key));
                    }

                    // Recursively sanitize value
                    match self.sanitize_value(val, depth + 1, changes, errors) {
                        Ok(clean_val) => {
                            new_map.insert(clean_key, clean_val);
                        }
                        Err(e) => {
                            errors.push(format!("Error in field '{}': {}", key, e));
                        }
                    }
                }
                Ok(Value::Object(new_map))
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::with_capacity(arr.len());
                for (i, val) in arr.into_iter().enumerate() {
                    match self.sanitize_value(val, depth + 1, changes, errors) {
                        Ok(clean_val) => new_arr.push(clean_val),
                        Err(e) => {
                            errors.push(format!("Error in array index {}: {}", i, e));
                        }
                    }
                }
                Ok(Value::Array(new_arr))
            }
            Value::String(s) => {
                let clean = self.sanitize_string(&s);
                if clean != s {
                    changes.push(format!("Sanitized string value"));
                }
                Ok(Value::String(clean))
            }
            // Numbers and booleans are considered safe as-is
            Value::Number(_) | Value::Bool(_) => Ok(value),
            Value::Null => Ok(value),
        }
    }

    /// Sanitizes a string value
    fn sanitize_string(&self, input: &str) -> String {
        let mut output = input.trim().to_string();

        // Truncate if too long
        if output.len() > self.max_string_length {
            output.truncate(self.max_string_length);
        }

        // Remove control characters
        output.retain(|c| !c.is_control() || c == '\n' || c == '\t');

        // Basic HTML escape (prevent XSS)
        output = output
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;");

        output
    }
} 
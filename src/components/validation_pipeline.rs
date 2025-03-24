use serde_json::Value;
use anyhow::{Result, anyhow};
use tracing::{debug, error, info, warn, instrument};
use crate::components::parameter_validation::ParameterValidator;
use mcp_core::Tool;
use std::collections::HashMap;

/// Represents the state of input validation
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationState {
    /// Input is valid and ready for processing
    Valid(Value),
    /// Input has been sanitized/modified but is valid
    Sanitized {
        original: Value,
        sanitized: Value,
        changes: Vec<String>,
    },
    /// Input is invalid but recovered with fallback strategies
    Recovered {
        original: Value,
        recovered: Value,
        strategies: Vec<RecoveryStrategy>,
        errors: Vec<String>,
    },
    /// Input is invalid and cannot be processed
    Invalid {
        input: Value,
        errors: Vec<String>,
        alternative_tools: Vec<String>,
    },
}

/// Represents a recovery strategy applied to fix invalid input
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryStrategy {
    /// Used default values from schema
    DefaultValue { field: String, value: Value },
    /// Used a fallback value from configured fallbacks
    FallbackValue { field: String, value: Value },
    /// Removed an invalid field
    RemovedField { field: String },
    /// Replaced an invalid value with a type-appropriate value
    ReplacedValue { field: String, original: Value, replacement: Value },
    /// Other recovery strategies
    Other { description: String },
}

impl ValidationState {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationState::Valid(_) | ValidationState::Sanitized { .. } | ValidationState::Recovered { .. })
    }

    pub fn get_value(&self) -> Option<&Value> {
        match self {
            ValidationState::Valid(v) => Some(v),
            ValidationState::Sanitized { sanitized, .. } => Some(sanitized),
            ValidationState::Recovered { recovered, .. } => Some(recovered),
            ValidationState::Invalid { .. } => None,
        }
    }

    pub fn get_changes(&self) -> Vec<String> {
        match self {
            ValidationState::Sanitized { changes, .. } => changes.clone(),
            ValidationState::Recovered { strategies, .. } => {
                strategies.iter()
                    .map(|s| s.to_string())
                    .collect()
            },
            _ => Vec::new(),
        }
    }

    pub fn get_errors(&self) -> Vec<String> {
        match self {
            ValidationState::Invalid { errors, .. } => errors.clone(),
            ValidationState::Recovered { errors, .. } => errors.clone(),
            _ => Vec::new(),
        }
    }
    
    pub fn get_alternative_tools(&self) -> Vec<String> {
        match self {
            ValidationState::Invalid { alternative_tools, .. } => alternative_tools.clone(),
            _ => Vec::new(),
        }
    }
}

impl RecoveryStrategy {
    pub fn to_string(&self) -> String {
        match self {
            RecoveryStrategy::DefaultValue { field, value } => {
                format!("Used default value for field '{}': {}", field, value)
            },
            RecoveryStrategy::FallbackValue { field, value } => {
                format!("Used fallback value for field '{}': {}", field, value)
            },
            RecoveryStrategy::RemovedField { field } => {
                format!("Removed invalid field: '{}'", field)
            },
            RecoveryStrategy::ReplacedValue { field, original, replacement } => {
                format!("Replaced invalid value in field '{}': {} -> {}", field, original, replacement)
            },
            RecoveryStrategy::Other { description } => description.clone(),
        }
    }
}

/// Pipeline for validating and sanitizing tool inputs
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationPipeline {
    /// Maximum depth for nested JSON structures
    max_depth: usize,
    /// Maximum length for string values
    max_string_length: usize,
    /// Whether to attempt fixing common issues
    auto_fix: bool,
    /// Fallback values for specific fields when validation fails
    fallback_values: HashMap<String, Value>,
    /// Whether to suggest alternative tools on validation failure
    suggest_alternatives: bool,
    /// Maximum number of alternative tools to suggest
    max_alternatives: usize,
    /// Available tools for alternative suggestions
    available_tools: Vec<Tool>,
}

impl Default for ValidationPipeline {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_string_length: 1000,
            auto_fix: true,
            fallback_values: HashMap::new(),
            suggest_alternatives: true,
            max_alternatives: 3,
            available_tools: Vec::new(),
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
    
    pub fn with_fallback(mut self, field: &str, value: Value) -> Self {
        self.fallback_values.insert(field.to_string(), value);
        self
    }
    
    pub fn with_suggest_alternatives(mut self, suggest: bool) -> Self {
        self.suggest_alternatives = suggest;
        self
    }
    
    pub fn with_max_alternatives(mut self, max: usize) -> Self {
        self.max_alternatives = max;
        self
    }
    
    pub fn with_available_tools(mut self, tools: Vec<Tool>) -> Self {
        self.available_tools = tools;
        self
    }
    
    /// Updates the list of available tools
    pub fn update_available_tools(&mut self, tools: Vec<Tool>) {
        self.available_tools = tools;
    }

    /// Validates and sanitizes input for a specific tool
    #[instrument(skip(self, input), fields(tool_name = %tool.name))]
    pub fn validate_input(&self, tool: &Tool, input: Value) -> ValidationState {
        let mut changes = Vec::new();
        let mut errors = Vec::new();
        
        // Preprocessing for filesystem tools: ensure path parameters have values
        let input = if input.is_object() && 
                       (tool.name.contains("create_directory") || 
                        tool.name.contains("list_directory") || 
                        tool.name.contains("read_file") || 
                        tool.name.contains("write_file") || 
                        tool.name.contains("filesystem")) {
            let mut modified_input = input.clone();
            let obj = modified_input.as_object_mut().unwrap();
            
            // Check if the path parameter is missing or empty
            let path_value = obj.get("path").map(|v| {
                if let Some(path_str) = v.as_str() {
                    path_str.trim().is_empty()
                } else {
                    true // Non-string values are considered empty
                }
            }).unwrap_or(true);
            
            // For filesystem tools, path is critically important
            if path_value {
                // Try to use a sensible default path
                let default_path = match tool.name.as_str() {
                    name if name.contains("create_directory") => Value::String("/Users/martinmaurer/Projects".to_string()),
                    name if name.contains("list_directory") => Value::String("/Users/martinmaurer/Projects".to_string()),
                    name if name.contains("read_file") => Value::String("/Users/martinmaurer/Projects/file.txt".to_string()),
                    name if name.contains("write_file") => Value::String("/Users/martinmaurer/Projects/file.txt".to_string()),
                    _ => Value::String("/Users/martinmaurer/Projects".to_string()),
                };
                
                obj.insert("path".to_string(), default_path);
                changes.push("Added default path parameter for filesystem operation".to_string());
            }
            
            modified_input
        } else {
            input.clone()
        };

        // First pass: Check basic structure and sanitize
        let sanitized = match self.sanitize_value(input.clone(), 0, &mut changes, &mut errors) {
            Ok(v) => v,
            Err(e) => {
                error!("Critical validation error: {}", e);
                let alternatives = if self.suggest_alternatives {
                    self.find_alternative_tools(tool, &input)
                } else {
                    Vec::new()
                };
                
                return ValidationState::Invalid {
                    input,
                    errors: vec![e.to_string()],
                    alternative_tools: alternatives,
                };
            }
        };

        // If we have errors but auto_fix is disabled, return invalid state
        if !errors.is_empty() && !self.auto_fix {
            let alternatives = if self.suggest_alternatives {
                self.find_alternative_tools(tool, &input)
            } else {
                Vec::new()
            };
            
            return ValidationState::Invalid {
                input,
                errors,
                alternative_tools: alternatives,
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
                            // Auto-fix failed, try recovery strategies
                            errors.push(e.to_string());
                            errors.push(format!("Auto-fix failed: {}", fix_err));
                            
                            // Try to recover using fallback values and other strategies
                            match self.recover_invalid_input(tool, &sanitized, &errors) {
                                Some((recovered, strategies)) => {
                                    // Successfully recovered with fallback strategies
                                    ValidationState::Recovered {
                                        original: input,
                                        recovered,
                                        strategies,
                                        errors,
                                    }
                                }
                                None => {
                                    // Could not recover, return invalid
                                    let alternatives = if self.suggest_alternatives {
                                        self.find_alternative_tools(tool, &input)
                                    } else {
                                        Vec::new()
                                    };
                                    
                                    ValidationState::Invalid {
                                        input,
                                        errors,
                                        alternative_tools: alternatives,
                                    }
                                }
                            }
                        }
                    }
                } else {
                    errors.push(e.to_string());
                    let alternatives = if self.suggest_alternatives {
                        self.find_alternative_tools(tool, &input)
                    } else {
                        Vec::new()
                    };
                    
                    ValidationState::Invalid {
                        input,
                        errors,
                        alternative_tools: alternatives,
                    }
                }
            }
        }
    }

    /// Attempts to recover from invalid input using fallback strategies
    fn recover_invalid_input(&self, tool: &Tool, input: &Value, errors: &[String]) -> Option<(Value, Vec<RecoveryStrategy>)> {
        debug!("Attempting to recover invalid input for tool: {}", tool.name);
        let mut strategies = Vec::new();
        
        // Only work with objects
        if !input.is_object() {
            return None;
        }
        
        let mut recovered = input.clone();
        let obj = recovered.as_object_mut().unwrap();
        
        // Special handling for filesystem tools
        let is_filesystem_tool = tool.name.contains("filesystem") || 
                                 tool.name.contains("directory") || 
                                 tool.name.contains("file");
        
        if is_filesystem_tool && !obj.contains_key("path") {
            // Add path parameter for filesystem tools
            let default_path = match tool.name.as_str() {
                name if name.contains("create_directory") => Value::String("/Users/martinmaurer/Projects".to_string()),
                name if name.contains("list_directory") => Value::String("/Users/martinmaurer/Projects".to_string()),
                name if name.contains("read_file") => Value::String("/Users/martinmaurer/Projects/file.txt".to_string()),
                name if name.contains("write_file") => Value::String("/Users/martinmaurer/Projects/file.txt".to_string()),
                _ => Value::String("/Users/martinmaurer/Projects".to_string()),
            };
            
            obj.insert("path".to_string(), default_path.clone());
            strategies.push(RecoveryStrategy::DefaultValue { 
                field: "path".to_string(), 
                value: default_path 
            });
        }
        
        // Parse errors to identify problematic fields
        let mut field_errors = self.extract_field_errors(errors);
        
        // If we couldn't extract specific fields from errors, try to apply all fallbacks
        if field_errors.is_empty() {
            // Extract all field names from the input and from required fields in schema
            for key in obj.keys() {
                field_errors.push(key.clone());
            }
            
            // Also check required fields from schema
            if let Some(required) = tool.input_schema.get("required") {
                if let Some(required_fields) = required.as_array() {
                    for field in required_fields {
                        if let Some(name) = field.as_str() {
                            if !field_errors.contains(&name.to_string()) {
                                field_errors.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        // Apply fallback values for problematic fields
        for field in field_errors {
            // Check if we have a fallback for this field
            if let Some(fallback) = self.fallback_values.get(&field) {
                obj.insert(field.clone(), fallback.clone());
                strategies.push(RecoveryStrategy::FallbackValue { 
                    field: field.clone(), 
                    value: fallback.clone() 
                });
                continue;
            }
            
            // Try to get the schema for this field
            if let Some(properties) = tool.input_schema.get("properties") {
                if let Some(props) = properties.as_object() {
                    if let Some(prop_schema) = props.get(&field) {
                        // Create default value based on schema
                        let default_value = ParameterValidator::create_default_value(prop_schema);
                        obj.insert(field.clone(), default_value.clone());
                        strategies.push(RecoveryStrategy::DefaultValue { 
                            field: field.clone(), 
                            value: default_value 
                        });
                        continue;
                    }
                }
            }
            
            // As a last resort, remove the field if it's not required
            if let Some(required) = tool.input_schema.get("required") {
                if let Some(required_fields) = required.as_array() {
                    let is_required = required_fields.iter()
                        .any(|f| f.as_str().map_or(false, |s| s == field));
                    
                    if !is_required {
                        obj.remove(&field);
                        strategies.push(RecoveryStrategy::RemovedField { field: field.clone() });
                    }
                }
            }
        }
        
        // Handle fields with wrong types (especially for arrays and integers)
        for (field, value) in obj.clone().iter() {
            if let Some(properties) = tool.input_schema.get("properties") {
                if let Some(props) = properties.as_object() {
                    if let Some(prop_schema) = props.get(field) {
                        if let Some(field_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                            let value_type_mismatch = match field_type {
                                "array" => !value.is_array(),
                                "integer" | "number" => !value.is_number(),
                                "string" => !value.is_string(),
                                "boolean" => !value.is_boolean(),
                                "object" => !value.is_object(),
                                _ => false,
                            };
                            
                            if value_type_mismatch {
                                let default_value = ParameterValidator::create_default_value(prop_schema);
                                obj.insert(field.clone(), default_value.clone());
                                strategies.push(RecoveryStrategy::ReplacedValue { 
                                    field: field.clone(), 
                                    original: value.clone(),
                                    replacement: default_value 
                                });
                            }
                        }
                    }
                }
            }
        }
        
        // If no recovery strategies were applied but we have fallbacks, try them all
        if strategies.is_empty() && !self.fallback_values.is_empty() {
            for (field, value) in &self.fallback_values {
                obj.insert(field.clone(), value.clone());
                strategies.push(RecoveryStrategy::FallbackValue {
                    field: field.clone(),
                    value: value.clone(),
                });
            }
        }
        
        // Validate the recovered input
        if ParameterValidator::validate_parameters(tool, &recovered).is_ok() {
            debug!("Input recovery successful with {} strategies", strategies.len());
            Some((recovered, strategies))
        } else {
            debug!("Input recovery failed after applying {} strategies", strategies.len());
            None
        }
    }
    
    /// Extracts field names from validation error messages
    fn extract_field_errors(&self, errors: &[String]) -> Vec<String> {
        let mut fields = Vec::new();
        
        for error in errors {
            // Look for common validation error patterns
            if let Some(start) = error.find("field '") {
                if let Some(end) = error[start + 7..].find('\'') {
                    let field = error[start + 7..start + 7 + end].to_string();
                    fields.push(field);
                    continue;
                }
            }
            
            // Look for JSON schema validation errors like "instance.fieldName"
            if let Some(start) = error.find("instance.") {
                let remaining = &error[start + 9..];
                if let Some(end) = remaining.find(|c: char| !c.is_alphanumeric() && c != '_') {
                    let field = remaining[..end].to_string();
                    fields.push(field);
                    continue;
                }
            }
            
            // Look for patterns like "property/field 'name'" or "property name"
            if let Some(start) = error.find("property '") {
                if let Some(end) = error[start + 10..].find('\'') {
                    let field = error[start + 10..start + 10 + end].to_string();
                    fields.push(field);
                    continue;
                }
            }
            
            // Look for patterns like "'name' is not valid"
            if let Some(start) = error.find('\'') {
                if let Some(end) = error[start + 1..].find('\'') {
                    let potential_field = error[start + 1..start + 1 + end].to_string();
                    // Avoid capturing things that are likely not field names
                    if !potential_field.contains(' ') && potential_field.len() < 30 {
                        fields.push(potential_field);
                        continue;
                    }
                }
            }
            
            // Check for property/field mentions using regular words
            for field_indicator in &["field", "property", "value", "parameter"] {
                if let Some(idx) = error.find(field_indicator) {
                    let after_indicator = &error[idx + field_indicator.len()..];
                    if let Some(name_start) = after_indicator.find(|c: char| c.is_alphanumeric()) {
                        let name_part = &after_indicator[name_start..];
                        if let Some(name_end) = name_part.find(|c: char| !c.is_alphanumeric() && c != '_') {
                            let field = name_part[..name_end].to_string();
                            // Avoid capturing things that are likely not field names
                            if field.len() < 30 {
                                fields.push(field);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        fields
    }
    
    /// Finds alternative tools that might be compatible with the input
    fn find_alternative_tools(&self, current_tool: &Tool, input: &Value) -> Vec<String> {
        debug!("Finding alternative tools for failed validation");
        
        let mut alternative_tools = Vec::new();
        let mut tool_scores = Vec::new();
        
        // Extract field names from input - only work with objects
        let input_fields = match input {
            Value::Object(obj) => obj.keys().cloned().collect::<Vec<String>>(),
            _ => return Vec::new(),
        };
        
        // Only process if we have available tools to check
        if self.available_tools.is_empty() {
            return self.get_common_alternatives(&current_tool.name);
        }
        
        for tool in &self.available_tools {
            // Skip the current tool
            if tool.name == current_tool.name {
                continue;
            }
            
            // Skip tools without a schema
            let Some(props) = tool.input_schema.get("properties") else {
                continue;
            };
            
            let Some(props_obj) = props.as_object() else {
                continue;
            };
            
            // Calculate a compatibility score based on field overlap
            let mut match_score = 0.0;
            let tool_fields = props_obj.keys().cloned().collect::<Vec<String>>();
            
            // Boost score for each input field that's in the tool schema
            for field in &input_fields {
                if props_obj.contains_key(field) {
                    match_score += 1.0;
                }
            }
            
            // Normalize score based on total number of fields
            let total_fields = (input_fields.len() + tool_fields.len()) as f64;
            if total_fields > 0.0 {
                match_score = match_score / (total_fields / 2.0);
            }
            
            // Only consider tools with some field overlap
            if match_score > 0.0 {
                tool_scores.push((tool.name.clone(), match_score));
            }
        }
        
        // Sort tools by match score (highest first)
        tool_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top N alternatives
        for (tool_name, _score) in tool_scores.iter().take(self.max_alternatives) {
            alternative_tools.push(tool_name.clone());
        }
        
        // Check if we have any recommendations based on known common alternatives
        let common_alternatives = self.get_common_alternatives(&current_tool.name);
        
        // Add any common alternatives not already included
        for alt in common_alternatives {
            if !alternative_tools.contains(&alt) && alternative_tools.len() < self.max_alternatives {
                alternative_tools.push(alt);
            }
        }
        
        alternative_tools
    }
    
    /// Returns common alternative tools for a given tool based on known patterns
    fn get_common_alternatives(&self, tool_name: &str) -> Vec<String> {
        match tool_name {
            "web_search" => vec!["mcp_github_search_repositories".to_string(), "mcp_github_search_code".to_string()],
            "read_file" => vec!["mcp_filesystem_read_file".to_string()],
            "write_file" => vec!["mcp_filesystem_write_file".to_string(), "mcp_filesystem_edit_file".to_string()],
            "edit_file" => vec!["mcp_filesystem_edit_file".to_string(), "mcp_filesystem_write_file".to_string()],
            "list_dir" => vec!["mcp_filesystem_list_directory".to_string()],
            "run_terminal_cmd" => vec!["mcp_filesystem_read_file".to_string(), "mcp_filesystem_write_file".to_string()],
            "grep_search" => vec!["codebase_search".to_string(), "file_search".to_string()],
            "file_search" => vec!["grep_search".to_string(), "codebase_search".to_string()],
            "codebase_search" => vec!["grep_search".to_string(), "file_search".to_string()],
            _ => Vec::new(),
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
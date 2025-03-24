use mcp_core::Tool;
use serde_json::Value;
use anyhow::{Result, anyhow};
use jsonschema::{JSONSchema, Draft};

/// Validates and potentially fixes parameters against a tool's schema
pub struct ParameterValidator;

impl ParameterValidator {
    /// Validates parameters against a tool's schema
    pub fn validate_parameters(tool: &Tool, parameters: &Value) -> Result<()> {
        let schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&tool.input_schema)
            .map_err(|e| anyhow!("Invalid schema: {}", e))?;

        schema.validate(parameters)
            .map_err(|errors| {
                let error_messages: Vec<String> = errors
                    .map(|error| format!("{}", error))
                    .collect();
                anyhow!("Parameter validation failed: {}", error_messages.join(", "))
            })
    }

    /// Creates a default value based on schema constraints
    pub fn create_default_value(prop_schema: &Value) -> Value {
        if let Some(default) = prop_schema.get("default") {
            return default.clone();
        }

        // First handle common parameter names with smart defaults
        if let Some(prop_name) = prop_schema.get("name").and_then(|n| n.as_str()) {
            // Smart defaults for common parameter names
            match prop_name {
                "path" => return Value::String("/path/to/file".to_string()),
                "directory" | "dir" => return Value::String("/path/to/directory".to_string()),
                "file" | "filename" => return Value::String("filename.txt".to_string()),
                "content" => return Value::String("Content goes here".to_string()),
                "query" => return Value::String("search query".to_string()),
                _ => {} // Continue with type-based defaults
            }
        }

        match prop_schema.get("type").and_then(|t| t.as_str()) {
            Some("string") => {
                // Check if the property has a format specified
                if let Some(format) = prop_schema.get("format").and_then(|f| f.as_str()) {
                    match format {
                        "date" => return Value::String("2023-01-01".to_string()),
                        "date-time" => return Value::String("2023-01-01T00:00:00Z".to_string()),
                        "email" => return Value::String("user@example.com".to_string()),
                        "uri" | "url" => return Value::String("https://example.com".to_string()),
                        _ => {} // Fall back to regular string handling
                    }
                }

                // Handle potential enum values by picking the first one
                if let Some(enum_values) = prop_schema.get("enum").and_then(|e| e.as_array()) {
                    if !enum_values.is_empty() {
                        if let Some(first_value) = enum_values.get(0) {
                            if first_value.is_string() {
                                return first_value.clone();
                            }
                        }
                    }
                }

                // Handle minLength constraint for strings
                if let Some(min_length) = prop_schema.get("minLength").and_then(|v| v.as_u64()) {
                    Value::String("x".repeat(min_length as usize))
                } else {
                    Value::String(String::new())
                }
            }
            Some("number") => {
                // Use minimum value if specified, or 0
                if let Some(min) = prop_schema.get("minimum").and_then(|v| v.as_f64()) {
                    if let Some(num) = serde_json::Number::from_f64(min) {
                        Value::Number(num)
                    } else {
                        Value::Number(0.into())
                    }
                } else {
                    Value::Number(0.into())
                }
            }
            Some("integer") => {
                // Use minimum value if specified, or 0
                if let Some(min) = prop_schema.get("minimum").and_then(|v| v.as_i64()) {
                    Value::Number(min.into())
                } else {
                    Value::Number(0.into())
                }
            }
            Some("boolean") => Value::Bool(false),
            Some("array") => {
                // If there are example items, use them
                if let Some(examples) = prop_schema.get("examples").and_then(|e| e.as_array()) {
                    if !examples.is_empty() {
                        return Value::Array(examples.clone());
                    }
                }
                
                // Create an empty array or one with default items
                if let Some(items) = prop_schema.get("items") {
                    if let Some(min_items) = prop_schema.get("minItems").and_then(|v| v.as_u64()) {
                        let mut array = Vec::new();
                        for _ in 0..min_items {
                            array.push(Self::create_default_value(items));
                        }
                        Value::Array(array)
                    } else {
                        Value::Array(Vec::new())
                    }
                } else {
                    Value::Array(Vec::new())
                }
            }
            Some("object") => {
                let mut obj = serde_json::Map::new();
                
                // Add defaults for required properties
                if let Some(properties) = prop_schema.get("properties").and_then(|p| p.as_object()) {
                    if let Some(required) = prop_schema.get("required").and_then(|r| r.as_array()) {
                        for req in required {
                            if let Some(field_name) = req.as_str() {
                                if let Some(field_schema) = properties.get(field_name) {
                                    obj.insert(field_name.to_string(), Self::create_default_value(field_schema));
                                }
                            }
                        }
                    }
                }
                
                Value::Object(obj)
            }
            _ => Value::Null,
        }
    }

    /// Attempts to fix invalid parameters by applying defaults or removing invalid fields
    pub fn fix_parameters(tool: &Tool, parameters: Value) -> Result<Value> {
        // Start with empty object if parameters is not an object
        let mut fixed = if parameters.is_object() {
            parameters
        } else {
            Value::Object(serde_json::Map::new())
        };

        if let Some(properties) = tool.input_schema.get("properties") {
            if let Some(props) = properties.as_object() {
                // Add missing required fields with default values
                if let Some(required) = tool.input_schema.get("required") {
                    if let Some(required_fields) = required.as_array() {
                        for field in required_fields {
                            if let Some(field_name) = field.as_str() {
                                if !fixed.get(field_name).is_some() {
                                    // Get default value from schema
                                    if let Some(prop_schema) = props.get(field_name) {
                                        let default_value = Self::create_default_value(prop_schema);
                                        if let Some(obj) = fixed.as_object_mut() {
                                            obj.insert(field_name.to_string(), default_value);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Validate the fixed parameters
        Self::validate_parameters(tool, &fixed)?;
        Ok(fixed)
    }
} 
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

        match prop_schema.get("type").and_then(|t| t.as_str()) {
            Some("string") => {
                // Handle minLength constraint for strings
                if let Some(min_length) = prop_schema.get("minLength").and_then(|v| v.as_u64()) {
                    Value::String("x".repeat(min_length as usize))
                } else {
                    Value::String(String::new())
                }
            }
            Some("number") => Value::Number(0.into()),
            Some("integer") => Value::Number(0.into()),
            Some("boolean") => Value::Bool(false),
            Some("array") => Value::Array(Vec::new()),
            Some("object") => Value::Object(serde_json::Map::new()),
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
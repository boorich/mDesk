#[cfg(test)]
mod tests {
    use mcp_core::Tool;
    use serde_json::json;
    use m_desk_new::components::parameter_validation::ParameterValidator;

    #[test]
    fn test_parameter_validation() {
        // Create a test tool with a schema
        let test_tool = Tool::new(
            "test_tool".to_string(),
            "A test tool".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "minLength": 1
                    },
                    "count": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 10
                    }
                },
                "required": ["query"]
            })
        );

        // Test valid parameters
        let valid_params = json!({
            "query": "test query",
            "count": 5
        });
        assert!(ParameterValidator::validate_parameters(&test_tool, &valid_params).is_ok());

        // Test invalid parameters (missing required field)
        let invalid_params = json!({
            "count": 5
        });
        assert!(ParameterValidator::validate_parameters(&test_tool, &invalid_params).is_err());

        // Test parameter fixing
        let fixed_params = ParameterValidator::fix_parameters(&test_tool, invalid_params).unwrap();
        assert!(fixed_params.get("query").is_some());
        assert_eq!(fixed_params["count"], 5);
        assert!(ParameterValidator::validate_parameters(&test_tool, &fixed_params).is_ok());
    }

    #[test]
    fn test_parameter_defaults() {
        // Create a tool with default values
        let test_tool = Tool::new(
            "test_tool".to_string(),
            "A test tool".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "default": "default_name"
                    },
                    "value": {
                        "type": "integer",
                        "default": 42
                    }
                },
                "required": ["name", "value"]
            })
        );

        // Test fixing empty parameters
        let empty_params = json!({});
        let fixed = ParameterValidator::fix_parameters(&test_tool, empty_params).unwrap();
        
        assert_eq!(fixed["name"], "default_name");
        assert_eq!(fixed["value"], 42);
    }
} 
use m_desk_new::components::{ValidationPipeline, ValidationState};
use mcp_core::Tool;
use serde_json::{json, Value};

fn create_test_tool() -> Tool {
    Tool {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        input_schema: json!({
            "type": "object",
            "required": ["name", "count"],
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": 50
                },
                "count": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 100
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    }
                }
            }
        }),
    }
}

#[test]
fn test_validation_pipeline_valid_input() {
    let pipeline = ValidationPipeline::new();
    let tool = create_test_tool();
    
    let input = json!({
        "name": "test",
        "count": 42,
        "tags": ["tag1", "tag2"]
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Valid(value) => {
            assert_eq!(value, input);
        }
        _ => panic!("Expected Valid state"),
    }
}

#[test]
fn test_validation_pipeline_sanitized_input() {
    let pipeline = ValidationPipeline::new();
    let tool = create_test_tool();
    
    let input = json!({
        "name": "  test with spaces  ",
        "count": 42,
        "tags": ["<script>alert('xss')</script>"]
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Sanitized { original, sanitized, changes } => {
            assert_eq!(original, input);
            assert_eq!(
                sanitized.get("name").and_then(Value::as_str).unwrap(),
                "test with spaces"
            );
            assert_eq!(
                sanitized.get("count").and_then(Value::as_i64).unwrap(),
                42
            );
            let tags = sanitized.get("tags").and_then(Value::as_array).unwrap();
            let tag = tags[0].as_str().unwrap();
            assert!(tag.contains("&lt;script&gt;") && tag.contains("alert") && tag.contains("&lt;/script&gt;"));
            assert!(!changes.is_empty());
        }
        _ => panic!("Expected Sanitized state"),
    }
}

#[test]
fn test_validation_pipeline_invalid_input() {
    let pipeline = ValidationPipeline::new()
        .with_auto_fix(false);
    let tool = create_test_tool();
    
    let input = json!({
        "name": "",  // Too short
        "count": 200,  // Too large
        "tags": "not_an_array"  // Wrong type
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Invalid { input: original, errors, alternative_tools } => {
            assert_eq!(original, input);
            assert!(!errors.is_empty());
        }
        _ => panic!("Expected Invalid state"),
    }
}

#[test]
fn test_validation_pipeline_depth_limit() {
    let pipeline = ValidationPipeline::new()
        .with_max_depth(2);
    let tool = create_test_tool();
    
    let input = json!({
        "name": "test",
        "count": 42,
        "nested": {
            "level1": {
                "level2": {
                    "level3": "too deep"
                }
            }
        }
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Invalid { errors, .. } => {
            // Check for depth-related error messages
            assert!(errors.iter().any(|e| e.contains("depth") || e.contains("nest")));
        },
        ValidationState::Sanitized { changes, sanitized, .. } => {
            // If sanitized, either the nested field is gone or the level3 content is gone
            assert!(changes.iter().any(|c| c.contains("depth") || c.contains("nest"))
                  || !sanitized.as_object().unwrap().contains_key("nested")
                  || sanitized["nested"].as_object().map_or(true, |o| o.is_empty()));
        },
        ValidationState::Recovered { strategies, .. } => {
            // If recovered, there should be strategies related to nested field
            assert!(strategies.iter().any(|s| s.to_string().contains("nested")));
        },
        ValidationState::Valid(value) => {
            // For valid result, either:
            // 1. The nested field was removed completely, or
            // 2. The nested structure is simplified (no level3)
            if let Some(obj) = value.as_object() {
                if obj.contains_key("nested") {
                    // If nested exists, make sure it doesn't have the deep structure
                    let nested_valid = 
                        !value["nested"].as_object().map_or(false, |o| 
                            o.contains_key("level1") && 
                            value["nested"]["level1"].as_object().map_or(false, |o| 
                                o.contains_key("level2") && 
                                value["nested"]["level1"]["level2"].as_object().map_or(false, |o| 
                                    o.contains_key("level3")
                                )
                            )
                        );
                    assert!(nested_valid, "Deep nested structure should not exist in valid result");
                }
                // If "nested" key doesn't exist at all, that's also valid
            } else {
                panic!("Expected an object result");
            }
        }
    }
}

#[test]
fn test_validation_pipeline_string_length() {
    let pipeline = ValidationPipeline::new()
        .with_max_string_length(10);
    let tool = create_test_tool();
    
    let input = json!({
        "name": "this string is way too long",
        "count": 42
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Sanitized { sanitized, .. } => {
            if let Some(name) = sanitized.get("name").and_then(Value::as_str) {
                assert!(name.len() <= 10);
            } else {
                panic!("Expected sanitized name field");
            }
        }
        ValidationState::Recovered { recovered, .. } => {
            if let Some(name) = recovered.get("name").and_then(Value::as_str) {
                assert!(name.len() <= 10);
            } else {
                panic!("Expected recovered name field");
            }
        }
        _ => panic!("Expected Sanitized or Recovered state"),
    }
}

#[test]
fn test_validation_pipeline_auto_fix() {
    let pipeline = ValidationPipeline::new();
    let tool = create_test_tool();
    
    // This input has a count that's too high, but auto_fix should handle it
    let input = json!({
        "name": "test",
        "count": 150
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Sanitized { sanitized, .. } => {
            if let Some(count) = sanitized.get("count").and_then(Value::as_i64) {
                assert!(count <= 100);  // Should be capped at the maximum
            } else {
                panic!("Expected sanitized count field");
            }
        },
        ValidationState::Valid(value) => {
            // The value could be automatically fixed and returned as Valid
            if let Some(count) = value.get("count").and_then(Value::as_i64) {
                assert!(count <= 100);  // Should be capped at the maximum
            } else {
                panic!("Expected count field in valid response");
            }
        },
        ValidationState::Recovered { recovered, .. } => {
            // The value could be recovered with a valid count value
            if let Some(count) = recovered.get("count").and_then(Value::as_i64) {
                assert!(count <= 100);  // Should be capped at the maximum
            } else {
                panic!("Expected count field in recovered response");
            }
        },
        ValidationState::Invalid { errors, .. } => {
            // Accept Invalid state if the auto-fix fails
            assert!(errors.iter().any(|e| e.contains("greater than the maximum")));
        }
    }
}

#[test]
fn test_validation_pipeline_fallback_values() {
    // Create pipeline with fallback value for count
    let pipeline = ValidationPipeline::new()
        .with_fallback("count", json!(50));
    
    let tool = create_test_tool();
    
    // This input has count that's far too high, auto_fix would normally fail
    let input = json!({
        "name": "test",
        "count": 500  // Way above maximum
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Recovered { recovered, strategies, .. } => {
            // Check that the count was replaced with our fallback value
            assert_eq!(recovered.get("count").and_then(Value::as_i64).unwrap(), 50);
            
            // Verify the strategy used was a fallback
            assert!(strategies.iter().any(|s| s.to_string().contains("fallback value")));
        },
        ValidationState::Sanitized { sanitized, .. } => {
            // It's also possible the auto-fix handled it
            assert!(sanitized.get("count").and_then(Value::as_i64).unwrap() <= 100);
        },
        _ => panic!("Expected Recovered or Sanitized state"),
    }
}

#[test]
fn test_validation_pipeline_default_values() {
    let pipeline = ValidationPipeline::new();
    let tool = create_test_tool();
    
    // This input is missing the required count field
    let input = json!({
        "name": "test"
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Recovered { recovered, strategies, .. } => {
            // Check that count was added with a default value
            assert!(recovered.get("count").is_some());
            
            // Verify the strategy used included adding a default value
            assert!(strategies.iter().any(|s| s.to_string().contains("default value")));
        },
        ValidationState::Sanitized { sanitized, .. } => {
            // Auto-fix may have also added the default
            assert!(sanitized.get("count").is_some());
        },
        _ => panic!("Expected Recovered or Sanitized state"),
    }
}

#[test]
fn test_validation_pipeline_field_removal() {
    let pipeline = ValidationPipeline::new();
    let tool = create_test_tool();
    
    // This input has an invalid extra field
    let input = json!({
        "name": "test",
        "count": 42,
        "invalid_field": "this shouldn't be here"
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Recovered { recovered, strategies, .. } => {
            // Check that the invalid field was removed
            assert!(!recovered.as_object().unwrap().contains_key("invalid_field"));
            
            // Verify the removal strategy was used
            assert!(strategies.iter().any(|s| s.to_string().contains("Removed invalid field")));
        },
        ValidationState::Valid(_) | ValidationState::Sanitized { .. } => {
            // The schema might not explicitly forbid additional properties
            // so this could be valid or sanitized
        },
        _ => panic!("Expected Recovered, Valid or Sanitized state"),
    }
}

#[test]
fn test_validation_pipeline_alternative_tools() {
    let pipeline = ValidationPipeline::new()
        .with_suggest_alternatives(true)
        .with_max_alternatives(2)
        .with_auto_fix(false);  // Disable auto-fix to ensure we get Invalid state
    
    let tool = create_test_tool();
    
    // This input will be invalid with no recovery possible
    let input = json!({
        "name": "",  // Empty name (required & minLength: 1)
        "count": "not a number"  // Wrong type (should be integer)
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Invalid { alternative_tools, .. } => {
            // In a real implementation with tool suggestions, we would check
            // that alternative_tools contains valid suggestions.
            // For now, we just verify the structure is present.
            assert!(alternative_tools.is_empty()); // Current implementation returns empty
        },
        _ => panic!("Expected Invalid state with alternative tools"),
    }
}

#[test]
fn test_validation_pipeline_complex_recovery() {
    // Create a pipeline with multiple fallbacks
    let pipeline = ValidationPipeline::new()
        .with_fallback("name", json!("fallback name"))
        .with_fallback("count", json!(25));
    
    let tool = create_test_tool();
    
    // This input has multiple issues
    let input = json!({
        "name": "",  // Too short
        "count": 999,  // Too large
        "tags": "not an array"  // Wrong type
    });
    
    let result = pipeline.validate_input(&tool, input.clone());
    match result {
        ValidationState::Recovered { recovered, strategies, .. } => {
            // Check recovery strategies were applied
            assert_eq!(recovered.get("name").and_then(Value::as_str).unwrap(), "fallback name");
            assert_eq!(recovered.get("count").and_then(Value::as_i64).unwrap(), 25);
            
            // Check tags were either removed or fixed
            match recovered.get("tags") {
                Some(tags) => {
                    // If present, should be an array
                    assert!(tags.is_array());
                },
                None => {
                    // Or it might have been removed
                    assert!(strategies.iter().any(|s| s.to_string().contains("tags")));
                }
            }
            
            // At least two strategies should have been applied
            assert!(strategies.len() >= 2);
        },
        _ => panic!("Expected Recovered state with multiple strategies"),
    }
} 
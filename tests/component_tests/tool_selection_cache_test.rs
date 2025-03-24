use m_desk_new::components::tool_selection_cache::ToolSelectionCache;
use m_desk_new::components::tool_selection::{RankedToolSelection, ToolMatch, ValidationStatus};
use mcp_core::Tool;
use serde_json::{json, Value};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

fn create_test_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "test_tool_1".to_string(),
            description: "A test tool one".to_string(),
            input_schema: json!({"type": "object"}),
        },
        Tool {
            name: "test_tool_2".to_string(),
            description: "A test tool two".to_string(),
            input_schema: json!({"type": "object"}),
        },
    ]
}

fn create_test_selection() -> RankedToolSelection {
    let tool = Tool {
        name: "test_tool_1".to_string(),
        description: "A test tool one".to_string(),
        input_schema: json!({"type": "object"}),
    };
    
    let tool_match = ToolMatch {
        tool,
        confidence: 0.8,
        suggested_parameters: Some(json!({})),
        reasoning: "Test reasoning".to_string(),
        validation_status: ValidationStatus::Valid,
    };
    
    RankedToolSelection::new(vec![tool_match])
}

#[test]
fn test_cache_hit_miss() {
    let cache = ToolSelectionCache::new(5, 10); // 5-second TTL, 10 max entries
    let tools = create_test_tools();
    let query = "Test query";
    let selection = create_test_selection();
    
    // Initial request should be a miss
    assert!(cache.get(query, &tools).is_none());
    
    // Store the selection
    cache.store(query, &tools, selection.clone());
    
    // Next request should be a hit
    let cached = cache.get(query, &tools);
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().len(), 1);
}

#[test]
fn test_cache_expiration() {
    let cache = ToolSelectionCache::new(1, 10); // 1-second TTL
    let tools = create_test_tools();
    let query = "Test query";
    let selection = create_test_selection();
    
    cache.store(query, &tools, selection);
    
    // Immediately should be a hit
    assert!(cache.get(query, &tools).is_some());
    
    // Sleep past the TTL
    sleep(Duration::from_secs(2));
    
    // Should be a miss now
    assert!(cache.get(query, &tools).is_none());
}

#[test]
fn test_should_cache() {
    let cache = ToolSelectionCache::new(60, 10);
    
    // These queries should be cacheable
    assert!(cache.should_cache("Find repositories about Rust"));
    assert!(cache.should_cache("How do I search for files"));
    
    // These queries should not be cacheable
    assert!(!cache.should_cache("What is the weather today"));
    assert!(!cache.should_cache("Show me my latest commits"));
    assert!(!cache.should_cache("What time is it now"));
}

#[test]
fn test_cache_invalidation() {
    let cache = ToolSelectionCache::new(60, 10);
    let tools = create_test_tools();
    let selection = create_test_selection();
    
    // Store a few entries
    cache.store("query1", &tools, selection.clone());
    cache.store("query2", &tools, selection.clone());
    
    // Verify they're cached
    assert!(cache.get("query1", &tools).is_some());
    assert!(cache.get("query2", &tools).is_some());
    
    // Invalidate all
    cache.invalidate_all();
    
    // Both should be misses now
    assert!(cache.get("query1", &tools).is_none());
    assert!(cache.get("query2", &tools).is_none());
}

#[test]
fn test_cache_stats() {
    let cache = ToolSelectionCache::new(60, 10);
    let tools = create_test_tools();
    let selection = create_test_selection();
    
    // Empty cache stats
    let stats = cache.stats();
    assert_eq!(stats["total_entries"], json!(0));
    assert_eq!(stats["max_entries"], json!(10));
    assert_eq!(stats["ttl_seconds"], json!(60));
    
    // Add some entries
    cache.store("query1", &tools, selection.clone());
    cache.store("query2", &tools, selection.clone());
    
    // Check stats again
    let stats = cache.stats();
    assert_eq!(stats["total_entries"], json!(2));
}

#[test]
fn test_tool_hashing() {
    let cache = ToolSelectionCache::new(60, 10);
    let tools1 = create_test_tools();
    let tools2 = vec![
        Tool {
            name: "test_tool_1".to_string(),
            description: "A test tool one".to_string(),
            input_schema: json!({"type": "object"}),
        },
        Tool {
            name: "test_tool_3".to_string(), // Different tool
            description: "A test tool three".to_string(),
            input_schema: json!({"type": "object"}),
        },
    ];
    
    let selection = create_test_selection();
    
    // Store with tools1
    cache.store("query", &tools1, selection.clone());
    
    // Verify hit with tools1
    assert!(cache.get("query", &tools1).is_some());
    
    // Should be a miss with tools2 since tool set changed
    assert!(cache.get("query", &tools2).is_none());
} 
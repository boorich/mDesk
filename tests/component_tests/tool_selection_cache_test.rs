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
    let cache = ToolSelectionCache::new(Duration::from_secs(5), 10); // 5-second TTL, 10 max entries
    let query = "Test query";
    
    // Initial request should be a miss
    assert!(cache.get(query).is_none());
    
    // Store the selection
    cache.add(query, "test_tool_1", 0.8, &json!({}));
    
    // Next request should be a hit
    let cached = cache.get(query);
    assert!(cached.is_some());
    let (tool_name, confidence, _) = cached.unwrap();
    assert_eq!(tool_name, "test_tool_1");
    assert_eq!(confidence, 0.8);
}

#[test]
fn test_cache_expiration() {
    let cache = ToolSelectionCache::new(Duration::from_secs(1), 10); // 1-second TTL
    let query = "Test query";
    
    cache.add(query, "test_tool_1", 0.8, &json!({}));
    
    // Immediately should be a hit
    assert!(cache.get(query).is_some());
    
    // Sleep past the TTL
    sleep(Duration::from_secs(2));
    
    // Should be a miss now
    assert!(cache.get(query).is_none());
}

#[test]
fn test_cache_invalidation() {
    let cache = ToolSelectionCache::new(Duration::from_secs(60), 10);
    
    // Store a few entries
    cache.add("query1", "test_tool_1", 0.8, &json!({}));
    cache.add("query2", "test_tool_1", 0.8, &json!({}));
    
    // Verify they're cached
    assert!(cache.get("query1").is_some());
    assert!(cache.get("query2").is_some());
    
    // Invalidate all
    cache.clear();
    
    // Both should be misses now
    assert!(cache.get("query1").is_none());
    assert!(cache.get("query2").is_none());
}

#[test]
fn test_cache_stats() {
    let cache = ToolSelectionCache::new(Duration::from_secs(60), 10);
    
    // Empty cache stats
    let stats = cache.stats();
    assert_eq!(stats["total_entries"], json!(0));
    assert_eq!(stats["max_entries"], json!(10));
    assert_eq!(stats["ttl_seconds"], json!(60));
    
    // Add some entries
    cache.add("query1", "test_tool_1", 0.8, &json!({}));
    cache.add("query2", "test_tool_1", 0.8, &json!({}));
    
    // Check stats again
    let stats = cache.stats();
    assert_eq!(stats["total_entries"], json!(2));
}

#[test]
fn test_tool_removal() {
    let cache = ToolSelectionCache::new(Duration::from_secs(60), 10);
    
    // Store entries for different tools
    cache.add("query1", "test_tool_1", 0.8, &json!({}));
    cache.add("query2", "test_tool_2", 0.8, &json!({}));
    
    // Verify they're cached
    assert!(cache.get("query1").is_some());
    assert!(cache.get("query2").is_some());
    
    // Remove entries for test_tool_1
    cache.remove_tool_entries("test_tool_1");
    
    // query1 should be a miss now
    assert!(cache.get("query1").is_none());
    // but query2 should still be a hit
    assert!(cache.get("query2").is_some());
}

#[test]
fn test_max_entries() {
    let cache = ToolSelectionCache::new(Duration::from_secs(60), 2);  // Only 2 max entries
    
    // Add more than max entries
    cache.add("query1", "test_tool_1", 0.8, &json!({}));
    // Access query1 to make it more recently used
    cache.get("query1");
    
    cache.add("query2", "test_tool_2", 0.8, &json!({}));
    // Adding a third entry should evict the oldest one, which is still query1
    cache.add("query3", "test_tool_3", 0.8, &json!({}));
    
    // Verify the oldest entry was removed
    assert!(cache.get("query2").is_some());
    assert!(cache.get("query3").is_some());
    // query1 should have been removed as the oldest
    assert!(cache.get("query1").is_none());
} 
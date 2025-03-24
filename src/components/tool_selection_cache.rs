use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde_json::Value;
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn, instrument};

/// Cache for tool selection results to avoid redundant LLM calls
#[derive(Debug, Clone)]
pub struct ToolSelectionCache {
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    ttl: Duration,
    max_entries: usize,
}

/// Cache entry for a specific query
#[derive(Debug, Clone, PartialEq)]
struct CacheEntry {
    tool_name: String,
    confidence: f64,
    arguments: Value,
    created_at: DateTime<Utc>,
    last_used: Instant,
    use_count: usize,
}

impl ToolSelectionCache {
    /// Create a new cache with specified TTL and maximum entries
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            max_entries,
        }
    }
    
    /// Get a cached tool suggestion for a query if it exists
    #[instrument(skip(self))]
    pub fn get(&self, query: &str) -> Option<(String, f64, Value)> {
        let now = Instant::now();
        let query_key = self.normalize_query(query);
        
        let mut cache = self.cache.lock().unwrap();
        
        if let Some(entry) = cache.get_mut(&query_key) {
            // Check if expired
            if now.duration_since(entry.last_used) > self.ttl {
                // Entry expired, remove it
                cache.remove(&query_key);
                debug!("Cache miss - expired entry removed for query: {}", query);
                return None;
            }
            
            // Update last used time
            entry.last_used = now;
            entry.use_count += 1;
            
            debug!("Cache hit for query: {}", query);
            return Some((
                entry.tool_name.clone(),
                entry.confidence,
                entry.arguments.clone()
            ));
        }
        
        debug!("Cache miss for query: {}", query);
        None
    }
    
    /// Add a tool suggestion to the cache
    #[instrument(skip(self, arguments))]
    pub fn add(&self, query: &str, tool_name: &str, confidence: f64, arguments: &Value) {
        let query_key = self.normalize_query(query);
        
        let mut cache = self.cache.lock().unwrap();
        
        // Enforce maximum size
        if cache.len() >= self.max_entries && !cache.contains_key(&query_key) {
            self.remove_oldest(&mut cache);
        }
        
        // Add or update entry
        let entry = CacheEntry {
            tool_name: tool_name.to_string(),
            confidence,
            arguments: arguments.clone(),
            created_at: Utc::now(),
            last_used: Instant::now(),
            use_count: 1,
        };
        
        cache.insert(query_key, entry);
        debug!("Added cache entry for tool: {}", tool_name);
    }
    
    /// Clear all entries in the cache
    #[instrument(skip(self))]
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        info!("Cache cleared");
    }
    
    /// Remove all entries related to a specific tool
    #[instrument(skip(self))]
    pub fn remove_tool_entries(&self, tool_name: &str) {
        let mut cache = self.cache.lock().unwrap();
        
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.tool_name == tool_name)
            .map(|(k, _)| k.clone())
            .collect();
        
        let removed_count = keys_to_remove.len();
        
        for key in &keys_to_remove {
            cache.remove(key);
        }
        
        info!("Removed {} entries for tool: {}", removed_count, tool_name);
    }
    
    /// Get cache statistics
    #[instrument(skip(self))]
    pub fn stats(&self) -> HashMap<String, Value> {
        let cache = self.cache.lock().unwrap();
        
        let mut stats = HashMap::new();
        stats.insert("total_entries".to_string(), Value::from(cache.len()));
        stats.insert("max_entries".to_string(), Value::from(self.max_entries));
        stats.insert("ttl_seconds".to_string(), Value::from(self.ttl.as_secs()));
        
        // Calculate total usage count
        let total_usage: usize = cache.values().map(|entry| entry.use_count).sum();
        stats.insert("total_usage".to_string(), Value::from(total_usage));
        
        stats
    }
    
    // Private helper methods
    
    fn normalize_query(&self, query: &str) -> String {
        // Simple normalization: lowercase and remove extra whitespace
        let normalized = query.to_lowercase();
        normalized
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }
    
    fn remove_oldest(&self, cache: &mut HashMap<String, CacheEntry>) {
        if let Some((key_to_remove, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
        {
            let key_to_remove = key_to_remove.clone();
            cache.remove(&key_to_remove);
            debug!("Removed oldest cache entry to make room");
        }
    }
} 
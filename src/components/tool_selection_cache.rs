use mcp_core::Tool;
use crate::components::tool_selection::RankedToolSelection;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use serde_json::Value;
use tracing::{debug, info, warn, instrument};

/// A key for the cache consisting of the query and a hash of available tools
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    query: String,
    tools_hash: u64,
}

/// Cached selection results with expiration
#[derive(Debug, Clone)]
struct CacheEntry {
    selection: RankedToolSelection,
    expires_at: Instant,
}

/// Cache for tool selection results to avoid repeated LLM calls
pub struct ToolSelectionCache {
    entries: Arc<Mutex<HashMap<CacheKey, CacheEntry>>>,
    default_ttl: Duration,
    max_entries: usize,
}

impl ToolSelectionCache {
    /// Create a new cache with the specified TTL and maximum size
    pub fn new(ttl_seconds: u64, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            default_ttl: Duration::from_secs(ttl_seconds),
            max_entries,
        }
    }
    
    /// Get a cached selection if available and not expired
    #[instrument(skip(self, tools))]
    pub fn get(&self, query: &str, tools: &[Tool]) -> Option<RankedToolSelection> {
        let key = self.create_key(query, tools);
        
        let mut entries = self.entries.lock().unwrap();
        
        // Remove expired entries
        self.cleanup_expired(&mut entries);
        
        if let Some(entry) = entries.get(&key) {
            if entry.expires_at > Instant::now() {
                debug!("Cache hit for query: {}", query);
                return Some(entry.selection.clone());
            } else {
                // This should not happen due to cleanup_expired, but just in case
                debug!("Expired entry found for query: {}", query);
                entries.remove(&key);
            }
        }
        
        debug!("Cache miss for query: {}", query);
        None
    }
    
    /// Store a selection in the cache
    #[instrument(skip(self, tools, selection))]
    pub fn store(&self, query: &str, tools: &[Tool], selection: RankedToolSelection) {
        let key = self.create_key(query, tools);
        let entry = CacheEntry {
            selection,
            expires_at: Instant::now() + self.default_ttl,
        };
        
        let mut entries = self.entries.lock().unwrap();
        
        // If we're at capacity, remove the oldest entry
        if entries.len() >= self.max_entries {
            self.remove_oldest(&mut entries);
        }
        
        entries.insert(key, entry);
        debug!("Stored selection in cache for query: {}", query);
    }
    
    /// Check if a query has parameters that would affect caching
    #[instrument(skip(self))]
    pub fn should_cache(&self, query: &str) -> bool {
        // Don't cache queries that are likely to be unique or context-dependent
        let query_lower = query.to_lowercase();
        
        // Don't cache queries with specific dates, times, usernames, or dynamic elements
        if query_lower.contains("today") || 
           query_lower.contains("yesterday") ||
           query_lower.contains("tomorrow") ||
           query_lower.contains("now") ||
           query_lower.contains("current") ||
           query_lower.contains("latest") ||
           query_lower.contains("@") ||
           query_lower.contains("me") ||
           query_lower.contains("my") ||
           query_lower.contains("random") {
            debug!("Query contains context-dependent terms, skipping cache: {}", query);
            return false;
        }
        
        true
    }
    
    /// Invalidate the entire cache
    #[instrument(skip(self))]
    pub fn invalidate_all(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        info!("Cache invalidated completely");
    }
    
    /// Invalidate cache entries that match a specific tool
    #[instrument(skip(self))]
    pub fn invalidate_for_tool(&self, tool_name: &str) {
        let mut entries = self.entries.lock().unwrap();
        
        // Create a list of keys to remove
        let keys_to_remove: Vec<CacheKey> = entries
            .iter()
            .filter(|(_, entry)| {
                entry.selection.matches().iter()
                    .any(|m| m.tool.name == tool_name)
            })
            .map(|(k, _)| k.clone())
            .collect();
        
        // Remove the entries
        for key in keys_to_remove {
            entries.remove(&key);
        }
        
        debug!("Invalidated cache entries for tool: {}", tool_name);
    }
    
    /// Get statistics about the cache
    #[instrument(skip(self))]
    pub fn stats(&self) -> HashMap<String, Value> {
        let entries = self.entries.lock().unwrap();
        
        let mut stats = HashMap::new();
        stats.insert("total_entries".to_string(), Value::from(entries.len()));
        stats.insert("max_entries".to_string(), Value::from(self.max_entries));
        stats.insert("ttl_seconds".to_string(), Value::from(self.default_ttl.as_secs()));
        
        // Count expired entries
        let now = Instant::now();
        let expired_count = entries.values().filter(|e| e.expires_at <= now).count();
        stats.insert("expired_entries".to_string(), Value::from(expired_count));
        
        stats
    }
    
    // Private helper methods
    
    fn create_key(&self, query: &str, tools: &[Tool]) -> CacheKey {
        // Create a normalized query for better cache hits
        let normalized_query = self.normalize_query(query);
        
        // Create a hash of tool names - if tools change, cache is invalidated
        let mut tools_hash = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        
        let mut tool_names: Vec<&str> = tools.iter()
            .map(|t| t.name.as_str())
            .collect();
        
        // Sort to ensure consistent hash regardless of order
        tool_names.sort();
        
        for name in tool_names {
            name.hash(&mut tools_hash);
        }
        
        CacheKey {
            query: normalized_query,
            tools_hash: tools_hash.finish(),
        }
    }
    
    fn normalize_query(&self, query: &str) -> String {
        // Basic normalization to improve cache hits
        let mut normalized = query.to_lowercase();
        normalized = normalized.trim().to_string();
        
        // Remove extra whitespace
        let whitespace_regex = regex::Regex::new(r"\s+").unwrap();
        normalized = whitespace_regex.replace_all(&normalized, " ").to_string();
        
        normalized
    }
    
    fn cleanup_expired(&self, entries: &mut HashMap<CacheKey, CacheEntry>) {
        let now = Instant::now();
        let expired_keys: Vec<CacheKey> = entries
            .iter()
            .filter(|(_, entry)| entry.expires_at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in expired_keys {
            entries.remove(&key);
        }
    }
    
    fn remove_oldest(&self, entries: &mut HashMap<CacheKey, CacheEntry>) {
        if let Some(oldest_key) = entries
            .iter()
            .min_by_key(|(_, entry)| entry.expires_at)
            .map(|(k, _)| k.clone())
        {
            entries.remove(&oldest_key);
            debug!("Removed oldest cache entry to make room");
        }
    }
} 
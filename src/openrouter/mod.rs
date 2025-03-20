use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Mutex;
use mcp_core::Tool;

#[derive(Debug, Clone)]
pub struct OpenRouterClient {
    api_key: String,
    client: Client,
    base_url: String,
    last_request_time: Arc<Mutex<Option<Instant>>>,
    min_request_interval: Duration, // Minimum time between requests to avoid rate limiting
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize, Debug)]
pub struct ChatCompletionChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Deserialize, Debug)]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

// Custom error type for OpenRouter client
#[derive(Debug, thiserror::Error)]
pub enum OpenRouterError {
    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl OpenRouterClient {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_default();
        
        Self {
            api_key,
            client,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            last_request_time: Arc::new(Mutex::new(None)),
            min_request_interval: Duration::from_millis(1000), // 1 second minimum between requests
        }
    }
    
    async fn throttle(&self) -> Result<(), OpenRouterError> {
        let mut last_request_time = self.last_request_time.lock().await;
        
        if let Some(last_time) = *last_request_time {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_request_interval {
                let wait_time = self.min_request_interval - elapsed;
                tokio::time::sleep(wait_time).await;
            }
        }
        
        *last_request_time = Some(Instant::now());
        Ok(())
    }
    
    pub async fn chat_completion(
        &self, 
        model: &str, 
        messages: Vec<ChatMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<ChatCompletionResponse, OpenRouterError> {
        // Throttle requests to avoid rate limiting
        self.throttle().await?;
        
        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            temperature,
            max_tokens,
            stream: Some(false),
        };
        
        let response = self.client
            .post(&format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://mdesk.app") // Identifying the application
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            return Err(match status.as_u16() {
                429 => OpenRouterError::RateLimitExceeded,
                _ => OpenRouterError::ApiError(format!("HTTP {}: {}", status, error_text)),
            });
        }
        
        let completion: ChatCompletionResponse = response.json().await?;
        Ok(completion)
    }
    
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, OpenRouterError> {
        // Throttle requests to avoid rate limiting
        self.throttle().await?;
        
        let response = self.client
            .get(&format!("{}/models", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://mdesk.app") // Identifying the application
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            
            return Err(match status.as_u16() {
                429 => OpenRouterError::RateLimitExceeded,
                _ => OpenRouterError::ApiError(format!("HTTP {}: {}", status, error_text)),
            });
        }
        
        let models: ModelListResponse = response.json().await?;
        Ok(models.data)
    }
}

// Tool selection algorithm
pub struct ToolSelector;

impl ToolSelector {
    pub fn new() -> Self {
        Self
    }
    
    pub fn select_tools(&self, query: &str, available_tools: &[Tool]) -> Vec<Tool> {
        // Simple keyword matching approach for initial implementation
        let query_lower = query.to_lowercase();
        let mut selected_tools = Vec::new();
        
        for tool in available_tools {
            // Check tool name and description for relevance
            if tool.name.to_lowercase().contains(&query_lower) || 
               tool.description.to_lowercase().contains(&query_lower) {
                selected_tools.push(tool.clone());
            }
        }
        
        // We could enhance this later with:
        // - Embedding similarity
        // - Intent classification
        // - Parameter matching
        
        selected_tools
    }
}

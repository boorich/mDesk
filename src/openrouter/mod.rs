use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::{sync::Arc, time::{Duration, Instant}};
use tokio::sync::Mutex;
use mcp_core::Tool;
use tracing::{debug, info, warn, error, instrument};

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

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_length: Option<usize>,
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelPricing {
    pub prompt: Option<f64>,
    pub completion: Option<f64>,
}

// Custom deserialization for ModelPricing to handle both string and numeric values
impl<'de> serde::Deserialize<'de> for ModelPricing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default, deserialize_with = "deserialize_string_or_number")]
            prompt: Option<f64>,
            #[serde(default, deserialize_with = "deserialize_string_or_number")]
            completion: Option<f64>,
        }

        let helper = Helper::deserialize(deserializer)?;
        Ok(ModelPricing {
            prompt: helper.prompt,
            completion: helper.completion,
        })
    }
}

// Helper function to deserialize strings or numbers into f64
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> serde::de::Visitor<'de> for StringOrNumberVisitor {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or number")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            match value.parse::<f64>() {
                Ok(num) => Ok(Some(num)),
                Err(_) => {
                    // For strings like "0", "0.0" etc.
                    if value == "0" || value == "0.0" {
                        Ok(Some(0.0))
                    } else {
                        // For other unparseable strings, just return None
                        Ok(None)
                    }
                }
            }
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value as f64))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(value as f64))
        }
        
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
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

// Credit balance response
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct CreditBalanceResponse {
    pub data: CreditBalanceData,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct CreditBalanceData {
    pub total_credits: f64,
    pub total_usage: f64,
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
    
    #[instrument(level = "debug", skip(self))]
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
    
    #[instrument(level = "debug", skip(self, messages), fields(model = model, msg_count = messages.len(), max_tokens = ?max_tokens))]
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
    
    #[instrument(level = "debug", skip(self))]
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
        
        // Get the raw response body as a string first for debugging
        let body_text = response.text().await?;
        
        // Try to parse the JSON response
        match serde_json::from_str::<ModelListResponse>(&body_text) {
            Ok(models) => Ok(models.data),
            Err(e) => {
                // Log the error with more context
                error!("JSON deserialization error: {} in response: {}", e, body_text);
                
                // Try to extract just the model IDs and names as a fallback
                let fallback_result = serde_json::from_str::<serde_json::Value>(&body_text);
                if let Ok(value) = fallback_result {
                    if let Some(data) = value.get("data").and_then(|d| d.as_array()) {
                        let fallback_models = data.iter()
                            .filter_map(|item| {
                                let id = item.get("id")?.as_str()?.to_string();
                                let name = item.get("name")?.as_str()?.to_string();
                                Some(ModelInfo {
                                    id,
                                    name,
                                    description: None,
                                    context_length: None,
                                    pricing: None,
                                })
                            })
                            .collect::<Vec<_>>();
                        
                        if !fallback_models.is_empty() {
                            warn!("Using fallback model parsing, recovered {} models", fallback_models.len());
                            return Ok(fallback_models);
                        }
                    }
                }
                
                // Failed to parse with fallback approach too, return a custom error
                Err(OpenRouterError::Unknown(format!("JSON deserialization error: {}", e)))
            }
        }
    }
    
    #[instrument(level = "debug", skip(self))]
    pub async fn get_credit_balance(&self) -> Result<CreditBalanceResponse, OpenRouterError> {
        // Throttle requests to avoid rate limiting
        self.throttle().await?;
        
        let response = self.client
            .get(&format!("{}/credits", self.base_url))
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
        
        let balance: CreditBalanceResponse = response.json().await?;
        Ok(balance)
    }
}

// Tool selection algorithm
pub struct ToolSelector;

impl ToolSelector {
    pub fn new() -> Self {
        Self
    }
    
    #[instrument(level = "debug", skip(self, available_tools), fields(query_len = query.len(), tools_count = available_tools.len()))]
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

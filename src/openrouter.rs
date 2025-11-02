use anyhow::Result;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use tracing::debug;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Message role in the chat conversation
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Provider sorting preference for OpenRouter
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProviderSort {
    Price,
    Throughput,
    Latency,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderPrefs>,
}

#[derive(Debug, Serialize)]
pub struct ProviderPrefs {
    pub sort: ProviderSort,
    // (optionally expose more fields later: order, only, ignore, allow_fallbacks, etc.)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub model: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

pub struct OpenRouterClient {
    client: reqwest::Client,
    api_key: SecretString,
}

impl OpenRouterClient {
    pub fn new(api_key: SecretString) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        // Log the request in a more readable format
        debug!("OpenRouter API request:");
        debug!("  Model: {}", request.model);
        if let Some(ref provider) = request.provider {
            debug!("  Provider sort: {}", provider.sort);
        }
        debug!("  Messages:");
        for (i, msg) in request.messages.iter().enumerate() {
            debug!("    [{}] Role: {}", i, msg.role);
            debug!("    [{}] Content:\n{}", i, msg.content);
        }

        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key.expose_secret()),
            )
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?
            .json::<ChatCompletionResponse>()
            .await?;

        Ok(response)
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatRequest {
    pub url: String,
    pub api_key: String,
    pub model: String,
    pub messages: Vec<AiMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<AiMessage>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: Option<String>,
}

#[tauri::command]
pub async fn ai_chat(request: AiChatRequest) -> Result<String, String> {
    tracing::info!("ai_chat called: model={}, messages={}", request.model, request.messages.len());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = OpenRouterRequest {
        model: request.model,
        messages: request.messages,
    };

    let res = client
        .post(&request.url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", request.api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("HTTP request failed: {}", e);
            format!("HTTP request failed: {}", e)
        })?;

    let status = res.status();
    tracing::info!("ai_chat response status: {}", status);

    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        tracing::error!("ai_chat API error {}: {}", status.as_u16(), text);
        return Err(format!("API error {}: {}", status.as_u16(), text));
    }

    let raw = res.text().await.map_err(|e| {
        tracing::error!("Failed to read response body: {}", e);
        format!("Failed to read response body: {}", e)
    })?;

    tracing::info!("ai_chat response length: {} bytes", raw.len());

    let data: OpenRouterResponse = serde_json::from_str(&raw).map_err(|e| {
        tracing::error!("Failed to parse response: {} — raw: {}", e, &raw[..raw.len().min(500)]);
        format!("Failed to parse response: {}", e)
    })?;

    if let Some(err) = data.error {
        let msg = err.message.unwrap_or_else(|| "Unknown API error".to_string());
        tracing::error!("ai_chat API returned error: {}", msg);
        return Err(msg);
    }

    let content = data
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .unwrap_or_default();

    if content.trim().is_empty() {
        tracing::error!("ai_chat returned empty content");
        return Err("Empty response from AI model".to_string());
    }

    tracing::info!("ai_chat success: {} chars", content.len());
    Ok(content)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiFetchModelsRequest {
    pub api_key: String,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub context_length: u64,
    pub pricing: ModelPricing,
}

#[derive(Debug, Serialize)]
pub struct ModelPricing {
    pub prompt: String,
    pub completion: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    name: Option<String>,
    description: Option<String>,
    context_length: Option<u64>,
    pricing: Option<ModelEntryPricing>,
}

#[derive(Debug, Deserialize)]
struct ModelEntryPricing {
    prompt: Option<String>,
    completion: Option<String>,
}

#[tauri::command]
pub async fn ai_fetch_models(request: AiFetchModelsRequest) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let res = client
        .get("https://openrouter.ai/api/v1/models")
        .header("Authorization", format!("Bearer {}", request.api_key))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }

    let data: ModelsResponse = res.json().await.map_err(|e| format!("Parse error: {}", e))?;

    Ok(data
        .data
        .into_iter()
        .map(|m| ModelInfo {
            id: m.id,
            name: m.name.unwrap_or_default(),
            description: m.description.unwrap_or_default(),
            context_length: m.context_length.unwrap_or(0),
            pricing: ModelPricing {
                prompt: m.pricing.as_ref().and_then(|p| p.prompt.clone()).unwrap_or_else(|| "0".to_string()),
                completion: m.pricing.as_ref().and_then(|p| p.completion.clone()).unwrap_or_else(|| "0".to_string()),
            },
        })
        .collect())
}

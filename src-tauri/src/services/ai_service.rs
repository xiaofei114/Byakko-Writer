use reqwest::Client;
use serde_json::json;
use crate::models::{AIConfig, ChatMessage, AIApiResponse, AiDecision, ToolDef};

/// 构建完整的 API URL
fn build_api_url(config: &AIConfig) -> String {
    if config.api_url.ends_with("/chat/completions") {
        config.api_url.clone()
    } else if config.api_url.ends_with("/") {
        format!("{}v1/chat/completions", config.api_url)
    } else if config.api_url.ends_with("/v1") {
        format!("{}/chat/completions", config.api_url)
    } else {
        format!("{}/v1/chat/completions", config.api_url)
    }
}

/// 调用 AI 生成摘要（max_tokens 默认 2000，可通过 override 指定）
pub async fn call_ai_for_summary(
    prompt: &str,
    system_prompt: &str,
    config: &AIConfig,
) -> anyhow::Result<String> {
    call_ai_with_max_tokens(prompt, system_prompt, config, 2000).await
}

/// 调用 AI 生成大容量摘要（用于 Story Bible 等需要大量输出的场景）
pub async fn call_ai_for_large_summary(
    prompt: &str,
    system_prompt: &str,
    config: &AIConfig,
    max_tokens: i32,
) -> anyhow::Result<String> {
    call_ai_with_max_tokens(prompt, system_prompt, config, max_tokens).await
}

async fn call_ai_with_max_tokens(
    prompt: &str,
    system_prompt: &str,
    config: &AIConfig,
    max_tokens: i32,
) -> anyhow::Result<String> {
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置"));
    }

    let api_url = build_api_url(config);
    let client = Client::new();

    let request_body = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.3,
        "max_tokens": max_tokens,
        "response_format": { "type": "json_object" }
    });

    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
    }

    let ai_response: AIApiResponse = response.json().await?;

    ai_response
        .choices
        .get(0)
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
}

/// 非流式发送 AI 消息（用于Agent内部调用，支持 function calling）
pub async fn send_ai_message(
    history: Vec<ChatMessage>,
    config: AIConfig,
    tools: Option<&[ToolDef]>,
) -> anyhow::Result<AiDecision> {
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置"));
    }

    let api_url = build_api_url(&config);
    let client = Client::new();

    let mut request_body = json!({
        "model": config.model,
        "messages": history,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "stream": false
    });

    if let Some(t) = tools {
        if !t.is_empty() {
            request_body["tools"] = json!(t);
            request_body["tool_choice"] = json!("auto");
        }
    }

    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
    }

    let ai_response: AIApiResponse = response.json().await?;

    let choice = ai_response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))?;

    Ok(AiDecision {
        content: choice.message.content.filter(|s| !s.is_empty()),
        tool_calls: choice.message.tool_calls.filter(|t| !t.is_empty()),
    })
}

/// 流式发送 AI 消息
pub async fn send_ai_message_stream(
    history: Vec<ChatMessage>,
    config: AIConfig,
) -> anyhow::Result<reqwest::Response> {
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置"));
    }

    let api_url = build_api_url(&config);
    let client = Client::new();

    let request_body = json!({
        "model": config.model,
        "messages": history,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "stream": true
    });

    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
    }

    Ok(response)
}

/// 调用 AI 进行风格分析
pub async fn call_ai_for_style_analysis(
    prompt: &str,
    system_prompt: &str,
) -> anyhow::Result<String> {
    // 获取 AI 配置
    let config = crate::services::config_service::load_config()?.ai;
    
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置，无法分析写作风格"));
    }

    let api_url = build_api_url(&config);
    let client = Client::new();

    let request_body = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.3,
        "max_tokens": 4000,
        "response_format": { "type": "json_object" }
    });

    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
    }

    let ai_response: AIApiResponse = response.json().await?;

    ai_response
        .choices
        .get(0)
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
}

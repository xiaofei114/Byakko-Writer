use reqwest::Client;
use serde_json::json;
use crate::models::{AIConfig, ChatMessage, AIApiResponse};

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

/// 发送 AI 消息（非流式）
pub async fn send_ai_message(
    message: String,
    history: Vec<ChatMessage>,
    config: AIConfig,
) -> anyhow::Result<String> {
    // 检查 API Key
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置，请在设置中配置 AI"));
    }
    
    let api_url = build_api_url(&config);
    
    log::info!("AI 请求 URL: {}", api_url);
    log::info!("AI 模型: {}", config.model);
    
    let client = Client::new();
    
    let mut messages = history;
    messages.push(ChatMessage::new("user", &message));
    
    let request_body = json!({
        "model": config.model,
        "messages": messages,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens
    });
    
    log::debug!("AI 请求体: {:?}", request_body);
    
    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;
    
    let status = response.status();
    log::info!("AI 响应状态: {}", status);
    
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        log::error!("AI 错误响应: {}", error_text);
        return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
    }
    
    let ai_response: AIApiResponse = response.json().await?;
    
    ai_response
        .choices
        .get(0)
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
}

/// 润色文本
pub async fn polish_text(
    text: &str,
    config: crate::models::AIConfig,
) -> anyhow::Result<String> {
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置"));
    }

    let api_url = build_api_url(&config);
    let client = Client::new();

    let system_prompt = r#"你是一位专业的小说编辑，擅长润色中文小说文本。

你的任务是对用户提供的文本进行润色，使其：
1. 语言更加流畅自然
2. 描写更加生动形象
3. 节奏更加紧凑有力
4. 保持原有的情节和人物设定不变
5. 保持原有的写作风格

请直接返回润色后的文本，不要添加任何解释或说明。"#;

    let user_prompt = format!("请润色以下文本：\n\n{}", text);

    let request_body = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_prompt
            }
        ],
        "temperature": 0.7,
        "max_tokens": 4000
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
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
}

/// 调用 AI 生成摘要
pub async fn call_ai_for_summary(
    prompt: &str,
    system_prompt: &str,
    config: &AIConfig,
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
        "max_tokens": 2000,
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
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
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
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow::anyhow!("AI 未返回内容"))
}

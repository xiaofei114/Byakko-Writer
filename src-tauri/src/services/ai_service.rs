use reqwest::Client;
use serde_json::json;
use crate::models::{AIConfig, ChatMessage, AIApiResponse, AiDecision, ToolDef};
use std::time::Duration;

/// 最大重试次数（网络错误/超时时重试）
const MAX_RETRIES: u32 = 2;
/// 普通请求超时（秒）
const REQUEST_TIMEOUT_SECS: u64 = 90;
/// 流式请求超时（秒，写一整章可能输出几千字，需要更久）
const STREAM_TIMEOUT_SECS: u64 = 300;

/// 构建带超时配置的 HTTP 客户端
fn build_client(timeout_secs: u64) -> Client {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP 客户端构建失败")
}

/// 判断错误是否可重试
///
/// 可重试：网络超时/连接失败、限流（429）、服务端 5xx、模型过载。
/// 不可重试：401/403（Key 或权限问题）、400（参数错）、404 等。
fn is_retryable_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();

    msg.contains("timeout")
        || msg.contains("timed out")
        || msg.contains("connection refused")
        || msg.contains("connection reset")
        || msg.contains("connection aborted")
        || msg.contains("dns error")
        || msg.contains("error sending request")
        || msg.contains("api错误 (429")
        || msg.contains("api错误 (500")
        || msg.contains("api错误 (502")
        || msg.contains("api错误 (503")
        || msg.contains("api错误 (504")
        || msg.contains("rate limit")
        || msg.contains("too many requests")
        || msg.contains("overloaded")
}

/// 带重试的请求执行器
async fn with_retry<F, Fut, T>(f: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut last_err = None;
    for attempt in 0..=MAX_RETRIES {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if is_retryable_error(&e) && attempt < MAX_RETRIES {
                    // 1s、2s 退避；限流/5xx 给服务端一点恢复时间
                    let delay = Duration::from_millis(1000 * 2u64.pow(attempt));
                    log::warn!("[AI] 请求失败（可重试），{} 秒后重试: {}", attempt + 1, e);
                    tokio::time::sleep(delay).await;
                    last_err = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("未知错误")))
}

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
    let client = build_client(REQUEST_TIMEOUT_SECS);
    let api_key = config.api_key.clone();

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

    with_retry(|| {
        let client = &client;
        let api_url = &api_url;
        let api_key = &api_key;
        let request_body = &request_body;
        async move {
            let response = client
                .post(api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(request_body)
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
    }).await
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
    let client = build_client(REQUEST_TIMEOUT_SECS);
    let api_key = config.api_key.clone();

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

    with_retry(|| {
        let client = &client;
        let api_url = &api_url;
        let api_key = &api_key;
        let request_body = &request_body;
        async move {
            let response = client
                .post(api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(request_body)
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
                reasoning: choice.message.reasoning_content.filter(|s| !s.trim().is_empty()),
            })
        }
    }).await
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
    let client = build_client(STREAM_TIMEOUT_SECS);
    let api_key = config.api_key.clone();

    let request_body = json!({
        "model": config.model,
        "messages": history,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "stream": true
    });

    with_retry(|| {
        let client = &client;
        let api_url = &api_url;
        let api_key = &api_key;
        let request_body = &request_body;
        async move {
            let response = client
                .post(api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(request_body)
                .send()
                .await?;

            let status = response.status();
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("API错误 ({}): {}", status, error_text));
            }

            Ok(response)
        }
    }).await
}

/// 调用 AI 进行风格分析
pub async fn call_ai_for_style_analysis(
    prompt: &str,
    system_prompt: &str,
) -> anyhow::Result<String> {
    let config = crate::services::config_service::load_config()?.ai;
    
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("API Key 未配置，无法分析写作风格"));
    }

    let api_url = build_api_url(&config);
    let client = build_client(REQUEST_TIMEOUT_SECS);
    let api_key = config.api_key.clone();

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

    with_retry(|| {
        let client = &client;
        let api_url = &api_url;
        let api_key = &api_key;
        let request_body = &request_body;
        async move {
            let response = client
                .post(api_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(request_body)
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
    }).await
}

/// 流式增量（正文 / 推理内容分开）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseDelta {
    pub content: String,
    pub reasoning: String,
}

/// 从流式响应块中提取增量文本（正文 + 推理内容）
///
/// 网络分块可能把一行 `data: {...}` 从中间切断，如果按块独立解析会丢字、丢句。
/// 这里把数据追加到调用方持有的缓冲区，按 SSE 事件（空行分隔）逐个解析。
///
/// 调用方在流结束时再喂一个 `b"\n\n"`，即可冲刷最后一个没有空行结尾的事件。
pub fn extract_sse_deltas_full(buffer: &mut String, chunk: &[u8]) -> Vec<SseDelta> {
    buffer.push_str(&String::from_utf8_lossy(chunk));

    let mut deltas = Vec::new();

    loop {
        let separator = match buffer.find("\n\n") {
            Some(pos) => Some((pos, 2usize)),
            None => buffer.find("\r\n\r\n").map(|pos| (pos, 4usize)),
        };

        let Some((pos, sep_len)) = separator else {
            break;
        };

        let event: String = buffer[..pos].to_string();
        // pos 一定位于换行符边界，drain 不会切断 UTF-8 字符
        buffer.drain(..pos + sep_len);

        for line in event.lines() {
            let line = line.trim_end_matches('\r');
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim_start();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                let delta = &json["choices"][0]["delta"];

                let content = delta["content"].as_str().unwrap_or("");
                let reasoning = delta["reasoning_content"]
                    .as_str()
                    .or_else(|| delta["reasoning"].as_str())
                    .unwrap_or("");

                if !content.is_empty() || !reasoning.is_empty() {
                    deltas.push(SseDelta {
                        content: content.to_string(),
                        reasoning: reasoning.to_string(),
                    });
                }
            }
        }
    }

    deltas
}

/// 只取正文增量（写作/润色用）
pub fn extract_sse_deltas(buffer: &mut String, chunk: &[u8]) -> Vec<String> {
    extract_sse_deltas_full(buffer, chunk)
        .into_iter()
        .filter_map(|delta| {
            if delta.content.is_empty() {
                None
            } else {
                Some(delta.content)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{extract_sse_deltas, extract_sse_deltas_full};

    #[test]
    fn parses_reasoning_delta() {
        let mut buffer = String::new();
        let deltas = extract_sse_deltas_full(
            &mut buffer,
            b"data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\n",
        );

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].reasoning, "think");
        assert!(deltas[0].content.is_empty());
    }

    #[test]
    fn parses_single_event() {
        let mut buffer = String::new();
        let deltas = extract_sse_deltas(
            &mut buffer,
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        );
        assert_eq!(deltas, vec!["hi".to_string()]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn handles_split_across_chunks() {
        let mut buffer = String::new();
        let first = extract_sse_deltas(
            &mut buffer,
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hel",
        );
        assert!(first.is_empty());

        let second = extract_sse_deltas(
            &mut buffer,
            b"lo\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"!\"}}]}\n\n",
        );
        assert_eq!(second, vec!["hello".to_string(), "!".to_string()]);
    }

    #[test]
    fn ignores_done_and_non_data_lines() {
        let mut buffer = String::new();
        let deltas = extract_sse_deltas(
            &mut buffer,
            b": keep-alive\n\ndata: [DONE]\n\ndata: {\"choices\":[{\"delta\":{}}]}\n\n",
        );
        assert!(deltas.is_empty());
    }

    #[test]
    fn flushes_trailing_event_without_blank_line() {
        let mut buffer = String::new();
        let first = extract_sse_deltas(
            &mut buffer,
            b"data: {\"choices\":[{\"delta\":{\"content\":\"tail\"}}]}",
        );
        assert!(first.is_empty());
        let flushed = extract_sse_deltas(&mut buffer, b"\n\n");
        assert_eq!(flushed, vec!["tail".to_string()]);
    }
}

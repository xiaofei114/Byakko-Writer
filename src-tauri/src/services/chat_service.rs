use crate::db::get_pool;
use crate::models::{AIConfig, ChatMessage, StreamEvent};
use crate::services::ai_service::send_ai_message_stream;
use crate::services::prompt_service::get_prompt_manager;
use crate::services::tool_call_service::{parse_tool_calls, execute_tool_call, contains_tool_calls};
use futures::StreamExt;
use sqlx::Row;
use tauri::{AppHandle, Emitter};

/// 获取所有章节摘要
async fn get_all_chapter_summaries(book_id: &str) -> anyhow::Result<Vec<(String, String, String)>> {
    let pool = get_pool().await?;
    
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.title, s.short_summary
        FROM chapters c
        LEFT JOIN chapter_summaries s ON c.id = s.chapter_id
        WHERE c.book_id = ?
        ORDER BY c.created_at ASC
        "#
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    
    let mut summaries = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let summary: Option<String> = row.try_get("short_summary")?;
        summaries.push((id, title, summary.unwrap_or_default()));
    }
    
    Ok(summaries)
}

/// 保存用户消息
pub async fn save_user_message(
    session_id: &str,
    book_id: &str,
    chapter_id: Option<&str>,
    content: &str,
) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp_millis();
    let message_id = format!("msg_{}_{}", now, rand::random::<u16>());
    
    sqlx::query(
        r#"
        INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp)
        VALUES (?1, ?2, ?3, ?4, 'user', ?5, ?6)
        "#
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(book_id)
    .bind(chapter_id)
    .bind(content)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(message_id)
}

/// 保存助手消息
pub async fn save_assistant_message(
    session_id: &str,
    book_id: &str,
    chapter_id: Option<&str>,
    content: &str,
) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp_millis();
    let message_id = format!("msg_{}_{}", now, rand::random::<u16>());
    
    sqlx::query(
        r#"
        INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp)
        VALUES (?1, ?2, ?3, ?4, 'assistant', ?5, ?6)
        "#
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(book_id)
    .bind(chapter_id)
    .bind(content)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(message_id)
}

/// 保存工具消息
pub async fn save_tool_message(
    session_id: &str,
    book_id: &str,
    chapter_id: Option<&str>,
    tool_name: &str,
    content: &str,
) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp_millis();
    let message_id = format!("msg_tool_{}_{}", now, rand::random::<u16>());
    
    // 工具名称映射（中文显示名）
    let tool_display_name = match tool_name {
        "query_chapter_summary" => "查询章节摘要",
        "query_chapter_content" => "查询章节内容",
        "list_all_chapters" => "列出所有章节",
        "get_outline" => "获取大纲",
        "save_outline" => "保存大纲",
        "list_character_cards" => "列出角色卡",
        "create_character_card" => "创建角色卡",
        "update_character_card" => "更新角色卡",
        "get_writing_style" => "获取写作风格",
        "analyze_writing_style" => "分析写作风格",
        "learn_writing_style" => "学习写作风格",
        _ => tool_name,
    };
    
    let full_content = format!("[工具调用: {}]\n{}", tool_display_name, content);
    
    sqlx::query(
        r#"
        INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, context_type, timestamp)
        VALUES (?1, ?2, ?3, ?4, 'tool', ?5, ?6, ?7)
        "#
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(book_id)
    .bind(chapter_id)
    .bind(&full_content)
    .bind(tool_display_name)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(message_id)
}

/// 获取或创建会话
async fn get_or_create_session(
    session_id: Option<String>,
    book_id: &str,
    chapter_id: Option<&str>,
    title: &str,
) -> anyhow::Result<String> {
    if let Some(id) = session_id {
        let pool = get_pool().await?;
        let now = chrono::Utc::now().timestamp_millis();
        
        sqlx::query(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE session_id = ?2"
        )
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await?;
        
        return Ok(id);
    }
    
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp_millis();
    let new_session_id = format!("session_{}_{}", now, rand::random::<u16>());
    
    sqlx::query(
        r#"
        INSERT INTO chat_sessions (session_id, book_id, chapter_id, title, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#
    )
    .bind(&new_session_id)
    .bind(book_id)
    .bind(chapter_id)
    .bind(title)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(new_session_id)
}

/// 构建系统提示词
pub fn build_system_prompt(book_id: &str, chapter_summaries: &[(String, String, String)]) -> String {
    let mut chapter_list = String::new();
    for (id, title, summary) in chapter_summaries {
        chapter_list.push_str(&format!("- [{}] {}: {}\n", id, title, summary));
    }
    
    let prompt_manager = get_prompt_manager();
    let mut prompt = prompt_manager.get_full_system_prompt(&chapter_list);
    
    prompt.push_str("\n\n## 当前上下文\n\n");
    prompt.push_str(&format!("当前书籍ID: {}\n", book_id));
    prompt.push_str("使用工具时，请使用上述书籍ID作为 bookId 参数。\n");
    
    prompt
}

/// 发送流式聊天消息（支持工具调用）
pub async fn send_chat_message_stream(
    app: AppHandle,
    session_id: Option<String>,
    book_id: String,
    chapter_id: Option<String>,
    message: String,
    config: AIConfig,
) -> anyhow::Result<String> {
    let session_title: String = message.chars().take(20).collect();
    let session_id = get_or_create_session(
        session_id,
        &book_id,
        chapter_id.as_deref(),
        &session_title,
    ).await?;

    save_user_message(&session_id, &book_id, chapter_id.as_deref(), &message).await?;

    let chapter_summaries = get_all_chapter_summaries(&book_id).await?;
    let mut system_prompt = build_system_prompt(&book_id, &chapter_summaries);

    if let Ok(style_result) = crate::services::style_service::get_style_prompt(Some(&book_id)).await {
        if style_result.is_enabled && !style_result.style_prompt.is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&style_result.style_prompt);
        }
    }

    let history = vec![ChatMessage::new("system", &system_prompt)];

    let _final_response = execute_conversation_with_tools(
        &app,
        &session_id,
        &book_id,
        chapter_id.as_deref(),
        message,
        history,
        config,
    ).await?;
    
    Ok(session_id)
}

/// 执行带工具调用的对话
async fn execute_conversation_with_tools(
    app: &AppHandle,
    session_id: &str,
    book_id: &str,
    chapter_id: Option<&str>,
    user_message: String,
    mut history: Vec<ChatMessage>,
    config: AIConfig,
) -> anyhow::Result<String> {
    history.push(ChatMessage::new("user", &user_message));
    
    let mut max_iterations = 10;
    let mut final_response = String::new();
    
    while max_iterations > 0 {
        max_iterations -= 1;

        // 使用流式请求
        let response = send_ai_message_stream(
            history.clone(),
            config.clone(),
        ).await?;

        let mut stream = response.bytes_stream();
        let mut ai_content = String::new();
        let mut buffer = String::new();
        let mut tool_call_detected = false;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                continue;
                            }

                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                    ai_content.push_str(content);

                                    // 如果还没有检测到工具调用，继续缓冲和发送
                                    if !tool_call_detected {
                                        buffer.push_str(content);

                                        // 检查是否包含工具调用开始标记（更精确的匹配）
                                        // 匹配 <tool 或 <tool name= 格式
                                        if buffer.contains("<tool ") || buffer.contains("<tool>") {
                                            tool_call_detected = true;

                                            // 发送工具调用前的文本
                                            if let Some(pos) = ai_content.find("<tool") {
                                                let before_tool = &ai_content[..pos];
                                                if !before_tool.is_empty() {
                                                    let event = StreamEvent {
                                                        session_id: session_id.to_string(),
                                                        chunk: before_tool.to_string(),
                                                        is_complete: false,
                                                        is_tool_call: false,
                                                        tool_name: None,
                                                        tool_display_name: None,
                                                        tool_params: None,
                                                    };
                                                    let _ = app.emit("ai-chat-stream", event);
                                                }
                                            }
                                        } else if buffer.len() > 200 {
                                            // 限制缓冲区大小，正常流式输出
                                            let event = StreamEvent {
                                                session_id: session_id.to_string(),
                                                chunk: buffer.clone(),
                                                is_complete: false,
                                                is_tool_call: false,
                                                tool_name: None,
                                                tool_display_name: None,
                                                tool_params: None,
                                            };
                                            let _ = app.emit("ai-chat-stream", event);
                                            buffer.clear();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[AI Chat] 流读取错误: {}", e);
                    break;
                }
            }
        }
        
        // 发送缓冲区剩余内容（如果没有工具调用）
        if !tool_call_detected && !buffer.is_empty() {
            let event = StreamEvent {
                session_id: session_id.to_string(),
                chunk: buffer.clone(),
                is_complete: false,
                is_tool_call: false,
                tool_name: None,
                tool_display_name: None,
                tool_params: None,
            };
            let _ = app.emit("ai-chat-stream", event);
        }
        
        // 如果检测到工具调用但内容为空，跳过本轮
        if tool_call_detected && ai_content.is_empty() {
            log::warn!("[AI Chat] 检测到工具调用但内容为空，跳过本轮");
            continue;
        }
        
        // 检查是否包含工具调用
        if contains_tool_calls(&ai_content) {
            let tool_calls = parse_tool_calls(&ai_content);

            if tool_calls.is_empty() {
                // 检测到工具调用但解析失败，可能是格式错误
                // 告诉 AI 格式错误，让它重试
                let error_msg = "注意：你尝试调用工具，但格式不正确。请使用正确的 XML 格式：<tool name=\"工具名\">{\"参数\": \"值\"}</tool>".to_string();

                let event = StreamEvent {
                    session_id: session_id.to_string(),
                    chunk: "\n[系统提示：工具调用格式错误，请使用正确格式重试]\n".to_string(),
                    is_complete: false,
                    is_tool_call: false,
                    tool_name: None,
                    tool_display_name: None,
                    tool_params: None,
                };
                let _ = app.emit("ai-chat-stream", event);

                history.push(ChatMessage::new("assistant", &ai_content));
                history.push(ChatMessage::new("user", &error_msg));

                continue;
            }

            if !tool_calls.is_empty() {
                save_assistant_message(session_id, book_id, chapter_id, &ai_content).await?;
                history.push(ChatMessage::new("assistant", &ai_content));

                let mut tool_results = Vec::new();

                for tool_call in &tool_calls {
                    
                    // 工具名称映射（中文显示名）
                    let tool_display_name = match tool_call.name.as_str() {
                        "query_chapter_summary" => "查询章节摘要",
                        "query_chapter_content" => "查询章节内容",
                        "list_all_chapters" => "列出所有章节",
                        "get_outline" => "获取大纲",
                        "save_outline" => "保存大纲",
                        "list_character_cards" => "列出角色卡",
                        "create_character_card" => "创建角色卡",
                        "update_character_card" => "更新角色卡",
                        "get_writing_style" => "获取写作风格",
                        "analyze_writing_style" => "分析写作风格",
                        "learn_writing_style" => "学习写作风格",
                        _ => &tool_call.name,
                    }.to_string();
                    
                    // 提取关键参数用于显示（根据工具类型决定显示什么）
                    let mut display_params = serde_json::Map::new();
                    
                    match tool_call.name.as_str() {
                        "query_chapter_summary" | "query_chapter_content" => {
                            // 查询章节相关工具 - 显示章节标题
                            if let Some(chapter_id) = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()) {
                                if let Ok(pool) = get_pool().await {
                                    if let Ok(row) = sqlx::query("SELECT title FROM chapters WHERE id = ?")
                                        .bind(chapter_id)
                                        .fetch_one(pool)
                                        .await
                                    {
                                        if let Ok(title) = row.try_get::<String, _>("title") {
                                            display_params.insert("章节".to_string(), serde_json::json!(title));
                                        }
                                    }
                                }
                            }
                        }
                        "get_outline" | "save_outline" => {
                            // 大纲相关工具 - 显示章节和类型
                            if let Some(chapter_id) = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()) {
                                if let Ok(pool) = get_pool().await {
                                    if let Ok(row) = sqlx::query("SELECT title FROM chapters WHERE id = ?")
                                        .bind(chapter_id)
                                        .fetch_one(pool)
                                        .await
                                    {
                                        if let Ok(title) = row.try_get::<String, _>("title") {
                                            display_params.insert("章节".to_string(), serde_json::json!(title));
                                        }
                                    }
                                }
                            }
                            if let Some(outline_type) = tool_call.arguments.get("outlineType").and_then(|v| v.as_str()) {
                                let type_name = match outline_type {
                                    "coarse" => "粗纲",
                                    "fine" => "细纲",
                                    _ => outline_type,
                                };
                                display_params.insert("类型".to_string(), serde_json::json!(type_name));
                            }
                        }
                        "list_all_chapters" => {
                            // 列出所有章节 - 不需要额外参数
                        }
                        "list_character_cards" | "create_character_card" | "update_character_card" => {
                            // 角色卡相关 - 显示角色名
                            if let Some(name) = tool_call.arguments.get("name").and_then(|v| v.as_str()) {
                                display_params.insert("角色".to_string(), serde_json::json!(name));
                            }
                        }
                        "learn_writing_style" => {
                            // 学习写作风格 - 显示书籍信息
                            if let Some(book_id) = tool_call.arguments.get("bookId").and_then(|v| v.as_str()) {
                                if let Ok(pool) = get_pool().await {
                                    if let Ok(row) = sqlx::query("SELECT title FROM books WHERE id = ?")
                                        .bind(book_id)
                                        .fetch_one(pool)
                                        .await
                                    {
                                        if let Ok(title) = row.try_get::<String, _>("title") {
                                            display_params.insert("书籍".to_string(), serde_json::json!(title));
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    
                    let event = StreamEvent {
                        session_id: session_id.to_string(),
                        chunk: format!("\n[正在使用工具: {}]\n", tool_display_name),
                        is_complete: false,
                        is_tool_call: true,
                        tool_name: Some(tool_call.name.clone()),
                        tool_display_name: Some(tool_display_name),
                        tool_params: Some(serde_json::Value::Object(display_params)),
                    };
                    let _ = app.emit("ai-chat-stream", event);
                    
                    match execute_tool_call(tool_call).await {
                        Ok(result) => {
                            log::info!("[AI Chat] 工具 {} 执行成功，结果长度: {}", tool_call.name, result.len());
                            save_tool_message(session_id, book_id, chapter_id, &tool_call.name, &result).await?;
                            tool_results.push(format!("工具 '{}' 执行结果:\n{}", tool_call.name, result));
                            
                            // 发送数据变更事件，通知前端刷新
                            let data_change = serde_json::json!({
                                "toolName": tool_call.name,
                                "bookId": book_id,
                                "chapterId": tool_call.arguments.get("chapterId").and_then(|v| v.as_str()),
                            });
                            let _ = app.emit("ai-data-changed", data_change);
                        }
                        Err(e) => {
                            let error_msg = format!("工具执行失败: {}", e);
                            save_tool_message(session_id, book_id, chapter_id, &tool_call.name, &error_msg).await?;
                            tool_results.push(format!("工具 '{}' 执行失败:\n{}", tool_call.name, error_msg));
                        }
                    }
                }

                let combined_results = tool_results.join("\n\n");
                let tool_result_msg = format!("请基于以下工具执行结果回答我的问题:\n\n{}", combined_results);
                history.push(ChatMessage::new("user", &tool_result_msg));

                continue;
            }
        }

        // 没有工具调用，这是最终回复
        final_response = ai_content.clone();

        if final_response.is_empty() {
            final_response = "抱歉，我没有生成任何回复。请再试一次。".to_string();
        }

        save_assistant_message(session_id, book_id, chapter_id, &final_response).await?;
        
        let event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: String::new(),
            is_complete: true,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
        };
        let _ = app.emit("ai-chat-stream", event);
        
        break;
    }
    
    // 如果达到最大迭代次数仍未完成
    if final_response.is_empty() {
        final_response = "抱歉，处理时间过长，请再试一次。".to_string();
        save_assistant_message(session_id, book_id, chapter_id, &final_response).await?;

        let event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: final_response.clone(),
            is_complete: true,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
        };
        let _ = app.emit("ai-chat-stream", event);
    }
    
    Ok(final_response)
}

/// 获取聊天历史
pub async fn get_chat_history(
    session_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<crate::models::AIChatMessage>> {
    let pool = get_pool().await?;
    
    let rows = sqlx::query(
        r#"
        SELECT id, session_id, book_id, chapter_id, role, content, context_type, timestamp
        FROM chat_messages
        WHERE session_id = ?1 AND role != 'tool'
        ORDER BY timestamp ASC
        LIMIT ?2
        "#
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    
    let mut messages = Vec::new();
    for row in rows {
        messages.push(crate::models::AIChatMessage {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            book_id: row.try_get("book_id")?,
            chapter_id: row.try_get("chapter_id")?,
            role: row.try_get("role")?,
            content: row.try_get("content")?,
            context_type: row.try_get("context_type")?,
            timestamp: row.try_get("timestamp")?,
        });
    }
    
    Ok(messages)
}

/// 获取会话列表
pub async fn get_chat_sessions(book_id: &str) -> anyhow::Result<Vec<crate::models::ChatSession>> {
    let pool = get_pool().await?;
    
    let rows = sqlx::query(
        r#"
        SELECT 
            s.session_id, 
            s.book_id, 
            s.chapter_id, 
            s.title, 
            s.created_at, 
            s.updated_at,
            COUNT(m.id) as message_count
        FROM chat_sessions s
        LEFT JOIN chat_messages m ON s.session_id = m.session_id AND m.role != 'tool'
        WHERE s.book_id = ?1
        GROUP BY s.session_id, s.book_id, s.chapter_id, s.title, s.created_at, s.updated_at
        ORDER BY s.updated_at DESC
        "#
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(crate::models::ChatSession {
            session_id: row.try_get("session_id")?,
            book_id: row.try_get("book_id")?,
            chapter_id: row.try_get("chapter_id")?,
            title: row.try_get("title")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            message_count: row.try_get::<i64, _>("message_count")? as i32,
        });
    }
    
    Ok(sessions)
}

/// 删除会话
pub async fn delete_chat_session(session_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM chat_messages WHERE session_id = ?1")
        .bind(session_id)
        .execute(pool)
        .await?;
    
    sqlx::query("DELETE FROM chat_sessions WHERE session_id = ?1")
        .bind(session_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

use crate::db::get_pool;
use crate::models::{AIChatMessage, AIConfig};
use sqlx::Row;
use tauri::{AppHandle, Emitter};

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

/// 保存工具调用摘要（恢复历史时显示工具调用流程，不含返回结果）
pub async fn save_tool_call_summary(
    session_id: &str,
    book_id: &str,
    summary_json: &str,
) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp_millis();
    let message_id = format!("tool_{}_{}", now, rand::random::<u16>());

    sqlx::query(
        r#"
        INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp)
        VALUES (?1, ?2, ?3, NULL, 'tool', ?4, ?5)
        "#
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(book_id)
    .bind(summary_json)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(message_id)
}

/// 更新单行修改消息处理状态
pub async fn update_line_edit_handled_status(message_id: &str, handled_status: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE chat_messages
        SET handled_status = ?1
        WHERE id = ?2
        "#
    )
    .bind(handled_status)
    .bind(message_id)
    .execute(pool)
    .await?;

    Ok(())
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

/// 发送聊天消息（普通对话）
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

    let _ = app.emit("ai-chat-stream", serde_json::json!({
        "sessionId": session_id,
        "chunk": "",
        "isComplete": false,
        "isToolCall": false,
        "sessionStarted": true,
    }));

    // 使用新的Agent架构处理对话
    crate::services::agent_orchestrator::AgentOrchestrator::process_chat(
        &app,
        &session_id,
        &book_id,
        &message,
        config,
    ).await?;

    Ok(session_id)
}

/// 发送润色请求（独立流程，不参与对话上下文）
pub async fn send_polish_request(
    app: AppHandle,
    session_id: Option<String>,
    book_id: String,
    chapter_id: Option<String>,
    original_text: String,
    instruction: String,
    config: AIConfig,
) -> anyhow::Result<String> {
    let session_title: String = format!("润色: {}", original_text.chars().take(15).collect::<String>());
    let session_id = get_or_create_session(
        session_id,
        &book_id,
        chapter_id.as_deref(),
        &session_title,
    ).await?;

    // 保存用户润色请求
    let user_content = format!("润色要求：{}\n\n原文：{}", instruction, original_text);
    save_user_message(&session_id, &book_id, chapter_id.as_deref(), &user_content).await?;

    // 使用润色专用流程
    crate::services::agent_orchestrator::AgentOrchestrator::process_polish(
        &app,
        &session_id,
        &book_id,
        &original_text,
        &instruction,
        config,
    ).await?;

    Ok(session_id)
}

/// 获取聊天历史
pub async fn get_chat_history(
    session_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<AIChatMessage>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT id, session_id, book_id, chapter_id, role, content, context_type, timestamp, polish_handled, handled_status
        FROM chat_messages
        WHERE session_id = ?1
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
        messages.push(AIChatMessage {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            book_id: row.try_get("book_id")?,
            chapter_id: row.try_get("chapter_id")?,
            role: row.try_get("role")?,
            content: row.try_get("content")?,
            context_type: row.try_get("context_type")?,
            timestamp: row.try_get("timestamp")?,
            polish_handled: row.try_get("polish_handled").unwrap_or(0),
            handled_status: row.try_get("handled_status").ok(),
        });
    }

    Ok(messages)
}

/// 获取会话列表
pub async fn get_chat_sessions(book_id: &str) -> anyhow::Result<Vec<crate::models::ChatSession>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT s.session_id, s.book_id, s.chapter_id, s.title, s.created_at, s.updated_at,
               COALESCE((SELECT COUNT(*) FROM chat_messages m WHERE m.session_id = s.session_id AND m.role != 'system'), 0) as message_count
        FROM chat_sessions s
        WHERE s.book_id = ?1
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
            message_count: row.try_get("message_count").unwrap_or(0),
        });
    }

    Ok(sessions)
}

/// 删除会话
pub async fn delete_chat_session(session_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;

    sqlx::query("DELETE FROM chat_sessions WHERE session_id = ?1")
        .bind(session_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// 更新消息的 polish_handled 状态
pub async fn update_message_polish_handled(message_id: &str, handled: bool) -> anyhow::Result<()> {
    let pool = get_pool().await?;

    sqlx::query(
        r#"
        UPDATE chat_messages
        SET polish_handled = ?1
        WHERE id = ?2
        "#
    )
    .bind(if handled { 1 } else { 0 })
    .bind(message_id)
    .execute(pool)
    .await?;

    Ok(())
}

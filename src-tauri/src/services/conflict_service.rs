use crate::db::get_pool;
use crate::models::{
    AIConfig, ChatMessage, ConflictDetectionStatus, DetectedConflict, FunctionDef, ToolDef,
};
use uuid::Uuid;

fn is_invalid_conflict_report(description: &str) -> bool {
    let lower = description.to_lowercase();
    description.is_empty()
        || lower.contains("未检测到")
        || lower.contains("未发现")
        || lower.contains("没有冲突")
        || lower.contains("没有问题")
        || lower.contains("无需修改")
        || lower.contains("设定一致")
}

pub async fn get_conflict_detection_status(
    book_id: &str,
) -> anyhow::Result<Option<ConflictDetectionStatus>> {
    let pool = get_pool().await?;
    let status = sqlx::query_as::<_, ConflictDetectionStatus>(
        "SELECT book_id, last_status, last_error_kind, last_error_message, last_error_at, last_auto_checked_at
         FROM conflict_check_progress WHERE book_id = ?1",
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    Ok(status)
}

pub async fn mark_detection_running(book_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    sqlx::query(
        "INSERT INTO conflict_check_progress (book_id, last_status, last_error_kind, last_error_message, last_error_at)
         VALUES (?1, 'running', NULL, NULL, 0)
         ON CONFLICT(book_id) DO UPDATE SET
         last_status = 'running',
         last_error_kind = NULL,
         last_error_message = NULL,
         last_error_at = 0",
    )
    .bind(book_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn mark_detection_failed(
    book_id: &str,
    error_kind: &str,
    error_message: &str,
) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO conflict_check_progress (book_id, last_status, last_error_kind, last_error_message, last_error_at)
         VALUES (?1, 'failed', ?2, ?3, ?4)
         ON CONFLICT(book_id) DO UPDATE SET
         last_status = 'failed',
         last_error_kind = ?2,
         last_error_message = ?3,
         last_error_at = ?4",
    )
    .bind(book_id)
    .bind(error_kind)
    .bind(error_message)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn save_detected_conflict(
    book_id: &str,
    description: &str,
    suggestion: &str,
) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let id = format!("conflict_{}", Uuid::new_v4().to_string().replace('-', "_"));
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        "INSERT INTO detected_conflicts (id, book_id, description, suggestion, detected_at, is_ignored)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
    )
    .bind(&id)
    .bind(book_id)
    .bind(description)
    .bind(suggestion)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

/// 检查是否需要执行冲突检测（阈值：字数增长20% 或 章节增长3章）
pub async fn check_should_detect(book_id: &str) -> anyhow::Result<bool> {
    let pool = get_pool().await?;

    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(SUM(word_count), 0), COUNT(*) FROM chapters WHERE book_id = ?1",
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or((0, 0));
    let (current_words, current_chapters) = row;

    let prev = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(last_checked_word_count, 0), COALESCE(last_checked_chapter_count, 0)
         FROM conflict_check_progress WHERE book_id = ?1",
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    let (prev_words, prev_chapters) = prev.unwrap_or((0, 0));

    if prev_words == 0 {
        return Ok(current_chapters >= 2 && current_words >= 1000);
    }

    let word_growth = current_words as f64 / prev_words.max(1) as f64;
    let chapter_growth = current_chapters - prev_chapters;

    Ok(word_growth >= 1.2 || chapter_growth >= 3)
}

/// 忽略指定冲突
pub async fn ignore_conflict(conflict_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp();
    sqlx::query("UPDATE detected_conflicts SET is_ignored = 1, ignored_at = ?1 WHERE id = ?2")
        .bind(now)
        .bind(conflict_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 取消忽略指定冲突
pub async fn unignore_conflict(conflict_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    sqlx::query("UPDATE detected_conflicts SET is_ignored = 0, ignored_at = NULL WHERE id = ?1")
        .bind(conflict_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 删除指定冲突
pub async fn delete_conflict(conflict_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    sqlx::query("DELETE FROM detected_conflicts WHERE id = ?1")
        .bind(conflict_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 获取书籍的所有未忽略冲突
pub async fn get_active_conflicts(book_id: &str) -> anyhow::Result<Vec<DetectedConflict>> {
    let pool = get_pool().await?;
    let rows = sqlx::query_as::<_, DetectedConflict>(
        "SELECT id, book_id, description, suggestion, detected_at, is_ignored, ignored_at
         FROM detected_conflicts WHERE book_id = ?1 AND is_ignored = 0 ORDER BY detected_at DESC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 获取书籍的所有冲突（包括已忽略的）
pub async fn get_all_conflicts(book_id: &str) -> anyhow::Result<Vec<DetectedConflict>> {
    let pool = get_pool().await?;
    let rows = sqlx::query_as::<_, DetectedConflict>(
        "SELECT id, book_id, description, suggestion, detected_at, is_ignored, ignored_at
         FROM detected_conflicts WHERE book_id = ?1 ORDER BY detected_at DESC",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 构建冲突检测专用工具（精简版：只保留核心查询 + 报告工具）
fn build_detection_tools() -> Vec<ToolDef> {
    vec![
        // 核心：故事圣经（必读）
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_story_memory".into(),
                description: "获取小说的全局故事记忆（故事圣经），包括全书梗概、分卷梗概、事件时间线、主角状态、重要角色现状、未解决伏笔、世界观设定。第一轮必须调用！".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["bookId"]
                }),
            },
        },
        // 核心：章节摘要（第一轮必须读完所有章节）
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_summary".into(),
                description: "获取指定章节的摘要信息（短摘要+长摘要+标签+角色+事件+伏笔+未解决线索）。用于快速了解章节内容，第一轮必须读完所有章节摘要！".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        // 验证：关键词搜索（用于验证疑点）
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "search_chapter_content".into(),
                description: "全文关键词搜索。用于验证摘要中的矛盾点，快速定位相关内容。优先使用此工具而非读取全文。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID（可选，不指定则搜索全书）"},
                        "keyword": {"type": "string", "description": "搜索关键字"},
                        "bookId": {"type": "string", "description": "书籍ID（搜索全书时必填）"}
                    },
                    "required": ["keyword"]
                }),
            },
        },
        // 验证：读取正文（用于精确验证）
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_content".into(),
                description: "获取指定章节的正文内容。仅在搜索后需要精确验证特定段落时使用。可指定行号范围，不指定则返回全文。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID"},
                        "startLine": {"type": "integer", "description": "起始行号（1开始），负数表示从末尾倒数"},
                        "endLine": {"type": "integer", "description": "结束行号（包含）"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        // 报告：创建冲突记录
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "report_conflict".into(),
                description: "报告一个确认后的设定冲突。必须已查原文核实过再报告。每个确认的冲突单独调用一次。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "description": {"type": "string", "description": "冲突描述，如'第1章叶铃音是黑发，第5章变成了银发'"},
                        "suggestion": {"type": "string", "description": "修改建议"},
                        "severity": {"type": "string", "description": "严重程度：high/medium/low"}
                    },
                    "required": ["description", "suggestion", "severity"]
                }),
            },
        },
    ]
}

/// 执行冲突检测（独立流程，不污染聊天上下文）
pub async fn run_detection(book_id: &str, config: &AIConfig) -> anyhow::Result<Vec<DetectedConflict>> {
    log::info!("[Conflict] ========== 开始冲突检测 book={} ==========", book_id);

    // 获取章节列表
    let chapter_list = crate::services::book_service::load_book(book_id.to_string())
        .await
        .map(|b| {
            let mut s = String::from("章节列表：\n");
            for (i, ch) in b.chapters.iter().enumerate() {
                s.push_str(&format!("{}. {} (ID:{})\n", i+1, ch.title, ch.id));
            }
            s
        })
        .unwrap_or_default();

    // 获取已有冲突（包括已忽略的），防止重复检测
    let existing_conflicts = get_all_conflicts(book_id).await.unwrap_or_default();
    let existing_conflicts_text = if existing_conflicts.is_empty() {
        "暂无已记录的冲突。".to_string()
    } else {
        let mut s = String::from("**已有冲突记录（请勿重复报告）：**\n\n");
        for (i, c) in existing_conflicts.iter().enumerate() {
            let status = if c.is_ignored == 1 { "【已忽略】" } else { "" };
            s.push_str(&format!(
                "{}. {} {}\n   建议：{}\n\n",
                i + 1,
                status,
                c.description,
                c.suggestion
            ));
        }
        s
    };

    let prompt_manager = crate::services::prompt_service::get_prompt_manager();
    let max_rounds = config.max_rounds.clamp(20, 50) as usize;
    let system_prompt = prompt_manager
        .conflict_detection
        .replace("{{CHAPTER_LIST}}", &chapter_list)
        .replace("{{MAX_ROUNDS}}", &max_rounds.to_string())
        .replace("{{EXISTING_CONFLICTS}}", &existing_conflicts_text);

    let messages = vec![
        ChatMessage::new("system", &system_prompt),
        ChatMessage::new("user", "开始冲突检测。\n\n**已有冲突记录已加载，请勿重复报告相同的冲突。**\n\n**第1轮必须完成**：\n1. 调用 get_story_memory 获取故事圣经\n2. 调用 query_chapter_summary 读完所有章节摘要\n\n**后续轮次**：\n- 分析摘要中的矛盾信号\n- 用 search_chapter_content 关键词搜索验证\n- 必要时用 query_chapter_content 读取具体段落\n- 确认冲突后用 report_conflict 报告\n\n**禁止**：\n- 不要没查数据就下结论\n- 不要把轮次浪费在无关章节上\n- 不要机械遍历所有章节\n- 不要重复报告已有冲突"),
    ];

    let tools = build_detection_tools();
    let mut messages = messages;
    let mut total_conflicts = 0;

    for round in 0..max_rounds {
        log::info!("[Conflict] 检测轮次 {}/{}", round + 1, max_rounds);

        let round_prompt = format!(
            "当前是第 {}/{} 轮工具调用。请根据剩余轮次规划查询策略，优先查询信息密度高、覆盖范围广的工具；如果剩余轮次不多，请收缩范围，只继续验证高概率冲突。",
            round + 1,
            max_rounds
        );
        messages.push(ChatMessage::new("system", &round_prompt));

        let decision = crate::services::ai_service::send_ai_message(
            messages.clone(), config.clone(), Some(&tools),
        ).await?;

        if let Some(tool_calls) = &decision.tool_calls {
            if tool_calls.is_empty() {
                let ai_response = decision.content.as_deref().unwrap_or("(空)");
                let preview: String = ai_response.chars().take(200).collect();
                log::info!("[Conflict] AI无工具调用，回答: {}", preview);
                break;
            }

            log::info!("[Conflict] AI调用 {} 个工具: {:?}",
                tool_calls.len(),
                tool_calls.iter().map(|t| t.function.name.as_str()).collect::<Vec<_>>()
            );

            // AI 的消息加入历史
            messages.push(ChatMessage {
                role: "assistant".into(),
                content: decision.content,
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
                name: None,
            });

            for tc in tool_calls {
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Null);

                if tc.function.name == "report_conflict" {
                    let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let sug = args.get("suggestion").and_then(|v| v.as_str()).unwrap_or("");
                    let sev = args.get("severity").and_then(|v| v.as_str()).unwrap_or("?");
                    if is_invalid_conflict_report(desc) {
                        log::info!("[Conflict] ⚠ 跳过无效报告: {}", desc);
                    } else {
                        log::info!("[Conflict] ⚠ 报告冲突 ({}): {} | 建议: {}", sev, desc, sug);
                        save_detected_conflict(book_id, desc, sug).await?;
                        total_conflicts += 1;
                    }
                    messages.push(ChatMessage {
                        role: "tool".into(),
                        content: Some(if is_invalid_conflict_report(desc) {
                            format!("跳过无效报告: {}", desc)
                        } else {
                            "冲突已记录".into()
                        }),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some("report_conflict".into()),
                    });
                } else {
                    // 查询工具：实际执行
                    log::info!("[Conflict] 执行查询工具: {} args={}", tc.function.name, args);
                    let result = match crate::services::tool_call_service::execute_tool_call(
                        &crate::models::ToolCall { name: tc.function.name.clone(), arguments: args },
                        book_id,
                    ).await {
                        Ok(result) => result,
                        Err(e) => {
                            let error_text = format!("工具 {} 执行失败：{}", tc.function.name, e);
                            log::error!("[Conflict] {}", error_text);
                            error_text
                        }
                    };
                    log::info!("[Conflict] 查询结果长度: {} 字", result.len());
                    messages.push(ChatMessage {
                        role: "tool".into(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tc.function.name.clone()),
                    });
                }
            }

        } else if let Some(content) = &decision.content {
            if !content.is_empty() {
                let preview: String = content.chars().take(300).collect();
log::info!("[Conflict] AI最终回答: {}", preview);
                break;
            }
        }
    }

    log::info!("[Conflict] ========== 检测完成，共发现 {} 个冲突 ==========", total_conflicts);

    // 更新检测进度
    let pool = get_pool().await?;
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(SUM(word_count), 0), COUNT(*) FROM chapters WHERE book_id = ?1"
    ).bind(book_id).fetch_optional(pool).await?.unwrap_or((0, 0));
    let now = chrono::Utc::now().timestamp();
    let _ = sqlx::query(
        "INSERT INTO conflict_check_progress (
            book_id,
            last_checked_word_count,
            last_checked_chapter_count,
            last_checked_at,
            last_status,
            last_error_kind,
            last_error_message,
            last_error_at,
            last_auto_checked_at
         )
         VALUES (?1, ?2, ?3, ?4, 'success', NULL, NULL, 0, ?4)
         ON CONFLICT(book_id) DO UPDATE SET
         last_checked_word_count = ?2,
         last_checked_chapter_count = ?3,
         last_checked_at = ?4,
         last_status = 'success',
         last_error_kind = NULL,
         last_error_message = NULL,
         last_error_at = 0,
         last_auto_checked_at = ?4"
    ).bind(book_id).bind(row.0).bind(row.1).bind(now).execute(pool).await;

    get_active_conflicts(book_id).await
}

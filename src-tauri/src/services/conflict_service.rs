use crate::db::get_pool;
use crate::models::{AIConfig, ChatMessage, ToolDef, FunctionDef, DetectedConflict};
use uuid::Uuid;

/// 检查是否需要执行冲突检测（阈值：字数增长20% 或 章节增长3章）
pub async fn check_should_detect(book_id: &str) -> anyhow::Result<bool> {
    let pool = get_pool().await?;

    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(SUM(word_count), 0), COUNT(*) FROM chapters WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or((0, 0));
    let (current_words, current_chapters) = row;

    let prev = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(last_checked_word_count, 0), COALESCE(last_checked_chapter_count, 0)
         FROM conflict_check_progress WHERE book_id = ?1"
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

/// 获取书籍的所有未忽略冲突
pub async fn get_active_conflicts(book_id: &str) -> anyhow::Result<Vec<DetectedConflict>> {
    let pool = get_pool().await?;
    let rows = sqlx::query_as::<_, DetectedConflict>(
        "SELECT id, book_id, description, suggestion, detected_at, is_ignored, ignored_at
         FROM detected_conflicts WHERE book_id = ?1 AND is_ignored = 0 ORDER BY detected_at DESC"
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 构建冲突检测专用工具（查询工具 + report_conflict）
fn build_detection_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_summary".into(),
                description: "获取章节摘要".into(),
                parameters: serde_json::json!({"type":"object","properties":{"chapterId":{"type":"string"}},"required":["chapterId"]}),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "list_character_cards".into(),
                description: "获取所有角色卡".into(),
                parameters: serde_json::json!({"type":"object","properties":{"bookId":{"type":"string"}},"required":["bookId"]}),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_character_card".into(),
                description: "获取角色详情".into(),
                parameters: serde_json::json!({"type":"object","properties":{"cardId":{"type":"string"}},"required":["cardId"]}),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "search_chapter_content".into(),
                description: "全文搜索。默认普通匹配，regex=true启用正则。结果太多时换更精确的关键词".into(),
                parameters: serde_json::json!({"type":"object","properties":{"chapterId":{"type":"string"},"keyword":{"type":"string"},"bookId":{"type":"string"},"regex":{"type":"boolean"}},"required":["keyword"]}),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_content".into(),
                description: "获取章节内容（可指定行号）".into(),
                parameters: serde_json::json!({"type":"object","properties":{"chapterId":{"type":"string"},"startLine":{"type":"integer"},"endLine":{"type":"integer"}},"required":["chapterId"]}),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "report_conflict".into(),
                description: "报告一个设定冲突。每个冲突单独调用，细节差异和写作风格变化不算".into(),
                parameters: serde_json::json!({
                    "type":"object",
                    "properties":{
                        "description":{"type":"string","description":"冲突描述"},
                        "suggestion":{"type":"string","description":"修改建议"},
                        "severity":{"type":"string","description":"high/medium/low"}
                    },
                    "required":["description","suggestion","severity"]
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

    let prompt_manager = crate::services::prompt_service::get_prompt_manager();
    let system_prompt = prompt_manager
        .conflict_detection
        .replace("{{CHAPTER_LIST}}", &chapter_list);

    let messages = vec![
        ChatMessage::new("system", &system_prompt),
        ChatMessage::new("user", "请逐项检查这本书的设定冲突。必须用工具查询实际数据来比对，不要凭感觉。\n\n检查步骤：\n1. 先查所有角色卡\n2. 逐章查询章节内容（每章至少查首尾部分）\n3. 逐项比对：角色外貌是否一致？性格是否突变？时间是否线性？地点是否一致？\n4. 发现矛盾后必须再次查原文确认\n\n注意：如果没有调用任何查询工具就直接回答\"未检测到冲突\"，说明你没有真正检查。必须先查数据再下结论。"),
    ];

    let tools = build_detection_tools();
    let mut messages = messages;
    let max_rounds = 10;
    let mut total_conflicts = 0;

    for round in 0..max_rounds {
        log::info!("[Conflict] 检测轮次 {}/{}", round + 1, max_rounds);

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
                    // 过滤"没问题"类的伪冲突
                    let lower = desc.to_lowercase();
                    let is_no_problem = desc.is_empty()
                        || lower.contains("未检测到")
                        || lower.contains("未发现")
                        || lower.contains("没有冲突")
                        || lower.contains("没有问题")
                        || lower.contains("无需修改")
                        || lower.contains("设定一致");
                    if is_no_problem {
                        log::info!("[Conflict] ⚠ 跳过无效报告: {}", desc);
                    } else {
                        log::info!("[Conflict] ⚠ 报告冲突 ({}): {} | 建议: {}", sev, desc, sug);
                        if !desc.is_empty() {
                            let pool = get_pool().await?;
                            let id = format!("conflict_{}", Uuid::new_v4().to_string().replace('-', "_"));
                            let now = chrono::Utc::now().timestamp();
                            let _ = sqlx::query(
                                "INSERT INTO detected_conflicts (id, book_id, description, suggestion, detected_at, is_ignored)
                                 VALUES (?1, ?2, ?3, ?4, ?5, 0)"
                            ).bind(&id).bind(book_id).bind(desc).bind(sug).bind(now).execute(pool).await;
                            total_conflicts += 1;
                        }
                        messages.push(ChatMessage {
                            role: "tool".into(),
                            content: Some("冲突已记录".into()),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                            name: Some("report_conflict".into()),
                        });
                    }
                } else {
                    // 查询工具：实际执行
                    log::info!("[Conflict] 执行查询工具: {} args={}", tc.function.name, args);
                    let result = crate::services::tool_call_service::execute_tool_call(
                        &crate::models::ToolCall { name: tc.function.name.clone(), arguments: args }
                    ).await?;
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
        "INSERT INTO conflict_check_progress (book_id, last_checked_word_count, last_checked_chapter_count, last_checked_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(book_id) DO UPDATE SET
         last_checked_word_count = ?2, last_checked_chapter_count = ?3, last_checked_at = ?4"
    ).bind(book_id).bind(row.0).bind(row.1).bind(now).execute(pool).await;

    get_active_conflicts(book_id).await
}

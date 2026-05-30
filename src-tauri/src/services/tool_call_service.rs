use crate::models::ToolCall;
use sqlx::Row;

/// 执行工具调用（bookId 由调用方自动注入）
pub async fn execute_tool_call(tool_call: &ToolCall, book_id: &str) -> anyhow::Result<String> {
    log::info!("执行工具调用: {}", tool_call.name);
    log::debug!("工具参数: {}", tool_call.arguments);

    match tool_call.name.as_str() {
        // 书籍信息工具
        "get_book_info" => {
            let book = crate::services::book_service::load_book(book_id.to_string()).await?;
            let volumes_text: String = book.volumes.iter()
                .map(|v| {
                    let ch_count = book.chapters.iter().filter(|c| c.volume_id == v.id).count();
                    format!("- {}（{}章，ID: {}）", v.title, ch_count, v.id)
                })
                .collect::<Vec<_>>()
                .join("\n");
            let result = format!(
                "书名：{}\n作者：{}\n简介：{}\n卷数：{}\n章节数：{}\n总字数：{}\n\n卷列表：\n{}",
                book.title,
                if book.author.is_empty() { "未设置" } else { &book.author },
                if book.description.is_empty() { "暂无简介" } else { &book.description },
                book.volumes.len(),
                book.chapters.len(),
                book.chapters.iter().map(|c| c.word_count).sum::<i64>(),
                if volumes_text.is_empty() { "暂无卷" } else { &volumes_text }
            );
            Ok(result)
        }
        // 章节相关工具
        "list_all_chapters" => {
            let chapters = get_all_chapter_summaries_for_tool(book_id).await?;
            Ok(serde_json::to_string(&chapters)?)
        }
        "query_chapter_summary" => {
            let chapter_id = tool_call.arguments.get("chapterId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 chapterId 参数"))?;
            let summary = crate::services::summary_service::query_chapter_summary(chapter_id)
                .await
                .unwrap_or_else(|e| format!("查询章节摘要失败：{}", e));
            Ok(summary)
        }
        "query_chapter_content" => {
            let chapter_id = tool_call.arguments.get("chapterId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 chapterId 参数"))?;
            let content = crate::services::summary_service::query_chapter_content(chapter_id)
                .await
                .unwrap_or_else(|e| format!("查询章节内容失败：{}", e));
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len() as i64;

            let start = tool_call.arguments.get("startLine").and_then(|v| v.as_i64());
            let end = tool_call.arguments.get("endLine").and_then(|v| v.as_i64());

            let (from, to) = match (start, end) {
                (None, None) => {
                    // 全文
                    return Ok(format!("章节共{}行，全文如下：\n{}", total, content));
                }
                (Some(s), None) => {
                    if s < 0 {
                        // 负数：倒数
                        let s = (total + s + 1).max(1);
                        (s, total)
                    } else {
                        (s.max(1), total)
                    }
                }
                (None, Some(e)) => {
                    (1, e.min(total))
                }
                (Some(s), Some(e)) => {
                    let s = if s < 0 { (total + s + 1).max(1) } else { s.max(1) };
                    (s, e.min(total))
                }
            };

            if from > total || from > to {
                return Ok(format!("章节共{}行，请求的行号范围无效", total));
            }

            let selected: Vec<String> = lines[(from as usize - 1)..(to as usize)]
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:>4}: {}", from as usize + i, line))
                .collect();

            Ok(format!(
                "章节共{}行，显示第{}-{}行：\n{}",
                total,
                from,
                to,
                selected.join("\n")
            ))
        }

        // 角色卡相关工具
        "create_character_card" => {
            let name = tool_call.arguments.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 name 参数"))?;

            let params = crate::models::CharacterCardParams {
                name: name.to_string(),
                aliases: tool_call.arguments.get("aliases")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
                gender: tool_call.arguments.get("gender").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                age: tool_call.arguments.get("age").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                appearance: tool_call.arguments.get("appearance").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                personality: tool_call.arguments.get("personality").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                background: tool_call.arguments.get("background").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                goals: tool_call.arguments.get("goals").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                relationships: Vec::new(),
                tags: Vec::new(),
                notes: tool_call.arguments.get("notes").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };
            let card = crate::services::character_service::create_character_card(book_id.to_string(), params).await?;
            Ok(serde_json::to_string(&card)?)
        }
        "get_character_card" => {
            let card_id = tool_call.arguments.get("cardId")
                .or_else(|| tool_call.arguments.get("characterId"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 cardId 或 characterId 参数"))?;
            let card = crate::services::character_service::get_character_card(card_id.to_string()).await?;
            Ok(serde_json::to_string(&card)?)
        }
        "list_character_cards" => {
            let cards = crate::services::character_service::list_character_cards(book_id.to_string()).await?;
            Ok(serde_json::to_string(&cards)?)
        }
        "update_character_card" => {
            let card_id = tool_call.arguments.get("cardId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 cardId 参数"))?
                .to_string();

            let params = crate::models::CharacterCardParams {
                name: tool_call.arguments.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                aliases: tool_call.arguments.get("aliases")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
                gender: tool_call.arguments.get("gender").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                age: tool_call.arguments.get("age").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                appearance: tool_call.arguments.get("appearance").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                personality: tool_call.arguments.get("personality").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                background: tool_call.arguments.get("background").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                goals: tool_call.arguments.get("goals").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                relationships: Vec::new(),
                tags: Vec::new(),
                notes: tool_call.arguments.get("notes").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            };
            crate::services::character_service::update_character_card(card_id, params).await?;
            Ok("{\"success\": true}".to_string())
        }

        // 大纲相关工具
        "save_outline" => {
            let outline_type = tool_call.arguments.get("outlineType")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 outlineType 参数"))?
                .to_string();
            let content = tool_call.arguments.get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 content 参数"))?
                .to_string();

            let volume_id = tool_call.arguments.get("volumeId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let chapter_id = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()).map(|s| s.to_string());

            crate::services::outline_service::save_outline(
                book_id.to_string(), volume_id, chapter_id, outline_type, content
            ).await?;
            Ok("{\"success\": true}".to_string())
        }
        "get_outline" => {
            // 优先使用 chapterId 查询，如果没有则使用 outlineId
            if let Some(chapter_id) = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()) {
                // 通过 chapterId 获取该章节的所有大纲（粗纲和细纲）
                let coarse = crate::services::outline_service::get_outline_by_level(
                    book_id.to_string(), None, Some(chapter_id.to_string()), "coarse".to_string()
                ).await.ok();

                let fine = crate::services::outline_service::get_outline_by_level(
                    book_id.to_string(), None, Some(chapter_id.to_string()), "fine".to_string()
                ).await.ok();

                let result = serde_json::json!({
                    "coarse": coarse,
                    "fine": fine
                });

                Ok(result.to_string())
            } else if let Some(outline_id) = tool_call.arguments.get("outlineId").and_then(|v| v.as_str()) {
                // 通过 outlineId 直接查询
                let outline = crate::services::outline_service::get_outline(outline_id.to_string()).await?;
                Ok(serde_json::to_string(&outline)?)
            } else {
                Err(anyhow::anyhow!("缺少必要参数：需要提供 chapterId 或 outlineId"))
            }
        }

        // 写作风格相关工具
        "learn_writing_style" => {
            let force = tool_call.arguments.get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let params = crate::models::LearnStyleParams {
                book_id: Some(book_id.to_string()),
                chapter_ids: None,
                force_relearn: force,
            };

            let result = crate::services::style_service::learn_writing_style(params).await?;
            Ok(serde_json::to_string(&result)?)
        }

        "search_chapter_content" => {
            let keyword = tool_call.arguments.get("keyword")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 keyword 参数"))?;
            let use_regex = tool_call.arguments.get("regex")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let re = if use_regex {
                Some(regex::Regex::new(keyword).map_err(|e| anyhow::anyhow!("正则无效: {}", e))?)
            } else {
                None
            };
            let kw_lower = if use_regex { String::new() } else { keyword.to_lowercase() };

            let is_match = |line: &str| -> bool {
                if let Some(ref re) = re {
                    re.is_match(line)
                } else {
                    line.to_lowercase().contains(&kw_lower)
                }
            };

            if let Some(chapter_id) = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()) {
                let content = crate::services::summary_service::query_chapter_content(chapter_id)
                    .await
                    .unwrap_or_else(|e| format!("查询章节内容失败：{}", e));
                let matches: Vec<String> = content.lines()
                    .enumerate()
                    .filter(|(_, line)| is_match(line))
                    .map(|(i, line)| format!("第{}行: {}", i + 1, line.trim()))
                    .collect();
                if matches.is_empty() {
                    Ok(format!("在章节 {} 中未找到匹配", chapter_id))
                } else {
                    Ok(format!("在章节 {} 搜索到 {} 处匹配：\n{}",
                        chapter_id, matches.len(), matches.join("\n")))
                }
            } else {
                let chapters = crate::services::book_service::load_book(book_id.to_string()).await?;
                let mut result = String::new();
                let mut total_all = 0;
                for ch in &chapters.chapters {
                    if ch.content.is_empty() { continue; }
                    let matches: Vec<String> = ch.content.lines()
                        .enumerate()
                        .filter(|(_, line)| is_match(line))
                        .map(|(i, line)| format!("  第{}行: {}", i + 1, line.trim()))
                        .collect();
                    if !matches.is_empty() {
                        total_all += matches.len();
                        result.push_str(&format!("「{}」(ID:{}): {} 处\n{}\n",
                            ch.title, ch.id, matches.len(), matches.join("\n")));
                    }
                }
                if total_all == 0 {
                    Ok(format!("全书未找到匹配"))
                } else {
                    Ok(format!("全书搜索到 {} 处匹配：\n{}", total_all, result))
                }
            }
        }

        // 故事记忆相关工具
        "get_story_memory" => {
            crate::services::story_memory_service::build_story_memory_text(book_id).await
        }
        "get_character_timeline" => {
            let name = tool_call.arguments.get("characterName")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 characterName 参数"))?;
            crate::services::story_memory_service::build_character_timeline(book_id, name).await
        }
        "list_chapters_in_volume" => {
            let volume_id = tool_call.arguments.get("volumeId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 volumeId 参数"))?;
            crate::services::story_memory_service::get_chapters_in_volume(book_id, volume_id).await
        }

        // ===== 新增工具 =====
        "propose_line_edit" => {
            // 此工具需要前端交互，后端只返回需要确认的信息
            let chapter_id = tool_call.arguments.get("chapterId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 chapterId 参数"))?;
            let line_number = tool_call.arguments.get("lineNumber")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| anyhow::anyhow!("缺少 lineNumber 参数"))?;
            let original_text = tool_call.arguments.get("originalText")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 originalText 参数"))?;
            let new_text = tool_call.arguments.get("newText")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 newText 参数"))?;
            let reason = tool_call.arguments.get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // 返回给前端处理的格式
            let result = serde_json::json!({
                "tool": "propose_line_edit",
                "chapterId": chapter_id,
                "lineNumber": line_number,
                "originalText": original_text,
                "newText": new_text,
                "reason": reason,
                "requiresUserConfirmation": true
            });
            Ok(result.to_string())
        }

        "get_volume_outline" => {
            let volume_name = tool_call.arguments.get("volumeName")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 volumeName 参数"))?;
            let outline_type = tool_call.arguments.get("outlineType")
                .and_then(|v| v.as_str())
                .unwrap_or("coarse");

            // 通过卷名查找卷ID
            let book = crate::services::book_service::load_book(book_id.to_string()).await?;
            let volume_id = book.volumes.iter()
                .find(|v| v.title == volume_name)
                .map(|v| v.id.as_str());

            let volume_id = match volume_id {
                Some(id) => id,
                None => {
                    let available = book.volumes.iter()
                        .map(|v| v.title.as_str())
                        .collect::<Vec<_>>()
                        .join("、");
                    return Ok(format!("未找到名为「{}」的卷。可用卷名：{}", volume_name, if available.is_empty() { "暂无" } else { &available }))
                }
            };

            let outline = crate::services::outline_service::get_outline_by_level(
                book_id.to_string(),
                Some(volume_id.to_string()),
                None,
                outline_type.to_string()
            ).await?;

            match outline {
                Some(o) => Ok(format!("「{}」{}大纲：\n{}", volume_name, if outline_type == "coarse" { "粗" } else { "细" }, o.content)),
                None => Ok(format!("「{}」未找到{}大纲", volume_name, if outline_type == "coarse" { "粗" } else { "细" }))
            }
        }

        "ask_user" => {
            // 此工具需要前端交互，返回问题信息给前端展示
            let question = tool_call.arguments.get("question")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 question 参数"))?;
            let context = tool_call.arguments.get("context")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let options = tool_call.arguments.get("options")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();

            let result = serde_json::json!({
                "tool": "ask_user",
                "question": question,
                "context": context,
                "options": options,
                "requiresUserResponse": true
            });
            Ok(result.to_string())
        }

        "create_chapter" => {
            // 获取volume_id：如果指定了就用指定的，否则用最后一卷
            let volume_id = if let Some(vid) = tool_call.arguments.get("volumeId").and_then(|v| v.as_str()) {
                vid.to_string()
            } else {
                // 查询最后一卷
                let pool = crate::db::get_pool().await?;
                let row = sqlx::query("SELECT id FROM volumes WHERE book_id = ? ORDER BY created_at DESC LIMIT 1")
                    .bind(book_id)
                    .fetch_optional(pool)
                    .await?;
                match row {
                    Some(r) => r.try_get::<String, _>("id")?,
                    None => return Err(anyhow::anyhow!("书籍 {} 没有卷，请先创建卷", book_id)),
                }
            };

            // 获取order：如果指定了就用指定的，否则自动计算
            let order = if let Some(ord) = tool_call.arguments.get("order").and_then(|v| v.as_i64()) {
                ord as i32
            } else {
                // 查询当前最大order
                let pool = crate::db::get_pool().await?;
                let row = sqlx::query("SELECT MAX(order_num) as max_order FROM chapters WHERE book_id = ?")
                    .bind(book_id)
                    .fetch_one(pool)
                    .await?;
                let max_order: Option<i32> = row.try_get("max_order")?;
                max_order.unwrap_or(0) + 1
            };

            // 获取或生成标题
            let title = if let Some(t) = tool_call.arguments.get("title").and_then(|v| v.as_str()) {
                t.to_string()
            } else {
                // 自动生成标题：第X章
                format!("第{}章", order)
            };

            // 创建章节
            let chapter = crate::services::book_service::create_chapter_with_order(
                book_id.to_string(),
                title.to_string(),
                volume_id,
                order,
            ).await?;

            Ok(serde_json::json!({
                "chapterId": chapter.id,
                "title": chapter.title,
                "order": chapter.order,
                "message": format!("成功创建章节：{}", chapter.title)
            }).to_string())
        }

        _ => Err(anyhow::anyhow!("未知工具: {}", tool_call.name)),
    }
}

/// 获取所有章节摘要（用于工具调用）
async fn get_all_chapter_summaries_for_tool(book_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    let pool = crate::db::get_pool().await?;

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

    let mut chapters: Vec<serde_json::Value> = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let short_summary: Option<String> = row.try_get("short_summary").ok();

        chapters.push(serde_json::json!({
            "id": id,
            "title": title,
            "shortSummary": short_summary.unwrap_or_default()
        }));
    }

    Ok(chapters)
}

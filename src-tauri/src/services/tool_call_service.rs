use crate::models::ToolCall;
use sqlx::Row;

/// 执行工具调用
pub async fn execute_tool_call(tool_call: &ToolCall) -> anyhow::Result<String> {
    log::info!("执行工具调用: {}", tool_call.name);
    log::debug!("工具参数: {}", tool_call.arguments);

    match tool_call.name.as_str() {
        // 章节相关工具
        "list_all_chapters" => {
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
            let chapters = get_all_chapter_summaries_for_tool(book_id).await?;
            Ok(serde_json::to_string(&chapters)?)
        }
        "query_chapter_summary" => {
            let chapter_id = tool_call.arguments.get("chapterId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 chapterId 参数"))?;
            let summary = crate::services::summary_service::query_chapter_summary(chapter_id).await?;
            Ok(summary)
        }
        "query_chapter_content" => {
            let chapter_id = tool_call.arguments.get("chapterId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 chapterId 参数"))?;
            let content = crate::services::summary_service::query_chapter_content(chapter_id).await?;
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
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
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
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
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
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?
                .to_string();
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
                book_id, volume_id, chapter_id, outline_type, content
            ).await?;
            Ok("{\"success\": true}".to_string())
        }
        "get_outline" => {
            // 优先使用 chapterId 查询，如果没有则使用 outlineId
            if let Some(chapter_id) = tool_call.arguments.get("chapterId").and_then(|v| v.as_str()) {
                // 通过 chapterId 获取该章节的所有大纲（粗纲和细纲）
                let book_id = tool_call.arguments.get("bookId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("使用 chapterId 查询时需要 bookId 参数"))?
                    .to_string();

                let coarse = crate::services::outline_service::get_outline_by_level(
                    book_id.clone(), None, Some(chapter_id.to_string()), "coarse".to_string()
                ).await.ok();

                let fine = crate::services::outline_service::get_outline_by_level(
                    book_id, None, Some(chapter_id.to_string()), "fine".to_string()
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
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let force = tool_call.arguments.get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let params = crate::models::LearnStyleParams {
                book_id,
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
                let content = crate::services::summary_service::query_chapter_content(chapter_id).await?;
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
                let book_id = tool_call.arguments.get("bookId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("搜索全书时需要 bookId 参数"))?;
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
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
            crate::services::story_memory_service::build_story_memory_text(book_id).await
        }
        "get_character_timeline" => {
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
            let name = tool_call.arguments.get("characterName")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 characterName 参数"))?;
            crate::services::story_memory_service::build_character_timeline(book_id, name).await
        }
        "list_chapters_in_volume" => {
            let book_id = tool_call.arguments.get("bookId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 bookId 参数"))?;
            let volume_id = tool_call.arguments.get("volumeId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 volumeId 参数"))?;
            crate::services::story_memory_service::get_chapters_in_volume(book_id, volume_id).await
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

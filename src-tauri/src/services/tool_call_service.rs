use crate::models::ToolCall;
use sqlx::Row;

/// 按字符截断文本
fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}…（内容过长已截断，共 {} 字；可用具体工具查询细节）",
            s.chars().take(max_chars).collect::<String>(),
            s.chars().count()
        )
    }
}

/// 执行工具调用（bookId 由调用方自动注入）
pub async fn execute_tool_call(tool_call: &ToolCall, book_id: &str) -> anyhow::Result<String> {
    log::info!("执行工具调用: {}", tool_call.name);
    log::debug!("工具参数: {}", tool_call.arguments);

    match tool_call.name.as_str() {
        // 书籍信息工具
        "get_book_info" => {
            // 只查元数据，不再 load_book（那会把全书正文都读进内存）
            let pool = crate::db::get_pool().await?;

            let row = sqlx::query("SELECT title, author, description FROM books WHERE id = ?1")
                .bind(book_id)
                .fetch_optional(pool)
                .await?;

            let Some(row) = row else {
                return Ok(format!("书籍 {} 不存在", book_id));
            };

            let title: String = row.try_get("title").unwrap_or_default();
            let author: String = row.try_get("author").unwrap_or_default();
            let description: String = row.try_get("description").unwrap_or_default();

            let volume_rows = sqlx::query(
                r#"SELECT id, title FROM volumes WHERE book_id = ?1 ORDER BY "order""#,
            )
            .bind(book_id)
            .fetch_all(pool)
            .await?;

            let chapter_stats = sqlx::query(
                r#"
                SELECT volume_id, COUNT(*) AS cnt, COALESCE(SUM(word_count), 0) AS words
                FROM chapters WHERE book_id = ?1 GROUP BY volume_id
                "#,
            )
            .bind(book_id)
            .fetch_all(pool)
            .await?;

            let mut stat_map: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
            for stat in &chapter_stats {
                let volume_id: String = stat.try_get("volume_id").unwrap_or_default();
                let count: i64 = stat.try_get("cnt").unwrap_or(0);
                let words: i64 = stat.try_get("words").unwrap_or(0);
                stat_map.insert(volume_id, (count, words));
            }

            let mut volumes_text = String::new();
            let mut total_chapters = 0i64;
            let mut total_words = 0i64;

            for volume in &volume_rows {
                let id: String = volume.try_get("id")?;
                let volume_title: String = volume.try_get("title")?;
                let (count, words) = stat_map.get(&id).copied().unwrap_or((0, 0));
                total_chapters += count;
                total_words += words;
                volumes_text.push_str(&format!(
                    "- {}（{}章，{}字，ID: {}）\n",
                    volume_title, count, words, id
                ));
            }

            let result = format!(
                "书名：{}\n作者：{}\n简介：{}\n卷数：{}\n章节数：{}\n总字数：{}\n\n卷列表：\n{}",
                title,
                if author.is_empty() { "未设置".to_string() } else { author },
                if description.is_empty() { "暂无简介".to_string() } else { description },
                volume_rows.len(),
                total_chapters,
                total_words,
                if volumes_text.is_empty() { "暂无卷".to_string() } else { volumes_text }
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
                    // 全文：过长时截断，避免一次把上下文塞满（AI 可用行号范围继续读）
                    let char_count = content.chars().count();
                    if char_count > 12000 {
                        let head: String = content.chars().take(12000).collect();
                        return Ok(format!(
                            "章节共{}行、{}字，内容较长，以下为前 12000 字；如需其余部分请用 startLine/endLine 读取：\n{}",
                            total, char_count, head
                        ));
                    }
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
                .ok_or_else(|| anyhow::anyhow!("缺少 keyword 参数"))?
                .trim();
            if keyword.is_empty() {
                return Err(anyhow::anyhow!("keyword 不能为空"));
            }

            let use_regex = tool_call.arguments.get("regex")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let chapter_id = tool_call.arguments.get("chapterId").and_then(|v| v.as_str());

            let re = if use_regex {
                Some(regex::Regex::new(keyword).map_err(|e| anyhow::anyhow!("正则无效: {}", e))?)
            } else {
                None
            };

            // 普通搜索：自动展开角色别名（林动 / 小动 / 林兄 都能命中同一人）
            let terms = if use_regex {
                vec![keyword.to_string()]
            } else {
                expand_search_terms(book_id, keyword).await?
            };

            search_book_content(book_id, chapter_id, &terms, keyword, re.as_ref()).await
        }

        // 故事记忆相关工具
        "get_story_memory" => {
            let text = crate::services::story_memory_service::build_story_memory_text(book_id).await?;
            // 长篇故事圣经可能很长，限制返回体积，避免一次把上下文吃满
            Ok(truncate_text(&text, 12000))
        }
        // 设定账本（Canon Ledger）
        "get_canon" => {
            let names: Vec<String> = tool_call.arguments.get("names")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            if names.is_empty() {
                return Err(anyhow::anyhow!("缺少 names 参数"));
            }
            let as_of = tool_call.arguments.get("asOfChapter")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            crate::services::canon_service::get_canon(book_id, &names, as_of).await
        }
        "search_canon" => {
            let query = tool_call.arguments.get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let kind = tool_call.arguments.get("kind").and_then(|v| v.as_str());
            crate::services::canon_service::search_canon(book_id, query, kind).await
        }
        "get_canon_timeline" => {
            let name = tool_call.arguments.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if name.is_empty() {
                return Err(anyhow::anyhow!("缺少 name 参数"));
            }
            let from = tool_call.arguments.get("fromChapter")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            let to = tool_call.arguments.get("toChapter")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);
            crate::services::canon_service::get_canon_timeline(book_id, name, from, to).await
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
                let row = sqlx::query("SELECT id FROM volumes WHERE book_id = ? ORDER BY \"order\" DESC LIMIT 1")
                    .bind(book_id)
                    .fetch_optional(pool)
                    .await?;
                match row {
                    Some(r) => r.try_get::<String, _>("id")?,
                    None => return Err(anyhow::anyhow!("书籍 {} 没有卷，请先创建卷", book_id)),
                }
            };

            // 获取order：如果指定了就用指定的，否则自动计算（列名是 "order"，不是 order_num）
            let order = if let Some(ord) = tool_call.arguments.get("order").and_then(|v| v.as_i64()) {
                ord as i32
            } else {
                let pool = crate::db::get_pool().await?;
                let max_order: Option<i32> = sqlx::query_scalar(
                    "SELECT MAX(\"order\") FROM chapters WHERE book_id = ?1"
                )
                .bind(book_id)
                .fetch_one(pool)
                .await?;
                max_order.unwrap_or(-1) + 1
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

// ==================== 全文搜索（别名展开 + 排序 + 截断） ====================

/// 单章最多展示多少处命中
const MAX_SNIPPETS_PER_CHAPTER: usize = 6;
/// 一次搜索最多展示多少章
const MAX_CHAPTERS: usize = 8;
/// 一次搜索最多展示多少处命中
const MAX_TOTAL_SNIPPETS: usize = 30;
/// 单条命中片段最大字符数
const MAX_SNIPPET_CHARS: usize = 160;

/// 把关键词扩展到角色卡里的规范名 + 别名。
///
/// 例：搜「小动」时若角色卡是 `林动` 别名 `[小动, 林兄]`，
/// 则同时搜索这三个称呼，避免同一角色因称呼不同而漏查。
async fn expand_search_terms(book_id: &str, keyword: &str) -> anyhow::Result<Vec<String>> {
    let keyword = keyword.trim();
    let mut terms: Vec<String> = vec![keyword.to_string()];

    // 单字关键词展开噪音太大，直接原样搜
    if keyword.chars().count() < 2 {
        return Ok(terms);
    }

    let kw = keyword.to_lowercase();
    let pool = crate::db::get_pool().await?;
    let rows = sqlx::query("SELECT name, aliases FROM character_cards WHERE book_id = ?1")
        .bind(book_id)
        .fetch_all(pool)
        .await?;

    for row in rows {
        let name: String = row.try_get("name").unwrap_or_default();
        if name.trim().is_empty() {
            continue;
        }

        let mut names = vec![name.clone()];
        let aliases_raw: Option<String> = row.try_get("aliases").ok();
        if let Some(raw) = aliases_raw {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&raw) {
                for alias in list {
                    let alias = alias.trim().to_string();
                    if !alias.is_empty() {
                        names.push(alias);
                    }
                }
            }
        }

        // 关键词与任一称呼互相包含即视为同一角色
        // 单字称呼（如「林」）参与模糊匹配会引入大量噪音，跳过
        let hit = names.iter().any(|n| {
            let n_lower = n.to_lowercase();
            if n_lower.chars().count() < 2 {
                return false;
            }
            n_lower.contains(&kw) || kw.contains(&n_lower)
        });

        if hit {
            for n in names {
                if !terms.iter().any(|t| t.eq_ignore_ascii_case(&n)) {
                    terms.push(n);
                }
            }
        }
    }

    Ok(terms)
}

/// 全书/指定章节搜索，返回带行号和上下文的命中片段（有排序和数量上限）
async fn search_book_content(
    book_id: &str,
    chapter_id: Option<&str>,
    terms: &[String],
    raw_keyword: &str,
    re: Option<&regex::Regex>,
) -> anyhow::Result<String> {
    let pool = crate::db::get_pool().await?;

    let rows = match chapter_id {
        Some(cid) => {
            sqlx::query(
                r#"SELECT id, title, content, "order" FROM chapters WHERE id = ?1 AND book_id = ?2"#
            )
            .bind(cid)
            .bind(book_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                r#"SELECT id, title, content, "order" FROM chapters WHERE book_id = ?1 ORDER BY "order""#
            )
            .bind(book_id)
            .fetch_all(pool)
            .await?
        }
    };

    struct Hit {
        chapter_id: String,
        title: String,
        order: i32,
        total: usize,
        snippets: Vec<String>,
        title_matched: bool,
    }

    let term_lowers: Vec<String> = terms.iter().map(|t| t.to_lowercase()).collect();
    let mut hits: Vec<Hit> = Vec::new();
    let mut total_all: usize = 0;

    for row in rows {
        let id: String = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let content: String = row.try_get("content").unwrap_or_default();
        let order: i32 = row.try_get("order").unwrap_or(0);

        let title_lower = title.to_lowercase();
        let title_matched = re
            .map(|r| r.is_match(&title))
            .unwrap_or_else(|| term_lowers.iter().any(|t| title_lower.contains(t)));

        if content.is_empty() && !title_matched {
            continue;
        }

        let mut total = 0usize;
        let mut snippets: Vec<String> = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let matched = match re {
                Some(r) => r.is_match(line),
                None => {
                    let lower = line.to_lowercase();
                    term_lowers.iter().any(|t| lower.contains(t))
                }
            };
            if !matched {
                continue;
            }

            total += 1;
            if snippets.len() >= MAX_SNIPPETS_PER_CHAPTER {
                continue;
            }

            let trimmed = line.trim();
            let snippet: String = if trimmed.chars().count() > MAX_SNIPPET_CHARS {
                format!("{}…", trimmed.chars().take(MAX_SNIPPET_CHARS).collect::<String>())
            } else {
                trimmed.to_string()
            };
            snippets.push(format!("  第{}行: {}", idx + 1, snippet));
        }

        if total > 0 || title_matched {
            total_all += total;
            hits.push(Hit {
                chapter_id: id,
                title,
                order,
                total,
                snippets,
                title_matched,
            });
        }
    }

    if hits.is_empty() {
        return Ok(if chapter_id.is_some() {
            format!("在指定章节中未找到「{}」", raw_keyword)
        } else {
            format!("全书未找到「{}」", raw_keyword)
        });
    }

    // 排序：标题命中优先，其次命中次数多的，同分按章节顺序
    hits.sort_by(|a, b| {
        b.title_matched
            .cmp(&a.title_matched)
            .then(b.total.cmp(&a.total))
            .then(a.order.cmp(&b.order))
    });

    let mut out = String::new();
    if terms.len() > 1 {
        let others: Vec<&str> = terms.iter().skip(1).map(|s| s.as_str()).collect();
        out.push_str(&format!("已按角色别名扩展搜索：{}\n", others.join("、")));
    }

    out.push_str(&format!(
        "{}「{}」共 {} 处（命中 {} 章）\n",
        if chapter_id.is_some() { "指定章节中" } else { "全书" },
        raw_keyword,
        total_all,
        hits.len()
    ));

    let mut shown_chapters = 0usize;
    let mut shown_snippets = 0usize;

    for hit in &hits {
        if shown_chapters >= MAX_CHAPTERS || shown_snippets >= MAX_TOTAL_SNIPPETS {
            break;
        }

        out.push_str(&format!(
            "「{}」(ID: {}) 共 {} 处\n",
            hit.title, hit.chapter_id, hit.total
        ));

        for snippet in &hit.snippets {
            if shown_snippets >= MAX_TOTAL_SNIPPETS {
                break;
            }
            out.push_str(snippet);
            out.push('\n');
            shown_snippets += 1;
        }

        shown_chapters += 1;
    }

    if total_all > shown_snippets || hits.len() > shown_chapters {
        out.push_str(&format!(
            "（结果已截断，仅展示前 {} 章 / {} 处；可用更具体的关键词，或指定 chapterId 精确查询）\n",
            shown_chapters, shown_snippets
        ));
    }

    Ok(out.trim_end().to_string())
}

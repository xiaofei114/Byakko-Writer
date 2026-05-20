use crate::models::ToolCall;
use serde_json::Value;
use sqlx::Row;

/// 检查内容是否包含工具调用
pub fn contains_tool_calls(content: &str) -> bool {
    // 检查 XML 格式（支持 <tool name="..."> 和 <tool> 两种格式）
    if (content.contains("<tool ") || content.contains("<tool>")) && content.contains("</tool>") {
        return true;
    }
    // 检查 Markdown 代码块格式
    if content.contains("```tool:") {
        return true;
    }
    // 检查 JSON 格式
    if content.contains("\"tool\"") && content.contains("\"arguments\"") {
        return true;
    }
    false
}

/// 解析 AI 响应中的工具调用
/// 
/// 支持的格式：
/// 1. XML 格式：<tool name="tool_name">{"arg": "value"}</tool>
/// 2. Markdown 代码块：```tool:tool_name\n{"arg": "value"}\n```
/// 3. JSON 格式：{"tool": "tool_name", "arguments": {...}}
pub fn parse_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    
    // 尝试解析 XML 格式
    tool_calls.extend(parse_xml_tool_calls(content));
    
    // 尝试解析 Markdown 代码块格式
    tool_calls.extend(parse_markdown_tool_calls(content));
    
    // 尝试解析 JSON 格式
    tool_calls.extend(parse_json_tool_calls(content));
    
    tool_calls
}

/// 解析 XML 格式的工具调用
/// 支持格式：
/// - <tool name="query_chapter_summary">{"chapterId": "xxx"}</tool>
/// - <tool name='query_chapter_summary'>...</tool>
fn parse_xml_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    
    // 使用正则表达式解析更健壮的格式
    let re = regex::Regex::new(r#"<tool\s+name=["']([^"']+)["']\s*>([\s\S]*?)</tool>"#).unwrap();
    
    for cap in re.captures_iter(content) {
        if let (Some(name_match), Some(args_match)) = (cap.get(1), cap.get(2)) {
            let tool_name = name_match.as_str().to_string();
            let args_str = args_match.as_str().trim();
            
            // 尝试解析 JSON 参数
            if let Ok(arguments) = serde_json::from_str::<Value>(args_str) {
                tool_calls.push(ToolCall {
                    name: tool_name,
                    arguments,
                });
            } else {
                log::warn!("[Tool Call] 无法解析工具参数: tool={}, args={}", tool_name, args_str);
            }
        }
    }
    
    tool_calls
}

/// 解析 Markdown 代码块格式的工具调用
/// ```tool:query_chapter_summary
/// {"chapterId": "xxx"}
/// ```
fn parse_markdown_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    
    // 手动解析 Markdown 代码块
    let prefix = "```tool:";
    let mut start = 0;
    
    while let Some(block_start) = content[start..].find(prefix) {
        let absolute_start = start + block_start;
        let name_start = absolute_start + prefix.len();
        
        if let Some(newline_pos) = content[name_start..].find('\n') {
            let tool_name = content[name_start..name_start + newline_pos].trim().to_string();
            let args_start = name_start + newline_pos + 1;
            
            if let Some(block_end) = content[args_start..].find("```") {
                let args_str = &content[args_start..args_start + block_end];
                
                if let Ok(arguments) = serde_json::from_str::<Value>(args_str.trim()) {
                    tool_calls.push(ToolCall {
                        name: tool_name,
                        arguments,
                    });
                }
                
                start = args_start + block_end + 3;
                continue;
            }
        }
        start = absolute_start + prefix.len();
    }
    
    tool_calls
}

/// 解析 JSON 格式的工具调用
/// {"tool": "query_chapter_summary", "arguments": {"chapterId": "xxx"}}
fn parse_json_tool_calls(content: &str) -> Vec<ToolCall> {
    let mut tool_calls = Vec::new();
    
    // 尝试直接解析整个内容为工具调用
    if let Ok(json) = serde_json::from_str::<Value>(content) {
        if let (Some(tool), Some(args)) = (json.get("tool"), json.get("arguments")) {
            if let (Some(tool_name), Some(args_obj)) = (tool.as_str(), args.as_object()) {
                tool_calls.push(ToolCall {
                    name: tool_name.to_string(),
                    arguments: Value::Object(args_obj.clone()),
                });
            }
        }
    }
    
    tool_calls
}

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
            Ok(content)
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
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("缺少 cardId 参数"))?;
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



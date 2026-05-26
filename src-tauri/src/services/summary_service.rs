use crate::db::get_pool;
use crate::models::{ChapterSummary, SummaryResponse, AIConfig};
use crate::services::ai_service::call_ai_for_summary;
use crate::services::prompt_service::get_prompt_manager;
use sqlx::Row;

/// 生成章节摘要
pub async fn generate_chapter_summary(
    chapter_id: String,
    chapter_title: String,
    content: String,
    config: AIConfig,
) -> anyhow::Result<ChapterSummary> {
    if content.len() < 50 {
        return Err(anyhow::anyhow!("章节内容太短，无法生成摘要"));
    }

    // 构建 Prompt
    let prompt = format!(
        "章节标题：{}\n\n章节内容：{}\n\n请生成摘要：",
        chapter_title,
        content
    );

    // 从提示词管理器获取系统提示词
    let prompt_manager = get_prompt_manager();
    let system_prompt = prompt_manager.get_summary_system_prompt()
        .map_err(|e| anyhow::anyhow!("获取摘要提示词失败: {}", e))?;

    // 调用 AI
    let summary_json = call_ai_for_summary(&prompt, &system_prompt, &config).await?;
    
    log::debug!("AI 返回的 JSON: {}", truncate_str(&summary_json, 500));

    // 解析 JSON 响应
    let summary_data: SummaryResponse = serde_json::from_str(&summary_json)
        .map_err(|e| {
            log::error!("解析 AI 响应失败: {}, JSON内容: {}", e, truncate_str(&summary_json, 200));
            anyhow::anyhow!("解析 AI 响应失败: {}", e)
        })?;
    
    log::debug!("解析成功 - 短摘要: {}, 长摘要长度: {}", 
        summary_data.short_summary, 
        summary_data.long_summary.len()
    );

    let summary = ChapterSummary {
        id: format!("summary_{}", chapter_id),
        chapter_id: chapter_id.clone(),
        short_summary: summary_data.short_summary,
        long_summary: summary_data.long_summary,
        tags: summary_data.tags,
        characters: summary_data.characters,
        locations: summary_data.locations,
        events: summary_data.events,
        generated_at: chrono::Utc::now().timestamp(),
        is_confirmed: false,
    };

    // 保存到数据库
    if let Err(e) = save_summary_to_db(&summary).await {
        log::error!("保存摘要失败: {}", e);
    }

    Ok(summary)
}

/// 保存摘要到数据库
async fn save_summary_to_db(summary: &ChapterSummary) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    let tags_json = serde_json::to_string(&summary.tags)?;
    let characters_json = serde_json::to_string(&summary.characters)?;
    let locations_json = serde_json::to_string(&summary.locations)?;
    let events_json = serde_json::to_string(&summary.events)?;
    
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO chapter_summaries 
        (id, chapter_id, short_summary, long_summary, tags, characters, locations, events, generated_at, is_confirmed)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#
    )
    .bind(&summary.id)
    .bind(&summary.chapter_id)
    .bind(&summary.short_summary)
    .bind(&summary.long_summary)
    .bind(&tags_json)
    .bind(&characters_json)
    .bind(&locations_json)
    .bind(&events_json)
    .bind(summary.generated_at)
    .bind(summary.is_confirmed)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 加载章节摘要
pub async fn load_chapter_summary(chapter_id: String) -> anyhow::Result<Option<ChapterSummary>> {
    let pool = get_pool().await?;
    
    let row = sqlx::query(
        r#"
        SELECT id, chapter_id, short_summary, long_summary, tags, characters, locations, events, generated_at, is_confirmed
        FROM chapter_summaries
        WHERE chapter_id = ?1
        "#
    )
    .bind(&chapter_id)
    .fetch_optional(pool)
    .await?;
    
    if let Some(row) = row {
        let tags_json: String = row.try_get("tags")?;
        let characters_json: String = row.try_get("characters")?;
        let locations_json: String = row.try_get("locations")?;
        let events_json: String = row.try_get("events")?;
        
        let summary = ChapterSummary {
            id: row.try_get("id")?,
            chapter_id: row.try_get("chapter_id")?,
            short_summary: row.try_get("short_summary")?,
            long_summary: row.try_get("long_summary")?,
            tags: serde_json::from_str(&tags_json)?,
            characters: serde_json::from_str(&characters_json)?,
            locations: serde_json::from_str(&locations_json)?,
            events: serde_json::from_str(&events_json)?,
            generated_at: row.try_get("generated_at")?,
            is_confirmed: row.try_get("is_confirmed")?,
        };
        
        Ok(Some(summary))
    } else {
        Ok(None)
    }
}

/// 查询章节详细摘要
pub async fn query_chapter_summary(chapter_id: &str) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    
    let row = sqlx::query(
        r#"
        SELECT c.title, s.short_summary, s.long_summary, s.tags, s.characters, s.locations, s.events
        FROM chapters c
        LEFT JOIN chapter_summaries s ON c.id = s.chapter_id
        WHERE c.id = ?
        "#
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Ok(format!("章节 {} 不存在或没有摘要", chapter_id)),
    };

    let title: String = row.try_get("title")?;
    let short_summary: Option<String> = row.try_get("short_summary").ok();
    let long_summary: Option<String> = row.try_get("long_summary").ok();
    let tags: Option<String> = row.try_get("tags").ok();
    let characters: Option<String> = row.try_get("characters").ok();
    let locations: Option<String> = row.try_get("locations").ok();
    let events: Option<String> = row.try_get("events").ok();
    
    let result = format!(
        "章节：{}\n短摘要：{}\n详细摘要：{}\n标签：{}\n角色：{}\n地点：{}\n事件：{}",
        title,
        short_summary.unwrap_or_default(),
        long_summary.unwrap_or_default(),
        tags.unwrap_or_default(),
        characters.unwrap_or_default(),
        locations.unwrap_or_default(),
        events.unwrap_or_default()
    );
    
    Ok(result)
}

/// 查询完整章节内容
pub async fn query_chapter_content(chapter_id: &str) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    
    let row = sqlx::query(
        "SELECT title, content FROM chapters WHERE id = ?"
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Ok(format!("章节 {} 不存在", chapter_id)),
    };

    let title: String = row.try_get("title")?;
    let content: String = row.try_get("content")?;
    
    Ok(format!("章节：{}\n\n{}", title, content))
}

/// 安全地截断字符串（按字符数，避免字节边界问题）
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

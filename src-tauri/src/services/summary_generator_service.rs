use crate::db::get_pool;
use crate::models::{AIConfig, ChapterSummary};
use crate::services::ai_service::call_ai_for_summary;
use crate::services::prompt_service::get_prompt_manager;
use crate::services::summary_service;
use sqlx::Row;
use std::collections::HashSet;
use std::sync::Mutex;
use once_cell::sync::Lazy;

/// 正在生成的章节ID集合（防止重复生成）
static GENERATING_CHAPTERS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| {
    Mutex::new(HashSet::new())
});

/// 检查章节是否正在生成摘要
fn is_generating(chapter_id: &str) -> bool {
    GENERATING_CHAPTERS.lock()
        .map(|set| set.contains(chapter_id))
        .unwrap_or(false)
}

/// 标记章节开始生成
fn mark_generating(chapter_id: &str) -> bool {
    GENERATING_CHAPTERS.lock()
        .map(|mut set| set.insert(chapter_id.to_string()))
        .unwrap_or(false)
}

/// 标记章节生成完成
fn mark_complete(chapter_id: &str) {
    let _ = GENERATING_CHAPTERS.lock()
        .map(|mut set| { set.remove(chapter_id); });
}

/// 获取章节信息
async fn get_chapter_info(chapter_id: &str) -> anyhow::Result<(String, String, String)> {
    let pool = get_pool().await?;
    
    let row = sqlx::query(
        "SELECT book_id, title, content FROM chapters WHERE id = ?"
    )
    .bind(chapter_id)
    .fetch_one(pool)
    .await?;
    
    let book_id: String = row.try_get("book_id")?;
    let title: String = row.try_get("title")?;
    let content: String = row.try_get("content")?;
    
    Ok((book_id, title, content))
}

/// 检查章节是否已有摘要
async fn has_summary(chapter_id: &str) -> anyhow::Result<bool> {
    let pool = get_pool().await?;
    
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chapter_summaries WHERE chapter_id = ?"
    )
    .bind(chapter_id)
    .fetch_one(pool)
    .await?;
    
    Ok(count > 0)
}

/// 获取所有没有摘要的章节
async fn get_chapters_without_summary(book_id: &str) -> anyhow::Result<Vec<(String, String, String)>> {
    let pool = get_pool().await?;
    
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.title, c.content
        FROM chapters c
        LEFT JOIN chapter_summaries s ON c.id = s.chapter_id
        WHERE c.book_id = ? AND s.id IS NULL AND LENGTH(c.content) >= 50
        ORDER BY c.created_at ASC
        "#
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    
    let mut chapters = Vec::new();
    for row in rows {
        let id: String = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let content: String = row.try_get("content")?;
        chapters.push((id, title, content));
    }
    
    Ok(chapters)
}

/// 生成单个章节摘要（带锁保护）
pub async fn generate_chapter_summary_with_lock(
    chapter_id: String,
    config: AIConfig,
) -> anyhow::Result<ChapterSummary> {
    // 检查是否已在生成中
    if is_generating(&chapter_id) {
        return Err(anyhow::anyhow!("章节 {} 的摘要正在生成中", chapter_id));
    }
    
    // 检查是否已有摘要
    if has_summary(&chapter_id).await? {
        return Err(anyhow::anyhow!("章节 {} 已有摘要", chapter_id));
    }
    
    // 标记为生成中
    if !mark_generating(&chapter_id) {
        return Err(anyhow::anyhow!("章节 {} 的摘要正在生成中", chapter_id));
    }
    
    // 确保完成后清除标记
    let result = generate_chapter_summary_inner(chapter_id.clone(), config).await;
    mark_complete(&chapter_id);
    
    result
}

/// 内部：实际生成摘要
async fn generate_chapter_summary_inner(
    chapter_id: String,
    config: AIConfig,
) -> anyhow::Result<ChapterSummary> {
    // 获取章节信息
    let (_book_id, title, content) = get_chapter_info(&chapter_id).await?;
    
    if content.len() < 50 {
        return Err(anyhow::anyhow!("章节内容太短，无法生成摘要"));
    }
    
    log::info!("[SummaryGenerator] 开始生成章节摘要: {} - {}", chapter_id, title);
    
    // 构建 Prompt
    let prompt = format!(
        "章节标题：{}\n\n章节内容：{}\n\n请生成摘要：",
        title,
        content
    );
    
    // 获取系统提示词
    let prompt_manager = get_prompt_manager();
    let system_prompt = prompt_manager.get_summary_system_prompt()
        .map_err(|e| anyhow::anyhow!("获取摘要提示词失败: {}", e))?;
    
    // 调用 AI
    let summary_json = call_ai_for_summary(&prompt, &system_prompt, &config).await?;
    
    log::debug!("[SummaryGenerator] AI 返回: {}", &summary_json[..summary_json.len().min(500)]);
    
    // 解析 JSON
    let summary_data: crate::models::SummaryResponse = serde_json::from_str(&summary_json)
        .map_err(|e| {
            log::error!("[SummaryGenerator] 解析失败: {}, JSON: {}", e, &summary_json[..summary_json.len().min(200)]);
            anyhow::anyhow!("解析 AI 响应失败: {}", e)
        })?;
    
    let summary = ChapterSummary {
        id: format!("summary_{}", chapter_id),
        chapter_id: chapter_id.clone(),
        short_summary: summary_data.short_summary,
        long_summary: summary_data.long_summary,
        tags: summary_data.tags,
        characters: summary_data.characters,
        locations: summary_data.locations,
        events: summary_data.events,
        plot_progression: summary_data.plot_progression,
        emotional_beats: summary_data.emotional_beats,
        foreshadowing: summary_data.foreshadowing,
        unresolved_threads: summary_data.unresolved_threads,
        generated_at: chrono::Utc::now().timestamp(),
        is_confirmed: false,
    };
    
    // 保存到数据库
    summary_service::save_summary_to_db(&summary).await?;
    
    log::info!("[SummaryGenerator] 章节摘要生成完成: {}", chapter_id);
    
    Ok(summary)
}

/// 后台生成所有缺失的摘要（创建新章节后调用）
pub async fn generate_missing_summaries_background(
    book_id: String,
    config: AIConfig,
) -> anyhow::Result<SummaryGenerationResult> {
    // 获取所有没有摘要的章节
    let chapters = get_chapters_without_summary(&book_id).await?;
    
    if chapters.is_empty() {
        return Ok(SummaryGenerationResult {
            total: 0,
            success: 0,
            failed: 0,
            skipped: 0,
            message: "没有需要生成摘要的章节".to_string(),
        });
    }
    
    let total = chapters.len();
    log::info!("[SummaryGenerator] 开始批量生成 {} 个章节的摘要", total);
    
    // 使用 tokio::spawn 并发处理
    let mut handles = Vec::new();
    
    for (chapter_id, _title, _content) in chapters {
        // 检查是否已在生成中
        if is_generating(&chapter_id) {
            log::debug!("[SummaryGenerator] 跳过正在生成中的章节: {}", chapter_id);
            continue;
        }
        
        // 检查是否已有摘要
        if has_summary(&chapter_id).await.unwrap_or(false) {
            log::debug!("[SummaryGenerator] 跳过已有摘要的章节: {}", chapter_id);
            continue;
        }
        
        let config_clone = config.clone();
        let chapter_id_clone = chapter_id.clone();
        
        let handle = tokio::spawn(async move {
            match generate_chapter_summary_with_lock(chapter_id_clone.clone(), config_clone).await {
                Ok(_) => {
                    log::info!("[SummaryGenerator] 成功: {}", chapter_id_clone);
                    (true, false)
                }
                Err(e) => {
                    log::error!("[SummaryGenerator] 失败: {} - {}", chapter_id_clone, e);
                    (false, true)
                }
            }
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    let mut success = 0;
    let mut failed = 0;
    
    for handle in handles {
        match handle.await {
            Ok((s, f)) => {
                if s { success += 1; }
                if f { failed += 1; }
            }
            Err(e) => {
                log::error!("[SummaryGenerator] 任务执行失败: {}", e);
                failed += 1;
            }
        }
    }
    
    let skipped = total - success - failed;
    let message = format!(
        "批量生成完成: 总计 {}, 成功 {}, 失败 {}, 跳过 {}",
        total, success, failed, skipped
    );
    
    log::info!("[SummaryGenerator] {}", message);
    
    Ok(SummaryGenerationResult {
        total,
        success,
        failed,
        skipped,
        message,
    })
}

/// 批量生成指定章节的摘要（带并发限制）
pub async fn batch_generate_summaries(
    chapter_ids: Vec<String>,
    config: AIConfig,
    max_concurrent: usize,
) -> anyhow::Result<SummaryGenerationResult> {
    let total = chapter_ids.len();
    
    if total == 0 {
        return Ok(SummaryGenerationResult {
            total: 0,
            success: 0,
            failed: 0,
            skipped: 0,
            message: "没有需要生成摘要的章节".to_string(),
        });
    }
    
    log::info!("[SummaryGenerator] 批量生成 {} 个章节摘要, 并发数: {}", total, max_concurrent);
    
    use futures::stream::{self, StreamExt};
    
    let results = stream::iter(chapter_ids)
        .map(|chapter_id| {
            let config = config.clone();
            async move {
                // 检查是否已在生成中
                if is_generating(&chapter_id) {
                    return (chapter_id, false, true); // skipped
                }
                
                // 检查是否已有摘要
                if has_summary(&chapter_id).await.unwrap_or(false) {
                    return (chapter_id, false, true); // skipped
                }
                
                match generate_chapter_summary_with_lock(chapter_id.clone(), config).await {
                    Ok(_) => (chapter_id, true, false),
                    Err(e) => {
                        log::error!("[SummaryGenerator] 生成失败 {}: {}", chapter_id, e);
                        (chapter_id, false, false)
                    }
                }
            }
        })
        .buffer_unordered(max_concurrent)
        .collect::<Vec<_>>()
        .await;
    
    let success = results.iter().filter(|(_, s, _)| *s).count();
    let skipped = results.iter().filter(|(_, _, sk)| *sk).count();
    let failed = total - success - skipped;
    
    let message = format!(
        "批量生成完成: 总计 {}, 成功 {}, 失败 {}, 跳过 {}",
        total, success, failed, skipped
    );
    
    log::info!("[SummaryGenerator] {}", message);
    
    Ok(SummaryGenerationResult {
        total,
        success,
        failed,
        skipped,
        message,
    })
}

/// 生成结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryGenerationResult {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped: usize,
    pub message: String,
}

/// 获取生成状态
pub fn get_generation_status() -> GenerationStatus {
    let count = GENERATING_CHAPTERS.lock()
        .map(|set| set.len())
        .unwrap_or(0);
    
    GenerationStatus {
        active_generations: count,
        is_busy: count > 0,
    }
}

/// 生成状态
#[derive(Debug, Clone, serde::Serialize)]
pub struct GenerationStatus {
    pub active_generations: usize,
    pub is_busy: bool,
}

/// 为 Story Bible 生成器提供的简化接口
/// 生成单个章节摘要（不检查锁，外部已确保不会重复）
pub async fn generate_chapter_summary_for_story_bible(
    _book_id: &str,
    chapter_id: &str,
    title: &str,
    config: &AIConfig,
) -> anyhow::Result<ChapterSummary> {
    // 获取章节内容
    let pool = get_pool().await?;
    let row = sqlx::query("SELECT content FROM chapters WHERE id = ?")
        .bind(chapter_id)
        .fetch_one(pool)
        .await?;
    let content: String = row.try_get("content")?;
    
    if content.len() < 50 {
        return Err(anyhow::anyhow!("章节内容太短"));
    }
    
    // 使用 summary_service 生成
    crate::services::summary_service::generate_chapter_summary(
        chapter_id.to_string(),
        title.to_string(),
        content,
        config.clone(),
    ).await
}

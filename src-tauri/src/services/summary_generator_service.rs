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

/// 自动摘要调度表：章节ID -> 最近一次保存时间戳（毫秒）
static AUTO_SUMMARY_TS: Lazy<Mutex<std::collections::HashMap<String, i64>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

/// 保存章节后调度一次自动摘要
///
/// - 防抖 20 秒，期间重复保存会取消上一轮调度
/// - 内容少于 600 字、已有摘要、未配置 API Key 时跳过
/// - 真正生成时复用 `generate_chapter_summary_with_lock` 的并发锁
pub fn schedule_auto_summary(chapter_id: String) {
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(mut guard) = AUTO_SUMMARY_TS.lock() {
        guard.insert(chapter_id.clone(), now);
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;

        // 期间又保存过 → 放弃本轮调度，由最后一次保存负责
        let is_latest = AUTO_SUMMARY_TS
            .lock()
            .map(|guard| guard.get(&chapter_id).copied() == Some(now))
            .unwrap_or(false);
        if !is_latest {
            return;
        }

        let config = match crate::services::config_service::load_config() {
            Ok(config) => config,
            Err(_) => return,
        };
        if config.ai.api_key.is_empty() {
            return;
        }

        let (_, _, content) = match get_chapter_info(&chapter_id).await {
            Ok(info) => info,
            Err(_) => return,
        };
        if content.chars().count() < 600 {
            return;
        }
        if has_summary(&chapter_id).await.unwrap_or(false) {
            return;
        }

        match generate_chapter_summary_with_lock(chapter_id.clone(), config.ai).await {
            Ok(_) => log::info!("[SummaryGenerator] 自动摘要完成: {}", chapter_id),
            Err(e) => log::debug!("[SummaryGenerator] 自动摘要跳过/失败: {} - {}", chapter_id, e),
        }
    });
}

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

/// 获取没有摘要的章节；传入 only_chapter_id 时只返回该章
async fn get_chapters_without_summary(
    book_id: &str,
    only_chapter_id: Option<&str>,
) -> anyhow::Result<Vec<(String, String, String)>> {
    let pool = get_pool().await?;

    let rows = match only_chapter_id {
        Some(chapter_id) => {
            sqlx::query(
                r#"
                SELECT c.id, c.title, c.content
                FROM chapters c
                LEFT JOIN chapter_summaries s ON c.id = s.chapter_id
                WHERE c.book_id = ?1 AND c.id = ?2 AND s.id IS NULL AND LENGTH(c.content) >= 50
                "#
            )
            .bind(book_id)
            .bind(chapter_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(
                r#"
                SELECT c.id, c.title, c.content
                FROM chapters c
                LEFT JOIN chapter_summaries s ON c.id = s.chapter_id
                WHERE c.book_id = ?1 AND s.id IS NULL AND LENGTH(c.content) >= 50
                ORDER BY c.created_at ASC
                "#
            )
            .bind(book_id)
            .fetch_all(pool)
            .await?
        }
    };

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

/// 后台生成缺失的摘要
///
/// - `only_chapter_id = Some(id)`：只处理指定章节（建章时使用，避免全库扫描）
/// - `only_chapter_id = None`：补齐全书缺失摘要，并发受限（最多 4 个同时请求）
pub async fn generate_missing_summaries_background(
    book_id: String,
    config: AIConfig,
    only_chapter_id: Option<String>,
) -> anyhow::Result<SummaryGenerationResult> {
    // 获取没有摘要的章节
    let chapters = get_chapters_without_summary(&book_id, only_chapter_id.as_deref()).await?;

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

    // 过滤掉正在生成/已有摘要的章节
    let mut pending = Vec::new();
    for (chapter_id, _title, _content) in chapters {
        if is_generating(&chapter_id) {
            log::debug!("[SummaryGenerator] 跳过正在生成中的章节: {}", chapter_id);
            continue;
        }
        if has_summary(&chapter_id).await.unwrap_or(false) {
            log::debug!("[SummaryGenerator] 跳过已有摘要的章节: {}", chapter_id);
            continue;
        }
        pending.push(chapter_id);
    }

    // 并发上限：避免一次创建几十个 AI 请求触发限流
    const MAX_CONCURRENCY: usize = 4;
    let mut success = 0;
    let mut failed = 0;

    for chunk in pending.chunks(MAX_CONCURRENCY) {
        let mut handles = Vec::new();
        for chapter_id in chunk {
            let config_clone = config.clone();
            let chapter_id_clone = chapter_id.clone();
            handles.push(tokio::spawn(async move {
                match generate_chapter_summary_with_lock(chapter_id_clone.clone(), config_clone).await {
                    Ok(_) => {
                        log::info!("[SummaryGenerator] 成功: {}", chapter_id_clone);
                        true
                    }
                    Err(e) => {
                        log::error!("[SummaryGenerator] 失败: {} - {}", chapter_id_clone, e);
                        false
                    }
                }
            }));
        }

        for handle in handles {
            match handle.await {
                Ok(true) => success += 1,
                Ok(false) => failed += 1,
                Err(e) => {
                    log::error!("[SummaryGenerator] 任务执行失败: {}", e);
                    failed += 1;
                }
            }
        }
    }

    let skipped = total.saturating_sub(success + failed);
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

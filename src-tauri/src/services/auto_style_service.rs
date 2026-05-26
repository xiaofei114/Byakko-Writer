use crate::db::get_pool;
use crate::models::LearnStyleParams;
use crate::services::style_service::learn_writing_style;
use sqlx::Row;
use tokio::time::{interval, Duration};

/// 自动风格分析配置
const MIN_WORDS_FOR_ANALYSIS: i64 = 3000; // 最少需要3000字才分析
const ANALYSIS_INTERVAL_HOURS: u64 = 24; // 每24小时检查一次
const MIN_DAYS_BETWEEN_ANALYSIS: i64 = 7; // 最少间隔7天重新分析

/// 启动自动风格分析服务
pub async fn start_auto_style_service() {
    log::info!("[Auto Style] 启动自动风格分析服务");
    
    let mut ticker = interval(Duration::from_secs(ANALYSIS_INTERVAL_HOURS * 3600));
    
    // 首次运行延迟5分钟，避免应用启动时立即执行
    tokio::time::sleep(Duration::from_secs(300)).await;
    
    loop {
        ticker.tick().await;
        
        if let Err(e) = check_and_analyze_styles().await {
            log::error!("[Auto Style] 自动分析失败: {}", e);
        }
    }
}

/// 检查并分析需要更新的风格
async fn check_and_analyze_styles() -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp();
    
    // 1. 检查全局风格是否需要更新
    let global_needs_update = check_global_style_needs_update(pool, now).await?;
    if global_needs_update {
        log::info!("[Auto Style] 全局风格需要更新，开始分析...");
        match learn_writing_style(LearnStyleParams {
            book_id: None,
            chapter_ids: None,
            force_relearn: false,
        }).await {
            Ok(result) => {
                if result.success {
                    log::info!("[Auto Style] 全局风格分析完成，分析了 {} 个章节，共 {} 字", 
                        result.analyzed_chapters, result.total_word_count);
                } else {
                    log::warn!("[Auto Style] 全局风格分析未完成: {}", result.message);
                }
            }
            Err(e) => {
                log::error!("[Auto Style] 全局风格分析失败: {}", e);
            }
        }
    }
    
    // 2. 检查各书籍的风格是否需要更新
    let books_needing_update = get_books_needing_style_update(pool, now).await?;
    for book_id in books_needing_update {
        log::info!("[Auto Style] 书籍 {} 的风格需要更新，开始分析...", book_id);
        match learn_writing_style(LearnStyleParams {
            book_id: Some(book_id.clone()),
            chapter_ids: None,
            force_relearn: false,
        }).await {
            Ok(result) => {
                if result.success {
                    log::info!("[Auto Style] 书籍 {} 风格分析完成，分析了 {} 个章节，共 {} 字", 
                        book_id, result.analyzed_chapters, result.total_word_count);
                } else {
                    log::warn!("[Auto Style] 书籍 {} 风格分析未完成: {}", book_id, result.message);
                }
            }
            Err(e) => {
                log::error!("[Auto Style] 书籍 {} 风格分析失败: {}", book_id, e);
            }
        }
    }
    
    // 3. 检查各书籍是否需要冲突检测（仅提示，不自动执行，用户点按钮触发）
    let books_to_check = get_books_needing_conflict_check(pool).await?;
    for book_id in books_to_check {
        log::info!("[Auto Style] 书籍 {} 字数增长较多，建议手动检查设定冲突", book_id);
    }

    Ok(())
}

/// 获取需要冲突检测的书籍列表（仅检查阈值，不做实际检测）
async fn get_books_needing_conflict_check(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> anyhow::Result<Vec<String>> {
    let books = sqlx::query_scalar::<_, String>("SELECT id FROM books")
        .fetch_all(pool)
        .await?;

    let mut needs_check = Vec::new();
    for book_id in books {
        match crate::services::conflict_service::check_should_detect(&book_id).await {
            Ok(true) => needs_check.push(book_id),
            Ok(false) => {},
            Err(e) => log::error!("[Auto Style] 检查书籍 {} 时出错: {}", book_id, e),
        }
    }
    Ok(needs_check)
}

/// 检查全局风格是否需要更新
async fn check_global_style_needs_update(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    now: i64,
) -> anyhow::Result<bool> {
    // 获取全局风格的最后更新时间
    let last_update: Option<i64> = sqlx::query_scalar(
        "SELECT updated_at FROM user_writing_styles ORDER BY updated_at DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;
    
    // 如果没有风格记录，检查是否有足够内容可以分析
    if last_update.is_none() {
        let total_words: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters"
        )
        .fetch_one(pool)
        .await?;
        
        return Ok(total_words >= MIN_WORDS_FOR_ANALYSIS * 2); // 全局需要更多内容
    }
    
    // 检查是否超过最小间隔时间
    let last_update = last_update.unwrap();
    let days_since_update = (now - last_update) / 86400;
    
    if days_since_update < MIN_DAYS_BETWEEN_ANALYSIS {
        return Ok(false); // 间隔太短，不需要更新
    }
    
    // 检查是否有新内容产生
    let new_content_words: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters 
        WHERE updated_at > ?1
        "#
    )
    .bind(last_update)
    .fetch_one(pool)
    .await?;
    
    Ok(new_content_words >= MIN_WORDS_FOR_ANALYSIS)
}

/// 获取需要更新风格的书籍列表
async fn get_books_needing_style_update(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    now: i64,
) -> anyhow::Result<Vec<String>> {
    let mut books_needing_update = Vec::new();
    
    // 获取所有书籍
    let book_rows = sqlx::query("SELECT id FROM books")
        .fetch_all(pool)
        .await?;
    
    for row in book_rows {
        let book_id: String = row.get("id");
        
        // 检查该书籍是否有风格记录
        let last_update: Option<i64> = sqlx::query_scalar(
            "SELECT updated_at FROM book_writing_styles WHERE book_id = ?1"
        )
        .bind(&book_id)
        .fetch_optional(pool)
        .await?;
        
        let needs_update = if let Some(last_update) = last_update {
            // 有风格记录，检查是否需要更新
            let days_since_update = (now - last_update) / 86400;
            
            if days_since_update >= MIN_DAYS_BETWEEN_ANALYSIS {
                // 检查是否有新内容
                let new_content_words: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters 
                    WHERE book_id = ?1 AND updated_at > ?2
                    "#
                )
                .bind(&book_id)
                .bind(last_update)
                .fetch_one(pool)
                .await?;
                
                new_content_words >= MIN_WORDS_FOR_ANALYSIS
            } else {
                false
            }
        } else {
            // 没有风格记录，检查是否有足够内容
            let total_words: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters WHERE book_id = ?1"
            )
            .bind(&book_id)
            .fetch_one(pool)
            .await?;
            
            total_words >= MIN_WORDS_FOR_ANALYSIS
        };
        
        if needs_update {
            books_needing_update.push(book_id);
        }
    }
    
    Ok(books_needing_update)
}

/// 手动触发风格分析（供前端调用）
pub async fn trigger_manual_style_analysis(book_id: Option<String>) -> anyhow::Result<String> {
    let params = LearnStyleParams {
        book_id: book_id.clone(),
        chapter_ids: None,
        force_relearn: true, // 手动触发强制重新学习
    };
    
    match learn_writing_style(params).await {
        Ok(result) => {
            if result.success {
                Ok(format!("风格分析完成！分析了 {} 个章节，共 {} 字", 
                    result.analyzed_chapters, result.total_word_count))
            } else {
                Ok(format!("分析未完成: {}", result.message))
            }
        }
        Err(e) => Err(anyhow::anyhow!("分析失败: {}", e)),
    }
}

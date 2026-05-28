use crate::db::get_pool;
use crate::models::{ConflictDetectionStatusEvent, LearnStyleParams};
use crate::services::style_service::learn_writing_style;
use sqlx::Row;
use tauri::{AppHandle, Emitter};
use tokio::time::{interval, Duration};

/// 自动风格分析配置
const MIN_WORDS_FOR_ANALYSIS: i64 = 3000; // 最少需要3000字才分析
const ANALYSIS_INTERVAL_HOURS: u64 = 24; // 每24小时检查一次
const MIN_DAYS_BETWEEN_ANALYSIS: i64 = 7; // 最少间隔7天重新分析

fn classify_conflict_error(error: &str) -> (&'static str, String) {
    let lower = error.to_lowercase();

    if lower.contains("402") || lower.contains("insufficient balance") || lower.contains("余额") {
        (
            "billing",
            "自动设定冲突检测失败：AI 服务余额不足，请充值或更换 API 配置后重试。".to_string(),
        )
    } else if lower.contains("401") || lower.contains("unauthorized") || lower.contains("api key") {
        (
            "auth",
            "自动设定冲突检测失败：API Key 无效或未授权，请检查 AI 设置。".to_string(),
        )
    } else if lower.contains("429") || lower.contains("rate limit") {
        (
            "rate_limit",
            "自动设定冲突检测暂时受限：请求过于频繁，请稍后再试。".to_string(),
        )
    } else if lower.contains("timeout") || lower.contains("connection") || lower.contains("dns") || lower.contains("network") {
        (
            "network",
            "自动设定冲突检测失败：无法连接 AI 服务，请检查网络或 API 地址。".to_string(),
        )
    } else if lower.contains("api错误") || lower.contains("api error") || lower.contains("server") || lower.contains("model") {
        (
            "provider",
            "自动设定冲突检测失败：AI 服务返回错误，请检查模型和服务状态。".to_string(),
        )
    } else {
        (
            "unknown",
            "自动设定冲突检测失败，请稍后重试或检查 AI 设置。".to_string(),
        )
    }
}

fn emit_conflict_status(app: &AppHandle, payload: ConflictDetectionStatusEvent) {
    let _ = app.emit("auto-conflict-detection-status", payload);
}

/// 启动自动风格分析服务
pub async fn start_auto_style_service(app: AppHandle) {
    log::info!("[Auto Style] 启动自动风格分析服务");

    let mut ticker = interval(Duration::from_secs(ANALYSIS_INTERVAL_HOURS * 3600));

    // 首次运行延迟5分钟，避免应用启动时立即执行
    tokio::time::sleep(Duration::from_secs(300)).await;

    loop {
        ticker.tick().await;

        if let Err(e) = check_and_analyze_styles(&app).await {
            log::error!("[Auto Style] 自动分析失败: {}", e);
        }
    }
}

/// 检查并分析需要更新的风格
async fn check_and_analyze_styles(app: &AppHandle) -> anyhow::Result<()> {
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
                    log::info!(
                        "[Auto Style] 全局风格分析完成，分析了 {} 个章节，共 {} 字",
                        result.analyzed_chapters,
                        result.total_word_count
                    );
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
                    log::info!(
                        "[Auto Style] 书籍 {} 风格分析完成，分析了 {} 个章节，共 {} 字",
                        book_id,
                        result.analyzed_chapters,
                        result.total_word_count
                    );
                } else {
                    log::warn!("[Auto Style] 书籍 {} 风格分析未完成: {}", book_id, result.message);
                }
            }
            Err(e) => {
                log::error!("[Auto Style] 书籍 {} 风格分析失败: {}", book_id, e);
            }
        }
    }

    // 3. 检查各书籍是否需要冲突检测，达到阈值后直接执行，失败则记录错误
    let books_to_check = get_books_needing_conflict_check(pool).await?;
    for book_id in books_to_check {
        let started_at = chrono::Utc::now().timestamp();
        log::info!("[Auto Style] 书籍 {} 已达到设定冲突检测阈值，开始执行 AI 检测", book_id);
        crate::services::conflict_service::mark_detection_running(&book_id).await?;
        emit_conflict_status(
            app,
            ConflictDetectionStatusEvent {
                book_id: book_id.clone(),
                source: "auto".into(),
                status: "started".into(),
                error_kind: None,
                user_message: None,
                technical_message: None,
                conflict_count: None,
                occurred_at: started_at,
            },
        );

        let config = crate::services::config_service::load_config()?.ai;
        match crate::services::conflict_service::run_detection(&book_id, &config).await {
            Ok(conflicts) => {
                log::info!(
                    "[Auto Style] 书籍 {} 设定冲突检测完成，当前活跃冲突 {} 条",
                    book_id,
                    conflicts.len()
                );
                emit_conflict_status(
                    app,
                    ConflictDetectionStatusEvent {
                        book_id: book_id.clone(),
                        source: "auto".into(),
                        status: "completed".into(),
                        error_kind: None,
                        user_message: None,
                        technical_message: None,
                        conflict_count: Some(conflicts.len()),
                        occurred_at: chrono::Utc::now().timestamp(),
                    },
                );
            }
            Err(e) => {
                let technical_message = e.to_string();
                let (error_kind, user_message) = classify_conflict_error(&technical_message);
                crate::services::conflict_service::mark_detection_failed(
                    &book_id,
                    error_kind,
                    &user_message,
                )
                .await?;
                log::error!("[Auto Style] 书籍 {} 设定冲突检测失败: {}", book_id, technical_message);
                emit_conflict_status(
                    app,
                    ConflictDetectionStatusEvent {
                        book_id: book_id.clone(),
                        source: "auto".into(),
                        status: "failed".into(),
                        error_kind: Some(error_kind.into()),
                        user_message: Some(user_message),
                        technical_message: Some(technical_message),
                        conflict_count: None,
                        occurred_at: chrono::Utc::now().timestamp(),
                    },
                );
            }
        }
    }

    Ok(())
}

/// 获取需要冲突检测的书籍列表（仅检查阈值）
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
            Ok(false) => {}
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

        return Ok(total_words >= MIN_WORDS_FOR_ANALYSIS * 2);
    }

    let last_update = last_update.unwrap();
    let days_since_update = (now - last_update) / 86400;

    if days_since_update < MIN_DAYS_BETWEEN_ANALYSIS {
        return Ok(false);
    }

    let new_content_words: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters
        WHERE updated_at > ?1
        "#,
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

    let book_rows = sqlx::query("SELECT id FROM books").fetch_all(pool).await?;

    for row in book_rows {
        let book_id: String = row.get("id");

        let last_update: Option<i64> = sqlx::query_scalar(
            "SELECT updated_at FROM book_writing_styles WHERE book_id = ?1",
        )
        .bind(&book_id)
        .fetch_optional(pool)
        .await?;

        let needs_update = if let Some(last_update) = last_update {
            let days_since_update = (now - last_update) / 86400;

            if days_since_update >= MIN_DAYS_BETWEEN_ANALYSIS {
                let new_content_words: i64 = sqlx::query_scalar(
                    r#"
                    SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters
                    WHERE book_id = ?1 AND updated_at > ?2
                    "#,
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
            let total_words: i64 = sqlx::query_scalar(
                "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters WHERE book_id = ?1",
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
        force_relearn: true,
    };

    match learn_writing_style(params).await {
        Ok(result) => {
            if result.success {
                Ok(format!(
                    "风格分析完成！分析了 {} 个章节，共 {} 字",
                    result.analyzed_chapters, result.total_word_count
                ))
            } else {
                Ok(format!("分析未完成: {}", result.message))
            }
        }
        Err(e) => Err(anyhow::anyhow!("分析失败: {}", e)),
    }
}

use crate::db::get_pool;
use crate::models::{
    BookWritingStyle, LearnStyleParams, LearnStyleResult, StyleAnalysis,
    StylePromptResult, UpdateStyleParams, UserWritingStyle,
};
use crate::services::ai_service::call_ai_for_style_analysis;
use sqlx::Row;
use uuid::Uuid;

/// 学习用户写作风格
pub async fn learn_writing_style(params: LearnStyleParams) -> anyhow::Result<LearnStyleResult> {
    let pool = get_pool().await?;

    // 确定要分析的章节
    let chapters_to_analyze = if let Some(chapter_ids) = params.chapter_ids {
        chapter_ids
    } else if let Some(book_id) = &params.book_id {
        // 自动选择书籍中内容最多的章节（最多10章）
        get_representative_chapters(pool, book_id, 10).await?
    } else {
        // 全局风格：从所有书籍中选择代表性章节（最多15章）
        get_global_representative_chapters(pool, 15).await?
    };

    if chapters_to_analyze.is_empty() {
        return Ok(LearnStyleResult {
            success: false,
            message: "没有找到可分析的章节".to_string(),
            analyzed_chapters: 0,
            total_word_count: 0,
            style_prompt_preview: String::new(),
        });
    }

    // 获取章节内容
    let mut combined_content = String::new();
    let mut total_word_count = 0;

    for chapter_id in &chapters_to_analyze {
        if let Ok(content) = get_chapter_content(pool, chapter_id).await {
            total_word_count += content.chars().count() as i64;
            combined_content.push_str(&format!("\n\n=== 章节 {} ===\n\n", chapter_id));
            combined_content.push_str(&content);
        }
    }

    if total_word_count < 1000 {
        return Ok(LearnStyleResult {
            success: false,
            message: "可分析的内容太少（至少需要1000字）".to_string(),
            analyzed_chapters: chapters_to_analyze.len() as i32,
            total_word_count,
            style_prompt_preview: String::new(),
        });
    }

    // 检查是否真的需要重新分析：字数增长不足20%则跳过
    if let Some(book_id) = &params.book_id {
        let existing = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COALESCE(total_word_count, 0), COALESCE(chapter_count, 0) FROM book_writing_styles WHERE book_id = ?1"
        )
        .bind(book_id)
        .fetch_optional(pool)
        .await?;

        if let Some((prev_words, prev_chapters)) = existing {
            if prev_words > 0 && !params.force_relearn {
                let word_growth = total_word_count as f64 / prev_words as f64;
                let chapter_growth = chapters_to_analyze.len() as i64 - prev_chapters;
                if word_growth < 1.2 && chapter_growth < 3 {
                    return Ok(LearnStyleResult {
                        success: true,
                        message: format!(
                            "风格分析已是最新（当前{}字/{}章，上次{}字/{}章，增长率{:.0}%），无需重新分析",
                            total_word_count, chapters_to_analyze.len(),
                            prev_words, prev_chapters,
                            (word_growth - 1.0) * 100.0
                        ),
                        analyzed_chapters: 0,
                        total_word_count,
                        style_prompt_preview: String::new(),
                    });
                }
            }
        }
    }

    // 调用 AI 分析写作风格
    let style_analysis = analyze_writing_style_with_ai(&combined_content).await?;

    // 生成风格提示词
    let style_prompt = generate_style_prompt(&style_analysis);

    // 保存到数据库
    let now = chrono::Utc::now().timestamp();

    if let Some(book_id) = &params.book_id {
        // 保存书籍级风格
        let existing = sqlx::query("SELECT id FROM book_writing_styles WHERE book_id = ?1")
            .bind(book_id)
            .fetch_optional(pool)
            .await?;

        let style_analysis_json = serde_json::to_string(&style_analysis)?;

        if existing.is_some() && !params.force_relearn {
            // 更新现有记录
            sqlx::query(
                r#"
                UPDATE book_writing_styles 
                SET style_analysis = ?1, style_prompt = ?2, total_word_count = ?3, 
                    chapter_count = ?4, updated_at = ?5
                WHERE book_id = ?6
                "#
            )
            .bind(&style_analysis_json)
            .bind(&style_prompt)
            .bind(total_word_count)
            .bind(chapters_to_analyze.len() as i64)
            .bind(now)
            .bind(book_id)
            .execute(pool)
            .await?;
        } else {
            // 创建新记录
            let id = format!("book_style_{}", Uuid::new_v4().to_string().replace("-", "_"));
            sqlx::query(
                r#"
                INSERT INTO book_writing_styles 
                (id, book_id, style_analysis, style_prompt, total_word_count, chapter_count, updated_at, is_enabled, inherit_global)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 1)
                "#
            )
            .bind(&id)
            .bind(book_id)
            .bind(&style_analysis_json)
            .bind(&style_prompt)
            .bind(total_word_count)
            .bind(chapters_to_analyze.len() as i64)
            .bind(now)
            .execute(pool)
            .await?;
        }
    } else {
        // 保存全局风格
        let existing = sqlx::query("SELECT id FROM user_writing_styles LIMIT 1")
            .fetch_optional(pool)
            .await?;

        let style_analysis_json = serde_json::to_string(&style_analysis)?;

        if existing.is_some() && !params.force_relearn {
            // 更新现有记录
            sqlx::query(
                r#"
                UPDATE user_writing_styles 
                SET style_analysis = ?1, style_prompt = ?2, total_word_count = ?3, 
                    chapter_count = ?4, updated_at = ?5
                "#
            )
            .bind(&style_analysis_json)
            .bind(&style_prompt)
            .bind(total_word_count)
            .bind(chapters_to_analyze.len() as i64)
            .bind(now)
            .execute(pool)
            .await?;
        } else {
            // 创建新记录
            let id = format!("user_style_{}", Uuid::new_v4().to_string().replace("-", "_"));
            sqlx::query(
                r#"
                INSERT INTO user_writing_styles 
                (id, style_analysis, style_prompt, total_word_count, chapter_count, updated_at, is_enabled)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
                "#
            )
            .bind(&id)
            .bind(&style_analysis_json)
            .bind(&style_prompt)
            .bind(total_word_count)
            .bind(chapters_to_analyze.len() as i64)
            .bind(now)
            .execute(pool)
            .await?;
        }
    }

    Ok(LearnStyleResult {
        success: true,
        message: "写作风格学习完成".to_string(),
        analyzed_chapters: chapters_to_analyze.len() as i32,
        total_word_count,
        style_prompt_preview: style_prompt.chars().take(200).collect::<String>() + "...",
    })
}

/// 获取风格提示词（用于 AI 调用时注入）
pub async fn get_style_prompt(book_id: Option<&str>) -> anyhow::Result<StylePromptResult> {
    let pool = get_pool().await?;

    // 1. 尝试获取书籍级风格
    if let Some(bid) = book_id {
        let book_style = sqlx::query(
            "SELECT style_prompt, is_enabled, inherit_global FROM book_writing_styles WHERE book_id = ?1"
        )
        .bind(bid)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = book_style {
            let is_enabled: bool = row.get("is_enabled");
            let inherit_global: bool = row.get("inherit_global");

            if is_enabled {
                let book_prompt: String = row.get("style_prompt");

                if inherit_global {
                    // 继承全局风格，合并两者
                    let global_prompt = get_global_style_prompt(pool).await?;
                    let combined = format!("{}\n\n{}", global_prompt, book_prompt);
                    return Ok(StylePromptResult {
                        style_prompt: combined,
                        source: "book+global".to_string(),
                        is_enabled: true,
                    });
                } else {
                    // 仅使用书籍级风格
                    return Ok(StylePromptResult {
                        style_prompt: book_prompt,
                        source: "book".to_string(),
                        is_enabled: true,
                    });
                }
            }
        }
    }

    // 2. 回退到全局风格
    let global_prompt = get_global_style_prompt(pool).await?;
    if !global_prompt.is_empty() {
        return Ok(StylePromptResult {
            style_prompt: global_prompt,
            source: "global".to_string(),
            is_enabled: true,
        });
    }

    // 3. 无可用风格
    Ok(StylePromptResult {
        style_prompt: String::new(),
        source: "none".to_string(),
        is_enabled: false,
    })
}

/// 获取全局风格提示词
async fn get_global_style_prompt(pool: &sqlx::Pool<sqlx::Sqlite>) -> anyhow::Result<String> {
    let result = sqlx::query(
        "SELECT style_prompt FROM user_writing_styles WHERE is_enabled = 1 LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.get::<String, _>("style_prompt")).unwrap_or_default())
}

/// 更新风格配置
pub async fn update_style_config(params: UpdateStyleParams) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp();

    if let Some(book_id) = params.book_id {
        // 更新书籍级风格配置
        let mut updates = vec![];
        let mut query = "UPDATE book_writing_styles SET ".to_string();

        if let Some(enabled) = params.is_enabled {
            updates.push(format!("is_enabled = {}", if enabled { 1 } else { 0 }));
        }
        if let Some(inherit) = params.inherit_global {
            updates.push(format!("inherit_global = {}", if inherit { 1 } else { 0 }));
        }
        if let Some(prompt) = params.custom_style_prompt {
            updates.push(format!("style_prompt = '{}'", prompt.replace("'", "''")));
        }

        if !updates.is_empty() {
            updates.push(format!("updated_at = {}", now));
            query.push_str(&updates.join(", "));
            query.push_str(" WHERE book_id = ?");

            sqlx::query(&query)
                .bind(&book_id)
                .execute(pool)
                .await?;
        }
    } else {
        // 更新全局风格配置
        if let Some(enabled) = params.is_enabled {
            sqlx::query("UPDATE user_writing_styles SET is_enabled = ?1, updated_at = ?2")
                .bind(if enabled { 1 } else { 0 })
                .bind(now)
                .execute(pool)
                .await?;
        }
        if let Some(prompt) = params.custom_style_prompt {
            sqlx::query("UPDATE user_writing_styles SET style_prompt = ?1, updated_at = ?2")
                .bind(&prompt)
                .bind(now)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

/// 获取用户全局写作风格
pub async fn get_user_writing_style() -> anyhow::Result<Option<UserWritingStyle>> {
    let pool = get_pool().await?;

    let result = sqlx::query(
        "SELECT * FROM user_writing_styles LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| UserWritingStyle {
        id: row.get("id"),
        style_analysis: row.get("style_analysis"),
        style_prompt: row.get("style_prompt"),
        total_word_count: row.get("total_word_count"),
        chapter_count: row.get("chapter_count"),
        updated_at: row.get("updated_at"),
        is_enabled: row.get::<i32, _>("is_enabled") != 0,
    }))
}

/// 获取书籍级写作风格
pub async fn get_book_writing_style(book_id: &str) -> anyhow::Result<Option<BookWritingStyle>> {
    let pool = get_pool().await?;

    let result = sqlx::query(
        "SELECT * FROM book_writing_styles WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| BookWritingStyle {
        id: row.get("id"),
        book_id: row.get("book_id"),
        style_analysis: row.get("style_analysis"),
        style_prompt: row.get("style_prompt"),
        total_word_count: row.get("total_word_count"),
        chapter_count: row.get("chapter_count"),
        updated_at: row.get("updated_at"),
        is_enabled: row.get::<i32, _>("is_enabled") != 0,
        inherit_global: row.get::<i32, _>("inherit_global") != 0,
    }))
}

/// 获取代表性章节（用于分析）
/// 策略：优先选择内容较多的章节，但确保覆盖前、中、后各个阶段
async fn get_representative_chapters(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    book_id: &str,
    limit: i32,
) -> anyhow::Result<Vec<String>> {
    // 首先获取所有符合条件的章节（按创建顺序）
    let rows = sqlx::query(
        r#"
        SELECT id, LENGTH(content) as content_len FROM chapters 
        WHERE book_id = ?1 AND LENGTH(content) > 500
        ORDER BY created_at ASC
        "#
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;

    let total_chapters = rows.len();
    if total_chapters == 0 {
        return Ok(Vec::new());
    }

    // 如果章节数少于限制，返回所有章节
    if total_chapters <= limit as usize {
        return Ok(rows.iter().map(|r| r.get::<String, _>("id")).collect());
    }

    // 选择策略：均匀分布选取章节
    // 包括：开头的前几章、中间的几章、以及最新的章节
    let mut selected_indices = Vec::new();

    // 总是包含前3章（开头部分）
    for i in 0..3.min(total_chapters) {
        selected_indices.push(i);
    }

    // 从剩余章节中均匀选择
    let remaining_slots = (limit as usize).saturating_sub(selected_indices.len());
    if remaining_slots > 0 && total_chapters > 3 {
        let remaining_chapters = total_chapters - 3;
        let step = remaining_chapters as f32 / (remaining_slots + 1) as f32;

        for i in 1..=remaining_slots {
            let idx = 3 + (i as f32 * step) as usize;
            if idx < total_chapters && !selected_indices.contains(&idx) {
                selected_indices.push(idx);
            }
        }
    }

    // 确保包含最新的一章（如果有空间）
    let last_idx = total_chapters - 1;
    if !selected_indices.contains(&last_idx) && selected_indices.len() < limit as usize {
        selected_indices.push(last_idx);
    }

    // 按索引排序，保持章节顺序
    selected_indices.sort();

    // 收集选中的章节ID
    let mut result = Vec::new();
    for idx in selected_indices {
        if let Some(row) = rows.get(idx) {
            result.push(row.get::<String, _>("id"));
        }
    }

    Ok(result)
}

/// 获取全局代表性章节
/// 策略：从所有书籍中均匀选择章节，优先选择每本书的前几章和最新章节
async fn get_global_representative_chapters(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    limit: i32,
) -> anyhow::Result<Vec<String>> {
    // 获取所有符合条件的章节，按书籍和创建时间分组
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.book_id, c.created_at 
        FROM chapters c
        WHERE LENGTH(c.content) > 500
        ORDER BY c.book_id, c.created_at ASC
        "#
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // 按书籍分组
    let mut book_chapters: std::collections::HashMap<String, Vec<(String, i64)>> = std::collections::HashMap::new();
    for row in &rows {
        let id: String = row.get("id");
        let book_id: String = row.get("book_id");
        let created_at: i64 = row.get("created_at");
        book_chapters.entry(book_id).or_default().push((id, created_at));
    }

    let book_count = book_chapters.len();
    let mut selected = Vec::new();

    // 每本书至少选择一章（开头），然后均匀分配剩余配额
    let base_per_book = 2; // 每本书至少2章（开头和最新）
    let remaining = (limit as usize).saturating_sub(book_count * base_per_book);

    for (_book_id, chapters) in book_chapters {
        let chapter_count = chapters.len();

        // 总是选择第一章
        selected.push(chapters[0].0.clone());

        // 如果有更多章节，选择最新的一章
        if chapter_count > 1 {
            selected.push(chapters[chapter_count - 1].0.clone());
        }

        // 从剩余配额中均匀选择中间章节
        if remaining > 0 && chapter_count > 2 {
            let extra = (remaining as f32 / book_count as f32).ceil() as usize;
            let step = (chapter_count - 2) as f32 / (extra + 1) as f32;

            for i in 1..=extra {
                let idx = (i as f32 * step) as usize;
                if idx > 0 && idx < chapter_count - 1 {
                    selected.push(chapters[idx].0.clone());
                }
            }
        }

        if selected.len() >= limit as usize {
            break;
        }
    }

    // 如果选多了，截断
    selected.truncate(limit as usize);

    Ok(selected)
}

/// 获取章节内容
async fn get_chapter_content(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    chapter_id: &str,
) -> anyhow::Result<String> {
    let row = sqlx::query("SELECT content FROM chapters WHERE id = ?1")
        .bind(chapter_id)
        .fetch_one(pool)
        .await?;

    Ok(row.get::<String, _>("content"))
}

/// 使用 AI 分析写作风格
async fn analyze_writing_style_with_ai(content: &str) -> anyhow::Result<StyleAnalysis> {
    use crate::services::prompt_service::get_prompt_manager;

    // 截取前 8000 字符进行分析（避免过长）
    // 使用 chars().take() 来正确处理 Unicode 字符边界
    let truncated_content = if content.chars().count() > 8000 {
        content.chars().take(8000).collect::<String>()
    } else {
        content.to_string()
    };
    let truncated_content = truncated_content.as_str();

    // 从提示词管理器获取提示词模板
    let prompt_manager = get_prompt_manager();
    let prompt_template = &prompt_manager.style_prompts.analyze_writing_style;
    let system_prompt = &prompt_manager.style_prompts.analyze_writing_style_system;

    // 提示词文件必须存在
    if prompt_template.is_empty() {
        return Err(anyhow::anyhow!("提示词文件 prompts/style/analyze_writing_style.md 不存在或为空"));
    }
    if system_prompt.is_empty() {
        return Err(anyhow::anyhow!("提示词文件 prompts/style/analyze_writing_style_system.md 不存在或为空"));
    }

    let prompt = prompt_template.replace("{text_content}", truncated_content);

    // 调用 AI 进行分析
    let response = call_ai_for_style_analysis(&prompt, system_prompt).await?;

    // 解析 JSON 响应
    let analysis: StyleAnalysis = serde_json::from_str(&response)
        .map_err(|e| anyhow::anyhow!("解析风格分析结果失败: {}\n响应内容: {}", e, &response[..response.len().min(500)]))?;

    Ok(analysis)
}

/// 根据风格分析生成风格提示词
fn generate_style_prompt(analysis: &StyleAnalysis) -> String {
    use crate::services::prompt_service::get_prompt_manager;

    let prompt_manager = get_prompt_manager();
    let template = &prompt_manager.style_prompts.writing_style_guide_template;

    // 提示词文件必须存在
    if template.is_empty() {
        panic!("提示词文件 prompts/style/writing_style_guide_template.md 不存在或为空");
    }

    // 使用模板生成提示词
    template
        .replace("{overall_tone}", &analysis.language_style.overall_tone)
        .replace("{word_choice}", &analysis.language_style.word_choice)
        .replace("{sentence_structure}", &analysis.language_style.sentence_structure)
        .replace("{rhetoric_density}", &analysis.language_style.rhetoric_density)
        .replace("{perspective}", &analysis.narrative_features.perspective)
        .replace("{chronology}", &analysis.narrative_features.chronology)
        .replace("{narrative_distance}", &analysis.narrative_features.narrative_distance)
        .replace("{psychological_depth}", &analysis.narrative_features.psychological_depth)
        .replace("{dialogue_density}", &analysis.dialogue_style.dialogue_density)
        .replace("{dialogue_characteristics}", &analysis.dialogue_style.dialogue_characteristics)
        .replace("{dialogue_tag_style}", &analysis.dialogue_style.dialogue_tag_style)
        .replace("{sensory_preference}", &analysis.description_preference.sensory_preference.join(", "))
        .replace("{environment_description_level}", &analysis.description_preference.environment_description_level)
        .replace("{action_description_style}", &analysis.description_preference.action_description_style)
        .replace("{psychological_description_style}", &analysis.description_preference.psychological_description_style)
        .replace("{overall_pacing}", &analysis.pacing_style.overall_pacing)
        .replace("{paragraph_length_preference}", &analysis.pacing_style.paragraph_length_preference)
        .replace("{scene_transition_frequency}", &analysis.pacing_style.scene_transition_frequency)
        .replace("{common_rhetoric}", &analysis.common_rhetoric.join(", "))
        .replace("{sentence_patterns}", &analysis.sentence_patterns.join(", "))
        .replace("{vocabulary_level}", &analysis.vocabulary_preference.vocabulary_level)
        .replace("{characteristic_words}", &analysis.vocabulary_preference.characteristic_words.join(", "))
}

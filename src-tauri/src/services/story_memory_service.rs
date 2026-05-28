use crate::db::get_pool;
use crate::models::{
    StoryMemory, VolumeSummaryItem, EventTimelineItem, KeyCharacterStatus,
    StoryMemoryUpdateResult, GroupProgress,
};
use crate::services::ai_service::call_ai_for_large_summary;
use crate::services::summary_generator_service::generate_chapter_summary_for_story_bible;
use sqlx::Row;
use tauri::AppHandle;

const GROUP_SIZE: i32 = 10;
const MIN_CHAPTER_WORDS: i64 = 100; // 非空白章节的最小字数

/// 从 DB 读取故事记忆
pub async fn get_story_memory(book_id: &str) -> anyhow::Result<Option<StoryMemory>> {
    let pool = get_pool().await?;

    let row = sqlx::query(
        "SELECT book_id, book_summary, volume_summaries, event_timeline,
         protagonist_status, key_character_statuses, unresolved_threads,
         world_rules, last_chapter_count, last_word_count, updated_at
         FROM story_memory WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let volume_summaries: String = r.try_get("volume_summaries")?;
            let event_timeline: String = r.try_get("event_timeline")?;
            let key_character_statuses: String = r.try_get("key_character_statuses")?;
            let unresolved_threads: String = r.try_get("unresolved_threads")?;

            Ok(Some(StoryMemory {
                book_id: r.try_get("book_id")?,
                book_summary: r.try_get("book_summary")?,
                volume_summaries: serde_json::from_str(&volume_summaries).unwrap_or_default(),
                event_timeline: serde_json::from_str(&event_timeline).unwrap_or_default(),
                protagonist_status: r.try_get("protagonist_status")?,
                key_character_statuses: serde_json::from_str(&key_character_statuses).unwrap_or_default(),
                unresolved_threads: serde_json::from_str(&unresolved_threads).unwrap_or_default(),
                world_rules: r.try_get("world_rules")?,
                last_chapter_count: r.try_get("last_chapter_count")?,
                last_word_count: r.try_get("last_word_count")?,
                updated_at: r.try_get("updated_at")?,
            }))
        }
        None => Ok(None),
    }
}

pub async fn save_story_memory(memory: &StoryMemory) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Utc::now().timestamp();

    let volume_summaries = serde_json::to_string(&memory.volume_summaries)?;
    let event_timeline = serde_json::to_string(&memory.event_timeline)?;
    let key_character_statuses = serde_json::to_string(&memory.key_character_statuses)?;
    let unresolved_threads = serde_json::to_string(&memory.unresolved_threads)?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO story_memory
        (book_id, book_summary, volume_summaries, event_timeline,
         protagonist_status, key_character_statuses, unresolved_threads,
         world_rules, last_chapter_count, last_word_count, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#
    )
    .bind(&memory.book_id)
    .bind(&memory.book_summary)
    .bind(&volume_summaries)
    .bind(&event_timeline)
    .bind(&memory.protagonist_status)
    .bind(&key_character_statuses)
    .bind(&unresolved_threads)
    .bind(&memory.world_rules)
    .bind(memory.last_chapter_count)
    .bind(memory.last_word_count)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn build_story_memory_text(book_id: &str) -> anyhow::Result<String> {
    match get_story_memory(book_id).await? {
        Some(memory) => Ok(format_story_memory(&memory)),
        None => Ok(String::new()),
    }
}

fn format_story_memory(m: &StoryMemory) -> String {
    let mut text = String::new();

    if !m.book_summary.is_empty() {
        text.push_str("【故事总览】\n");
        text.push_str(&m.book_summary);
        text.push_str("\n\n");
    }

    if !m.volume_summaries.is_empty() {
        text.push_str("【分卷梗概】\n");
        for v in &m.volume_summaries {
            text.push_str(&format!("{}：{}\n", v.title, v.summary));
        }
        text.push('\n');
    }

    if !m.event_timeline.is_empty() {
        text.push_str("【关键事件时间线】\n");
        for e in &m.event_timeline {
            let arc_label = if !e.arc.is_empty() { format!(" [{}]", e.arc) } else { String::new() };
            text.push_str(&format!("第{}章「{}」：{}{}\n", e.chapter, e.title, e.event, arc_label));
            if !e.impact.is_empty() {
                text.push_str(&format!("  → 影响：{}\n", e.impact));
            }
        }
        text.push('\n');
    }

    if !m.protagonist_status.is_empty() {
        text.push_str("【主角当前状态】\n");
        text.push_str(&m.protagonist_status);
        text.push_str("\n\n");
    }

    if !m.key_character_statuses.is_empty() {
        text.push_str("【重要角色现状】\n");
        for c in &m.key_character_statuses {
            text.push_str(&format!("- {}：{}，{}\n", c.name, c.status, c.location));
        }
        text.push('\n');
    }

    if !m.unresolved_threads.is_empty() {
        text.push_str("【未解决伏笔】\n");
        for t in &m.unresolved_threads {
            text.push_str(&format!("- {}\n", t));
        }
        text.push('\n');
    }

    if !m.world_rules.is_empty() {
        text.push_str("【世界观关键设定】\n");
        text.push_str(&m.world_rules);
        text.push('\n');
    }

    text
}

pub async fn get_chapters_in_volume(book_id: &str, volume_id: &str) -> anyhow::Result<String> {
    let book = crate::services::book_service::load_book(book_id.to_string()).await?;

    let vol_chapters: Vec<_> = book.chapters.iter()
        .filter(|c| c.volume_id == volume_id)
        .collect();

    if vol_chapters.is_empty() {
        return Ok("该卷下没有章节".to_string());
    }

    let volume_name = book.volumes.iter()
        .find(|v| v.id == volume_id)
        .map(|v| v.title.as_str())
        .unwrap_or("未知卷");

    let mut result = format!("「{}」章节列表：\n", volume_name);
    for (i, ch) in vol_chapters.iter().enumerate() {
        result.push_str(&format!("{}. {} (ID: {})\n", i + 1, ch.title, ch.id));
    }

    Ok(result)
}

pub async fn build_character_timeline(book_id: &str, name: &str) -> anyhow::Result<String> {
    let memory = match get_story_memory(book_id).await? {
        Some(m) => m,
        None => return Ok("故事记忆尚未生成，无法查询角色时间线。请先生成章节摘要后更新故事记忆。".to_string()),
    };

    let name_lower = name.to_lowercase();
    let mut events: Vec<String> = Vec::new();

    for e in &memory.event_timeline {
        if e.event.to_lowercase().contains(&name_lower) || e.title.to_lowercase().contains(&name_lower) {
            events.push(format!("第{}章「{}」：{}", e.chapter, e.title, e.event));
        }
    }

    let char_status = memory.key_character_statuses.iter()
        .find(|c| c.name.to_lowercase().contains(&name_lower));

    if events.is_empty() && char_status.is_none() {
        return Ok(format!("在故事记忆中未找到与「{}」相关的记录。", name));
    }

    let mut result = format!("「{}」的相关信息：\n\n", name);

    if let Some(cs) = char_status {
        result.push_str(&format!("当前状态：{}，{}\n\n", cs.status, cs.location));
    }

    if !events.is_empty() {
        result.push_str("关键事件：\n");
        for e in &events {
            result.push_str(&format!("- {}\n", e));
        }
    }

    Ok(result)
}

pub async fn check_should_update_story_memory(book_id: &str) -> anyhow::Result<bool> {
    let pool = get_pool().await?;

    let chapter_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chapters WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_one(pool)
    .await?;

    let memory = get_story_memory(book_id).await?;
    match memory {
        Some(m) => Ok((chapter_count - m.last_chapter_count).abs() >= 3),
        None => Ok(chapter_count >= 5),
    }
}

// ==================== 分组并行更新 ====================

pub async fn update_story_memory_auto(
    book_id: &str,
) -> anyhow::Result<StoryMemoryUpdateResult> {
    update_story_memory_inner(book_id, false).await
}

pub async fn update_story_memory(
    _app: &AppHandle,
    book_id: &str,
    force: bool,
) -> anyhow::Result<StoryMemoryUpdateResult> {
    update_story_memory_inner(book_id, force).await
}

/// 核心：三步流程生成 Story Bible
/// 第一步：为所有无摘要的非空白章节生成摘要（force=true时重新生成所有）
/// 第二步：每10章分组，并行生成分组总结（force=true时清除缓存重新生成）
/// 第三步：汇总所有分组，生成最终 Story Bible
async fn update_story_memory_inner(
    book_id: &str,
    force: bool,
) -> anyhow::Result<StoryMemoryUpdateResult> {
    let config = crate::services::config_service::load_config()?.ai;

    let book = crate::services::book_service::load_book(book_id.to_string()).await?;
    let chapter_count = book.chapters.len() as i32;
    let total_word_count: i64 = book.chapters.iter().map(|c| c.word_count).sum();

    if book.chapters.is_empty() {
        return Ok(StoryMemoryUpdateResult {
            success: false,
            message: "没有章节，无法生成故事记忆".into(),
            chapter_count: 0, total_word_count: 0,
            groups: vec![], groups_cached: 0, groups_generated: 0,
        });
    }

    let start_time = std::time::Instant::now();

    // ========== 第一步：确保所有非空白章节都有摘要 ==========
    log::info!("[StoryBible] 第一步：检查并生成{}章节摘要...", if force { "所有（强制模式）" } else { "缺失的" });
    let summary_result = if force {
        regenerate_all_chapter_summaries(book_id, &book, &config).await?
    } else {
        ensure_all_chapter_summaries(book_id, &book, &config).await?
    };
    log::info!("[StoryBible] 章节摘要处理完成：{} 个已存在，{} 个新生成，{} 个失败",
        summary_result.existing, summary_result.generated, summary_result.failed);

    // ========== 第二步：将章节分成 10 章一组，并行处理 ==========
    let groups = split_into_groups(&book);
    log::info!("[StoryBible] 第二步：开始并行处理 {} 个分组{}...", groups.len(), if force { "（强制模式，清除缓存）" } else { "" });

    // 强制模式：清除该书的缓存
    if force {
        if let Err(e) = clear_group_cache(book_id).await {
            log::warn!("[StoryBible] 清除缓存失败: {}", e);
        } else {
            log::info!("[StoryBible] 已清除分组缓存");
        }
    }

    let (group_summaries, progress_list) = process_groups_parallel(book_id, &groups, &config).await;

    let groups_cached = progress_list.iter().filter(|p| p.status == "cached").count() as i32;
    let groups_generated = progress_list.iter().filter(|p| p.status == "generated").count() as i32;
    log::info!("[StoryBible] 分组处理完成：{} 个缓存命中，{} 个新生成",
        groups_cached, groups_generated);

    // ========== 第三步：汇总所有分组摘要，生成最终 Story Bible ==========
    log::info!("[StoryBible] 第三步：生成最终 Story Bible...");
    let volume_text = build_volume_structure_text(&book);
    let group_combined: String = group_summaries.iter()
        .enumerate()
        .map(|(i, s)| format!("## 第{}组（第{}-{}章）\n{}", i + 1,
            groups[i].start_chapter, groups[i].end_chapter, s))
        .collect();

    let current_memory = get_story_memory(book_id).await?;
    let current_memory_text = match &current_memory {
        Some(m) => format_story_memory(m),
        None => String::from("（首次创建故事圣经）"),
    };

    let prompt = if current_memory.is_some() {
        format!(
            "【卷结构】\n{}\n\n【所有分组摘要】\n{}\n\n【当前故事圣经】\n{}\n\n请基于分组摘要更新故事圣经。",
            volume_text, group_combined, current_memory_text
        )
    } else {
        format!(
            "【卷结构】\n{}\n\n【所有分组摘要】\n{}\n\n请基于分组摘要创建故事圣经。",
            volume_text, group_combined
        )
    };

    let system_prompt = include_str!("../../prompts/agents/story_bible_update.md");
    // 大总结需要大量输出（5000-8000字JSON），使用 32000 max_tokens
    let final_max_tokens = config.max_tokens.max(32000);
    let response = call_ai_for_large_summary(&prompt, system_prompt, &config, final_max_tokens).await?;
    let response = clean_json_response(&response);

    // 如果 JSON 被截断，尝试修复
    let response = repair_truncated_json(&response);

    let elapsed = start_time.elapsed();

    match parse_story_memory_json(book_id, &response) {
        Ok(mut memory) => {
            memory.last_chapter_count = chapter_count as i64;
            memory.last_word_count = total_word_count;
            save_story_memory(&memory).await?;

            log::info!("[StoryBible] Story Bible 生成完成，{} 章，{} 字，总耗时 {:.1}s",
                chapter_count, total_word_count, elapsed.as_secs_f32());
            Ok(StoryMemoryUpdateResult {
                success: true,
                message: format!(
                    "Story Bible 已更新（{} 章，{} 字）。章节摘要：{} 个已存在/{} 个新生成/{} 个失败；分组：{} 个缓存/{} 个新生成。总耗时 {:.1}s",
                    chapter_count, total_word_count,
                    summary_result.existing, summary_result.generated, summary_result.failed,
                    groups_cached, groups_generated, elapsed.as_secs_f32()
                ),
                chapter_count, total_word_count,
                groups: progress_list,
                groups_cached,
                groups_generated,
            })
        }
        Err(e) => {
            log::error!("[StoryBible] 大总结 AI 返回解析失败: {}，原始: {}",
                e, &response[..response.len().min(300)]);
            Ok(StoryMemoryUpdateResult {
                success: false,
                message: format!("最终汇总失败: {}", e),
                chapter_count: 0, total_word_count: 0,
                groups: progress_list,
                groups_cached: 0, groups_generated: 0,
            })
        }
    }
}

/// 章节摘要处理结果
struct SummaryGenerationResult {
    existing: usize,
    generated: usize,
    failed: usize,
}

/// 第一步：确保所有非空白章节都有摘要
/// - 检查哪些章节没有摘要
/// - 使用 summary_generator_service 批量生成
async fn ensure_all_chapter_summaries(
    book_id: &str,
    book: &crate::models::Book,
    config: &crate::models::AIConfig,
) -> anyhow::Result<SummaryGenerationResult> {
    let pool = get_pool().await?;
    let mut existing = 0;
    let mut to_generate = Vec::new();

    // 检查每个非空白章节的摘要状态
    for chapter in &book.chapters {
        // 跳过空白章节（字数太少）
        if chapter.word_count < MIN_CHAPTER_WORDS {
            continue;
        }

        let has_summary: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM chapter_summaries WHERE chapter_id = ?1"
        )
        .bind(&chapter.id)
        .fetch_optional(pool)
        .await?;

        if has_summary.is_some() {
            existing += 1;
        } else {
            to_generate.push((chapter.id.clone(), chapter.title.clone()));
        }
    }

    if to_generate.is_empty() {
        log::info!("[StoryBible] 所有 {} 个非空白章节已有摘要", existing);
        return Ok(SummaryGenerationResult { existing, generated: 0, failed: 0 });
    }

    log::info!("[StoryBible] 需要为 {} 个章节生成摘要（{} 个已存在）", to_generate.len(), existing);

    // 并行生成摘要
    let mut tasks = Vec::new();
    for (chapter_id, title) in to_generate {
        let book_id = book_id.to_string();
        let chapter_id_clone = chapter_id.clone();
        let title_clone = title.clone();
        let config = config.clone();
        tasks.push(tokio::spawn(async move {
            match generate_chapter_summary_for_story_bible(&book_id, &chapter_id_clone, &title_clone, &config).await {
                Ok(_) => {
                    log::info!("[StoryBible] 章节 '{}' 摘要生成成功", title_clone);
                    true
                }
                Err(e) => {
                    log::error!("[StoryBible] 章节 '{}' 摘要生成失败: {}", title_clone, e);
                    false
                }
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;
    let generated = results.iter().filter(|r| r.as_ref().map(|s| *s).unwrap_or(false)).count();
    let failed = results.len() - generated;

    Ok(SummaryGenerationResult { existing, generated, failed })
}

/// 强制重新生成所有非空白章节的摘要
async fn regenerate_all_chapter_summaries(
    book_id: &str,
    book: &crate::models::Book,
    config: &crate::models::AIConfig,
) -> anyhow::Result<SummaryGenerationResult> {
    let mut to_generate = Vec::new();

    // 收集所有非空白章节
    for chapter in &book.chapters {
        if chapter.word_count >= MIN_CHAPTER_WORDS {
            to_generate.push((chapter.id.clone(), chapter.title.clone()));
        }
    }

    if to_generate.is_empty() {
        log::info!("[StoryBible] 没有需要生成摘要的非空白章节");
        return Ok(SummaryGenerationResult { existing: 0, generated: 0, failed: 0 });
    }

    log::info!("[StoryBible] 强制模式：重新生成 {} 个章节的摘要", to_generate.len());

    // 先清除该书的现有摘要
    if let Err(e) = clear_chapter_summaries(book_id).await {
        log::warn!("[StoryBible] 清除现有摘要失败: {}", e);
    } else {
        log::info!("[StoryBible] 已清除现有章节摘要");
    }

    // 并行重新生成摘要
    let mut tasks = Vec::new();
    for (chapter_id, title) in to_generate {
        let book_id = book_id.to_string();
        let chapter_id_clone = chapter_id.clone();
        let title_clone = title.clone();
        let config = config.clone();
        tasks.push(tokio::spawn(async move {
            match generate_chapter_summary_for_story_bible(&book_id, &chapter_id_clone, &title_clone, &config).await {
                Ok(_) => {
                    log::info!("[StoryBible] 章节 '{}' 摘要重新生成成功", title_clone);
                    true
                }
                Err(e) => {
                    log::error!("[StoryBible] 章节 '{}' 摘要重新生成失败: {}", title_clone, e);
                    false
                }
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;
    let generated = results.iter().filter(|r| r.as_ref().map(|s| *s).unwrap_or(false)).count();
    let failed = results.len() - generated;

    Ok(SummaryGenerationResult { existing: 0, generated, failed })
}

/// 清除该书的章节摘要
async fn clear_chapter_summaries(book_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query(
        r#"
        DELETE FROM chapter_summaries 
        WHERE chapter_id IN (
            SELECT id FROM chapters WHERE book_id = ?1
        )
        "#
    )
    .bind(book_id)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 清除该书的分组缓存
async fn clear_group_cache(book_id: &str) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM story_memory_groups WHERE book_id = ?1")
        .bind(book_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

/// 章节分组
struct ChapterGroup {
    group_index: i32,
    start_chapter: i32,
    end_chapter: i32,
    chapters: Vec<ChapterInfo>,
}

#[derive(Clone)]
struct ChapterInfo {
    id: String,
    title: String,
    short_summary: String,
    long_summary: String,
    events: String,
    word_count: i64,
}

fn split_into_groups(book: &crate::models::Book) -> Vec<ChapterGroup> {
    // 按 created_at 排序（已经是按 order 排的）
    let chapters: Vec<_> = book.chapters.iter()
        .collect();

    let total = chapters.len();
    let group_count = ((total as f64) / (GROUP_SIZE as f64)).ceil() as usize;
    let mut groups = Vec::with_capacity(group_count);

    for g in 0..group_count {
        let start_idx = g * GROUP_SIZE as usize;
        let end_idx = ((g + 1) * GROUP_SIZE as usize).min(total);

        let group_chapters: Vec<ChapterInfo> = chapters[start_idx..end_idx].iter()
            .map(|ch| ChapterInfo {
                id: ch.id.clone(),
                title: ch.title.clone(),
                short_summary: String::new(),
                long_summary: String::new(),
                events: String::new(),
                word_count: ch.word_count,
            })
            .collect();

        groups.push(ChapterGroup {
            group_index: g as i32,
            start_chapter: (start_idx as i32) + 1,
            end_chapter: end_idx as i32,
            chapters: group_chapters,
        });
    }

    groups
}

/// 并发处理所有分组：数据库缓存命中直接复用，未命中调 AI
async fn process_groups_parallel(
    book_id: &str,
    groups: &[ChapterGroup],
    config: &crate::models::AIConfig,
) -> (Vec<String>, Vec<GroupProgress>) {
    let system_prompt = include_str!("../../prompts/agents/story_bible_group.md");
    let pool = get_pool().await.ok();

    let mut tasks = Vec::new();

    for group in groups {
        let book_id = book_id.to_string();
        let group_index = group.group_index;
        let start_ch = group.start_chapter;
        let end_ch = group.end_chapter;
        let chapter_ids: Vec<String> = group.chapters.iter().map(|c| c.id.clone()).collect();
        let word_count: i64 = group.chapters.iter().map(|c| c.word_count).sum();
        // clone chapters 数据避免生命周期问题
        let owned_chapters: Vec<ChapterInfo> = group.chapters.clone();
        let config = config.clone();
        let sp = system_prompt.to_string();
        let pool = pool.cloned();

        tasks.push(tokio::spawn(async move {
            // 1. 检查缓存（按章节ID集合匹配，不按位置）
            if let Some(ref pool) = pool {
                if let Ok(Some((cached_summary, _cached_idx))) =
                    find_cached_group(pool, &book_id, &chapter_ids, word_count).await
                {
                    log::info!("[StoryBible] 分组 {}（第{}-{}章）命中缓存，跳过", group_index, start_ch, end_ch);
                    return (group_index, cached_summary, GroupProgress {
                        group_index, start_chapter: start_ch, end_chapter: end_ch,
                        status: "cached".into(),
                        message: format!("第{}-{}章（缓存命中）", start_ch, end_ch),
                    });
                }
            }

            // 2. 缓存未命中，填充章节摘要并调 AI
            let filled_chapters = fill_chapter_summaries(&book_id, &owned_chapters).await;

            let chapters_text: String = filled_chapters.iter()
                .enumerate()
                .map(|(i, ch)| format!(
                    "### 第{}章「{}」\n短摘要：{}\n详细摘要：{}\n关键事件：{}\n",
                    i + 1 + start_ch as usize - 1, ch.title,
                    ch.short_summary, ch.long_summary, ch.events
                ))
                .collect::<Vec<_>>()
                .join("\n");

            let prompt = format!(
                "请为以下章节组（第{}-{}章）生成分组摘要：\n\n{}",
                start_ch, end_ch, chapters_text
            );

            log::info!("[StoryBible] 分组 {}（第{}-{}章）调用AI...", group_index, start_ch, end_ch);

            match call_ai_for_large_summary(&prompt, &sp, &config, 8000).await {
                Ok(response) => {
                    let cleaned = clean_json_response(&response);
                    let summary_text = extract_group_summary(&cleaned);
                    let summary_clone = summary_text.clone();

                    if let Some(ref pool) = pool {
                        let _ = save_group_to_db(pool, &book_id, group_index,
                            start_ch, end_ch, &chapter_ids, &summary_text, word_count).await;
                    }

                    log::info!("[StoryBible] 分组 {}（第{}-{}章）AI完成: {}字",
                        group_index, start_ch, end_ch, summary_clone.chars().count());

                    (group_index, summary_clone, GroupProgress {
                        group_index, start_chapter: start_ch, end_chapter: end_ch,
                        status: "generated".into(),
                        message: format!("第{}-{}章（已生成）", start_ch, end_ch),
                    })
                }
                Err(e) => {
                    log::error!("[StoryBible] 分组 {} 失败: {}", group_index, e);
                    (group_index, format!("[生成失败: {}]", e), GroupProgress {
                        group_index, start_chapter: start_ch, end_chapter: end_ch,
                        status: "error".into(),
                        message: format!("第{}-{}章（{}）", start_ch, end_ch, e),
                    })
                }
            }
        }));
    }

    let results = futures::future::join_all(tasks).await;

    // 按 group_index 排序结果
    let mut ordered: Vec<(i32, String, GroupProgress)> = results.into_iter()
        .filter_map(|r| r.ok())
        .collect();
    ordered.sort_by_key(|(idx, _, _)| *idx);

    let summaries: Vec<String> = ordered.iter().map(|(_, s, _)| s.clone()).collect();
    let progress: Vec<GroupProgress> = ordered.into_iter().map(|(_, _, p)| p).collect();

    (summaries, progress)
}

/// 查找缓存：遍历该书所有缓存行，找章节ID集合匹配的那一行
async fn find_cached_group(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    book_id: &str,
    current_ids: &[String],
    current_word_count: i64,
) -> anyhow::Result<Option<(String, i32)>> {
    // 对当前 group 的章节ID排序，用于集合比较
    let mut sorted_current: Vec<&str> = current_ids.iter().map(|s| s.as_str()).collect();
    sorted_current.sort();

    let rows = sqlx::query(
        "SELECT group_index, chapter_ids, summary, word_count FROM story_memory_groups WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let group_index: i32 = row.try_get("group_index")?;
        let cached_ids_json: String = row.try_get("chapter_ids")?;
        let summary: String = row.try_get("summary")?;
        let word_count: i64 = row.try_get("word_count")?;

        let cached_ids: Vec<String> = serde_json::from_str(&cached_ids_json).unwrap_or_default();
        let mut sorted_cached: Vec<&str> = cached_ids.iter().map(|s| s.as_str()).collect();
        sorted_cached.sort();

        // 章节集合相同 + 字数没变 → 缓存命中
        if sorted_cached == sorted_current && word_count == current_word_count && !summary.is_empty() {
            return Ok(Some((summary, group_index)));
        }
    }

    Ok(None)
}

async fn save_group_to_db(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    book_id: &str,
    group_index: i32,
    start_chapter: i32,
    end_chapter: i32,
    chapter_ids: &[String],
    summary: &str,
    word_count: i64,
) -> anyhow::Result<()> {
    let ids_json = serde_json::to_string(chapter_ids)?;
    let now = chrono::Utc::now().timestamp();

    // 1. 先删除同一章节集合的旧缓存（可能因为章节增删导致 group_index 变了）
    let mut sorted_new: Vec<&str> = chapter_ids.iter().map(|s| s.as_str()).collect();
    sorted_new.sort();

    let existing = sqlx::query(
        "SELECT group_index, chapter_ids FROM story_memory_groups WHERE book_id = ?1"
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;

    for row in existing {
        let existing_ids_json: String = row.try_get("chapter_ids")?;
        let existing_ids: Vec<String> = serde_json::from_str(&existing_ids_json).unwrap_or_default();
        let mut sorted_existing: Vec<&str> = existing_ids.iter().map(|s| s.as_str()).collect();
        sorted_existing.sort();
        if sorted_existing == sorted_new {
            let old_idx: i32 = row.try_get("group_index")?;
            sqlx::query("DELETE FROM story_memory_groups WHERE book_id = ?1 AND group_index = ?2")
                .bind(book_id)
                .bind(old_idx)
                .execute(pool)
                .await?;
            break;
        }
    }

    // 2. 插入新缓存
    sqlx::query(
        "INSERT INTO story_memory_groups
         (book_id, group_index, start_chapter, end_chapter, chapter_ids, summary, word_count, generated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
    )
    .bind(book_id)
    .bind(group_index)
    .bind(start_chapter)
    .bind(end_chapter)
    .bind(&ids_json)
    .bind(summary)
    .bind(word_count)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

/// 从 AI 返回的 JSON 中提取 group_summary（保留完整 JSON 给后续摘要用）
fn extract_group_summary(json_str: &str) -> String {
    // 返回完整 JSON 字符串作为分组摘要（后续大总结用全部字段）
    json_str.to_string()
}

/// 从 DB 填充章节摘要信息（使用增强摘要格式）
async fn fill_chapter_summaries(_book_id: &str, chapters: &[ChapterInfo]) -> Vec<ChapterInfo> {
    let pool = match get_pool().await {
        Ok(p) => p,
        Err(_) => return chapters.to_vec(),
    };

    let mut filled = Vec::new();
    for ch in chapters {
        let mut info = ch.clone();

        if let Ok(Some(row)) = sqlx::query(
            r#"SELECT 
                short_summary, long_summary, tags, characters, locations, events,
                plot_progression, emotional_beats, foreshadowing, unresolved_threads
            FROM chapter_summaries WHERE chapter_id = ?1"#
        )
        .bind(&ch.id)
        .fetch_optional(pool)
        .await
        {
            info.short_summary = row.try_get::<String, _>("short_summary").unwrap_or_default();
            info.long_summary = row.try_get::<String, _>("long_summary").unwrap_or_default();

            // 构建增强的 events 信息
            let mut events_parts = Vec::new();

            // 基础事件
            if let Ok(events_json) = row.try_get::<String, _>("events") {
                if let Ok(events) = serde_json::from_str::<Vec<String>>(&events_json) {
                    if !events.is_empty() {
                        events_parts.push(format!("【关键事件】{}", events.join("；")));
                    }
                }
            }

            // 剧情推进点
            if let Ok(plot) = row.try_get::<String, _>("plot_progression") {
                if !plot.is_empty() {
                    events_parts.push(format!("【剧情推进】{}", plot));
                }
            }

            // 情感节点
            if let Ok(emotional_json) = row.try_get::<String, _>("emotional_beats") {
                if let Ok(beats) = serde_json::from_str::<Vec<String>>(&emotional_json) {
                    if !beats.is_empty() {
                        events_parts.push(format!("【情感节点】{}", beats.join("；")));
                    }
                }
            }

            // 伏笔
            if let Ok(foreshadowing_json) = row.try_get::<String, _>("foreshadowing") {
                if let Ok(foreshadows) = serde_json::from_str::<Vec<String>>(&foreshadowing_json) {
                    if !foreshadows.is_empty() {
                        events_parts.push(format!("【埋下伏笔】{}", foreshadows.join("；")));
                    }
                }
            }

            // 未解决线索
            if let Ok(unresolved_json) = row.try_get::<String, _>("unresolved_threads") {
                if let Ok(threads) = serde_json::from_str::<Vec<String>>(&unresolved_json) {
                    if !threads.is_empty() {
                        events_parts.push(format!("【未解决线索】{}", threads.join("；")));
                    }
                }
            }

            info.events = if events_parts.is_empty() {
                String::new()
            } else {
                events_parts.join("\n")
            };
        }

        filled.push(info);
    }

    filled
}

// ==================== 辅助函数 ====================

fn build_volume_structure_text(book: &crate::models::Book) -> String {
    let mut text = String::new();
    for volume in &book.volumes {
        let ch_count = book.chapters.iter().filter(|c| c.volume_id == volume.id).count();
        text.push_str(&format!("- {}（{}章）\n", volume.title, ch_count));
    }
    if text.is_empty() {
        text = String::from("（无卷结构）");
    }
    text
}

fn clean_json_response(response: &str) -> String {
    let response = response.trim();
    if response.starts_with("```json") {
        response[7..].trim_end_matches("```").trim().to_string()
    } else if response.starts_with("```") {
        response[3..].trim_end_matches("```").trim().to_string()
    } else {
        response.to_string()
    }
}

/// 尝试修复被 max_tokens 截断的 JSON
fn repair_truncated_json(json_str: &str) -> String {
    // 先尝试直接解析
    if serde_json::from_str::<serde_json::Value>(json_str).is_ok() {
        return json_str.to_string();
    }

    let trimmed = json_str.trim().to_string();

    // 策略1: 找到最后一个完整的字符串，补全引号
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    if len == 0 { return trimmed; }

    // 移除末尾被截断的字符串（最后一个未闭合的引号之后的内容）
    let mut in_string = false;
    let mut last_complete_pos = 0;
    let mut i = 0;
    while i < len {
        if chars[i] == '\\' { i += 2; continue; }
        if chars[i] == '"' {
            in_string = !in_string;
            if !in_string { last_complete_pos = i + 1; }
        }
        i += 1;
    }

    // 如果在字符串中间被截断，截断到最后一个完整字符串
    let mut repaired: String = if in_string && last_complete_pos > 0 && last_complete_pos < len {
        chars[..last_complete_pos].iter().collect()
    } else {
        trimmed.clone()
    };

    // 策略2: 补全缺失的闭合括号
    let open_braces = repaired.matches('{').count() as i32;
    let close_braces = repaired.matches('}').count() as i32;
    let open_brackets = repaired.matches('[').count() as i32;
    let close_brackets = repaired.matches(']').count() as i32;

    let missing_braces = open_braces - close_braces;
    let missing_brackets = open_brackets - close_brackets;

    // 如果缺少太多闭合括号，可能是严重截断，尝试截断到最后一个值
    if missing_braces > 20 || missing_brackets > 20 {
        // 找到最后一个完整的 , 或 } 处截断，再补上需要的闭合
        if let Some(pos) = repaired.rfind("},") {
            repaired.truncate(pos + 1);
        }
    }

    for _ in 0..missing_brackets {
        repaired.push(']');
    }
    for _ in 0..missing_braces {
        repaired.push('}');
    }

    // 验证修复结果
    if serde_json::from_str::<serde_json::Value>(&repaired).is_err() {
        log::warn!("[StoryBible] JSON修复后仍无法解析，返回原始截断文本");
        return trimmed;
    }

    log::info!("[StoryBible] JSON截断修复成功，补全了 {} 个括号、{} 个方括号", missing_braces, missing_brackets);
    repaired
}

fn parse_story_memory_json(book_id: &str, json_str: &str) -> anyhow::Result<StoryMemory> {
    let v: serde_json::Value = serde_json::from_str(json_str)?;

    // protagonist_status 可能是对象或字符串
    let protagonist_text = if v["protagonist_status"].is_object() {
        let ps = &v["protagonist_status"];
        let name = ps["name"].as_str().unwrap_or("主角");
        let state = ps["current_state"].as_str().unwrap_or("");
        let location = ps["current_location"].as_str().unwrap_or("");
        let goal = ps["current_goal"].as_str().unwrap_or("");
        let emotional = ps["emotional_state"].as_str().unwrap_or("");
        let recent = ps["recent_development"].as_str().unwrap_or("");
        let mut text = format!("{}：{}，位于{}。目标：{}。", name, state, location, goal);
        if !emotional.is_empty() { text.push_str(&format!(" 情感状态：{}。", emotional)); }
        if !recent.is_empty() { text.push_str(&format!(" 近期发展：{}。", recent)); }
        // 关键关系
        if let Some(rels) = ps["key_relationships"].as_array() {
            if !rels.is_empty() {
                text.push_str(" 关键关系：");
                let rel_strs: Vec<String> = rels.iter().map(|r| {
                    let rname = r["name"].as_str().unwrap_or("");
                    let rel = r["relationship"].as_str().unwrap_or("");
                    let dyna = r["current_dynamic"].as_str().unwrap_or("");
                    if dyna.is_empty() {
                        format!("{}（{}）", rname, rel)
                    } else {
                        format!("{}（{}，{}）", rname, rel, dyna)
                    }
                }).collect();
                text.push_str(&rel_strs.join("；"));
                text.push('。');
            }
        }
        text
    } else {
        v["protagonist_status"].as_str().unwrap_or("").to_string()
    };

    // world_rules 可能是对象或字符串
    let world_text = if v["world_rules"].is_object() {
        let wr = &v["world_rules"];
        let mut text = String::new();
        if let Some(p) = wr["power_system"].as_str() { if !p.is_empty() { text.push_str("修炼体系："); text.push_str(p); text.push(';'); } }
        if let Some(s) = wr["social_structure"].as_str() { if !s.is_empty() { text.push_str("社会格局："); text.push_str(s); text.push(';'); } }
        if let Some(arr) = wr["key_locations"].as_array() {
            let locs: Vec<String> = arr.iter().map(|l| {
                format!("{}（{}）", l["name"].as_str().unwrap_or(""), l["description"].as_str().unwrap_or(""))
            }).collect();
            if !locs.is_empty() { text.push_str("关键地点："); text.push_str(&locs.join("、").as_str()); text.push(';'); }
        }
        if let Some(arr) = wr["important_rules"].as_array() {
            let rules: Vec<&str> = arr.iter().filter_map(|r| r.as_str()).collect();
            if !rules.is_empty() { text.push_str("重要规则："); text.push_str(&rules.join("、").as_str()); text.push(';'); }
        }
        if let Some(arr) = wr["factions"].as_array() {
            let facs: Vec<String> = arr.iter().map(|f| {
                format!("{}（{}）", f["name"].as_str().unwrap_or(""), f["description"].as_str().unwrap_or(""))
            }).collect();
            if !facs.is_empty() { text.push_str("势力："); text.push_str(&facs.join("、").as_str()); text.push(';'); }
        }
        text
    } else {
        v["world_rules"].as_str().unwrap_or("").to_string()
    };

    // story_lines 拼入 book_summary 后面
    let story_lines_text = if let Some(arr) = v["story_lines"].as_array() {
        let lines: Vec<String> = arr.iter().map(|sl| {
            let name = sl["name"].as_str().unwrap_or("");
            let status = sl["status"].as_str().unwrap_or("");
            let summary = sl["summary"].as_str().unwrap_or("");
            format!("- {}（{}）：{}", name, status, summary)
        }).collect();
        if !lines.is_empty() {
            format!("\n\n【故事线】\n{}", lines.join("\n"))
        } else { String::new() }
    } else { String::new() };

    let book_summary = format!(
        "{}{}",
        v["book_summary"].as_str().unwrap_or(""),
        story_lines_text
    );

    // unresolved_threads 可能是字符串数组或对象数组
    let unresolved: Vec<String> = if let Some(arr) = v["unresolved_threads"].as_array() {
        arr.iter().map(|item| {
            if item.is_object() {
                let thread = item["thread"].as_str().unwrap_or("");
                let ch = item["introduced_chapter"].as_i64().unwrap_or(0);
                format!("{}（第{}章引入）", thread, ch)
            } else {
                item.as_str().unwrap_or("").to_string()
            }
        }).filter(|s| !s.is_empty()).collect()
    } else {
        vec![]
    };

    // key_character_statuses 可能包含 arc_summary 等新字段
    let characters: Vec<KeyCharacterStatus> = v["key_character_statuses"].as_array()
        .map(|arr| arr.iter()
            .map(|item| {
                let name = item["name"].as_str().unwrap_or("").to_string();
                let status = item["current_state"].as_str()
                    .or_else(|| item["status"].as_str())
                    .unwrap_or("").to_string();
                let location = item["current_location"].as_str()
                    .or_else(|| item["location"].as_str())
                    .unwrap_or("").to_string();
                // 如果有 arc_summary，拼入 status
                let status = if let Some(arc) = item["arc_summary"].as_str() {
                    if !arc.is_empty() { format!("{}。{}", status, arc) } else { status }
                } else { status };
                KeyCharacterStatus { name, status, location }
            })
            .collect())
        .unwrap_or_default();

    Ok(StoryMemory {
        book_id: book_id.to_string(),
        book_summary,
        volume_summaries: v["volume_summaries"].as_array()
            .map(|arr| arr.iter()
                .map(|item| VolumeSummaryItem {
                    title: item["title"].as_str().unwrap_or("").to_string(),
                    summary: item["summary"].as_str().unwrap_or("").to_string(),
                })
                .collect())
            .unwrap_or_default(),
        event_timeline: v["event_timeline"].as_array()
            .map(|arr| arr.iter()
                .map(|item| EventTimelineItem {
                    chapter: item["chapter"].as_i64().unwrap_or(0) as i32,
                    title: item["title"].as_str().unwrap_or("").to_string(),
                    event: item["event"].as_str().unwrap_or("").to_string(),
                    impact: item["impact"].as_str().unwrap_or("").to_string(),
                    arc: item["arc"].as_str().unwrap_or("").to_string(),
                })
                .collect())
            .unwrap_or_default(),
        protagonist_status: protagonist_text,
        key_character_statuses: characters,
        unresolved_threads: unresolved,
        world_rules: world_text,
        last_chapter_count: 0,
        last_word_count: 0,
        updated_at: 0,
    })
}

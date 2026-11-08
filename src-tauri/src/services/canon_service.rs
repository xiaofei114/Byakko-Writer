//! 设定账本（Canon Ledger）
//!
//! 把章节里的"事实"结构化存下来，供 AI 精确查询：
//! - `canon_entities`：角色/地点/势力/物品/功法/术语
//! - `canon_aliases`：别名归一（小动 → 林动）
//! - `canon_states`：状态时间线（境界/位置/伤势/持有物…），按 seq 追加
//! - `canon_events`：事件流，只追加不压缩
//!
//! 每章摘要生成后自动抽取一次；已有书籍可通过"强制重新生成故事记忆"触发全书回填。

use anyhow::Result;
use once_cell::sync::Lazy;
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashSet;
use std::sync::Mutex;

use crate::db::get_pool;
use crate::models::{AIConfig, CanonEntityDraft, CanonExtraction, ChatMessage};
use crate::services::ai_service::send_ai_message;
use crate::services::prompt_service::get_prompt_manager;

/// 正在抽取的章节（防止重复抽取）
static EXTRACTING: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// 同时最多 3 个抽取请求，避免批量重建时打爆 API 限流
static CANON_SEMAPHORE: Lazy<tokio::sync::Semaphore> =
    Lazy::new(|| tokio::sync::Semaphore::new(3));

struct ExtractGuard(String);

impl Drop for ExtractGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = EXTRACTING.lock() {
            set.remove(&self.0);
        }
    }
}

fn alias_key(name: &str) -> String {
    name.trim().to_lowercase()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// 从干净/带代码块的文本中提取 JSON 对象
fn clean_json(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fence = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    // 截取第一个 { 到最后一个 }
    match (without_fence.find('{'), without_fence.rfind('}')) {
        (Some(start), Some(end)) if end > start => without_fence[start..=end].to_string(),
        _ => without_fence.to_string(),
    }
}

/// 抽取单章设定并落库，返回写入条数
pub async fn extract_from_chapter(
    book_id: &str,
    chapter_id: &str,
    config: &AIConfig,
) -> Result<usize> {
    if config.api_key.is_empty() {
        return Ok(0);
    }

    // 去重：同一章同时只允许一个抽取任务
    {
        let mut set = match EXTRACTING.lock() {
            Ok(set) => set,
            Err(_) => return Ok(0),
        };
        if set.contains(chapter_id) {
            return Ok(0);
        }
        set.insert(chapter_id.to_string());
    }
    let _guard = ExtractGuard(chapter_id.to_string());

    // 限制并发：批量回填时排队执行
    let _permit = match CANON_SEMAPHORE.acquire().await {
        Ok(permit) => permit,
        Err(_) => return Ok(0),
    };

    let content = crate::services::book_service::load_chapter_content(chapter_id.to_string())
        .await
        .unwrap_or_default();
    if content.trim().chars().count() < 150 {
        return Ok(0);
    }

    let pool = get_pool().await?;

    let chapter = sqlx::query(r#"SELECT title, "order" FROM chapters WHERE id = ?1"#)
        .bind(chapter_id)
        .fetch_optional(pool)
        .await?;
    let Some(chapter) = chapter else {
        return Ok(0);
    };
    let title: String = chapter.try_get("title").unwrap_or_default();
    let order: i32 = chapter.try_get("order").unwrap_or(0);

    // 已有实体清单，帮助 AI 复用规范名
    let existing_rows = sqlx::query(
        "SELECT kind, name, summary FROM canon_entities WHERE book_id = ?1 ORDER BY updated_at DESC LIMIT 60",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;
    let existing = existing_rows
        .iter()
        .map(|row| {
            format!(
                "- [{}] {}：{}",
                row.try_get::<String, _>("kind").unwrap_or_default(),
                row.try_get::<String, _>("name").unwrap_or_default(),
                truncate_chars(&row.try_get::<String, _>("summary").unwrap_or_default(), 40)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = get_prompt_manager().canon_extract.clone();
    let user_prompt = format!(
        "## 章节\n第{}章「{}」\n\n## 已有设定（尽量复用规范名，不要重复建立同义实体）\n{}\n\n## 正文\n{}",
        order,
        title,
        if existing.is_empty() { "（暂无）" } else { &existing },
        truncate_chars(&content, 12000)
    );

    let mut cfg = config.clone();
    cfg.temperature = 0.2;
    cfg.max_tokens = cfg.max_tokens.max(6000);

    let decision = send_ai_message(
        vec![
            ChatMessage::new("system", &system_prompt),
            ChatMessage::new("user", &user_prompt),
        ],
        cfg,
        None,
    )
    .await?;

    let raw = decision.content.unwrap_or_default();
    let extraction: CanonExtraction = match serde_json::from_str(&clean_json(&raw)) {
        Ok(value) => value,
        Err(e) => {
            log::warn!(
                "[Canon] 抽取结果解析失败: {}；原文开头: {}",
                e,
                truncate_chars(&raw, 200)
            );
            return Ok(0);
        }
    };

    let mut written = 0usize;

    // 抽取结果为空多半是模型抖动：保留本章旧设定，等下次重试
    if extraction.entities.is_empty()
        && extraction.states.is_empty()
        && extraction.events.is_empty()
    {
        log::warn!("[Canon] 第{}章抽取结果为空，保留旧设定", order);
        return Ok(0);
    }

    // 重抽取前清掉本章旧事实，保证幂等（改稿后不会残留旧状态/旧事件）
    let _ = sqlx::query("DELETE FROM canon_states WHERE chapter_id = ?1")
        .bind(chapter_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM canon_events WHERE chapter_id = ?1")
        .bind(chapter_id)
        .execute(pool)
        .await;

    for entity in &extraction.entities {
        if entity.name.trim().is_empty() {
            continue;
        }
        if upsert_entity(book_id, pool, entity, order).await.is_ok() {
            written += 1;
        }
    }

    for state in &extraction.states {
        if state.field.trim().is_empty() || state.entity.trim().is_empty() {
            continue;
        }
        if let Some(entity_id) = find_entity_id(book_id, pool, &state.entity).await? {
            if insert_state(book_id, pool, &entity_id, chapter_id, order, state)
                .await
                .is_ok()
            {
                written += 1;
            }
        }
    }

    for event in &extraction.events {
        if event.predicate.trim().is_empty() {
            continue;
        }
        let subject_id = find_entity_id(book_id, pool, &event.subject).await?;
        if insert_event(book_id, pool, chapter_id, order, subject_id.as_deref(), event)
            .await
            .is_ok()
        {
            written += 1;
        }
    }

    // 记录抽取版本，供保存后判断账本是否过期
    let content_updated_at: i64 = sqlx::query_scalar(
        "SELECT updated_at FROM chapters WHERE id = ?1",
    )
    .bind(chapter_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(0);

    let _ = sqlx::query(
        r#"
        INSERT OR REPLACE INTO canon_extractions
        (chapter_id, book_id, content_updated_at, extracted_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
    )
    .bind(chapter_id)
    .bind(book_id)
    .bind(content_updated_at)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await;

    if written > 0 {
        let _ = sqlx::query(
            "UPDATE canon_entities SET last_chapter = ?1 WHERE book_id = ?2 AND last_chapter < ?1",
        )
        .bind(order)
        .bind(book_id)
        .execute(pool)
        .await;
    }

    log::info!("[Canon] 第{}章「{}」抽取完成，写入 {} 条", order, title, written);
    Ok(written)
}

async fn upsert_entity(
    book_id: &str,
    pool: &Pool<Sqlite>,
    entity: &CanonEntityDraft,
    order: i32,
) -> Result<()> {
    let kind = if entity.kind.trim().is_empty() {
        "character"
    } else {
        entity.kind.trim()
    };
    let now = chrono::Utc::now().timestamp_millis();

    let existing: Option<(String, i32)> = sqlx::query_as(
        "SELECT id, manual FROM canon_entities WHERE book_id = ?1 AND kind = ?2 AND name = ?3",
    )
    .bind(book_id)
    .bind(kind)
    .bind(&entity.name)
    .fetch_optional(pool)
    .await?;

    let entity_id = match existing {
        Some((id, manual)) => {
            if manual == 1 {
                // 人工修正过的简介/属性不自动覆盖，只刷新出场章节
                sqlx::query(
                    "UPDATE canon_entities SET last_chapter = ?1, updated_at = ?2 WHERE id = ?3",
                )
                .bind(order)
                .bind(now)
                .bind(&id)
                .execute(pool)
                .await?;
            } else {
                sqlx::query(
                    r#"
                    UPDATE canon_entities
                    SET summary = CASE WHEN ?1 = '' THEN summary ELSE ?1 END,
                        attrs = CASE WHEN ?2 = '' THEN attrs ELSE ?2 END,
                        last_chapter = ?3,
                        updated_at = ?4
                    WHERE id = ?5
                    "#,
                )
                .bind(&entity.summary)
                .bind(&entity.attrs)
                .bind(order)
                .bind(now)
                .bind(&id)
                .execute(pool)
                .await?;
            }
            id
        }
        None => {
            let id = format!("canon_{}", uuid::Uuid::new_v4());
            sqlx::query(
                r#"
                INSERT INTO canon_entities
                (id, book_id, kind, name, summary, attrs, first_chapter, last_chapter, status, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 'active', ?8, ?8)
                "#,
            )
            .bind(&id)
            .bind(book_id)
            .bind(kind)
            .bind(&entity.name)
            .bind(&entity.summary)
            .bind(&entity.attrs)
            .bind(order)
            .bind(now)
            .execute(pool)
            .await?;
            id
        }
    };

    for alias in &entity.aliases {
        if alias.trim().is_empty() {
            continue;
        }
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO canon_aliases (book_id, alias, alias_key, entity_id) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(book_id)
        .bind(alias.trim())
        .bind(alias_key(alias))
        .bind(&entity_id)
        .execute(pool)
        .await;
    }

    Ok(())
}

async fn find_entity_id(book_id: &str, pool: &Pool<Sqlite>, name: &str) -> Result<Option<String>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }

    let direct: Option<String> = sqlx::query_scalar(
        "SELECT id FROM canon_entities WHERE book_id = ?1 AND name = ?2 LIMIT 1",
    )
    .bind(book_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;

    if direct.is_some() {
        return Ok(direct);
    }

    let via_alias: Option<String> = sqlx::query_scalar(
        "SELECT entity_id FROM canon_aliases WHERE book_id = ?1 AND alias_key = ?2 LIMIT 1",
    )
    .bind(book_id)
    .bind(alias_key(name))
    .fetch_optional(pool)
    .await?;

    Ok(via_alias)
}

async fn insert_state(
    book_id: &str,
    pool: &Pool<Sqlite>,
    entity_id: &str,
    chapter_id: &str,
    order: i32,
    state: &crate::models::CanonStateDraft,
) -> Result<()> {
    let next_seq: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM canon_states WHERE book_id = ?1")
        .bind(book_id)
        .fetch_one(pool)
        .await?;

    sqlx::query(
        r#"
        INSERT INTO canon_states
        (id, book_id, entity_id, chapter_id, chapter_order, seq, field, value, quote, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(format!("cs_{}", uuid::Uuid::new_v4()))
    .bind(book_id)
    .bind(entity_id)
    .bind(chapter_id)
    .bind(order)
    .bind(next_seq)
    .bind(&state.field)
    .bind(&state.value)
    .bind(&state.quote)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_event(
    book_id: &str,
    pool: &Pool<Sqlite>,
    chapter_id: &str,
    order: i32,
    subject_id: Option<&str>,
    event: &crate::models::CanonEventDraft,
) -> Result<()> {
    let next_seq: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM canon_events WHERE book_id = ?1")
        .bind(book_id)
        .fetch_one(pool)
        .await?;

    let importance = if event.importance.trim().is_empty() {
        "normal"
    } else {
        event.importance.trim()
    };

    sqlx::query(
        r#"
        INSERT INTO canon_events
        (id, book_id, chapter_id, chapter_order, seq, subject_id, subject_name, predicate, object, quote, importance, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(format!("ce_{}", uuid::Uuid::new_v4()))
    .bind(book_id)
    .bind(chapter_id)
    .bind(order)
    .bind(next_seq)
    .bind(subject_id)
    .bind(&event.subject)
    .bind(&event.predicate)
    .bind(&event.object)
    .bind(&event.quote)
    .bind(importance)
    .bind(chrono::Utc::now().timestamp_millis())
    .execute(pool)
    .await?;

    Ok(())
}

/// 查询若干实体的设定卡（名字或别名均可）
pub async fn get_canon(
    book_id: &str,
    names: &[String],
    as_of_chapter: Option<i32>,
) -> Result<String> {
    let pool = get_pool().await?;
    let mut out = String::new();

    for name in names.iter().take(10) {
        let Some(entity_id) = find_entity_id(book_id, pool, name).await? else {
            out.push_str(&format!("「{}」：账本中未收录\n\n", name));
            continue;
        };

        let row = sqlx::query(
            "SELECT kind, name, summary, status, first_chapter, last_chapter FROM canon_entities WHERE id = ?1",
        )
        .bind(&entity_id)
        .fetch_one(pool)
        .await?;

        let kind: String = row.try_get("kind").unwrap_or_default();
        let canonical: String = row.try_get("name").unwrap_or_default();
        let summary: String = row.try_get("summary").unwrap_or_default();
        let status: String = row.try_get("status").unwrap_or_default();
        let first: i32 = row.try_get("first_chapter").unwrap_or(0);
        let last: i32 = row.try_get("last_chapter").unwrap_or(0);

        let aliases: Vec<String> = sqlx::query_scalar(
            "SELECT alias FROM canon_aliases WHERE book_id = ?1 AND entity_id = ?2",
        )
        .bind(book_id)
        .bind(&entity_id)
        .fetch_all(pool)
        .await?;

        let state_rows = sqlx::query(
            "SELECT field, value, chapter_order FROM canon_states WHERE entity_id = ?1 ORDER BY seq ASC",
        )
        .bind(&entity_id)
        .fetch_all(pool)
        .await?;

        // 每个字段取「截至目标章的最后一条」
        let mut latest: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for state in &state_rows {
            let order: i32 = state.try_get("chapter_order").unwrap_or(0);
            if let Some(limit) = as_of_chapter {
                if order > limit {
                    continue;
                }
            }
            let field: String = state.try_get("field").unwrap_or_default();
            let value: String = state.try_get("value").unwrap_or_default();
            latest.insert(field, value);
        }

        let event_rows = sqlx::query(
            r#"
            SELECT chapter_order, predicate, object, importance FROM canon_events
            WHERE book_id = ?1 AND (subject_id = ?2 OR subject_name = ?3)
            ORDER BY seq DESC LIMIT 5
            "#,
        )
        .bind(book_id)
        .bind(&entity_id)
        .bind(&canonical)
        .fetch_all(pool)
        .await?;

        out.push_str(&format!(
            "【{}】{}（别名：{}）\n首次：第{}章  最近：第{}章  状态：{}\n{}\n",
            kind_label(&kind),
            canonical,
            if aliases.is_empty() { "无".to_string() } else { aliases.join("、") },
            first,
            last,
            status,
            summary
        ));

        if !latest.is_empty() {
            let mut states: Vec<String> = latest
                .iter()
                .map(|(field, value)| format!("{}={}", field, value))
                .collect();
            states.sort();
            out.push_str(&format!("当前状态：{}\n", states.join("；")));
        }

        if !event_rows.is_empty() {
            out.push_str("最近事件：\n");
            for event in &event_rows {
                out.push_str(&format!(
                    "- 第{}章 {} {}{}\n",
                    event.try_get::<i32, _>("chapter_order").unwrap_or(0),
                    event.try_get::<String, _>("predicate").unwrap_or_default(),
                    event.try_get::<String, _>("object").unwrap_or_default(),
                    match event.try_get::<String, _>("importance").unwrap_or_default().as_str() {
                        "high" => "（关键）",
                        _ => "",
                    }
                ));
            }
        }

        out.push('\n');
    }

    Ok(out.trim_end().to_string())
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "character" => "角色",
        "location" => "地点",
        "faction" => "势力",
        "item" => "物品",
        "skill" => "功法",
        "term" => "术语",
        _ => "设定",
    }
}

/// 模糊检索设定
pub async fn search_canon(
    book_id: &str,
    query: &str,
    kind: Option<&str>,
) -> Result<String> {
    let pool = get_pool().await?;
    let pattern = format!("%{}%", query.trim());
    if query.trim().is_empty() {
        return Ok("搜索词为空".to_string());
    }

    let rows = match kind {
        Some(kind) if !kind.is_empty() => sqlx::query(
            "SELECT name, kind, summary FROM canon_entities WHERE book_id = ?1 AND kind = ?2 AND (name LIKE ?3 OR summary LIKE ?3) LIMIT 15",
        )
        .bind(book_id)
        .bind(kind)
        .bind(&pattern)
        .fetch_all(pool)
        .await?,
        _ => sqlx::query(
            "SELECT name, kind, summary FROM canon_entities WHERE book_id = ?1 AND (name LIKE ?2 OR summary LIKE ?2) LIMIT 15",
        )
        .bind(book_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await?,
    };

    if rows.is_empty() {
        // 再从别名里找
        let alias_rows = sqlx::query(
            r#"
            SELECT e.name, e.kind, e.summary
            FROM canon_aliases a JOIN canon_entities e ON e.id = a.entity_id
            WHERE a.book_id = ?1 AND a.alias LIKE ?2 LIMIT 10
            "#,
        )
        .bind(book_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await?;

        if alias_rows.is_empty() {
            return Ok(format!("未找到与「{}」相关的设定", query));
        }
        return Ok(format_rows(&alias_rows));
    }

    Ok(format_rows(&rows))
}

fn format_rows(rows: &[sqlx::sqlite::SqliteRow]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                "- [{}] {}：{}",
                kind_label(&row.try_get::<String, _>("kind").unwrap_or_default()),
                row.try_get::<String, _>("name").unwrap_or_default(),
                truncate_chars(&row.try_get::<String, _>("summary").unwrap_or_default(), 80)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 为写作上下文拼装「相关设定卡」（按名字/别名匹配上下文）
pub async fn build_context_notes(book_id: &str, context: &str) -> String {
    let Ok(pool) = get_pool().await else {
        return String::new();
    };

    let Ok(rows) = sqlx::query(
        "SELECT id, kind, name, summary FROM canon_entities WHERE book_id = ?1 ORDER BY last_chapter DESC LIMIT 200",
    )
    .bind(book_id)
    .fetch_all(pool)
    .await
    else {
        return String::new();
    };

    let mut notes = String::new();
    let mut matched = 0usize;

    // 一次取出全部别名，避免逐个实体查询
    let alias_rows = sqlx::query("SELECT entity_id, alias FROM canon_aliases WHERE book_id = ?1")
        .bind(book_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let mut alias_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in &alias_rows {
        let entity_id: String = row.try_get("entity_id").unwrap_or_default();
        let alias: String = row.try_get("alias").unwrap_or_default();
        alias_map.entry(entity_id).or_default().push(alias);
    }

    for row in &rows {
        if matched >= 8 {
            break;
        }
        let entity_id: String = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        if name.is_empty() {
            continue;
        }

        let aliases = alias_map.get(&entity_id).cloned().unwrap_or_default();

        let mut names = vec![name.clone()];
        names.extend(aliases.iter().cloned());
        if !names.iter().any(|n| n.chars().count() >= 2 && context.contains(n.as_str())) {
            continue;
        }

        let kind: String = row.try_get("kind").unwrap_or_default();
        let summary: String = row.try_get("summary").unwrap_or_default();

        // 最新状态（不限章节）
        let state_rows = sqlx::query(
            "SELECT field, value FROM canon_states WHERE entity_id = ?1 ORDER BY seq ASC",
        )
        .bind(&entity_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut latest: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for state in &state_rows {
            latest.insert(
                state.try_get("field").unwrap_or_default(),
                state.try_get("value").unwrap_or_default(),
            );
        }

        matched += 1;
        notes.push_str(&format!(
            "- [{}] {}（{}）：{}{}\n",
            kind_label(&kind),
            name,
            if aliases.is_empty() { "无别名".to_string() } else { aliases.join("、") },
            truncate_chars(&summary, 120),
            if latest.is_empty() {
                String::new()
            } else {
                let mut states: Vec<String> = latest
                    .iter()
                    .map(|(field, value)| format!("{}={}", field, value))
                    .collect();
                states.sort();
                format!("；当前：{}", states.join("，"))
            }
        ));
    }

    if matched == 0 {
        return String::new();
    }

    format!("## 相关设定（来自设定账本，必须遵守）\n{}", notes)
}

// ========== 保存后自动重抽（账本随改稿更新） ==========

static REEXTRACT_TS: Lazy<Mutex<std::collections::HashMap<String, i64>>> =
    Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

/// 章节保存后调度一次账本重抽（防抖 30 秒）
///
/// 只有「书里已经有账本数据」且「本章内容比抽取时新」才会真正调用 AI，
/// 没启用账本的书不会因为写字而多花钱。
pub fn schedule_reextract(chapter_id: String) {
    let now = chrono::Utc::now().timestamp_millis();

    if let Ok(mut guard) = REEXTRACT_TS.lock() {
        guard.insert(chapter_id.clone(), now);
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        let is_latest = REEXTRACT_TS
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

        let pool = match get_pool().await {
            Ok(pool) => pool,
            Err(_) => return,
        };

        let book_id: Option<String> =
            sqlx::query_scalar("SELECT book_id FROM chapters WHERE id = ?1")
                .bind(&chapter_id)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        let Some(book_id) = book_id else {
            return;
        };

        let entity_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM canon_entities WHERE book_id = ?1")
                .bind(&book_id)
                .fetch_one(pool)
                .await
                .unwrap_or(0);
        if entity_count == 0 {
            return;
        }

        let updated_at: i64 = sqlx::query_scalar("SELECT updated_at FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or(0);
        let extracted_at: Option<i64> = sqlx::query_scalar(
            "SELECT content_updated_at FROM canon_extractions WHERE chapter_id = ?1",
        )
        .bind(&chapter_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if extracted_at == Some(updated_at) {
            return;
        }

        if let Err(e) =
            extract_from_chapter(&book_id, &chapter_id, &config.ai).await
        {
            log::warn!("[Canon] 改稿后重抽失败 {}: {}", chapter_id, e);
        }
    });
}

// ========== 账本管理（界面用） ==========

/// 列出实体（可按关键词/类型过滤）
pub async fn list_entities(
    book_id: &str,
    keyword: Option<String>,
    kind: Option<String>,
) -> Result<Vec<serde_json::Value>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT id, kind, name, summary, status, first_chapter, last_chapter, manual
        FROM canon_entities WHERE book_id = ?1
        ORDER BY last_chapter DESC, name ASC
        LIMIT 2000
        "#,
    )
    .bind(book_id)
    .fetch_all(pool)
    .await?;

    let keyword = keyword.unwrap_or_default().trim().to_lowercase();
    let kind = kind.unwrap_or_default();

    // 一次取出全部别名，避免每个实体一次查询
    let alias_rows = sqlx::query("SELECT entity_id, alias FROM canon_aliases WHERE book_id = ?1")
        .bind(book_id)
        .fetch_all(pool)
        .await?;
    let mut alias_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in &alias_rows {
        let entity_id: String = row.try_get("entity_id").unwrap_or_default();
        let alias: String = row.try_get("alias").unwrap_or_default();
        alias_map.entry(entity_id).or_default().push(alias);
    }

    let mut result = Vec::new();

    for row in rows {
        let entity_kind: String = row.try_get("kind").unwrap_or_default();
        if !kind.is_empty() && kind != "all" && entity_kind != kind {
            continue;
        }

        let entity_id: String = row.try_get("id").unwrap_or_default();
        let name: String = row.try_get("name").unwrap_or_default();
        let summary: String = row.try_get("summary").unwrap_or_default();
        let aliases = alias_map.get(&entity_id).cloned().unwrap_or_default();

        if !keyword.is_empty() {
            let haystack = format!(
                "{} {} {}",
                name.to_lowercase(),
                summary.to_lowercase(),
                aliases.join(" ").to_lowercase()
            );
            if !haystack.contains(&keyword) {
                continue;
            }
        }

        result.push(serde_json::json!({
            "id": entity_id,
            "kind": entity_kind,
            "name": name,
            "summary": summary,
            "status": row.try_get::<String, _>("status").unwrap_or_default(),
            "firstChapter": row.try_get::<i32, _>("first_chapter").unwrap_or(0),
            "lastChapter": row.try_get::<i32, _>("last_chapter").unwrap_or(0),
            "manual": row.try_get::<i32, _>("manual").unwrap_or(0) == 1,
            "aliases": aliases,
        }));
    }

    Ok(result)
}

/// 单个实体的详情（含状态时间线与事件）
pub async fn entity_detail(book_id: &str, entity_id: &str) -> Result<serde_json::Value> {
    let pool = get_pool().await?;

    let entity = sqlx::query(
        r#"
        SELECT id, kind, name, summary, attrs, status, first_chapter, last_chapter, manual
        FROM canon_entities WHERE id = ?1 AND book_id = ?2
        "#,
    )
    .bind(entity_id)
    .bind(book_id)
    .fetch_optional(pool)
    .await?;

    let Some(entity) = entity else {
        return Ok(serde_json::json!({ "error": "实体不存在" }));
    };

    let aliases: Vec<String> =
        sqlx::query_scalar("SELECT alias FROM canon_aliases WHERE entity_id = ?1")
            .bind(entity_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let state_rows = sqlx::query(
        "SELECT id, field, value, chapter_order, quote FROM canon_states WHERE entity_id = ?1 ORDER BY chapter_order ASC, seq ASC",
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;

    let states: Vec<serde_json::Value> = state_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.try_get::<String, _>("id").unwrap_or_default(),
                "field": row.try_get::<String, _>("field").unwrap_or_default(),
                "value": row.try_get::<String, _>("value").unwrap_or_default(),
                "chapterOrder": row.try_get::<i32, _>("chapter_order").unwrap_or(0),
                "quote": row.try_get::<String, _>("quote").unwrap_or_default(),
            })
        })
        .collect();

    let event_rows = sqlx::query(
        r#"
        SELECT id, predicate, object, quote, importance, chapter_order
        FROM canon_events
        WHERE book_id = ?1 AND (subject_id = ?2 OR subject_name = ?3)
        ORDER BY chapter_order ASC, seq ASC
        "#,
    )
    .bind(book_id)
    .bind(entity_id)
    .bind(entity.try_get::<String, _>("name").unwrap_or_default())
    .fetch_all(pool)
    .await?;

    let events: Vec<serde_json::Value> = event_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.try_get::<String, _>("id").unwrap_or_default(),
                "predicate": row.try_get::<String, _>("predicate").unwrap_or_default(),
                "object": row.try_get::<String, _>("object").unwrap_or_default(),
                "quote": row.try_get::<String, _>("quote").unwrap_or_default(),
                "importance": row.try_get::<String, _>("importance").unwrap_or_default(),
                "chapterOrder": row.try_get::<i32, _>("chapter_order").unwrap_or(0),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "id": entity.try_get::<String, _>("id").unwrap_or_default(),
        "kind": entity.try_get::<String, _>("kind").unwrap_or_default(),
        "name": entity.try_get::<String, _>("name").unwrap_or_default(),
        "summary": entity.try_get::<String, _>("summary").unwrap_or_default(),
        "attrs": entity.try_get::<String, _>("attrs").unwrap_or_default(),
        "status": entity.try_get::<String, _>("status").unwrap_or_default(),
        "firstChapter": entity.try_get::<i32, _>("first_chapter").unwrap_or(0),
        "lastChapter": entity.try_get::<i32, _>("last_chapter").unwrap_or(0),
        "manual": entity.try_get::<i32, _>("manual").unwrap_or(0) == 1,
        "aliases": aliases,
        "states": states,
        "events": events,
    }))
}

/// 人工修正实体（名称/类型/简介/别名）
pub async fn update_entity(
    book_id: &str,
    entity_id: &str,
    name: &str,
    kind: &str,
    summary: &str,
    aliases: Vec<String>,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err(anyhow::anyhow!("名称不能为空"));
    }

    let pool = get_pool().await?;

    let duplicated: Option<String> = sqlx::query_scalar(
        "SELECT id FROM canon_entities WHERE book_id = ?1 AND kind = ?2 AND name = ?3 AND id <> ?4",
    )
    .bind(book_id)
    .bind(kind)
    .bind(name.trim())
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    if duplicated.is_some() {
        return Err(anyhow::anyhow!("同类型下已存在同名实体"));
    }

    sqlx::query(
        r#"
        UPDATE canon_entities
        SET name = ?1, kind = ?2, summary = ?3, manual = 1, updated_at = ?4
        WHERE id = ?5 AND book_id = ?6
        "#,
    )
    .bind(name.trim())
    .bind(kind)
    .bind(summary)
    .bind(chrono::Utc::now().timestamp_millis())
    .bind(entity_id)
    .bind(book_id)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM canon_aliases WHERE entity_id = ?1")
        .bind(entity_id)
        .execute(pool)
        .await?;

    for alias in aliases {
        if alias.trim().is_empty() {
            continue;
        }
        // 别名全局唯一：人工指定时把旧归属清掉
        let _ = sqlx::query(
            "DELETE FROM canon_aliases WHERE book_id = ?1 AND alias_key = ?2 AND entity_id <> ?3",
        )
        .bind(book_id)
        .bind(alias_key(alias.trim()))
        .bind(entity_id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "INSERT OR IGNORE INTO canon_aliases (book_id, alias, alias_key, entity_id) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(book_id)
        .bind(alias.trim())
        .bind(alias_key(alias.trim()))
        .bind(entity_id)
        .execute(pool)
        .await;
    }

    Ok(())
}

/// 删除实体（别名/状态级联删除，事件手动清理）
pub async fn delete_entity(book_id: &str, entity_id: &str) -> Result<()> {
    let pool = get_pool().await?;
    let _ = sqlx::query("DELETE FROM canon_events WHERE book_id = ?1 AND subject_id = ?2")
        .bind(book_id)
        .bind(entity_id)
        .execute(pool)
        .await;
    sqlx::query("DELETE FROM canon_entities WHERE id = ?1 AND book_id = ?2")
        .bind(entity_id)
        .bind(book_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 删除一条状态
pub async fn delete_state(book_id: &str, state_id: &str) -> Result<()> {
    let pool = get_pool().await?;
    sqlx::query("DELETE FROM canon_states WHERE id = ?1 AND book_id = ?2")
        .bind(state_id)
        .bind(book_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 删除一条事件
pub async fn delete_event(book_id: &str, event_id: &str) -> Result<()> {
    let pool = get_pool().await?;
    sqlx::query("DELETE FROM canon_events WHERE id = ?1 AND book_id = ?2")
        .bind(event_id)
        .bind(book_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 账本统计
pub async fn stats(book_id: &str) -> Result<serde_json::Value> {
    let pool = get_pool().await?;

    let entities: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM canon_entities WHERE book_id = ?1")
            .bind(book_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let aliases: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM canon_aliases WHERE book_id = ?1")
            .bind(book_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let states: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canon_states WHERE book_id = ?1")
        .bind(book_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM canon_events WHERE book_id = ?1")
        .bind(book_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let extracted_chapters: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM canon_extractions WHERE book_id = ?1")
            .bind(book_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    Ok(serde_json::json!({
        "entities": entities,
        "aliases": aliases,
        "states": states,
        "events": events,
        "extractedChapters": extracted_chapters,
    }))
}

/// 时间线查询：某实体在指定章节区间内的状态与事件
pub async fn get_canon_timeline(
    book_id: &str,
    name: &str,
    from_chapter: Option<i32>,
    to_chapter: Option<i32>,
) -> Result<String> {
    let pool = get_pool().await?;

    let Some(entity_id) = find_entity_id(book_id, pool, name).await? else {
        return Ok(format!("账本中未收录「{}」，可用 search_canon 先检索", name));
    };

    let canonical: String = sqlx::query_scalar("SELECT name FROM canon_entities WHERE id = ?1")
        .bind(&entity_id)
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| name.to_string());

    let from = from_chapter.unwrap_or(1);
    let to = to_chapter.unwrap_or(i32::MAX);

    let state_rows = sqlx::query(
        "SELECT chapter_order, field, value FROM canon_states WHERE entity_id = ?1 ORDER BY chapter_order ASC, seq ASC",
    )
    .bind(&entity_id)
    .fetch_all(pool)
    .await?;

    let event_rows = sqlx::query(
        r#"
        SELECT chapter_order, predicate, object, importance FROM canon_events
        WHERE book_id = ?1 AND (subject_id = ?2 OR subject_name = ?3)
        ORDER BY chapter_order ASC, seq ASC
        "#,
    )
    .bind(book_id)
    .bind(&entity_id)
    .bind(&canonical)
    .fetch_all(pool)
    .await?;

    let mut items: Vec<(i32, String)> = Vec::new();

    for row in &state_rows {
        let order: i32 = row.try_get("chapter_order").unwrap_or(0);
        if order < from || order > to {
            continue;
        }
        items.push((
            order,
            format!(
                "状态：{} = {}",
                row.try_get::<String, _>("field").unwrap_or_default(),
                row.try_get::<String, _>("value").unwrap_or_default()
            ),
        ));
    }

    for row in &event_rows {
        let order: i32 = row.try_get("chapter_order").unwrap_or(0);
        if order < from || order > to {
            continue;
        }
        items.push((
            order,
            format!(
                "事件：{}{}{}",
                row.try_get::<String, _>("predicate").unwrap_or_default(),
                row.try_get::<String, _>("object").unwrap_or_default(),
                match row.try_get::<String, _>("importance").unwrap_or_default().as_str() {
                    "high" => "（关键）",
                    _ => "",
                }
            ),
        ));
    }

    if items.is_empty() {
        return Ok(format!(
            "「{}」在第 {}-{} 章之间没有记录",
            canonical, from, to
        ));
    }

    items.sort_by_key(|(order, _)| *order);

    let mut out = format!("「{}」时间线：\n", canonical);
    for (order, text) in items.iter().take(200) {
        out.push_str(&format!("- 第{}章｜{}\n", order, text));
    }

    Ok(out)
}

/// 全书回填：逐章重新抽取（后台执行，通过事件上报进度）
pub async fn rebuild_book(
    app: &tauri::AppHandle,
    book_id: &str,
    config: &AIConfig,
) -> Result<usize> {
    use tauri::Emitter;

    let pool = get_pool().await?;
    let rows = sqlx::query(r#"SELECT id FROM chapters WHERE book_id = ?1 ORDER BY "order" ASC"#)
        .bind(book_id)
        .fetch_all(pool)
        .await?;

    let total = rows.len();
    let mut items = 0usize;

    for (index, row) in rows.iter().enumerate() {
        let chapter_id: String = row.try_get("id")?;
        match extract_from_chapter(book_id, &chapter_id, config).await {
            Ok(count) => items += count,
            Err(e) => log::warn!("[Canon] 回填章节 {} 失败: {}", chapter_id, e),
        }

        let _ = app.emit(
            "canon-rebuild-progress",
            serde_json::json!({
                "bookId": book_id,
                "done": index + 1,
                "total": total,
            }),
        );
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::clean_json;

    #[test]
    fn strips_code_fence() {
        assert_eq!(clean_json("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(clean_json("text before {\"a\":1} after"), "{\"a\":1}");
    }
}
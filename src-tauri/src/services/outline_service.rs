use crate::db::get_pool;
use crate::models::Outline;
use serde::{Deserialize, Serialize};

/// 一次大纲变更（支持书籍/卷/章节三级 + 粗纲/细纲）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutlineChange {
    /// book | volume | chapter
    pub level: String,
    #[serde(alias = "volumeId", default)]
    pub volume_id: Option<String>,
    #[serde(alias = "chapterId", default)]
    pub chapter_id: Option<String>,
    #[serde(alias = "outlineType")]
    pub outline_type: String,
    pub content: String,
}

impl OutlineChange {
    /// 校验并归一化出 (volume_id, chapter_id)
    fn resolve_target(&self) -> anyhow::Result<(Option<String>, Option<String>)> {
        match self.level.as_str() {
            "book" => Ok((None, None)),
            "volume" => {
                let volume_id = self
                    .volume_id
                    .clone()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("卷级大纲缺少 volumeId"))?;
                Ok((Some(volume_id), None))
            }
            "chapter" => {
                let chapter_id = self
                    .chapter_id
                    .clone()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("章节大纲缺少 chapterId"))?;
                Ok((None, Some(chapter_id)))
            }
            other => Err(anyhow::anyhow!("未知大纲层级: {}", other)),
        }
    }

    pub fn label(&self) -> String {
        let level = match self.level.as_str() {
            "book" => "书籍",
            "volume" => "卷",
            "chapter" => "章节",
            other => other,
        };
        let outline = if self.outline_type == "fine" { "细纲" } else { "粗纲" };
        format!("{}·{}", level, outline)
    }
}

/// 批量保存大纲（AI 一次提交多层级修改时使用，顺序写入，单条失败不影响其它）
pub async fn save_outlines_batch(
    book_id: &str,
    changes: &[OutlineChange],
) -> anyhow::Result<(usize, Vec<String>)> {
    let mut saved = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for change in changes {
        let (volume_id, chapter_id) = match change.resolve_target() {
            Ok(target) => target,
            Err(e) => {
                failures.push(format!("{}：{}", change.label(), e));
                continue;
            }
        };

        if change.outline_type != "coarse" && change.outline_type != "fine" {
            failures.push(format!("{}：大纲类型必须是 coarse/fine", change.label()));
            continue;
        }

        match save_outline(
            book_id.to_string(),
            volume_id,
            chapter_id,
            change.outline_type.clone(),
            change.content.clone(),
        )
        .await
        {
            Ok(_) => saved += 1,
            Err(e) => failures.push(format!("{}：{}", change.label(), e)),
        }
    }

    Ok((saved, failures))
}

/// 创建或更新大纲
pub async fn save_outline(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
    outline_type: String,
    content: String,
) -> anyhow::Result<Outline> {
    let pool = get_pool().await?;
    
    let id = format!(
        "outline_{}_{}_{}",
        outline_type,
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>()
    );
    let now = chrono::Utc::now().timestamp();
    
    // 检查是否已存在
    let existing: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT id FROM outlines
        WHERE book_id = ?1 AND volume_id IS ?2 AND chapter_id IS ?3 AND outline_type = ?4
        "#
    )
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&chapter_id)
    .bind(&outline_type)
    .fetch_optional(pool)
    .await?;
    
    if let Some((existing_id,)) = existing {
        // 更新现有大纲
        sqlx::query(
            r#"
            UPDATE outlines
            SET content = ?1, updated_at = ?2
            WHERE id = ?3
            "#
        )
        .bind(&content)
        .bind(now)
        .bind(&existing_id)
        .execute(pool)
        .await?;
        
        get_outline(existing_id).await
    } else {
        // 创建新大纲
        sqlx::query(
            r#"
            INSERT INTO outlines 
            (id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#
        )
        .bind(&id)
        .bind(&book_id)
        .bind(&volume_id)
        .bind(&chapter_id)
        .bind(&outline_type)
        .bind(&content)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;
        
        Ok(Outline {
            id,
            book_id,
            volume_id,
            chapter_id,
            outline_type,
            content,
            created_at: now,
            updated_at: now,
        })
    }
}

/// 获取单个大纲
pub async fn get_outline(outline_id: String) -> anyhow::Result<Outline> {
    let pool = get_pool().await?;
    
    let outline: Outline = sqlx::query_as(
        r#"
        SELECT id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at
        FROM outlines
        WHERE id = ?1
        "#
    )
    .bind(&outline_id)
    .fetch_one(pool)
    .await?;
    
    Ok(outline)
}

/// 获取特定层级的大纲
pub async fn get_outline_by_level(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
    outline_type: String,
) -> anyhow::Result<Option<Outline>> {
    let pool = get_pool().await?;
    
    let outline: Option<Outline> = sqlx::query_as(
        r#"
        SELECT id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at
        FROM outlines
        WHERE book_id = ?1 AND volume_id IS ?2 AND chapter_id IS ?3 AND outline_type = ?4
        "#
    )
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&chapter_id)
    .bind(&outline_type)
    .fetch_optional(pool)
    .await?;
    
    Ok(outline)
}

/// 获取书籍的所有大纲
pub async fn list_book_outlines(book_id: String) -> anyhow::Result<Vec<Outline>> {
    let pool = get_pool().await?;
    
    let outlines: Vec<Outline> = sqlx::query_as(
        r#"
        SELECT id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at
        FROM outlines
        WHERE book_id = ?1 AND volume_id IS NULL AND chapter_id IS NULL
        ORDER BY outline_type, updated_at DESC
        "#
    )
    .bind(&book_id)
    .fetch_all(pool)
    .await?;
    
    Ok(outlines)
}

/// 获取卷的所有大纲
pub async fn list_volume_outlines(
    book_id: String,
    volume_id: String,
) -> anyhow::Result<Vec<Outline>> {
    let pool = get_pool().await?;
    
    let outlines: Vec<Outline> = sqlx::query_as(
        r#"
        SELECT id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at
        FROM outlines
        WHERE book_id = ?1 AND volume_id = ?2 AND chapter_id IS NULL
        ORDER BY outline_type, updated_at DESC
        "#
    )
    .bind(&book_id)
    .bind(&volume_id)
    .fetch_all(pool)
    .await?;
    
    Ok(outlines)
}

/// 获取章节的所有大纲
pub async fn list_chapter_outlines(
    book_id: String,
    chapter_id: String,
) -> anyhow::Result<Vec<Outline>> {
    let pool = get_pool().await?;
    
    let outlines: Vec<Outline> = sqlx::query_as(
        r#"
        SELECT id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at
        FROM outlines
        WHERE book_id = ?1 AND chapter_id = ?2
        ORDER BY outline_type, updated_at DESC
        "#
    )
    .bind(&book_id)
    .bind(&chapter_id)
    .fetch_all(pool)
    .await?;
    
    Ok(outlines)
}

/// 删除大纲
pub async fn delete_outline(outline_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM outlines WHERE id = ?1")
        .bind(&outline_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

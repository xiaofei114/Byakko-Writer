use crate::db::get_pool;
use crate::models::{Outline, OutlineStats};

/// 创建或更新大纲
pub async fn save_outline(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
    outline_type: String,
    content: String,
) -> anyhow::Result<Outline> {
    let pool = get_pool().await?;
    
    let id = format!("outline_{}_{}", 
        outline_type,
        chrono::Utc::now().timestamp_millis()
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

/// 获取大纲统计信息
pub async fn get_outline_stats(
    book_id: String,
    volume_id: Option<String>,
    chapter_id: Option<String>,
) -> anyhow::Result<OutlineStats> {
    let pool = get_pool().await?;
    
    // 查询粗纲
    let coarse: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT updated_at FROM outlines
        WHERE book_id = ?1 AND volume_id IS ?2 AND chapter_id IS ?3 AND outline_type = 'coarse'
        "#
    )
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&chapter_id)
    .fetch_optional(pool)
    .await?;
    
    // 查询细纲
    let fine: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT updated_at FROM outlines
        WHERE book_id = ?1 AND volume_id IS ?2 AND chapter_id IS ?3 AND outline_type = 'fine'
        "#
    )
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&chapter_id)
    .fetch_optional(pool)
    .await?;
    
    Ok(OutlineStats {
        book_id,
        volume_id,
        chapter_id,
        has_coarse_outline: coarse.is_some(),
        has_fine_outline: fine.is_some(),
        coarse_outline_updated_at: coarse.map(|(t,)| t),
        fine_outline_updated_at: fine.map(|(t,)| t),
    })
}

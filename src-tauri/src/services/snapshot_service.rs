use std::path::PathBuf;
use std::str::FromStr;
use sqlx::sqlite::{SqlitePoolOptions, SqliteConnectOptions};
use sqlx::{Pool, Sqlite};
use chrono::Utc;
// use log;
use crate::models::{ChapterSnapshot, SnapshotDiff};

/// 获取数据库路径（按书籍分库）
fn get_db_path(book_id: &str) -> anyhow::Result<PathBuf> {
    let app_data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?;
    
    let app_dir = app_data_dir.join("byakko-writer");
    let books_dir = app_dir.join("books");
    let book_dir = books_dir.join(book_id);
    
    std::fs::create_dir_all(&book_dir)?;
    
    let db_path = book_dir.join("chapter_snapshots.db");
    // log::info!("[Snapshot] 数据库文件路径: {}", db_path.display());
    
    Ok(db_path)
}

/// 初始化数据库连接池
async fn init_db(book_id: &str) -> anyhow::Result<Pool<Sqlite>> {
    let db_path = get_db_path(book_id)?;
    let path_str = db_path.to_str().unwrap();
    let normalized = path_str.replace('\\', "/");
    
    let db_url = if normalized.len() >= 2 && normalized.chars().nth(1) == Some(':') {
        format!("sqlite://{}", normalized)
    } else {
        format!("sqlite:///{}", normalized)
    };
    
    // log::info!("[Snapshot] 数据库路径: {}", db_url);

    let connect_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .map_err(|e| anyhow::anyhow!("连接数据库失败 ({}): {}", db_url, e))?;
    
    // 创建表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chapter_snapshots (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            chapter_id TEXT NOT NULL,
            chapter_title TEXT NOT NULL,
            name TEXT NOT NULL,
            content TEXT NOT NULL,
            word_count INTEGER NOT NULL,
            created_at TEXT NOT NULL
        )
        "#
    )
    .execute(&pool)
    .await?;
    
    // 创建索引
    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_chapter_snapshots_chapter_id 
        ON chapter_snapshots(chapter_id)
        "#
    )
    .execute(&pool)
    .await?;
    
    Ok(pool)
}

/// 创建章节快照
pub async fn create_chapter_snapshot(
    book_id: String,
    chapter_id: String,
    chapter_title: String,
    name: String,
    content: String,
) -> anyhow::Result<ChapterSnapshot> {
    let pool = init_db(&book_id).await?;
    
    let snapshot_id = format!("snap_{}", Utc::now().timestamp_millis());
    let created_at = Utc::now().to_rfc3339();
    let word_count = content.len() as i64;
    
    sqlx::query(
        r#"
        INSERT INTO chapter_snapshots 
        (id, book_id, chapter_id, chapter_title, name, content, word_count, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#
    )
    .bind(&snapshot_id)
    .bind(&book_id)
    .bind(&chapter_id)
    .bind(&chapter_title)
    .bind(&name)
    .bind(&content)
    .bind(word_count)
    .bind(&created_at)
    .execute(&pool)
    .await?;
    
    pool.close().await;
    
    Ok(ChapterSnapshot {
        id: snapshot_id,
        book_id,
        chapter_id,
        chapter_title,
        name,
        content,
        word_count,
        created_at,
    })
}

/// 获取章节的快照列表
pub async fn list_chapter_snapshots(
    book_id: String,
    chapter_id: String,
) -> anyhow::Result<Vec<ChapterSnapshot>> {
    let pool = init_db(&book_id).await?;
    
    let snapshots: Vec<ChapterSnapshot> = sqlx::query_as(
        r#"
        SELECT id, book_id, chapter_id, chapter_title, name, content, word_count, created_at
        FROM chapter_snapshots
        WHERE book_id = ?1 AND chapter_id = ?2
        ORDER BY created_at DESC
        "#
    )
    .bind(&book_id)
    .bind(&chapter_id)
    .fetch_all(&pool)
    .await?;
    
    pool.close().await;
    
    Ok(snapshots)
}

/// 获取单个快照详情
pub async fn get_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> anyhow::Result<ChapterSnapshot> {
    let pool = init_db(&book_id).await?;
    
    let snapshot: ChapterSnapshot = sqlx::query_as(
        r#"
        SELECT id, book_id, chapter_id, chapter_title, name, content, word_count, created_at
        FROM chapter_snapshots
        WHERE id = ?1
        "#
    )
    .bind(&snapshot_id)
    .fetch_one(&pool)
    .await?;
    
    pool.close().await;
    
    Ok(snapshot)
}

/// 删除快照
pub async fn delete_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> anyhow::Result<()> {
    let pool = init_db(&book_id).await?;
    
    sqlx::query("DELETE FROM chapter_snapshots WHERE id = ?1")
        .bind(&snapshot_id)
        .execute(&pool)
        .await?;
    
    pool.close().await;
    
    Ok(())
}

/// 清理章节快照（保留最近 N 个）
pub async fn cleanup_chapter_snapshots(
    book_id: String,
    chapter_id: String,
    keep_count: i32,
) -> anyhow::Result<i32> {
    let pool = init_db(&book_id).await?;
    
    // 获取要删除的快照ID
    let ids_to_delete: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT id FROM chapter_snapshots
        WHERE book_id = ?1 AND chapter_id = ?2
        ORDER BY created_at DESC
        LIMIT -1 OFFSET ?3
        "#
    )
    .bind(&book_id)
    .bind(&chapter_id)
    .bind(keep_count)
    .fetch_all(&pool)
    .await?;
    
    let deleted_count = ids_to_delete.len() as i32;
    
    for (id,) in ids_to_delete {
        sqlx::query("DELETE FROM chapter_snapshots WHERE id = ?1")
            .bind(&id)
            .execute(&pool)
            .await?;
    }
    
    pool.close().await;
    
    Ok(deleted_count)
}

/// 对比两个快照
pub async fn compare_snapshots(
    book_id: String,
    snapshot_id1: String,
    snapshot_id2: String,
) -> anyhow::Result<SnapshotDiff> {
    let snapshot1 = get_chapter_snapshot(book_id.clone(), snapshot_id1).await?;
    let snapshot2 = get_chapter_snapshot(book_id, snapshot_id2).await?;
    
    // 简单的行级对比
    let lines1: Vec<&str> = snapshot1.content.lines().collect();
    let lines2: Vec<&str> = snapshot2.content.lines().collect();
    
    let mut added = vec![];
    let mut removed = vec![];
    
    // 使用简单的 LCS 算法找出差异
    let mut i = 0;
    let mut j = 0;
    
    while i < lines1.len() || j < lines2.len() {
        if i < lines1.len() && j < lines2.len() && lines1[i] == lines2[j] {
            i += 1;
            j += 1;
        } else if j < lines2.len() {
            added.push(crate::models::DiffChunk {
                text: lines2[j].to_string(),
                start: j,
                end: j + 1,
            });
            j += 1;
        } else if i < lines1.len() {
            removed.push(crate::models::DiffChunk {
                text: lines1[i].to_string(),
                start: i,
                end: i + 1,
            });
            i += 1;
        }
    }
    
    Ok(SnapshotDiff { added, removed })
}

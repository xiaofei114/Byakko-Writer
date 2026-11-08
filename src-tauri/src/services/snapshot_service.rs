use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use sqlx::sqlite::{SqlitePoolOptions, SqliteConnectOptions};
use sqlx::{Pool, Sqlite};
use chrono::Utc;
use crate::models::snapshot::{ChapterSnapshot, DiffBlock, DiffBlockType, SnapshotComparison};

/// 每本书一个独立的快照数据库，池子按书籍缓存复用
static BOOK_POOLS: OnceLock<Mutex<HashMap<String, Pool<Sqlite>>>> = OnceLock::new();

fn pool_cache() -> &'static Mutex<HashMap<String, Pool<Sqlite>>> {
    BOOK_POOLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 书籍数据目录（不创建目录）
fn book_dir_path(book_id: &str) -> anyhow::Result<PathBuf> {
    let app_data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?;

    // 防止 book_id 中出现路径穿越字符
    if book_id.is_empty()
        || book_id.contains("..")
        || book_id.contains('/')
        || book_id.contains('\\')
    {
        return Err(anyhow::anyhow!("非法的书籍 ID: {}", book_id));
    }

    Ok(app_data_dir
        .join("byakko-writer")
        .join("books")
        .join(book_id))
}

/// 获取书籍数据目录（存在则创建）
fn get_book_dir(book_id: &str) -> anyhow::Result<PathBuf> {
    let book_dir = book_dir_path(book_id)?;
    std::fs::create_dir_all(&book_dir)?;
    Ok(book_dir)
}

/// 拼接 SQLite 连接串
/// Windows 盘符路径（C:/...）用 sqlite://C:/...，其它路径用 sqlite:///...
fn build_db_url(db_path: &PathBuf) -> anyhow::Result<String> {
    let path_str = db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("数据库路径包含非法字符"))?;
    let normalized = path_str.replace('\\', "/");

    if normalized.len() >= 2 && normalized.chars().nth(1) == Some(':') {
        Ok(format!("sqlite://{}", normalized))
    } else {
        Ok(format!("sqlite:///{}", normalized))
    }
}

/// 打开（或复用）某本书的快照数据库连接池
async fn get_pool(book_id: &str) -> anyhow::Result<Pool<Sqlite>> {
    if let Ok(guard) = pool_cache().lock() {
        if let Some(pool) = guard.get(book_id) {
            return Ok(pool.clone());
        }
    }

    let db_path = get_book_dir(book_id)?.join("chapter_snapshots.db");
    let db_url = build_db_url(&db_path)?;

    let connect_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(connect_options)
        .await
        .map_err(|e| anyhow::anyhow!("连接快照数据库失败 ({}): {}", db_url, e))?;

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

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_chapter_snapshots_chapter_id 
        ON chapter_snapshots(chapter_id)
        "#
    )
    .execute(&pool)
    .await?;

    if let Ok(mut guard) = pool_cache().lock() {
        guard.insert(book_id.to_string(), pool.clone());
    }

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
    let pool = get_pool(&book_id).await?;

    // 毫秒时间戳不足以区分同一毫秒内的两次快照，追加随机后缀
    let snapshot_id = format!(
        "snap_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    );
    let created_at = Utc::now().to_rfc3339();
    let word_count = content.chars().count() as i64;

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
    let pool = get_pool(&book_id).await?;

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

    Ok(snapshots)
}

/// 获取单个快照详情
pub async fn get_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> anyhow::Result<ChapterSnapshot> {
    let pool = get_pool(&book_id).await?;

    let snapshot: ChapterSnapshot = sqlx::query_as(
        r#"
        SELECT id, book_id, chapter_id, chapter_title, name, content, word_count, created_at
        FROM chapter_snapshots
        WHERE book_id = ?1 AND id = ?2
        "#
    )
    .bind(&book_id)
    .bind(&snapshot_id)
    .fetch_one(&pool)
    .await?;

    Ok(snapshot)
}

/// 删除快照
pub async fn delete_chapter_snapshot(
    book_id: String,
    snapshot_id: String,
) -> anyhow::Result<()> {
    let pool = get_pool(&book_id).await?;

    sqlx::query("DELETE FROM chapter_snapshots WHERE book_id = ?1 AND id = ?2")
        .bind(&book_id)
        .bind(&snapshot_id)
        .execute(&pool)
        .await?;

    Ok(())
}

/// 清理章节快照（保留最近 N 个）
pub async fn cleanup_chapter_snapshots(
    book_id: String,
    chapter_id: String,
    keep_count: i32,
) -> anyhow::Result<i32> {
    let pool = get_pool(&book_id).await?;
    let keep = keep_count.max(1) as i64;

    let result = sqlx::query(
        r#"
        DELETE FROM chapter_snapshots
        WHERE book_id = ?1 AND chapter_id = ?2 AND id NOT IN (
            SELECT id FROM chapter_snapshots
            WHERE book_id = ?1 AND chapter_id = ?2
            ORDER BY created_at DESC
            LIMIT ?3
        )
        "#
    )
    .bind(&book_id)
    .bind(&chapter_id)
    .bind(keep)
    .execute(&pool)
    .await?;

    Ok(result.rows_affected() as i32)
}

/// 删除若干章节的全部快照（章节/卷被删除时调用）
pub async fn delete_snapshots_for_chapters(
    book_id: &str,
    chapter_ids: &[String],
) -> anyhow::Result<()> {
    if chapter_ids.is_empty() {
        return Ok(());
    }

    // 从未创建过快照库时无需清理，也不要顺手把库建出来
    let db_path = book_dir_path(book_id)?.join("chapter_snapshots.db");
    if !db_path.exists() {
        return Ok(());
    }

    let pool = get_pool(book_id).await?;

    for chapter_id in chapter_ids {
        sqlx::query("DELETE FROM chapter_snapshots WHERE book_id = ?1 AND chapter_id = ?2")
            .bind(book_id)
            .bind(chapter_id)
            .execute(&pool)
            .await?;
    }

    Ok(())
}

/// 删除某本书的全部快照数据（同时清理连接池与磁盘目录）
pub async fn delete_book_snapshots(book_id: &str) -> anyhow::Result<()> {
    let cached = pool_cache()
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(book_id));

    if let Some(pool) = cached {
        pool.close().await;
    }

    let app_data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?;
    let book_dir = app_data_dir
        .join("byakko-writer")
        .join("books")
        .join(book_id);

    if book_dir.exists() {
        std::fs::remove_dir_all(&book_dir)
            .map_err(|e| anyhow::anyhow!("删除快照目录失败 {}: {}", book_dir.display(), e))?;
    }

    Ok(())
}

/// 对比两个快照，返回行级差异块（equal / insert / delete）
pub async fn compare_snapshots(
    book_id: String,
    old_snapshot_id: String,
    new_snapshot_id: String,
) -> anyhow::Result<SnapshotComparison> {
    let old_snapshot = get_chapter_snapshot(book_id.clone(), old_snapshot_id).await?;
    let new_snapshot = get_chapter_snapshot(book_id, new_snapshot_id).await?;

    let diff_blocks = diff_lines(&old_snapshot.content, &new_snapshot.content);

    Ok(SnapshotComparison {
        old_snapshot,
        new_snapshot,
        diff_blocks,
    })
}

/// 行级差异算法：
/// 1. 先去掉公共前后缀，缩小 LCS 规模
/// 2. 中段做 LCS（规模超限时退化为「整段删除 + 整段新增」）
/// 3. 把连续同类操作合并成块
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffBlock> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < old_lines.len().saturating_sub(prefix)
        && suffix < new_lines.len().saturating_sub(prefix)
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mut blocks: Vec<DiffBlock> = Vec::new();

    if prefix > 0 {
        push_block(&mut blocks, DiffBlockType::Equal, &old_lines[..prefix], &new_lines[..prefix]);
    }

    let old_mid = &old_lines[prefix..old_lines.len() - suffix];
    let new_mid = &new_lines[prefix..new_lines.len() - suffix];

    let max_cells: usize = 4_000_000;
    if !old_mid.is_empty() || !new_mid.is_empty() {
        if old_mid.len().saturating_mul(new_mid.len()) > max_cells {
            // 规模过大，退化为整段替换，避免内存爆炸
            push_block(&mut blocks, DiffBlockType::Delete, old_mid, &[]);
            push_block(&mut blocks, DiffBlockType::Insert, &[], new_mid);
        } else {
            let ops = lcs_ops(old_mid, new_mid);
            for (op, old_idx, new_idx) in ops {
                match op {
                    DiffBlockType::Equal => {
                        push_block(&mut blocks, op, &[old_mid[old_idx]], &[new_mid[new_idx]]);
                    }
                    DiffBlockType::Delete => {
                        push_block(&mut blocks, op, &[old_mid[old_idx]], &[]);
                    }
                    DiffBlockType::Insert => {
                        push_block(&mut blocks, op, &[], &[new_mid[new_idx]]);
                    }
                }
            }
        }
    }

    if suffix > 0 {
        push_block(
            &mut blocks,
            DiffBlockType::Equal,
            &old_lines[old_lines.len() - suffix..],
            &new_lines[new_lines.len() - suffix..],
        );
    }

    blocks
}

/// 追加一个差异块，和上一个块类型相同则合并
fn push_block(
    blocks: &mut Vec<DiffBlock>,
    block_type: DiffBlockType,
    old_part: &[&str],
    new_part: &[&str],
) {
    let old_text = old_part.join("\n");
    let new_text = new_part.join("\n");

    if let Some(last) = blocks.last_mut() {
        if last.block_type == block_type.as_str() {
            if !old_text.is_empty() {
                if !last.old_text.is_empty() {
                    last.old_text.push('\n');
                }
                last.old_text.push_str(&old_text);
            }
            if !new_text.is_empty() {
                if !last.new_text.is_empty() {
                    last.new_text.push('\n');
                }
                last.new_text.push_str(&new_text);
            }
            return;
        }
    }

    blocks.push(DiffBlock {
        block_type: block_type.as_str().to_string(),
        old_text,
        new_text,
    });
}

/// 基于 LCS 的逐行编辑操作序列，返回 (类型, old 下标, new 下标)
fn lcs_ops(old: &[&str], new: &[&str]) -> Vec<(DiffBlockType, usize, usize)> {
    let n = old.len();
    let m = new.len();

    // dp[i][j] = old[i..] 与 new[j..] 的 LCS 长度
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let stride = m + 1;

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * stride + j] = if old[i] == new[j] {
                dp[(i + 1) * stride + (j + 1)] + 1
            } else {
                dp[(i + 1) * stride + j].max(dp[i * stride + (j + 1)])
            };
        }
    }

    let mut ops = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);

    while i < n && j < m {
        if old[i] == new[j] {
            ops.push((DiffBlockType::Equal, i, j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * stride + j] >= dp[i * stride + (j + 1)] {
            ops.push((DiffBlockType::Delete, i, 0));
            i += 1;
        } else {
            ops.push((DiffBlockType::Insert, 0, j));
            j += 1;
        }
    }

    while i < n {
        ops.push((DiffBlockType::Delete, i, 0));
        i += 1;
    }
    while j < m {
        ops.push((DiffBlockType::Insert, 0, j));
        j += 1;
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(blocks: &[DiffBlock]) -> Vec<(String, String, String)> {
        blocks
            .iter()
            .map(|b| (b.block_type.clone(), b.old_text.clone(), b.new_text.clone()))
            .collect()
    }

    #[test]
    fn identical_text_is_one_equal_block() {
        let blocks = diff_lines("a\nb\nc", "a\nb\nc");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "equal");
        assert_eq!(blocks[0].new_text, "a\nb\nc");
    }

    #[test]
    fn insertion_is_detected() {
        let blocks = diff_lines("a\nc", "a\nb\nc");
        assert!(blocks.iter().any(|b| b.block_type == "insert" && b.new_text == "b"));
        assert!(blocks.iter().all(|b| b.block_type != "delete"));
    }

    #[test]
    fn deletion_is_detected() {
        let blocks = diff_lines("a\nb\nc", "a\nc");
        assert!(blocks.iter().any(|b| b.block_type == "delete" && b.old_text == "b"));
        assert!(blocks.iter().all(|b| b.block_type != "insert"));
    }

    #[test]
    fn equal_lines_are_preserved_between_changes() {
        let blocks = diff_lines("a\nb\nc\nd", "a\nX\nc\nd");
        let joined = summary(&blocks);
        assert_eq!(joined.first().map(|b| b.0.as_str()), Some("equal"));
        assert!(joined.iter().any(|b| b.0 == "delete" && b.1 == "b"));
        assert!(joined.iter().any(|b| b.0 == "insert" && b.2 == "X"));
    }

    #[test]
    fn handles_empty_inputs() {
        assert!(diff_lines("", "").is_empty());
        let added = diff_lines("", "x\ny");
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].block_type, "insert");
        let removed = diff_lines("x\ny", "");
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].block_type, "delete");
    }
}

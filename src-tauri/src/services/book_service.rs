use crate::db::get_pool;
use crate::models::{Book, BookListItem, Chapter, Volume};

/// 创建书籍
///
/// 创建时一并建好「第一卷」和「第一章」，保证新书可以直接开始写作。
/// 所有写入在同一个事务中完成，避免出现只有书没有卷的半成品。
pub async fn create_book(
    title: String,
    author: String,
    description: String,
) -> anyhow::Result<Book> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let book_id = format!("book_{}", now);
    let volume_id = format!("volume_{}_first", now);
    let chapter_id = format!("chapter_{}_first", now);

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO books (id, title, author, description, current_chapter_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
        "#
    )
    .bind(&book_id)
    .bind(&title)
    .bind(&author)
    .bind(&description)
    .bind(&chapter_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO volumes (id, book_id, title, "order", is_collapsed, created_at, updated_at)
        VALUES (?1, ?2, ?3, 0, 0, ?4, ?4)
        "#
    )
    .bind(&volume_id)
    .bind(&book_id)
    .bind("第一卷")
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO chapters (id, book_id, volume_id, title, "order", content, word_count, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 0, '', 0, ?5, ?5)
        "#
    )
    .bind(&chapter_id)
    .bind(&book_id)
    .bind(&volume_id)
    .bind("第一章")
    .bind(now)
    .execute(&mut *tx)
    .await?;

    // 与前端 createChapter 保持一致：为新章节预置粗纲/细纲空记录
    for outline_type in ["coarse", "fine"] {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO outlines (id, book_id, volume_id, chapter_id, outline_type, content, created_at, updated_at)
            VALUES (?1, ?2, NULL, ?3, ?4, '', ?5, ?5)
            "#
        )
        .bind(format!("outline_{}_{}", outline_type, now))
        .bind(&book_id)
        .bind(&chapter_id)
        .bind(outline_type)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let volume = Volume {
        id: volume_id,
        title: "第一卷".to_string(),
        order: 0,
        is_collapsed: false,
        created_at: now,
        updated_at: now,
    };
    let chapter = Chapter {
        id: chapter_id.clone(),
        title: "第一章".to_string(),
        order: 0,
        volume_id: volume.id.clone(),
        content: String::new(),
        word_count: 0,
        created_at: now,
        updated_at: now,
    };

    Ok(Book {
        id: book_id,
        title,
        author,
        description,
        volumes: vec![volume],
        chapters: vec![chapter],
        current_chapter_id: Some(chapter_id),
        created_at: now,
        updated_at: now,
    })
}

/// 获取书籍列表
pub async fn get_books_list() -> anyhow::Result<Vec<BookListItem>> {
    let pool = get_pool().await?;
    
    let books = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT id, title, author, description, updated_at FROM books ORDER BY updated_at DESC"
    )
    .fetch_all(pool)
    .await?;
    
    Ok(books
        .into_iter()
        .map(|(id, title, author, description, updated_at)| BookListItem {
            id,
            title,
            author,
            description,
            updated_at,
        })
        .collect())
}

/// 加载书籍
pub async fn load_book(book_id: String) -> anyhow::Result<Book> {
    let pool = get_pool().await?;
    
    // 加载书籍基本信息
    let book_row = sqlx::query_as::<_, (String, String, String, String, Option<String>, i64, i64)>(
        "SELECT id, title, author, description, current_chapter_id, created_at, updated_at FROM books WHERE id = ?1"
    )
    .bind(&book_id)
    .fetch_optional(pool)
    .await?;
    
    let book_row = match book_row {
        Some(row) => row,
        None => return Err(anyhow::anyhow!("书籍 {} 不存在", book_id)),
    };
    
    // 加载卷
    let volumes: Vec<Volume> = sqlx::query_as::<_, Volume>(
        r#"
        SELECT id, title, "order", is_collapsed, created_at, updated_at 
        FROM volumes WHERE book_id = ?1 ORDER BY "order"
        "#
    )
    .bind(&book_id)
    .fetch_all(pool)
    .await?;
    
    // 加载章节
    let chapters: Vec<Chapter> = sqlx::query_as::<_, Chapter>(
        r#"
        SELECT id, title, "order", volume_id, content, word_count, created_at, updated_at 
        FROM chapters WHERE book_id = ?1 ORDER BY "order"
        "#
    )
    .bind(&book_id)
    .fetch_all(pool)
    .await?;
    
    Ok(Book {
        id: book_row.0,
        title: book_row.1,
        author: book_row.2,
        description: book_row.3,
        current_chapter_id: book_row.4,
        volumes,
        chapters,
        created_at: book_row.5,
        updated_at: book_row.6,
    })
}

/// 保存书籍
pub async fn save_book(book: Book) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query(
        r#"
        UPDATE books SET 
            title = ?1, 
            author = ?2, 
            description = ?3, 
            current_chapter_id = ?4, 
            updated_at = ?5 
        WHERE id = ?6
        "#
    )
    .bind(&book.title)
    .bind(&book.author)
    .bind(&book.description)
    .bind(&book.current_chapter_id)
    .bind(book.updated_at)
    .bind(&book.id)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 更新书籍信息
pub async fn update_book(book_id: String, title: String, author: String, description: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    
    sqlx::query(
        r#"
        UPDATE books SET title = ?1, author = ?2, description = ?3, updated_at = ?4 WHERE id = ?5
        "#
    )
    .bind(&title)
    .bind(&author)
    .bind(&description)
    .bind(now)
    .bind(&book_id)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 删除书籍
pub async fn delete_book(book_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    // 外键已启用（sqlx 默认 PRAGMA foreign_keys=ON），子表会级联删除
    sqlx::query("DELETE FROM books WHERE id = ?1")
        .bind(&book_id)
        .execute(pool)
        .await?;

    // 章节快照存放在独立数据库文件中，需要一并清理
    if let Err(e) = crate::services::snapshot_service::delete_book_snapshots(&book_id).await {
        log::warn!("[Book] 清理书籍快照目录失败 {}: {}", book_id, e);
    }

    Ok(())
}

/// 创建卷
pub async fn create_volume(book_id: String, title: String) -> anyhow::Result<Volume> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let volume_id = format!("volume_{}", now);
    
    // 获取当前最大 order
    let max_order: Option<i32> = sqlx::query_scalar(
        r#"SELECT MAX("order") FROM volumes WHERE book_id = ?1"#
    )
    .bind(&book_id)
    .fetch_one(pool)
    .await?;
    
    let order = max_order.unwrap_or(-1) + 1;
    
    sqlx::query(
        r#"
        INSERT INTO volumes (id, book_id, title, "order", is_collapsed, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)
        "#
    )
    .bind(&volume_id)
    .bind(&book_id)
    .bind(&title)
    .bind(order)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(Volume {
        id: volume_id,
        title,
        order,
        is_collapsed: false,
        created_at: now,
        updated_at: now,
    })
}

/// 删除卷（该卷下的章节会被级联删除，快照一并清理）
pub async fn delete_volume(book_id: String, volume_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;

    let chapter_ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM chapters WHERE volume_id = ?1"
    )
    .bind(&volume_id)
    .fetch_all(pool)
    .await?;

    sqlx::query("DELETE FROM volumes WHERE id = ?1 AND book_id = ?2")
        .bind(&volume_id)
        .bind(&book_id)
        .execute(pool)
        .await?;

    if let Err(e) = crate::services::snapshot_service::delete_snapshots_for_chapters(&book_id, &chapter_ids).await {
        log::warn!("[Book] 清理卷内章节快照失败 {}: {}", volume_id, e);
    }

    Ok(())
}

/// 更新卷标题
pub async fn update_volume_title(volume_id: String, title: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    
    sqlx::query(
        r#"
        UPDATE volumes SET title = ?1, updated_at = ?2 WHERE id = ?3
        "#
    )
    .bind(&title)
    .bind(now)
    .bind(&volume_id)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 创建章节
pub async fn create_chapter(
    book_id: String,
    title: String,
    volume_id: String,
) -> anyhow::Result<Chapter> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let chapter_id = format!("chapter_{}", now);

    // 获取当前最大 order
    let max_order: Option<i32> = sqlx::query_scalar(
        r#"SELECT MAX("order") FROM chapters WHERE volume_id = ?1"#
    )
    .bind(&volume_id)
    .fetch_one(pool)
    .await?;

    let order = max_order.unwrap_or(-1) + 1;

    sqlx::query(
        r#"
        INSERT INTO chapters (id, book_id, volume_id, title, "order", content, word_count, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, '', 0, ?6, ?6)
        "#
    )
    .bind(&chapter_id)
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&title)
    .bind(order)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Chapter {
        id: chapter_id,
        title,
        order,
        volume_id,
        content: String::new(),
        word_count: 0,
        created_at: now,
        updated_at: now,
    })
}

/// 创建章节（指定order）- 用于AI工具调用
pub async fn create_chapter_with_order(
    book_id: String,
    title: String,
    volume_id: String,
    order: i32,
) -> anyhow::Result<Chapter> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let chapter_id = format!("chapter_{}", now);

    sqlx::query(
        r#"
        INSERT INTO chapters (id, book_id, volume_id, title, "order", content, word_count, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, '', 0, ?6, ?6)
        "#
    )
    .bind(&chapter_id)
    .bind(&book_id)
    .bind(&volume_id)
    .bind(&title)
    .bind(order)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(Chapter {
        id: chapter_id,
        title,
        order,
        volume_id,
        content: String::new(),
        word_count: 0,
        created_at: now,
        updated_at: now,
    })
}

/// 删除章节（级联删除摘要/大纲/消息引用，并清理该章快照）
pub async fn delete_chapter(book_id: String, chapter_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;

    sqlx::query("DELETE FROM chapters WHERE id = ?1 AND book_id = ?2")
        .bind(&chapter_id)
        .bind(&book_id)
        .execute(pool)
        .await?;

    let ids = vec![chapter_id.clone()];
    if let Err(e) = crate::services::snapshot_service::delete_snapshots_for_chapters(&book_id, &ids).await {
        log::warn!("[Book] 清理章节快照失败 {}: {}", chapter_id, e);
    }

    Ok(())
}

/// 加载章节内容
pub async fn load_chapter_content(chapter_id: String) -> anyhow::Result<String> {
    let pool = get_pool().await?;
    
    let content: String = sqlx::query_scalar("SELECT content FROM chapters WHERE id = ?1")
        .bind(&chapter_id)
        .fetch_one(pool)
        .await?;
    
    Ok(content)
}

/// 保存章节内容
pub async fn save_chapter_content(
    chapter_id: String,
    content: String,
) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let word_count = content.chars().count() as i64;
    
    sqlx::query(
        r#"
        UPDATE chapters 
        SET content = ?1, word_count = ?2, updated_at = ?3 
        WHERE id = ?4
        "#
    )
    .bind(&content)
    .bind(word_count)
    .bind(now)
    .bind(&chapter_id)
    .execute(pool)
    .await?;

    // 保存后调度自动摘要（防抖 20 秒，内容过短/已有摘要会跳过）
    crate::services::summary_generator_service::schedule_auto_summary(chapter_id.clone());
    // 账本随改稿更新（防抖 30 秒；未启用账本的书会跳过）
    crate::services::canon_service::schedule_reextract(chapter_id.clone());
    // 本章改稿后，关联的未解决冲突标记为「待复核」
    crate::services::conflict_service::mark_chapter_conflicts_for_review(&chapter_id).await;

    Ok(())
}

/// 更新章节标题
pub async fn update_chapter_title(
    chapter_id: String,
    title: String,
) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    
    sqlx::query(
        "UPDATE chapters SET title = ?1, updated_at = ?2 WHERE id = ?3"
    )
    .bind(&title)
    .bind(now)
    .bind(&chapter_id)
    .execute(pool)
    .await?;
    
    Ok(())
}

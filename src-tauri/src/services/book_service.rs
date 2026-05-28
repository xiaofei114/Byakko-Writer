use crate::db::get_pool;
use crate::models::{Book, BookListItem, Chapter, Volume};

/// 创建书籍
pub async fn create_book(title: String) -> anyhow::Result<Book> {
    let pool = get_pool().await?;
    let now = chrono::Local::now().timestamp_millis();
    let book_id = format!("book_{}", now);
    
    sqlx::query(
        r#"
        INSERT INTO books (id, title, author, description, created_at, updated_at)
        VALUES (?1, ?2, '', '', ?3, ?3)
        "#
    )
    .bind(&book_id)
    .bind(&title)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(Book {
        id: book_id,
        title,
        author: String::new(),
        description: String::new(),
        volumes: vec![],
        chapters: vec![],
        current_chapter_id: None,
        created_at: now,
        updated_at: now,
    })
}

/// 获取书籍列表
pub async fn get_books_list() -> anyhow::Result<Vec<BookListItem>> {
    let pool = get_pool().await?;
    
    let books = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, title, updated_at FROM books ORDER BY updated_at DESC"
    )
    .fetch_all(pool)
    .await?;
    
    Ok(books
        .into_iter()
        .map(|(id, title, updated_at)| BookListItem {
            id,
            title,
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

/// 删除书籍
pub async fn delete_book(book_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM books WHERE id = ?1")
        .bind(&book_id)
        .execute(pool)
        .await?;
    
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

/// 删除卷
pub async fn delete_volume(volume_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM volumes WHERE id = ?1")
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

/// 删除章节
pub async fn delete_chapter(chapter_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM chapters WHERE id = ?1")
        .bind(&chapter_id)
        .execute(pool)
        .await?;
    
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

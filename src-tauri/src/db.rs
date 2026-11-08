use sqlx::{sqlite::{SqlitePoolOptions, SqliteConnectOptions}, Pool, Sqlite};
use std::path::PathBuf;
use std::str::FromStr;

/// 数据库连接池类型
pub type DbPool = Pool<Sqlite>;

/// 获取应用数据目录
fn get_app_data_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?;
    Ok(data_dir.join("byakko-writer"))
}

/// 获取数据库文件路径
pub fn get_db_path() -> anyhow::Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    Ok(app_dir.join("data.db"))
}

/// 初始化数据库连接池
pub async fn init_db() -> anyhow::Result<DbPool> {
    let db_path = get_db_path()?;
    
    // 确保目录存在
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("创建目录失败 {}: {}", parent.display(), e))?;
    }
    
    // 检查目录是否可写
    if let Some(parent) = db_path.parent() {
        let test_file = parent.join(".write_test");
        match std::fs::write(&test_file, "test") {
            Ok(_) => { let _ = std::fs::remove_file(&test_file); }
            Err(e) => return Err(anyhow::anyhow!("目录不可写 {}: {}", parent.display(), e)),
        }
    }
    
    // Windows 路径需要特殊处理
    let path_str = db_path.to_str().unwrap().replace('\\', "/");
    
    // 使用 SqliteConnectOptions 来创建数据库文件（如果不存在）
    let db_url = format!("sqlite:///{}", path_str);
    log::debug!("数据库 URL: {}", db_url);
    let connect_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        // sqlx 默认已开启，这里显式声明，保证 ON DELETE CASCADE 生效
        .foreign_keys(true);
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
        .map_err(|e| anyhow::anyhow!("连接数据库失败: {}", e))?;
    
    // 创建表
    create_tables(&pool).await?;
    
    Ok(pool)
}

/// 创建数据库表
async fn create_tables(pool: &DbPool) -> anyhow::Result<()> {
    // 书籍表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            author TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            current_chapter_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // 卷表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS volumes (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            title TEXT NOT NULL,
            "order" INTEGER NOT NULL,
            is_collapsed INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // 章节表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chapters (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            volume_id TEXT NOT NULL,
            title TEXT NOT NULL,
            "order" INTEGER NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            word_count INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (volume_id) REFERENCES volumes(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // 说明：章节快照保存在每本书独立的 books/<book_id>/chapter_snapshots.db 中，
    // 主库不再创建旧的整书快照表（snapshots / snapshot_chapters）。
    // 老版本数据库里如果存在这两张空表，保留不影响运行。

    // 章节摘要表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chapter_summaries (
            id TEXT PRIMARY KEY,
            chapter_id TEXT NOT NULL UNIQUE,
            short_summary TEXT NOT NULL DEFAULT '',
            long_summary TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '[]',
            characters TEXT NOT NULL DEFAULT '[]',
            locations TEXT NOT NULL DEFAULT '[]',
            events TEXT NOT NULL DEFAULT '[]',
            generated_at INTEGER NOT NULL,
            is_confirmed INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // AI 对话会话表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_sessions (
            session_id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            chapter_id TEXT,
            title TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL
        )
        "#
    )
    .execute(pool)
    .await?;

    // AI 对话消息表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            book_id TEXT NOT NULL,
            chapter_id TEXT,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            context_type TEXT,
            timestamp INTEGER NOT NULL,
            polish_handled INTEGER NOT NULL DEFAULT 0,
            handled_status TEXT,
            FOREIGN KEY (session_id) REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE SET NULL
        )
        "#
    )
    .execute(pool)
    .await?;

    // 迁移：为已存在的表添加 polish_handled 字段
    let _ = sqlx::query("ALTER TABLE chat_messages ADD COLUMN polish_handled INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // 迁移：为已存在的表添加 handled_status 字段
    let _ = sqlx::query("ALTER TABLE chat_messages ADD COLUMN handled_status TEXT")
        .execute(pool)
        .await;

    // 迁移：为 books 表添加 author 和 description 字段
    let _ = sqlx::query("ALTER TABLE books ADD COLUMN author TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE books ADD COLUMN description TEXT NOT NULL DEFAULT ''")
        .execute(pool)
        .await;
    
    // 创建索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_volumes_book_id ON volumes(book_id)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapters_book_id ON chapters(book_id)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapters_volume_id ON chapters(volume_id)")
        .execute(pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_snapshots_book_id ON snapshots(book_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chapter_summaries_chapter_id ON chapter_summaries(chapter_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chat_sessions_book_id ON chat_sessions(book_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chat_messages_session_id ON chat_messages(session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_chat_messages_book_id ON chat_messages(book_id)")
        .execute(pool)
        .await?;

    // 角色卡表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS character_cards (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            name TEXT NOT NULL,
            aliases TEXT NOT NULL DEFAULT '[]',
            gender TEXT NOT NULL DEFAULT '',
            age TEXT NOT NULL DEFAULT '',
            appearance TEXT NOT NULL DEFAULT '',
            personality TEXT NOT NULL DEFAULT '',
            background TEXT NOT NULL DEFAULT '',
            goals TEXT NOT NULL DEFAULT '',
            relationships TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]',
            notes TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // 大纲表（支持书籍/卷/章节三级大纲）
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS outlines (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            volume_id TEXT,
            chapter_id TEXT,
            outline_type TEXT NOT NULL DEFAULT 'coarse',
            content TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (volume_id) REFERENCES volumes(id) ON DELETE CASCADE,
            FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
            UNIQUE(book_id, volume_id, chapter_id, outline_type)
        )
        "#
    )
    .execute(pool)
    .await?;

    // 创建索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_character_cards_book_id ON character_cards(book_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_outlines_book_id ON outlines(book_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_outlines_chapter_id ON outlines(chapter_id)")
        .execute(pool)
        .await?;

    // 用户全局写作风格表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS user_writing_styles (
            id TEXT PRIMARY KEY,
            style_analysis TEXT NOT NULL DEFAULT '',
            style_prompt TEXT NOT NULL DEFAULT '',
            total_word_count INTEGER NOT NULL DEFAULT 0,
            chapter_count INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            is_enabled INTEGER NOT NULL DEFAULT 1
        )
        "#
    )
    .execute(pool)
    .await?;

    // 书籍级写作风格表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS book_writing_styles (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL UNIQUE,
            style_analysis TEXT NOT NULL DEFAULT '',
            style_prompt TEXT NOT NULL DEFAULT '',
            total_word_count INTEGER NOT NULL DEFAULT 0,
            chapter_count INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            inherit_global INTEGER NOT NULL DEFAULT 1,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // 创建索引
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_book_writing_styles_book_id ON book_writing_styles(book_id)")
        .execute(pool)
        .await?;

    // 设定冲突检测表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detected_conflicts (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            description TEXT NOT NULL,
            suggestion TEXT NOT NULL DEFAULT '',
            detected_at INTEGER NOT NULL,
            is_ignored INTEGER NOT NULL DEFAULT 0,
            ignored_at INTEGER,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_conflicts_book_id ON detected_conflicts(book_id)")
        .execute(pool)
        .await?;

    // 冲突与章节/修复状态关联（用于改稿后提示「可能已修复」）
    let _ = sqlx::query("ALTER TABLE detected_conflicts ADD COLUMN chapter_id TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE detected_conflicts ADD COLUMN needs_review INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE detected_conflicts ADD COLUMN resolved_at INTEGER")
        .execute(pool)
        .await;

    // 故事记忆分组摘要表（每 10 章一组的缓存）
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS story_memory_groups (
            book_id TEXT NOT NULL,
            group_index INTEGER NOT NULL,
            start_chapter INTEGER NOT NULL,
            end_chapter INTEGER NOT NULL,
            chapter_ids TEXT NOT NULL DEFAULT '[]',
            summary TEXT NOT NULL DEFAULT '',
            word_count INTEGER NOT NULL DEFAULT 0,
            generated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (book_id, group_index),
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // 故事记忆表（Story Bible）—— 最终大总结
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS story_memory (
            book_id TEXT PRIMARY KEY,
            book_summary TEXT NOT NULL DEFAULT '',
            volume_summaries TEXT NOT NULL DEFAULT '[]',
            event_timeline TEXT NOT NULL DEFAULT '[]',
            protagonist_status TEXT NOT NULL DEFAULT '',
            key_character_statuses TEXT NOT NULL DEFAULT '[]',
            unresolved_threads TEXT NOT NULL DEFAULT '[]',
            world_rules TEXT NOT NULL DEFAULT '',
            last_chapter_count INTEGER NOT NULL DEFAULT 0,
            last_word_count INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // ========== 设定账本（Canon Ledger） ==========
    // 结构化事实：实体 / 别名 / 状态时间线 / 事件流，用于长篇连续性核对
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS canon_entities (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            attrs TEXT NOT NULL DEFAULT '{}',
            first_chapter INTEGER NOT NULL DEFAULT 0,
            last_chapter INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            UNIQUE(book_id, kind, name)
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS canon_aliases (
            book_id TEXT NOT NULL,
            alias TEXT NOT NULL,
            alias_key TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            PRIMARY KEY (book_id, alias_key),
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (entity_id) REFERENCES canon_entities(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS canon_states (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            chapter_id TEXT NOT NULL,
            chapter_order INTEGER NOT NULL DEFAULT 0,
            seq INTEGER NOT NULL,
            field TEXT NOT NULL,
            value TEXT NOT NULL,
            quote TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
            FOREIGN KEY (entity_id) REFERENCES canon_entities(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS canon_events (
            id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            chapter_id TEXT NOT NULL,
            chapter_order INTEGER NOT NULL DEFAULT 0,
            seq INTEGER NOT NULL,
            subject_id TEXT,
            subject_name TEXT NOT NULL DEFAULT '',
            predicate TEXT NOT NULL,
            object TEXT NOT NULL DEFAULT '',
            quote TEXT NOT NULL DEFAULT '',
            importance TEXT NOT NULL DEFAULT 'normal',
            created_at INTEGER NOT NULL,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_canon_entities_book ON canon_entities(book_id, kind)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_canon_states_lookup ON canon_states(book_id, entity_id, seq)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_canon_events_book ON canon_events(book_id, seq)")
        .execute(pool)
        .await?;

    // 每章抽取记录：用章节 updated_at 判断账本是否过期
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS canon_extractions (
            chapter_id TEXT PRIMARY KEY,
            book_id TEXT NOT NULL,
            content_updated_at INTEGER NOT NULL DEFAULT 0,
            extracted_at INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    // 人工编辑标记：手工改过的实体不被自动抽取覆盖简介
    let _ = sqlx::query("ALTER TABLE canon_entities ADD COLUMN manual INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // 冲突检测进度跟踪表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conflict_check_progress (
            book_id TEXT PRIMARY KEY,
            last_checked_word_count INTEGER NOT NULL DEFAULT 0,
            last_checked_chapter_count INTEGER NOT NULL DEFAULT 0,
            last_checked_at INTEGER NOT NULL DEFAULT 0,
            last_status TEXT NOT NULL DEFAULT 'idle',
            last_error_kind TEXT,
            last_error_message TEXT,
            last_error_at INTEGER NOT NULL DEFAULT 0,
            last_auto_checked_at INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
        )
        "#
    )
    .execute(pool)
    .await?;

    let _ = sqlx::query("ALTER TABLE conflict_check_progress ADD COLUMN last_status TEXT NOT NULL DEFAULT 'idle'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE conflict_check_progress ADD COLUMN last_error_kind TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE conflict_check_progress ADD COLUMN last_error_message TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE conflict_check_progress ADD COLUMN last_error_at INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE conflict_check_progress ADD COLUMN last_auto_checked_at INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // 章节摘要表新增字段（用于增强摘要）
    let _ = sqlx::query("ALTER TABLE chapter_summaries ADD COLUMN plot_progression TEXT")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE chapter_summaries ADD COLUMN emotional_beats TEXT NOT NULL DEFAULT '[]'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE chapter_summaries ADD COLUMN foreshadowing TEXT NOT NULL DEFAULT '[]'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE chapter_summaries ADD COLUMN unresolved_threads TEXT NOT NULL DEFAULT '[]'")
        .execute(pool)
        .await;

    // 冲突表新增严重程度字段（历史数据默认 medium）
    let _ = sqlx::query("ALTER TABLE detected_conflicts ADD COLUMN severity TEXT NOT NULL DEFAULT 'medium'")
        .execute(pool)
        .await;

    Ok(())
}

/// 获取数据库连接池（单例）
use std::sync::OnceLock;
static DB_POOL: OnceLock<DbPool> = OnceLock::new();
/// 初始化锁：避免并发首次调用时创建多个连接池
static INIT_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

pub async fn get_pool() -> anyhow::Result<&'static DbPool> {
    if let Some(pool) = DB_POOL.get() {
        return Ok(pool);
    }

    let lock = INIT_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;

    // 双重检查：等锁期间可能已被其它任务初始化
    if let Some(pool) = DB_POOL.get() {
        return Ok(pool);
    }

    let pool = init_db().await?;
    let _ = DB_POOL.set(pool);

    DB_POOL
        .get()
        .ok_or_else(|| anyhow::anyhow!("数据库连接池初始化失败"))
}

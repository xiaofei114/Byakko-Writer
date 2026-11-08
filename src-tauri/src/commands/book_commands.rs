use crate::models::{Book, Chapter};
use crate::services::book_service;
use crate::services::summary_generator_service;
use crate::services::config_service;

/// 创建书籍（同时创建第一卷与第一章）
#[tauri::command]
pub async fn create_book(
    title: String,
    author: Option<String>,
    description: Option<String>,
) -> Result<Book, String> {
    book_service::create_book(
        title,
        author.unwrap_or_default(),
        description.unwrap_or_default(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// 获取书籍列表
#[tauri::command]
pub async fn get_books_list() -> Result<Vec<serde_json::Value>, String> {
    let books = book_service::get_books_list().await.map_err(|e| e.to_string())?;
    
    let result: Vec<serde_json::Value> = books
        .into_iter()
        .map(|book| {
            serde_json::json!({
                "id": book.id,
                "title": book.title,
                "author": book.author,
                "description": book.description,
                "updated_at": book.updated_at
            })
        })
        .collect();
    
    Ok(result)
}

/// 加载书籍
#[tauri::command]
pub async fn load_book(book_id: String) -> Result<Book, String> {
    book_service::load_book(book_id).await.map_err(|e| e.to_string())
}

/// 保存书籍
#[tauri::command]
pub async fn save_book(book: Book) -> Result<(), String> {
    book_service::save_book(book).await.map_err(|e| e.to_string())
}

/// 删除书籍
#[tauri::command]
pub async fn delete_book(book_id: String) -> Result<(), String> {
    book_service::delete_book(book_id).await.map_err(|e| e.to_string())
}

/// 更新书籍信息
#[tauri::command]
pub async fn update_book(book_id: String, title: String, author: String, description: String) -> Result<(), String> {
    book_service::update_book(book_id, title, author, description).await.map_err(|e| e.to_string())
}

/// 创建卷
#[tauri::command]
pub async fn create_volume(book_id: String, title: String) -> Result<serde_json::Value, String> {
    let volume = book_service::create_volume(book_id, title).await.map_err(|e| e.to_string())?;
    
    Ok(serde_json::json!({
        "id": volume.id,
        "title": volume.title,
        "order": volume.order,
        "isCollapsed": volume.is_collapsed,
        "createdAt": volume.created_at,
        "updatedAt": volume.updated_at,
    }))
}

/// 删除卷
#[tauri::command]
pub async fn delete_volume(book_id: String, volume_id: String) -> Result<(), String> {
    book_service::delete_volume(book_id, volume_id).await.map_err(|e| e.to_string())
}

/// 更新卷标题
#[tauri::command]
pub async fn update_volume_title(volume_id: String, title: String) -> Result<(), String> {
    book_service::update_volume_title(volume_id, title).await.map_err(|e| e.to_string())
}

/// 创建章节
#[tauri::command]
pub async fn create_chapter(
    book_id: String,
    title: String,
    volume_id: String,
) -> Result<Chapter, String> {
    let chapter = book_service::create_chapter(book_id.clone(), title, volume_id).await.map_err(|e| e.to_string())?;

    // 只需处理这一章：新建章节内容为空，实际不会产生 AI 调用；
    // 避免每次建章都全库扫描 + 批量生成（token 与限流风险）。
    let chapter_id_for_task = chapter.id.clone();
    let book_id_for_task = book_id.clone();
    tokio::spawn(async move {
        match config_service::load_config() {
            Ok(config) => {
                if !config.ai.api_key.is_empty() {
                    match summary_generator_service::generate_missing_summaries_background(
                        book_id_for_task,
                        config.ai,
                        Some(chapter_id_for_task),
                    ).await {
                        Ok(result) => {
                            log::info!("[CreateChapter] 新章节摘要处理完成: {:?}", result);
                        }
                        Err(e) => {
                            log::error!("[CreateChapter] 新章节摘要处理失败: {}", e);
                        }
                    }
                } else {
                    log::warn!("[CreateChapter] API Key 未配置，跳过摘要生成");
                }
            }
            Err(e) => {
                log::error!("[CreateChapter] 加载配置失败: {}", e);
            }
        }
    });

    Ok(chapter)
}

/// 删除章节
#[tauri::command]
pub async fn delete_chapter(book_id: String, chapter_id: String) -> Result<(), String> {
    book_service::delete_chapter(book_id, chapter_id).await.map_err(|e| e.to_string())
}

/// 加载章节内容
#[tauri::command]
pub async fn load_chapter_content(chapter_id: String) -> Result<String, String> {
    book_service::load_chapter_content(chapter_id).await.map_err(|e| e.to_string())
}

/// 保存章节内容
#[tauri::command]
pub async fn save_chapter_content(
    chapter_id: String,
    content: String,
) -> Result<(), String> {
    book_service::save_chapter_content(chapter_id, content).await.map_err(|e| e.to_string())
}

/// 更新章节标题
#[tauri::command]
pub async fn update_chapter_title(
    chapter_id: String,
    title: String,
) -> Result<(), String> {
    book_service::update_chapter_title(chapter_id, title).await.map_err(|e| e.to_string())
}

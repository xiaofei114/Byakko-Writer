use crate::models::{Book, Chapter};
use crate::services::book_service;
use crate::services::summary_generator_service;
use crate::services::config_service;

/// 创建书籍
#[tauri::command]
pub async fn create_book(title: String) -> Result<Book, String> {
    book_service::create_book(title).await.map_err(|e| e.to_string())
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
pub async fn delete_volume(volume_id: String) -> Result<(), String> {
    book_service::delete_volume(volume_id).await.map_err(|e| e.to_string())
}

/// 创建章节
#[tauri::command]
pub async fn create_chapter(
    book_id: String,
    title: String,
    volume_id: String,
) -> Result<Chapter, String> {
    let chapter = book_service::create_chapter(book_id.clone(), title, volume_id).await.map_err(|e| e.to_string())?;
    
    // 后台触发缺失摘要的生成（包含新创建的章节）
    let book_id_for_task = book_id.clone();
    tokio::spawn(async move {
        match config_service::load_config() {
            Ok(config) => {
                if !config.ai.api_key.is_empty() {
                    log::info!("[CreateChapter] 触发后台摘要生成 for book: {}", book_id_for_task);
                    match summary_generator_service::generate_missing_summaries_background(
                        book_id_for_task,
                        config.ai,
                    ).await {
                        Ok(result) => {
                            log::info!("[CreateChapter] 后台摘要生成完成: {:?}", result);
                        }
                        Err(e) => {
                            log::error!("[CreateChapter] 后台摘要生成失败: {}", e);
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
pub async fn delete_chapter(chapter_id: String) -> Result<(), String> {
    book_service::delete_chapter(chapter_id).await.map_err(|e| e.to_string())
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

/// 应用单行修改（AI提议的修改）
#[tauri::command]
pub async fn apply_line_edit(
    chapter_id: String,
    line_number: i64,
    new_text: String,
) -> Result<(), String> {
    // 加载当前章节内容
    let content = book_service::load_chapter_content(chapter_id.clone()).await.map_err(|e| e.to_string())?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len() as i64;

    if line_number < 1 || line_number > total_lines {
        return Err(format!("行号 {} 超出范围（1-{}）", line_number, total_lines));
    }

    // 构建新内容
    let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    new_lines[(line_number - 1) as usize] = new_text;

    // 保存修改后的内容
    let new_content = new_lines.join("\n");
    book_service::save_chapter_content(chapter_id, new_content).await.map_err(|e| e.to_string())?;

    Ok(())
}

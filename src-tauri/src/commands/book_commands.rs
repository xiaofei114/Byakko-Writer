use crate::models::{Book, Chapter};
use crate::services::book_service;

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
    book_service::create_chapter(book_id, title, volume_id).await.map_err(|e| e.to_string())
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

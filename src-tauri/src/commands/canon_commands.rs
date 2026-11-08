use tauri::{AppHandle, Emitter};

/// 列出账本实体
#[tauri::command]
pub async fn list_canon_entities(
    book_id: String,
    keyword: Option<String>,
    kind: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    crate::services::canon_service::list_entities(&book_id, keyword, kind)
        .await
        .map_err(|e| e.to_string())
}

/// 单个实体详情（状态时间线 + 事件）
#[tauri::command]
pub async fn get_canon_entity(
    book_id: String,
    entity_id: String,
) -> Result<serde_json::Value, String> {
    crate::services::canon_service::entity_detail(&book_id, &entity_id)
        .await
        .map_err(|e| e.to_string())
}

/// 人工修正实体
#[tauri::command]
pub async fn update_canon_entity(
    book_id: String,
    entity_id: String,
    name: String,
    kind: String,
    summary: String,
    aliases: Vec<String>,
) -> Result<(), String> {
    crate::services::canon_service::update_entity(
        &book_id, &entity_id, &name, &kind, &summary, aliases,
    )
    .await
    .map_err(|e| e.to_string())
}

/// 删除实体
#[tauri::command]
pub async fn delete_canon_entity(book_id: String, entity_id: String) -> Result<(), String> {
    crate::services::canon_service::delete_entity(&book_id, &entity_id)
        .await
        .map_err(|e| e.to_string())
}

/// 删除状态
#[tauri::command]
pub async fn delete_canon_state(book_id: String, state_id: String) -> Result<(), String> {
    crate::services::canon_service::delete_state(&book_id, &state_id)
        .await
        .map_err(|e| e.to_string())
}

/// 删除事件
#[tauri::command]
pub async fn delete_canon_event(book_id: String, event_id: String) -> Result<(), String> {
    crate::services::canon_service::delete_event(&book_id, &event_id)
        .await
        .map_err(|e| e.to_string())
}

/// 账本统计
#[tauri::command]
pub async fn canon_stats(book_id: String) -> Result<serde_json::Value, String> {
    crate::services::canon_service::stats(&book_id)
        .await
        .map_err(|e| e.to_string())
}

/// 时间线查询（给界面/调试用）
#[tauri::command]
pub async fn get_canon_timeline(
    book_id: String,
    name: String,
    from_chapter: Option<i32>,
    to_chapter: Option<i32>,
) -> Result<String, String> {
    crate::services::canon_service::get_canon_timeline(&book_id, &name, from_chapter, to_chapter)
        .await
        .map_err(|e| e.to_string())
}

/// 全书回填（后台执行，进度通过 canon-rebuild-progress 事件推送）
#[tauri::command]
pub async fn rebuild_canon(app: AppHandle, book_id: String) -> Result<String, String> {
    let config = crate::services::config_service::load_config().map_err(|e| e.to_string())?;
    if config.ai.api_key.is_empty() {
        return Err("API Key 未配置，无法回填".to_string());
    }

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        match crate::services::canon_service::rebuild_book(&app_clone, &book_id, &config.ai).await {
            Ok(count) => log::info!("[Canon] 回填完成，写入 {} 条", count),
            Err(e) => log::error!("[Canon] 回填失败: {}", e),
        }
        let _ = app_clone.emit(
            "canon-rebuild-finished",
            serde_json::json!({ "bookId": book_id }),
        );
    });

    Ok("已开始回填".to_string())
}

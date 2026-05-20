use crate::models::{CharacterCard, CharacterRelationship};
use crate::services::character_service;
use crate::models::CharacterCardParams;

/// 创建角色卡
#[tauri::command]
pub async fn create_character_card(
    book_id: String,
    name: String,
    aliases: Vec<String>,
    gender: String,
    age: String,
    appearance: String,
    personality: String,
    background: String,
    goals: String,
    relationships: Vec<CharacterRelationship>,
    tags: Vec<String>,
    notes: String,
) -> Result<CharacterCard, String> {
    let params = CharacterCardParams {
        name,
        aliases,
        gender,
        age,
        appearance,
        personality,
        background,
        goals,
        relationships,
        tags,
        notes,
    };
    
    character_service::create_character_card(book_id, params)
        .await
        .map_err(|e| e.to_string())
}

/// 获取书籍的所有角色卡
#[tauri::command]
pub async fn list_character_cards(book_id: String) -> Result<Vec<CharacterCard>, String> {
    character_service::list_character_cards(book_id).await.map_err(|e| e.to_string())
}

/// 获取单个角色卡
#[tauri::command]
pub async fn get_character_card(card_id: String) -> Result<CharacterCard, String> {
    character_service::get_character_card(card_id).await.map_err(|e| e.to_string())
}

/// 更新角色卡
#[tauri::command]
pub async fn update_character_card(
    card_id: String,
    name: String,
    aliases: Vec<String>,
    gender: String,
    age: String,
    appearance: String,
    personality: String,
    background: String,
    goals: String,
    relationships: Vec<CharacterRelationship>,
    tags: Vec<String>,
    notes: String,
) -> Result<CharacterCard, String> {
    let params = CharacterCardParams {
        name,
        aliases,
        gender,
        age,
        appearance,
        personality,
        background,
        goals,
        relationships,
        tags,
        notes,
    };
    
    character_service::update_character_card(card_id, params)
        .await
        .map_err(|e| e.to_string())
}

/// 删除角色卡
#[tauri::command]
pub async fn delete_character_card(card_id: String) -> Result<(), String> {
    character_service::delete_character_card(card_id).await.map_err(|e| e.to_string())
}

/// 搜索角色卡
#[tauri::command]
pub async fn search_character_cards(
    book_id: String,
    query: String,
) -> Result<Vec<CharacterCard>, String> {
    character_service::search_character_cards(book_id, query).await.map_err(|e| e.to_string())
}

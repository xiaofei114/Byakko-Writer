use crate::db::get_pool;
use crate::models::{CharacterCard, CharacterCardParams};

/// 创建角色卡
pub async fn create_character_card(
    book_id: String,
    params: CharacterCardParams,
) -> anyhow::Result<CharacterCard> {
    let pool = get_pool().await?;
    
    let id = format!("char_{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().timestamp();
    
    let aliases_json = serde_json::to_string(&params.aliases)?;
    let relationships_json = serde_json::to_string(&params.relationships)?;
    let tags_json = serde_json::to_string(&params.tags)?;
    
    sqlx::query(
        r#"
        INSERT INTO character_cards 
        (id, book_id, name, aliases, gender, age, appearance, personality, background, goals, relationships, tags, notes, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#
    )
    .bind(&id)
    .bind(&book_id)
    .bind(&params.name)
    .bind(&aliases_json)
    .bind(&params.gender)
    .bind(&params.age)
    .bind(&params.appearance)
    .bind(&params.personality)
    .bind(&params.background)
    .bind(&params.goals)
    .bind(&relationships_json)
    .bind(&tags_json)
    .bind(&params.notes)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(CharacterCard {
        id,
        book_id,
        name: params.name,
        aliases: aliases_json,
        gender: params.gender,
        age: params.age,
        appearance: params.appearance,
        personality: params.personality,
        background: params.background,
        goals: params.goals,
        relationships: relationships_json,
        tags: tags_json,
        notes: params.notes,
        created_at: now,
        updated_at: now,
    })
}

/// 获取书籍的所有角色卡
pub async fn list_character_cards(book_id: String) -> anyhow::Result<Vec<CharacterCard>> {
    let pool = get_pool().await?;
    
    let cards: Vec<CharacterCard> = sqlx::query_as(
        r#"
        SELECT id, book_id, name, aliases, gender, age, appearance, personality, background, goals, relationships, tags, notes, created_at, updated_at
        FROM character_cards
        WHERE book_id = ?1
        ORDER BY updated_at DESC
        "#
    )
    .bind(&book_id)
    .fetch_all(pool)
    .await?;
    
    Ok(cards)
}

/// 获取单个角色卡
pub async fn get_character_card(card_id: String) -> anyhow::Result<CharacterCard> {
    let pool = get_pool().await?;
    
    let card: CharacterCard = sqlx::query_as(
        r#"
        SELECT id, book_id, name, aliases, gender, age, appearance, personality, background, goals, relationships, tags, notes, created_at, updated_at
        FROM character_cards
        WHERE id = ?1
        "#
    )
    .bind(&card_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("角色卡 {} 不存在", card_id))?;

    Ok(card)
}

/// 更新角色卡
pub async fn update_character_card(
    card_id: String,
    params: CharacterCardParams,
) -> anyhow::Result<CharacterCard> {
    let pool = get_pool().await?;
    
    let now = chrono::Utc::now().timestamp();
    let aliases_json = serde_json::to_string(&params.aliases)?;
    let relationships_json = serde_json::to_string(&params.relationships)?;
    let tags_json = serde_json::to_string(&params.tags)?;
    
    sqlx::query(
        r#"
        UPDATE character_cards
        SET name = ?1, aliases = ?2, gender = ?3, age = ?4, appearance = ?5, 
            personality = ?6, background = ?7, goals = ?8, relationships = ?9, 
            tags = ?10, notes = ?11, updated_at = ?12
        WHERE id = ?13
        "#
    )
    .bind(&params.name)
    .bind(&aliases_json)
    .bind(&params.gender)
    .bind(&params.age)
    .bind(&params.appearance)
    .bind(&params.personality)
    .bind(&params.background)
    .bind(&params.goals)
    .bind(&relationships_json)
    .bind(&tags_json)
    .bind(&params.notes)
    .bind(now)
    .bind(&card_id)
    .execute(pool)
    .await?;
    
    get_character_card(card_id).await
}

/// 删除角色卡
pub async fn delete_character_card(card_id: String) -> anyhow::Result<()> {
    let pool = get_pool().await?;
    
    sqlx::query("DELETE FROM character_cards WHERE id = ?1")
        .bind(&card_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

/// 搜索角色卡
pub async fn search_character_cards(
    book_id: String,
    query: String,
) -> anyhow::Result<Vec<CharacterCard>> {
    let pool = get_pool().await?;
    
    let search_pattern = format!("%{}%", query);
    
    let cards: Vec<CharacterCard> = sqlx::query_as(
        r#"
        SELECT id, book_id, name, aliases, gender, age, appearance, personality, background, goals, relationships, tags, notes, created_at, updated_at
        FROM character_cards
        WHERE book_id = ?1 AND (
            name LIKE ?2 OR
            aliases LIKE ?2 OR
            tags LIKE ?2 OR
            notes LIKE ?2
        )
        ORDER BY updated_at DESC
        "#
    )
    .bind(&book_id)
    .bind(&search_pattern)
    .fetch_all(pool)
    .await?;
    
    Ok(cards)
}

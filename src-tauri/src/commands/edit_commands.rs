use serde::Serialize;

use crate::services::book_service;
use crate::services::edit_proposal_service::{self, ApplyResult, EditItemArgs};
use crate::services::outline_service::{self, OutlineChange};
use crate::services::snapshot_service;

/// 批量大纲保存结果
#[derive(Debug, Clone, Serialize)]
pub struct OutlineApplyResult {
    pub saved: usize,
    pub failures: Vec<String>,
}

/// 应用 AI 的批量正文修改
///
/// - `edits`：按原文锚定的多条修改（与提案卡片里勾选的一致）
/// - `replace_all`：整章替换内容（与 edits 二选一）
///
/// 成功应用前会自动创建「AI 修改前」快照，方便在快照面板里撤回。
#[tauri::command]
pub async fn apply_edit_proposal(
    book_id: String,
    chapter_id: String,
    edits: Vec<EditItemArgs>,
    replace_all: Option<String>,
) -> Result<ApplyResult, String> {
    let content = book_service::load_chapter_content(chapter_id.clone())
        .await
        .map_err(|e| e.to_string())?;

    let is_replace_all = replace_all.is_some();

    let (new_content, applied, failed, outcomes) = match replace_all {
        Some(full) if !full.trim().is_empty() => {
            let full = full.replace("\r\n", "\n");
            (full, 1usize, 0usize, Vec::new())
        }
        _ => {
            let result = edit_proposal_service::apply_edits(&content, &edits);
            (result.new_content, result.applied, result.failed, result.outcomes)
        }
    };

    if applied == 0 {
        return Ok(ApplyResult {
            applied: 0,
            failed,
            new_content: content,
            outcomes,
            snapshot_id: None,
        });
    }

    // 应用前快照（失败不阻塞应用）
    let chapter_title: String = match crate::db::get_pool().await {
        Ok(pool) => sqlx::query_scalar("SELECT title FROM chapters WHERE id = ?1")
            .bind(&chapter_id)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|_| "未命名章节".to_string()),
        Err(_) => "未命名章节".to_string(),
    };

    let snapshot_id = match snapshot_service::create_chapter_snapshot(
        book_id.clone(),
        chapter_id.clone(),
        chapter_title,
        "AI 修改前".to_string(),
        content.clone(),
    )
    .await
    {
        Ok(snapshot) => Some(snapshot.id),
        Err(e) => {
            log::warn!("[Edit] 应用 AI 修改前创建快照失败: {}", e);
            None
        }
    };

    book_service::save_chapter_content(chapter_id.clone(), new_content.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 整章替换后旧摘要必然过期：删掉让自动摘要重新生成
    if is_replace_all {
        if let Err(e) =
            crate::services::summary_service::invalidate_chapter_summary(&chapter_id).await
        {
            log::warn!("[Edit] 失效旧摘要失败: {}", e);
        }
    }

    Ok(ApplyResult {
        applied,
        failed,
        new_content,
        outcomes,
        snapshot_id,
    })
}

/// 应用 AI 的大纲提案（一次写入多层级 / 多类型大纲）
#[tauri::command]
pub async fn apply_outline_proposal(
    book_id: String,
    outlines: Vec<OutlineChange>,
) -> Result<OutlineApplyResult, String> {
    let (saved, failures) = outline_service::save_outlines_batch(&book_id, &outlines)
        .await
        .map_err(|e| e.to_string())?;

    Ok(OutlineApplyResult { saved, failures })
}

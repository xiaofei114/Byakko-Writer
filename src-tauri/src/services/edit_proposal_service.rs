//! 批量编辑提案
//!
//! 解决旧方案「AI 一条一条 propose_line_edit / save_outline，用户一条一条点确认」的问题：
//! - AI 一次可以提交多条修改（每条用原文锚定，行号只作提示）
//! - 后端统一计算每条能否定位、替换前的原文片段
//! - 前端一张卡片全选 / 批量应用，一次落库
//!
//! 这里只做「纯函数 + 落库」两件事，工具层负责拼 proposal 并推给前端。

use serde::{Deserialize, Serialize};

/// AI 提交的单条修改
#[derive(Debug, Clone, Deserialize)]
pub struct EditItemArgs {
    /// 行号（可选，仅作定位提示，原文锚定优先）
    #[serde(alias = "lineNumber", default)]
    pub line_number: Option<i64>,
    /// 原文（建议提供，便于精确定位；为空时按行号整行替换）
    #[serde(alias = "originalText", default)]
    pub original_text: String,
    /// 替换后的文本（可包含换行）
    #[serde(alias = "newText")]
    pub new_text: String,
    /// 修改理由（可选）
    #[serde(default)]
    pub reason: String,
}

/// 单条修改的执行结果（也用于前端预览）
#[derive(Debug, Clone, Serialize)]
pub struct EditOutcome {
    pub index: usize,
    pub matched: bool,
    pub matched_line: Option<i64>,
    /// 替换前的原文片段，便于前端做前后对比
    pub before: String,
    pub message: String,
}

/// 批量应用结果
#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub applied: usize,
    pub failed: usize,
    pub new_content: String,
    pub outcomes: Vec<EditOutcome>,
    /// 应用前自动创建的快照 ID（应用成功时才有）
    pub snapshot_id: Option<String>,
}

/// 把 AI 的修改批量应用到正文
///
/// 定位策略（按优先级）：
/// 1. 原文精确出现在某一行 → 替换该行内的原文片段（保留行内其它内容）
/// 2. 原文跨行 → 在整段文本里做一次精确替换
/// 3. 没有原文但有行号 → 整行替换
pub fn apply_edits(content: &str, edits: &[EditItemArgs]) -> ApplyResult {
    let mut lines: Vec<String> = content
        .replace("\r\n", "\n")
        .replace('\r', "")
        .split('\n')
        .map(str::to_string)
        .collect();
    let mut outcomes: Vec<EditOutcome> = Vec::with_capacity(edits.len());
    let mut applied = 0usize;

    for (index, edit) in edits.iter().enumerate() {
        let new_text = edit.new_text.clone();
        let original = edit.original_text.trim().to_string();

        // 1) 没有原文：按行号整行替换
        if original.is_empty() {
            match edit.line_number {
                Some(n) if n >= 1 && (n as usize) <= lines.len() => {
                    let i = (n - 1) as usize;
                    let before = lines[i].clone();
                    let replacement: Vec<String> = new_text.split('\n').map(str::to_string).collect();
                    lines.splice(i..=i, replacement);
                    applied += 1;
                    outcomes.push(EditOutcome {
                        index,
                        matched: true,
                        matched_line: Some(n),
                        before,
                        message: format!("已替换第 {} 行", n),
                    });
                }
                _ => outcomes.push(EditOutcome {
                    index,
                    matched: false,
                    matched_line: None,
                    before: String::new(),
                    message: format!("#{}：缺少 originalText 或行号越界", index + 1),
                }),
            }
            continue;
        }

        // 2) 原文能落在某一行内
        let candidates: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(&original))
            .map(|(i, _)| i)
            .collect();

        if !candidates.is_empty() {
            let target = match edit.line_number {
                Some(n) if n >= 1 && candidates.contains(&((n - 1) as usize)) => (n - 1) as usize,
                _ => candidates[0],
            };

            let before = lines[target].clone();
            let new_line = if new_text.contains('\n') {
                // 新内容跨行：整行替换（保留原文所在行的缩进前缀）
                let indent: String = before
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                format!("{}{}", indent, new_text)
            } else {
                before.replacen(&original, &new_text, 1)
            };

            let replacement: Vec<String> = new_line.split('\n').map(str::to_string).collect();
            lines.splice(target..=target, replacement);
            applied += 1;

            let extra = if candidates.len() > 1 {
                format!("（原文出现 {} 次，已替换第 {} 行）", candidates.len(), target + 1)
            } else {
                String::new()
            };
            outcomes.push(EditOutcome {
                index,
                matched: true,
                matched_line: Some(target as i64 + 1),
                before,
                message: format!("已替换第 {} 行{}", target + 1, extra),
            });
            continue;
        }

        // 3) 跨行原文：整段精确替换
        let joined = lines.join("\n");
        if let Some(pos) = joined.find(&original) {
            let new_joined = format!(
                "{}{}{}",
                &joined[..pos],
                new_text,
                &joined[pos + original.len()..]
            );
            lines = new_joined.split('\n').map(str::to_string).collect();
            applied += 1;
            outcomes.push(EditOutcome {
                index,
                matched: true,
                matched_line: None,
                before: original.clone(),
                message: "已按整段匹配替换".to_string(),
            });
            continue;
        }

        // 4) 找不到
        outcomes.push(EditOutcome {
            index,
            matched: false,
            matched_line: None,
            before: String::new(),
            message: format!("#{}：未找到原文「{}」", index + 1, truncate(&original, 30)),
        });
    }

    let failed = outcomes.iter().filter(|o| !o.matched).count();
    let new_content = lines.join("\n");

    // applied 统计以实际替换数计（匹配数应与 applied 一致）
    ApplyResult {
        applied,
        failed,
        new_content,
        outcomes,
        snapshot_id: None,
    }
}

/// 只做定位预览（不落库），供工具层生成提案时使用
pub fn preview_edits(content: &str, edits: &[EditItemArgs]) -> Vec<EditOutcome> {
    apply_edits(content, edits).outcomes
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(original: &str, new_text: &str, line: Option<i64>) -> EditItemArgs {
        EditItemArgs {
            line_number: line,
            original_text: original.to_string(),
            new_text: new_text.to_string(),
            reason: String::new(),
        }
    }

    #[test]
    fn replaces_inline_occurrence() {
        let content = "他抬头看了看天。\n然后转身离开了。";
        let result = apply_edits(content, &[edit("转身离开", "快步走开", None)]);
        assert_eq!(result.applied, 1);
        assert_eq!(result.new_content, "他抬头看了看天。\n然后快步走开了。");
    }

    #[test]
    fn replaces_whole_line_by_number() {
        let content = "第一行\n第二行\n第三行";
        let result = apply_edits(content, &[edit("", "新的第二行", Some(2))]);
        assert_eq!(result.applied, 1);
        assert_eq!(result.new_content, "第一行\n新的第二行\n第三行");
    }

    #[test]
    fn missing_original_is_reported() {
        let result = apply_edits("abc", &[edit("不存在的句子", "x", None)]);
        assert_eq!(result.applied, 0);
        assert_eq!(result.failed, 1);
        assert!(!result.outcomes[0].matched);
    }

    #[test]
    fn multiline_replacement_expands_lines() {
        let content = "a\nb\nc";
        let result = apply_edits(content, &[edit("", "B1\nB2", Some(2))]);
        assert_eq!(result.new_content, "a\nB1\nB2\nc");
    }

    #[test]
    fn duplicate_original_prefers_hinted_line() {
        let content = "重复\n中间\n重复";
        let result = apply_edits(content, &[edit("重复", "唯一", Some(3))]);
        assert_eq!(result.new_content, "重复\n中间\n唯一");
    }
}

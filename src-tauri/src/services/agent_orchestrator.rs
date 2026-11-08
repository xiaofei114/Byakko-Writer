use crate::models::{AIConfig, ChatMessage, StreamEvent, ToolCall, ToolDef, FunctionDef};
use crate::services::ai_service::{send_ai_message, send_ai_message_stream};
use crate::services::prompt_service::get_prompt_manager;
use crate::services::tool_call_service::execute_tool_call;
use futures::StreamExt;
use tauri::{AppHandle, Emitter};
use std::sync::Mutex;
use std::collections::HashMap;

/// 错误追踪信息
#[derive(Debug, Clone)]
struct ErrorTracker {
    /// 错误原因（用于判断是否是同一种错误）
    reason: String,
    /// 连续出现次数
    count: u32,
}

/// 会话状态
#[derive(Debug, Clone)]
struct SessionState {
    messages: Vec<ChatMessage>,
    round_count: u32,
    compressed_context: String,
    waiting_for_user: bool, // 是否正在等待用户答复（ask_user工具）
    last_error: Option<ErrorTracker>, // 最近一次工具执行错误
    last_active: i64,      // 最近活跃时间（秒），用于淘汰旧会话
}

/// 全局会话状态管理
static SESSION_STATES: Mutex<Option<HashMap<String, SessionState>>> = Mutex::new(None);

/// 内存中最多保留的会话数，超出后淘汰最久未使用的会话
const MAX_SESSION_STATES: usize = 20;

/// 注入对话上下文的故事圣经字符预算（超出时优先丢弃时间线细节）
const STORY_BIBLE_BUDGET_CHARS: usize = 6000;

/// 章节列表只看当前卷及相邻卷，避免 100 章后每轮都带上完整目录
const CHAPTER_LIST_WINDOW: usize = 40;

/// 单轮对话最多做几次「工具后反思」，避免工具多时请求爆炸
const MAX_REFLECTIONS: usize = 5;

/// 按字符截断，避免中文切在 UTF-8 字节边界上
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}

fn get_session_state(session_id: &str) -> Option<SessionState> {
    let states = SESSION_STATES.lock().ok()?;
    states.as_ref()?.get(session_id).cloned()
}

fn set_session_state(session_id: String, mut state: SessionState) {
    state.last_active = chrono::Utc::now().timestamp();

    if let Ok(mut guard) = SESSION_STATES.lock() {
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        if let Some(states) = guard.as_mut() {
            states.insert(session_id, state);

            // 超出上限时淘汰最久未活跃的会话，避免内存无限增长
            while states.len() > MAX_SESSION_STATES {
                let oldest: Option<String> = states
                    .iter()
                    .min_by_key(|(_, s)| s.last_active)
                    .map(|(id, _)| id.to_string());
                match oldest {
                    Some(id) => {
                        log::debug!("[Agent] 淘汰旧会话状态: {}", id);
                        states.remove(&id);
                    }
                    None => break,
                }
            }
        }
    }
}

/// 删除会话时同步清理内存状态（删除会话 / 切换书籍时调用）
pub fn forget_session_state(session_id: &str) {
    if let Ok(mut guard) = SESSION_STATES.lock() {
        if let Some(states) = guard.as_mut() {
            states.remove(session_id);
        }
    }
}

/// 构建所有可用工具的 function calling 定义
fn build_tools() -> Vec<ToolDef> {
    vec![
        // ===== 查询类 =====
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_book_info".into(),
                description: "获取当前书籍的基本信息（书名、作者、简介）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "list_all_chapters".into(),
                description: "获取当前书籍的所有章节列表（含章节ID和标题）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_summary".into(),
                description: "获取指定章节的摘要信息（短摘要+长摘要+剧情推进+情感节点+伏笔+未解决线索+标签+角色+地点+事件）。核对设定连续性时优先用它".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "search_chapter_content".into(),
                description: "全文搜索（查人物/物品/设定在哪一章出现过就用它）。默认普通匹配，会自动把角色别名一起搜（例如搜「小动」也会命中「林动」）；regex=true 时按正则匹配且不做别名扩展。结果按相关度排序并自动截断为最多 30 处，返回章节ID、行号与上下文片段。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID（可选，不指定则搜索全书）"},
                        "keyword": {"type": "string", "description": "搜索关键字或正则表达式"},
                        "regex": {"type": "boolean", "description": "是否使用正则表达式（默认false=普通模糊匹配）"}
                    },
                    "required": ["keyword"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_content".into(),
                description: "获取指定章节的正文内容。可指定行号范围，不指定则返回全文。startLine=1,endLine=10 返回前10行；startLine=11,endLine=14 返回11-14行；只设endLine=5 返回前5行；只设startLine=-5 返回最后5行".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID"},
                        "startLine": {"type": "integer", "description": "起始行号（1开始），负数表示从末尾倒数"},
                        "endLine": {"type": "integer", "description": "结束行号（包含）"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "list_character_cards".into(),
                description: "获取当前书籍的所有角色卡列表".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_character_card".into(),
                description: "获取指定角色的详细信息（外貌、性格、背景等）。name参数用于前端显示，需从list_character_cards结果中获取".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "cardId": {"type": "string", "description": "角色卡ID"},
                        "name": {"type": "string", "description": "角色名（可选，用于显示）"}
                    },
                    "required": ["cardId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_outline".into(),
                description: "获取指定章节的大纲（粗纲或细纲）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        // ===== 操作类 =====
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "create_character_card".into(),
                description: "创建新的角色卡".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "角色名"},
                        "gender": {"type": "string", "description": "性别"},
                        "age": {"type": "string", "description": "年龄"},
                        "appearance": {"type": "string", "description": "外貌描述"},
                        "personality": {"type": "string", "description": "性格"},
                        "background": {"type": "string", "description": "背景故事"},
                        "goals": {"type": "string", "description": "目标"},
                        "notes": {"type": "string", "description": "备注"}
                    },
                    "required": ["name"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "update_character_card".into(),
                description: "更新已有角色卡的信息".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "cardId": {"type": "string", "description": "角色卡ID"},
                        "name": {"type": "string", "description": "角色名"},
                        "gender": {"type": "string", "description": "性别"},
                        "personality": {"type": "string", "description": "性格"},
                        "background": {"type": "string", "description": "背景故事"}
                    },
                    "required": ["cardId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "propose_edits".into(),
                description: "提交正文修改提案（支持一次多处的批量修改）。**任何对正文的修改都必须用它**：润色、改错、重写段落、调整多行，统一把全部改动放进 edits 数组一次提交，不要一条一条调用。每条用 originalText 精确引用原文（建议同时带上 lineNumber 作提示）；整章重写时改用 content 传完整新正文。提交后前端会展示一张带勾选的卡片，由用户一次性应用；应用前会自动创建快照。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "目标章节ID"},
                        "summary": {"type": "string", "description": "一句话说明本次修改目的，如'修正第3章中人物称呼不一致'"},
                        "edits": {
                            "type": "array",
                            "description": "要修改的条目列表，一次可以放多条",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "lineNumber": {"type": "integer", "description": "行号（1开始，可选，仅作定位提示）"},
                                    "originalText": {"type": "string", "description": "原文片段（必须与正文完全一致，用于精确定位）"},
                                    "newText": {"type": "string", "description": "替换后的文本"},
                                    "reason": {"type": "string", "description": "修改理由（可选）"}
                                },
                                "required": ["originalText", "newText"]
                            }
                        },
                        "content": {"type": "string", "description": "可选：整章替换时的完整新正文（与 edits 二选一）"}
                    },
                    "required": ["chapterId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "propose_outlines".into(),
                description: "提交大纲修改提案（支持书籍/卷/章节三级、粗纲/细纲，一次可提交多项）。**任何大纲的创建或修改都必须用它**，把全部变更放进 outlines 数组一次提交，用户确认后一次性写入。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "summary": {"type": "string", "description": "一句话说明本次大纲调整目的"},
                        "outlines": {
                            "type": "array",
                            "description": "大纲变更列表",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "level": {"type": "string", "description": "book | volume | chapter"},
                                    "volumeId": {"type": "string", "description": "卷ID（level=volume 时必填）"},
                                    "chapterId": {"type": "string", "description": "章节ID（level=chapter 时必填）"},
                                    "outlineType": {"type": "string", "description": "coarse(粗纲) 或 fine(细纲)"},
                                    "content": {"type": "string", "description": "大纲内容"}
                                },
                                "required": ["level", "outlineType", "content"]
                            }
                        }
                    },
                    "required": ["outlines"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "report_conflict".into(),
                description: "报告一个确认后的设定冲突。必须已查原文核实过再报告。每个冲突只报告一次，不要重复报告已经讨论过的冲突。用户正在修复冲突时不要调用此工具".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "description": {"type": "string", "description": "冲突描述，如'第1章叶铃音是黑发，第5章变成了银发'"},
                        "suggestion": {"type": "string", "description": "修改建议"},
                        "severity": {"type": "string", "description": "严重程度：high/medium/low"},
                        "chapterId": {"type": "string", "description": "可选：冲突主要涉及的章节ID，便于改稿后提示复核"}
                    },
                    "required": ["description", "suggestion", "severity"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_canon".into(),
                description: "查询「设定账本」中实体的设定卡：别名、首次/最近出场章、当前状态（境界/位置/伤势/持有物…）、最近事件。核对人物状态、关系、物品来历、称呼时优先用它，比读正文快且不容易漏。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "names": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "实体名或别名，可一次传多个"
                        },
                        "asOfChapter": {
                            "type": "integer",
                            "description": "可选：只看截至该章序号（order）的状态，用于核对历史时间点"
                        }
                    },
                    "required": ["names"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "search_canon".into(),
                description: "在设定账本里模糊检索实体/设定（按名字、别名或简介匹配）。不知道准确名字时先用它定位。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "搜索词"},
                        "kind": {"type": "string", "description": "可选：character/location/faction/item/skill/term"}
                    },
                    "required": ["query"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_canon_timeline".into(),
                description: "查询设定账本中某实体的时间线：按章节顺序列出状态变化与关联事件，可限定章节范围。回答“某人什么时候突破/得到什么/关系怎么变”时用它。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "description": "实体名或别名"},
                        "fromChapter": {"type": "integer", "description": "可选：起始章序号"},
                        "toChapter": {"type": "integer", "description": "可选：结束章序号"}
                    },
                    "required": ["name"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_volume_outline".into(),
                description: "获取指定卷的粗纲或细纲。volumeName参数传卷名（如'第一卷'），从get_book_info结果中获取卷名列表".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "volumeName": {"type": "string", "description": "卷名，如'第一卷'、'觉醒篇'等"},
                        "outlineType": {"type": "string", "description": "coarse(粗纲) 或 fine(细纲)，默认coarse"}
                    },
                    "required": ["volumeName"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "create_chapter".into(),
                description: "创建新章节。如果不指定volumeId则使用最后一卷，不指定order则自动计算，不指定title则自动生成".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "volumeId": {"type": "string", "description": "卷ID（可选）"},
                        "title": {"type": "string", "description": "章节标题（可选）"},
                        "order": {"type": "integer", "description": "排序号（可选）"}
                    },
                    "required": []
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "learn_writing_style".into(),
                description: "分析并学习当前书籍的写作风格".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "write_chapter".into(),
                description: "触发写作AI进行创作。调用此工具后，专业写作AI会根据你查询到的上下文进行写作，生成的内容会自动展示给用户确认插入。你不需要自己写正文".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "目标章节ID"},
                        "description": {"type": "string", "description": "简短描述，如'续写500字'或'重写第3-5段'"},
                        "instruction": {"type": "string", "description": "用户原始需求（如'续写第6章'）"}
                    },
                    "required": ["chapterId", "description"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "ask_user".into(),
                description: "当信息不足或需要用户做选择时，向用户提问并等待答复。调用后对话会暂停，直到用户答复后才继续".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "question": {"type": "string", "description": "问题内容"},
                        "context": {"type": "string", "description": "上下文说明（可选）"},
                        "options": {"type": "array", "items": {"type": "string"}, "description": "选项列表（可选，不提供则用户自由输入）"}
                    },
                    "required": ["question"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_character_timeline".into(),
                description: "按角色名查询该角色在各章节中出现的记录，用于核对角色行为/状态是否前后一致".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "characterName": {"type": "string", "description": "角色名"}
                    },
                    "required": ["characterName"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "list_chapters_in_volume".into(),
                description: "获取指定卷内的章节列表（含章节ID与标题）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "volumeId": {"type": "string", "description": "卷ID"}
                    },
                    "required": ["volumeId"]
                }),
            },
        },
    ]
}

/// Agent编排器
pub struct AgentOrchestrator;

impl AgentOrchestrator {
    /// 处理润色请求
    pub async fn process_polish(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        original_text: &str,
        instruction: &str,
        config: AIConfig,
    ) -> anyhow::Result<()> {
        Self::emit_phase_event(app, session_id, "polishing").await;

        let prompt_manager = get_prompt_manager();

        let style = crate::services::style_service::get_style_prompt(Some(book_id)).await
            .map(|r| r.style_prompt)
            .unwrap_or_default();

        let style_guide = if style.trim().is_empty() {
            prompt_manager.style_guide.clone()
        } else {
            format!(
                "{}\n\n---\n\n## 作者个人写作风格（从往期作品分析得出，优先级高于上面的通用规范）\n{}",
                prompt_manager.style_guide, style
            )
        };
        log::info!(
            "[Agent] 润色风格指南: {} 字（个人风格 {} 字）",
            style_guide.chars().count(),
            style.chars().count()
        );
        let system_prompt = prompt_manager
            .polish_agent
            .replace("{{STYLE_GUIDE}}", &style_guide);

        let context_json = serde_json::json!({
            "original_text": original_text,
            "instruction": instruction,
            "style": style,
        });

        let system_prompt = system_prompt.replace("{{CONTEXT_JSON}}", &context_json.to_string());

        let messages = vec![
            ChatMessage::new("system", &system_prompt),
            ChatMessage::new("user", "请润色上述文本"),
        ];

        // 润色长段落时输出可能超过默认上限，避免被截断
        let mut polish_config = config.clone();
        polish_config.max_tokens = polish_config.max_tokens.max(6000);

        Self::stream_ai_response(app, session_id, book_id, messages, polish_config, true).await
    }

    /// 处理普通对话请求 - 使用 function calling
    pub async fn process_chat(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        chapter_id: Option<&str>,
        outline_target: Option<&serde_json::Value>,
        user_message: &str,
        config: AIConfig,
    ) -> anyhow::Result<()> {
        let prompt_manager = get_prompt_manager();

        let mut state = get_session_state(session_id).unwrap_or_else(|| SessionState {
            messages: vec![],
            round_count: 0,
            compressed_context: String::new(),
            waiting_for_user: false,
            last_error: None,
            last_active: chrono::Utc::now().timestamp(),
        });

        // 进程重启后内存里没有会话状态：从数据库恢复最近几轮，
        // 避免 AI「失忆」后重复提问或前后说法不一致
        if state.messages.is_empty() {
            match crate::services::chat_service::load_recent_messages(session_id, 8).await {
                Ok(mut history) => {
                    // 本轮用户消息已在外部保存，这里去掉避免重复
                    let is_current_message = history
                        .last()
                        .map(|m| {
                            m.role == "user"
                                && m.content.as_deref() == Some(user_message)
                        })
                        .unwrap_or(false);
                    if is_current_message {
                        history.pop();
                    }
                    state.messages = history;
                }
                Err(e) => log::warn!("[Agent] 恢复历史上下文失败: {}", e),
            }
        }

        // 是否是从 ask_user 暂停状态恢复的对话
        let resumed = state.waiting_for_user;
        if resumed {
            log::info!("[Agent] 恢复对话，用户答复: {}", user_message);
            state.waiting_for_user = false;
        }

        // 构建 system prompt（每次都刷新，确保 STORY_BIBLE / CHAPTER_LIST 最新）
        // 两者都按当前章节做裁剪，避免 100 章后每一轮决策都重复塞入全文
        let chapter_list = Self::get_chapter_list(book_id, chapter_id).await.unwrap_or_default();
        let story_bible = crate::services::story_memory_service::build_story_memory_text_for_context(
            book_id,
            chapter_id,
            STORY_BIBLE_BUDGET_CHARS,
        )
        .await
        .unwrap_or_default();
        let system_prompt = prompt_manager
            .decision_agent
            .replace("{{BOOK_ID}}", book_id)
            .replace("{{CHAPTER_LIST}}", &chapter_list)
            .replace("{{STORY_BIBLE}}", &story_bible)
            .replace("{{COMPRESSED_CONTEXT}}", &state.compressed_context);

        // 用户正在编辑哪一层大纲：让 AI 知道“这个大纲/这里”指什么
        let focus_note = Self::describe_outline_focus(book_id, outline_target).await;
        let system_prompt = if focus_note.is_empty() {
            system_prompt
        } else {
            format!("{}\n\n{}", system_prompt, focus_note)
        };

        // 学习到的作者风格：决策 AI 生成 propose_edits 正文时同样要遵守
        let decision_style = crate::services::style_service::get_style_prompt(Some(book_id))
            .await
            .map(|result| result.style_prompt)
            .unwrap_or_default();
        let mut system_prompt = if decision_style.trim().is_empty() {
            system_prompt
        } else {
            log::info!(
                "[Agent] 决策上下文注入个人写作风格: {} 字",
                decision_style.chars().count()
            );
            format!(
                "{}\n\n## 作者个人写作风格（修改正文时必须遵守）\n{}",
                system_prompt, decision_style
            )
        };

        // 思考模式：先让 AI 单独做一次分析（目标/风险/计划/自检），再进入工具循环
        if config.enable_thinking {
            if let Some(plan) = Self::run_thinking(
                app,
                session_id,
                book_id,
                chapter_id,
                user_message,
                &state.messages,
                &story_bible,
                &focus_note,
                &config,
            )
            .await
            {
                log::info!("[Agent] 思考模式已生成计划，{} 字", plan.chars().count());
                system_prompt = format!(
                    "{}\n\n## 本轮内部计划（执行时参考，不要原样展示给用户）\n{}",
                    system_prompt, plan
                );
            }
        }

        // 应用后台压缩好的上下文（如果有）：只压缩历史，当前这条用户消息必须保留
        let had_compressed = !state.compressed_context.is_empty();
        if had_compressed {
            log::info!("[Agent] 应用后台压缩结果，长度: {} 字", state.compressed_context.chars().count());
            state.compressed_context.clear();
            state.messages.clear();
        }

        // 保证 system 消息在首位且内容最新
        let has_system = state
            .messages
            .first()
            .map(|m| m.role == "system")
            .unwrap_or(false);

        if has_system {
            if let Some(first) = state.messages.first_mut() {
                first.content = Some(system_prompt);
            }
        } else {
            state.messages.insert(0, ChatMessage::new("system", &system_prompt));
        }

        // 追加本轮用户消息（压缩后同样要保留，否则 AI 看不到本次提问）
        state.messages.push(ChatMessage::new("user", user_message));

        // 新消息才增加轮次（恢复对话不重复计数）
        if !resumed {
            state.round_count += 1;
        }

        let tools = build_tools();
        let max_rounds = config.max_rounds.max(3) as usize;
        let mut reflection_count = 0usize;

        for round in 0..max_rounds {
            log::info!("[Agent] 决策轮次 {}/{}", round + 1, max_rounds);

            let decision = send_ai_message(
                state.messages.clone(),
                config.clone(),
                Some(&tools),
            ).await?;

            // 推理模型可能额外返回原生思考内容（本轮不单独展示，仅记录）
            if let Some(reasoning) = &decision.reasoning {
                log::debug!(
                    "[Agent] 模型返回原生思考 {} 字",
                    reasoning.chars().count()
                );
            }

            // 有 tool_calls → 执行工具
            if decision.is_tool_call() {
                let tool_calls = decision.tool_calls.as_ref().unwrap();

                // 将AI的tool_calls消息加入历史
                state.messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: decision.content,
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                let mut tool_results = Vec::new();
                let mut last_tool_name = String::new();
                // 收集工具调用摘要（用于持久化到数据库，恢复历史时显示）
                let mut tool_call_summary: Vec<serde_json::Value> = Vec::new();
                // 暂存冲突报告，在所有工具调用完成后再发送事件
                let mut pending_conflicts: Vec<(String, String, String, String)> = Vec::new(); // (conflict_id, desc, sug, sev)

                for tc in tool_calls {
                    let tool_name = &tc.function.name;
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null);

                    log::info!("[Agent] 执行工具: {} args={}", tool_name, args);

                    Self::emit_tool_event(app, session_id, tool_name, Some(args.clone())).await;

                    // 收集工具摘要数据（前端恢复历史时用 buildToolDisplayText 重建显示文本）
                    // 保存所有可能的参数，让前端能正确显示工具调用详情
                    let mut summary = serde_json::json!({
                        "name": tool_name,
                        "chapterId": args.get("chapterId").and_then(|v| v.as_str()).unwrap_or(""),
                        "outlineType": args.get("outlineType").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": args.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        "startLine": args.get("startLine").and_then(|v| v.as_i64()).unwrap_or(0),
                        "endLine": args.get("endLine").and_then(|v| v.as_i64()).unwrap_or(0),
                    });
                    // 添加其他可能用到的参数
                    if let Some(keyword) = args.get("keyword").and_then(|v| v.as_str()) {
                        summary["keyword"] = serde_json::json!(keyword);
                    }
                    if let Some(regex) = args.get("regex").and_then(|v| v.as_bool()) {
                        summary["regex"] = serde_json::json!(regex);
                    }
                    if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                        summary["name"] = serde_json::json!(name);
                    }
                    if let Some(line_number) = args.get("lineNumber").and_then(|v| v.as_i64()) {
                        summary["lineNumber"] = serde_json::json!(line_number);
                    }
                    if let Some(original_text) = args.get("originalText").and_then(|v| v.as_str()) {
                        summary["originalText"] = serde_json::json!(original_text);
                    }
                    if let Some(new_text) = args.get("newText").and_then(|v| v.as_str()) {
                        summary["newText"] = serde_json::json!(new_text);
                    }
                    if let Some(reason) = args.get("reason").and_then(|v| v.as_str()) {
                        summary["reason"] = serde_json::json!(reason);
                    }
                    tool_call_summary.push(summary);

                    // save_outline / write_chapter 特殊处理：不真保存，发事件给前端渲染带按钮的卡片
                    let result = if tool_name == "propose_edits" {
                        let ch_id = args.get("chapterId").and_then(|v| v.as_str()).unwrap_or("");
                        let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                        let replace_all = args.get("content").and_then(|v| v.as_str()).map(str::to_string);
                        let edits: Vec<crate::services::edit_proposal_service::EditItemArgs> = args
                            .get("edits")
                            .cloned()
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default();

                        let has_replace_all = replace_all
                            .as_deref()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false);

                        if edits.is_empty() && !has_replace_all {
                            "修改提案为空：请提供 edits 数组或整章 content。".to_string()
                        } else {
                            Self::propose_edits(app, session_id, book_id, ch_id, summary, edits, replace_all)
                                .await
                                .unwrap_or_else(|e| format!("生成修改提案失败: {}", e))
                        }
                    } else if tool_name == "propose_outlines" {
                        let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                        let outlines: Vec<crate::services::outline_service::OutlineChange> = args
                            .get("outlines")
                            .cloned()
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default();

                        if outlines.is_empty() {
                            "大纲提案为空：请提供 outlines 数组。".to_string()
                        } else {
                            Self::propose_outlines(app, session_id, book_id, summary, outlines)
                                .await
                                .unwrap_or_else(|e| format!("生成大纲提案失败: {}", e))
                        }
                    } else if tool_name == "save_outline" {
                        // 旧工具兼容：转成单项大纲提案（章节级）
                        let ch_id = args.get("chapterId").and_then(|v| v.as_str()).unwrap_or("");
                        let outline_type = args.get("outlineType").and_then(|v| v.as_str()).unwrap_or("coarse");
                        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");

                        if ch_id.is_empty() {
                            "缺少 chapterId，无法保存大纲。".to_string()
                        } else {
                            let change = crate::services::outline_service::OutlineChange {
                                level: "chapter".to_string(),
                                volume_id: None,
                                chapter_id: Some(ch_id.to_string()),
                                outline_type: outline_type.to_string(),
                                content: content.to_string(),
                            };
                            Self::propose_outlines(app, session_id, book_id, "更新章节大纲", vec![change])
                                .await
                                .unwrap_or_else(|e| format!("生成大纲提案失败: {}", e))
                        }
                    } else if tool_name == "propose_line_edit" {
                        // 旧工具兼容：转成单条修改提案
                        let ch_id = args.get("chapterId").and_then(|v| v.as_str()).unwrap_or("");
                        let edit = crate::services::edit_proposal_service::EditItemArgs {
                            line_number: args.get("lineNumber").and_then(|v| v.as_i64()),
                            original_text: args.get("originalText").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            new_text: args.get("newText").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            reason: args.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        };
                        Self::propose_edits(app, session_id, book_id, ch_id, "单行修改", vec![edit], None)
                            .await
                            .unwrap_or_else(|e| format!("生成修改提案失败: {}", e))
                    } else if tool_name == "report_conflict" {
                        let desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let sug = args.get("suggestion").and_then(|v| v.as_str()).unwrap_or("");
                        let sev = args.get("severity").and_then(|v| v.as_str()).unwrap_or("medium");
                        // 过滤掉"没问题"类的伪冲突
                        let lower = desc.to_lowercase();
                        let is_no_problem = desc.is_empty()
                            || lower.contains("未检测到")
                            || lower.contains("未发现")
                            || lower.contains("没有冲突")
                            || lower.contains("没有问题")
                            || lower.contains("无需修改")
                            || lower.contains("设定一致");
                        if is_no_problem {
                            format!("跳过无效报告: {}", desc)
                        } else {
                            // 保存到数据库（尽量关联章节，改稿后可提示复核）
                            let conflict_chapter = args
                                .get("chapterId")
                                .and_then(|v| v.as_str())
                                .map(|value| value.to_string())
                                .or_else(|| chapter_id.map(|value| value.to_string()));
                            let conflict_id = format!("conflict_{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
                            let now = chrono::Utc::now().timestamp();
                            if let Ok(pool) = crate::db::get_pool().await {
                                let _ = sqlx::query(
                                    "INSERT INTO detected_conflicts (id, book_id, description, suggestion, severity, detected_at, is_ignored, chapter_id, needs_review, resolved_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, 0, NULL)"
                                )
                                .bind(&conflict_id)
                                .bind(book_id)
                                .bind(desc)
                                .bind(sug)
                                .bind(sev)
                                .bind(now)
                                .bind(conflict_chapter)
                                .execute(pool)
                                .await;
                            }
                            // 暂存冲突报告，在所有工具调用完成后再发送事件
                            pending_conflicts.push((conflict_id, desc.to_string(), sug.to_string(), sev.to_string()));
                            format!("冲突已记录: {}", desc)
                        }
                    } else if tool_name == "write_chapter" {
                        let ch_id = args.get("chapterId").and_then(|v| v.as_str()).unwrap_or("");
                        let w_desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let instruction = args.get("instruction").and_then(|v| v.as_str()).unwrap_or("");
                        log::info!("[Agent] 触发写作AI: chapterId={}, desc={}", ch_id, w_desc);

                        if ch_id.is_empty() {
                            log::warn!("[Agent] write_chapter 缺少 chapterId，已拒绝");
                            "缺少 chapterId，无法创作。请先确认要写入的章节。".to_string()
                        } else {
                        // 发射开始事件
                        let _ = app.emit("ai-write-start", serde_json::json!({
                            "sessionId": session_id,
                            "bookId": book_id,
                            "chapterId": ch_id,
                            "description": w_desc,
                        }));

                        // 提取上下文（工具返回的章节内容、角色信息等）
                        let mut context = Self::extract_writer_context(&state.messages);

                        // 自动获取当前章大纲（如果有）
                        if !ch_id.is_empty() {
                            if let Ok(coarse) = crate::services::outline_service::get_outline_by_level(
                                book_id.to_string(), None, Some(ch_id.to_string()), "coarse".to_string()
                            ).await {
                                if let Some(outline) = coarse {
                                    context = format!("## 当前章粗纲\n{}\n\n{}", outline.content, context);
                                    log::info!("[Agent] 已注入当前章粗纲");
                                }
                            }
                            if let Ok(fine) = crate::services::outline_service::get_outline_by_level(
                                book_id.to_string(), None, Some(ch_id.to_string()), "fine".to_string()
                            ).await {
                                if let Some(outline) = fine {
                                    context = format!("## 当前章细纲\n{}\n\n{}", outline.content, context);
                                    log::info!("[Agent] 已注入当前章细纲");
                                }
                            }
                        }

                        // 注入上下文里出现的角色卡（不依赖决策 AI 是否查过）
                        let character_notes = Self::build_character_notes(book_id, &context).await;
                        if !character_notes.is_empty() {
                            context = format!("{}\n\n{}", character_notes, context);
                            log::info!("[Agent] 已注入相关角色设定");
                        }

                        // 注入设定账本中与上下文相关的实体（状态/事件）
                        let canon_notes = crate::services::canon_service::build_context_notes(book_id, &context).await;
                        if !canon_notes.is_empty() {
                            context = format!("{}\n\n{}", canon_notes, context);
                            log::info!("[Agent] 已注入设定账本");
                        }

                        // 自动注入上一章结尾，保证衔接自然（续写时最容易断的地方）
                        if !ch_id.is_empty() {
                            if let Some(prev_ending) = Self::previous_chapter_ending(book_id, ch_id).await {
                                context = format!(
                                    "## 上一章结尾（衔接参考）\n{}\n\n{}",
                                    prev_ending, context
                                );
                                log::info!("[Agent] 已注入上一章结尾");
                            }
                        }

                        log::info!("[Agent] 写作上下文长度: {} 字", context.chars().count());

                        // 调用写作AI生成内容
                        let prompt_manager = get_prompt_manager();
                        // 通用风格指南 + 从作者作品中学到的个人风格（如果有）
                        let learned_style = crate::services::style_service::get_style_prompt(Some(book_id))
                            .await
                            .map(|r| r.style_prompt)
                            .unwrap_or_default();
                        let style_guide = if learned_style.trim().is_empty() {
                            prompt_manager.style_guide.to_string()
                        } else {
                            format!(
                                "{}\n\n---\n\n## 作者个人写作风格（从往期作品分析得出，优先级高于上面的通用规范）\n{}",
                                prompt_manager.style_guide,
                                learned_style
                            )
                        };
                        log::info!(
                            "[Agent] 写作风格指南: {} 字（个人风格 {} 字）",
                            style_guide.chars().count(),
                            learned_style.chars().count()
                        );
                        let writer_prompt = prompt_manager.writer_agent
                            .replace("{{STYLE_GUIDE}}", &style_guide)
                            .replace("{{CONTEXT}}", &context);

                        // 把用户原始需求一起带上，避免上下文里的旧需求覆盖本次任务
                        let task = match (w_desc.trim().is_empty(), instruction.trim().is_empty()) {
                            (false, false) => format!("{}（用户原始需求：{}）", w_desc, instruction),
                            (false, true) => w_desc.to_string(),
                            (true, false) => instruction.to_string(),
                            (true, true) => "按上下文继续创作".to_string(),
                        };

                        let writer_messages = vec![
                            ChatMessage::new("system", &writer_prompt),
                            ChatMessage::new("user", &format!("请根据以上上下文进行创作。任务：{}", task)),
                        ];

                        // 正文输出通常远长于对话，避免被 max_tokens 截断
                        let mut writer_config = config.clone();
                        writer_config.max_tokens = writer_config.max_tokens.max(8000);

                        // 流式调用写作AI
                        match Self::stream_writer_ai(app, session_id, writer_messages, writer_config).await {
                            Ok(content) => {
                                let char_count = content.chars().count();
                                log::info!("[Agent] 写作AI完成，内容长度: {} 字", char_count);
                                let _ = app.emit("ai-write-result", serde_json::json!({
                                    "sessionId": session_id,
                                    "bookId": book_id,
                                    "chapterId": ch_id,
                                    "content": content,
                                    "description": w_desc,
                                }));
                                // 返回写作结果摘要给决策AI，让它知道写了什么
                                let summary = if char_count > 200 {
                                    let preview: String = content.chars().take(200).collect();
                                    format!("创作完成（{}字），内容已展示给用户。开头：{}...", char_count, preview)
                                } else {
                                    format!("创作完成（{}字），内容已展示给用户。全文：{}", char_count, content)
                                };
                                summary
                            }
                            Err(e) => {
                                log::error!("[Agent] 写作AI失败: {}", e);
                                format!("写作失败: {}", e)
                            }
                        }
                        }
                    } else if tool_name == "ask_user" {
                        let question = args.get("question").and_then(|v| v.as_str()).unwrap_or("");
                        let context = args.get("context").and_then(|v| v.as_str()).unwrap_or("");
                        let options = args.get("options").and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                            .unwrap_or_default();

                        // 保存到数据库
                        let message_id = format!("ask_user_{}_{}", chrono::Utc::now().timestamp_millis(), rand::random::<u16>());
                        if let Ok(pool) = crate::db::get_pool().await {
                            let _ = sqlx::query(
                                "INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4, 'ask_user', ?5, ?6)"
                            )
                            .bind(&message_id)
                            .bind(session_id)
                            .bind(book_id)
                            .bind("")
                            .bind(serde_json::json!({
                                "question": question,
                                "context": context,
                                "options": options,
                                "handled": false
                            }).to_string())
                            .bind(chrono::Utc::now().timestamp_millis())
                            .execute(pool)
                            .await;
                        }

                        // 发送事件给前端显示
                        let _ = app.emit("ai-ask-user-result", serde_json::json!({
                            "sessionId": session_id,
                            "bookId": book_id,
                            "messageId": message_id,
                            "question": question,
                            "context": context,
                            "options": options,
                        }));
                        format!("询问用户：{}", question)
                    } else {
                        match execute_tool_call(&ToolCall {
                            name: tool_name.clone(),
                            arguments: args.clone(),
                        }, book_id).await {
                            Ok(result) => {
                                // 工具执行成功，清除错误计数
                                state.last_error = None;
                                result
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                log::warn!("[Agent] 工具 {} 执行失败: {}", tool_name, error_msg);

                                // 检查是否是同一种错误
                                let is_same_error = state.last_error.as_ref()
                                    .map(|tracker| tracker.reason == error_msg)
                                    .unwrap_or(false);

                                if is_same_error {
                                    // 同一种错误，计数+1
                                    if let Some(ref mut tracker) = state.last_error {
                                        tracker.count += 1;
                                    }

                                    // 超过3次，停止并返回错误
                                    if state.last_error.as_ref().map(|t| t.count).unwrap_or(0) >= 3 {
                                        log::error!("[Agent] 同一种错误连续出现3次，停止重试: {}", error_msg);
                                        return Err(anyhow::anyhow!("工具 {} 连续3次失败: {}", tool_name, error_msg));
                                    }

                                    format!("工具执行失败（第{}次）: {}。请尝试其他方式完成任务。", 
                                        state.last_error.as_ref().map(|t| t.count).unwrap_or(1), error_msg)
                                } else {
                                    // 不同的错误，重置计数
                                    state.last_error = Some(ErrorTracker {
                                        reason: error_msg.clone(),
                                        count: 1,
                                    });
                                    format!("工具执行失败: {}。请尝试其他方式完成任务。", error_msg)
                                }
                            }
                        }
                    };

                    tool_results.push(format!("工具 {} 返回: {}", tool_name, result));
                    last_tool_name = tool_name.clone();

                    // 将工具结果加入历史（AI上下文用，不存数据库）
                    // 单条结果限制在 8000 字内，并明确告知已截断，避免 AI 误以为看全了
                    let result_chars = result.chars().count();
                    let history_content = if result_chars > 8000 {
                        format!(
                            "{}…（结果已截断，原文共 {} 字；需要更多请用行号范围或更精确的查询）",
                            truncate_chars(&result, 8000),
                            result_chars
                        )
                    } else {
                        result.clone()
                    };
                    state.messages.push(ChatMessage {
                        role: "tool".into(),
                        content: Some(history_content),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                        name: Some(tool_name.clone()),
                    });
                }

                let _ = app.emit("ai-data-changed", serde_json::json!({
                    "toolName": last_tool_name,
                    "bookId": book_id,
                    "sessionId": session_id,
                }));

                // 检查是否包含 ask_user 工具
                let has_ask_user = tool_calls.iter().any(|tc| tc.function.name == "ask_user");
                // 只提交提案/写作的批次属于收尾动作，不需要再反思
                let only_final_actions = tool_calls.iter().all(|tc| {
                    matches!(
                        tc.function.name.as_str(),
                        "propose_edits"
                            | "propose_outlines"
                            | "propose_line_edit"
                            | "save_outline"
                            | "write_chapter"
                    )
                });

                // 持久化工具调用摘要（恢复历史时显示，不含工具返回的长数据）
                if let Err(e) = crate::services::chat_service::save_tool_call_summary(
                    session_id, book_id, &serde_json::to_string(&tool_call_summary).unwrap_or_default(),
                ).await {
                    log::error!("[Agent] 保存工具调用摘要失败: {}", e);
                }

                // 在所有工具调用完成后，发送暂存的冲突报告事件
                for (conflict_id, desc, sug, sev) in pending_conflicts {
                    let _ = app.emit("ai-conflict-result", serde_json::json!({
                        "sessionId": session_id,
                        "bookId": book_id,
                        "conflictId": conflict_id,
                        "description": desc,
                        "suggestion": sug,
                        "severity": sev,
                    }));
                }

                // 如果包含 ask_user 工具，保存状态并暂停等待用户答复
                if has_ask_user {
                    log::info!("[Agent] 检测到 ask_user 工具，暂停等待用户答复");
                    state.waiting_for_user = true;
                    set_session_state(session_id.to_string(), state.clone());

                    // 暂停也算本轮结束：通知前端收起 loading/工具轮状态
                    let _ = app.emit("ai-chat-stream", serde_json::json!({
                        "sessionId": session_id,
                        "chunk": "",
                        "isComplete": true,
                        "isToolCall": false,
                        "isError": false,
                    }));
                    return Ok(());
                }

                // 每批工具执行后反思：判断是否查够、有无矛盾、下一步做什么
                if config.enable_thinking
                    && reflection_count < MAX_REFLECTIONS
                    && !tool_results.is_empty()
                    && !last_tool_name.is_empty()
                    && !only_final_actions
                {
                    reflection_count += 1;

                    if let Some(reflection) = Self::run_reflection(
                        app,
                        session_id,
                        book_id,
                        chapter_id,
                        round + 1,
                        user_message,
                        &tool_results,
                        &config,
                    )
                    .await
                    {
                        log::info!(
                            "[Agent] 第{}轮工具后反思完成，{} 字",
                            round + 1,
                            reflection.chars().count()
                        );

                        // 追加到 system prompt，供下一轮决策参考
                        if let Some(system) = state.messages.first_mut() {
                            if system.role == "system" {
                                let existing = system.content.take().unwrap_or_default();
                                system.content = Some(format!(
                                    "{}\n\n## 第{}轮反思（下一步参考）\n{}",
                                    existing,
                                    round + 1,
                                    reflection
                                ));
                            }
                        }
                    }
                }

                log::info!("[Agent] 工具执行完成，继续决策");
                continue;
            }

            // 有 content → 自然语言回答
            if let Some(content) = &decision.content {
                if !content.is_empty() {
                    Self::emit_direct_response(app, session_id, content).await;
                    if let Err(e) = crate::services::chat_service::save_assistant_message(
                        session_id, book_id, None, content,
                    ).await {
                        log::error!("[Agent] 保存助手消息失败: {}", e);
                    }
                    state.messages.push(ChatMessage::new("assistant", content));
                    set_session_state(session_id.to_string(), state.clone());
                    Self::spawn_background_compress(session_id, state, config);
                    return Ok(());
                }
            }

            // 既无 content 也无 tool_calls → 异常
            log::warn!("[Agent] AI返回了空的content和tool_calls");
            return Err(anyhow::anyhow!("AI 未返回有效响应"));
        }

        set_session_state(session_id.to_string(), state.clone());
        Self::spawn_background_compress(session_id, state, config);
        Err(anyhow::anyhow!("决策轮次超过最大限制"))
    }

    /// 后台触发上下文压缩（不阻塞当前对话）
    fn spawn_background_compress(session_id: &str, state: SessionState, config: AIConfig) {
        let sid = session_id.to_string();
        // 只有 round_count 是 3 的倍数时才需要压缩（下一轮将触发）
        if state.round_count < 3 || state.round_count % 3 != 0 {
            return;
        }
        log::info!("[Agent] 触发后台上下文压缩，round={}", state.round_count);

        tauri::async_runtime::spawn(async move {
            log::info!("[Agent] 后台压缩开始，消息数: {}", state.messages.len());
            let start = std::time::Instant::now();

            match Self::do_compress(state, config).await {
                Ok(compressed) => {
                    let elapsed = start.elapsed();
                    log::info!("[Agent] 后台压缩完成，耗时 {:?}，结果: {} 字",
                        elapsed, compressed.len());

                    // 更新全局状态
                    if let Ok(mut guard) = SESSION_STATES.lock() {
                        if let Some(states) = guard.as_mut() {
                            if let Some(s) = states.get_mut(&sid) {
                                s.compressed_context = compressed;
                                log::info!("[Agent] 压缩结果已写入会话 {}", &sid[..sid.len().min(8)]);
                            } else {
                                log::warn!("[Agent] 会话 {} 已不存在，压缩结果丢弃", &sid[..sid.len().min(8)]);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[Agent] 后台压缩失败: {}", e);
                }
            }
        });
    }

    /// 执行压缩（纯函数，返回压缩后的上下文字符串）
    async fn do_compress(mut state: SessionState, config: AIConfig) -> anyhow::Result<String> {
        Self::compress_context(&mut state, config).await?;
        Ok(state.compressed_context)
    }

    /// 压缩上下文
    async fn compress_context(state: &mut SessionState, config: AIConfig) -> anyhow::Result<()> {
        let prompt_manager = get_prompt_manager();

        let history: String = state.messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                let role_label = match m.role.as_str() {
                    "user" => "用户",
                    "assistant" => {
                        if m.tool_calls.as_ref().map_or(false, |t| !t.is_empty()) {
                            let names: Vec<_> = m.tool_calls.as_ref().unwrap().iter()
                                .map(|t| t.function.name.as_str())
                                .collect();
                            return format!("AI调用了工具: {}", names.join(", "));
                        }
                        "AI"
                    }
                    "tool" => {
                        let content = m.content.as_deref().unwrap_or("");
                        // 按字符截断，避免中文被切在 UTF-8 字节边界上 panic
                        let char_count = content.chars().count();
                        let truncated: String = if char_count > 300 {
                            let head: String = content.chars().take(300).collect();
                            format!("{}...（已截断，共{}字）", head, char_count)
                        } else {
                            content.to_string()
                        };
                        return format!("工具返回: {}", truncated);
                    }
                    _ => &m.role,
                };
                format!("{}: {}", role_label, m.content.as_deref().unwrap_or(""))
            })
            .collect::<Vec<_>>()
            .join("\n");

        let compress_prompt = prompt_manager.compress_agent.clone();

        let messages = vec![
            ChatMessage::new("system", &compress_prompt),
            ChatMessage::new("user", &format!("请压缩以下对话历史：\n\n{}", history)),
        ];

        let decision = send_ai_message(messages, config, None).await?;
        let response = decision.content.unwrap_or_default();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
            let summary = json["summary"].as_str().unwrap_or("").to_string();
            let key_points = json["key_points"].as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(|s| format!("- {}", s)))
                    .collect::<Vec<_>>()
                    .join("\n"))
                .unwrap_or_default();
            let ctx = json["context"].as_str().unwrap_or("").to_string();

            let mut compressed = String::from("【历史对话摘要】\n");
            if !summary.is_empty() {
                compressed.push_str(&format!("总结：{}\n", summary));
            }
            if !key_points.is_empty() {
                compressed.push_str(&format!("关键信息：\n{}\n", key_points));
            }
            if !ctx.is_empty() {
                compressed.push_str(&format!("待办上下文：{}\n", ctx));
            }
            state.compressed_context = compressed;
        } else {
            state.compressed_context = format!("【历史对话摘要】\n总结：{}\n", response);
        }

        let system_msg = state.messages.iter()
            .find(|m| m.role == "system")
            .cloned();

        state.messages.clear();
        if let Some(sys) = system_msg {
            state.messages.push(sys);
        }

        if !state.compressed_context.is_empty() {
            state.messages.push(ChatMessage::new("user", &state.compressed_context));
        }

        Ok(())
    }

    /// 流式输出AI响应
    async fn stream_ai_response(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        messages: Vec<ChatMessage>,
        config: AIConfig,
        _is_polish: bool,
    ) -> anyhow::Result<()> {
        let response = send_ai_message_stream(messages, config).await?;
        let mut stream = response.bytes_stream();
        let mut full_content = String::new();
        // 网络分块可能把一行 data: 从中间切断，必须跨块缓冲
        let mut sse_buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    for content in crate::services::ai_service::extract_sse_deltas(&mut sse_buffer, &chunk) {
                        full_content.push_str(&content);

                        let event = StreamEvent {
                            session_id: session_id.to_string(),
                            chunk: content,
                            is_complete: false,
                            is_tool_call: false,
                            tool_name: None,
                            tool_display_name: None,
                            tool_params: None,
                            is_error: false,
                            error_message: None,
                        };
                        let _ = app.emit("ai-chat-stream", event);
                    }
                }
                Err(e) => {
                    log::error!("[Agent] 流读取错误: {}", e);
                    let error_msg = format!("AI 流式响应失败：{}", e);
                    let error_event = StreamEvent {
                        session_id: session_id.to_string(),
                        chunk: String::new(),
                        is_complete: true,
                        is_tool_call: false,
                        tool_name: None,
                        tool_display_name: None,
                        tool_params: None,
                        is_error: true,
                        error_message: Some(error_msg),
                    };
                    let _ = app.emit("ai-chat-stream", error_event);
                    break;
                }
            }
        }

        // 冲刷缓冲区里最后一个没有空行结尾的事件
        for content in crate::services::ai_service::extract_sse_deltas(&mut sse_buffer, b"\n\n") {
            full_content.push_str(&content);
            let event = StreamEvent {
                session_id: session_id.to_string(),
                chunk: content,
                is_complete: false,
                is_tool_call: false,
                tool_name: None,
                tool_display_name: None,
                tool_params: None,
                is_error: false,
                error_message: None,
            };
            let _ = app.emit("ai-chat-stream", event);
        }

        let complete_event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: String::new(),
            is_complete: true,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
            is_error: false,
            error_message: None,
        };
        let _ = app.emit("ai-chat-stream", complete_event);

        if let Err(e) = crate::services::chat_service::save_assistant_message(
            session_id,
            book_id,
            None,
            &full_content,
        ).await {
            log::error!("[Agent] 保存助手消息失败: {}", e);
        }

        Ok(())
    }

    /// 思考模式：生成一份简短的内部计划，并推送给前端展示
    async fn run_thinking(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        chapter_id: Option<&str>,
        user_message: &str,
        messages: &[ChatMessage],
        story_bible: &str,
        focus_note: &str,
        config: &AIConfig,
    ) -> Option<String> {
        let prompt_manager = get_prompt_manager();

        let chapter_title = match chapter_id {
            Some(chapter_id) => match crate::db::get_pool().await {
                Ok(pool) => {
                    sqlx::query_scalar::<_, String>("SELECT title FROM chapters WHERE id = ?1")
                        .bind(chapter_id)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten()
                }
                Err(_) => None,
            },
            None => None,
        };

        let recent: Vec<String> = messages
            .iter()
            .filter(|msg| msg.role == "user" || msg.role == "assistant")
            .filter_map(|msg| {
                msg.content.as_ref().map(|content| {
                    let who = if msg.role == "user" { "用户" } else { "助手" };
                    format!("{}：{}", who, truncate_chars(content, 400))
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let bible_excerpt = if story_bible.trim().is_empty() {
            "（暂无）".to_string()
        } else {
            truncate_chars(story_bible, 1600)
        };

        let user_prompt = format!(
            "## 当前章节\n{}\n\n## 当前编辑目标\n{}\n\n## 故事圣经（节选）\n{}\n\n## 最近对话\n{}\n\n## 用户请求\n{}",
            chapter_title
                .map(|title| format!("「{}」", title))
                .unwrap_or_else(|| "未指定章节".to_string()),
            if focus_note.trim().is_empty() {
                "（未指定，按用户请求判断）".to_string()
            } else {
                focus_note.to_string()
            },
            bible_excerpt,
            if recent.is_empty() {
                "（无）".to_string()
            } else {
                recent.join("\n")
            },
            user_message
        );

        Self::stream_thinking(
            app,
            session_id,
            book_id,
            chapter_id,
            "plan",
            "思考",
            &prompt_manager.think_agent,
            &user_prompt,
            2200,
            config,
        )
        .await
    }

    /// 流式思考（计划 / 每轮反思共用），返回完整文本
    #[allow(clippy::too_many_arguments)]
    async fn stream_thinking(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        chapter_id: Option<&str>,
        block: &str,
        label: &str,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: i32,
        config: &AIConfig,
    ) -> Option<String> {
        let mut thinking_config = config.clone();
        thinking_config.temperature = 0.35;
        thinking_config.max_tokens = thinking_config.max_tokens.max(max_tokens);

        let thinking_messages = vec![
            ChatMessage::new("system", system_prompt),
            ChatMessage::new("user", user_prompt),
        ];

        let response = match send_ai_message_stream(thinking_messages, thinking_config).await {
            Ok(response) => response,
            Err(e) => {
                log::warn!("[Agent] 思考请求失败（{}，已跳过）: {}", label, e);
                return None;
            }
        };

        let _ = app.emit(
            "ai-thinking",
            serde_json::json!({
                "sessionId": session_id,
                "bookId": book_id,
                "phase": "start",
                "block": block,
                "label": label,
            }),
        );

        let mut stream = response.bytes_stream();
        let mut sse_buffer = String::new();
        let mut text = String::new();
        let mut stream_error: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    for delta in
                        crate::services::ai_service::extract_sse_deltas_full(&mut sse_buffer, &chunk)
                    {
                        let piece = if !delta.reasoning.is_empty() {
                            delta.reasoning
                        } else {
                            delta.content
                        };
                        if piece.is_empty() {
                            continue;
                        }

                        text.push_str(&piece);
                        let _ = app.emit(
                            "ai-thinking",
                            serde_json::json!({
                                "sessionId": session_id,
                                "bookId": book_id,
                                "phase": "chunk",
                                "block": block,
                                "content": piece,
                            }),
                        );
                    }
                }
                Err(e) => {
                    stream_error = Some(e.to_string());
                    break;
                }
            }
        }

        // 冲刷最后一个没有空行结尾的事件
        for delta in
            crate::services::ai_service::extract_sse_deltas_full(&mut sse_buffer, b"\n\n")
        {
            let piece = if !delta.reasoning.is_empty() {
                delta.reasoning
            } else {
                delta.content
            };
            if piece.is_empty() {
                continue;
            }
            text.push_str(&piece);
            let _ = app.emit(
                "ai-thinking",
                serde_json::json!({
                    "sessionId": session_id,
                    "bookId": book_id,
                    "phase": "chunk",
                    "block": block,
                    "content": piece,
                }),
            );
        }

        let text = text.trim().to_string();

        let _ = app.emit(
            "ai-thinking",
            serde_json::json!({
                "sessionId": session_id,
                "bookId": book_id,
                "phase": "done",
                "block": block,
                "content": text,
            }),
        );

        if let Some(error) = stream_error {
            log::warn!(
                "[Agent] 思考流中断（{}）: {}（保留 {} 字）",
                label,
                error,
                text.chars().count()
            );
        }

        // 落库，刷新/切换会话后还能回看；反思带上轮次标签
        if !text.is_empty() {
            let stored = if label == "思考" {
                text.clone()
            } else {
                format!("{}\n{}", label, text)
            };
            if let Err(e) = crate::services::chat_service::save_thinking_message(
                session_id,
                book_id,
                chapter_id,
                &stored,
            )
            .await
            {
                log::warn!("[Agent] 保存思考过程失败: {}", e);
            }
        }

        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// 每批工具执行后的反思（判断是否查够、有无矛盾、下一步做什么）
    async fn run_reflection(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        chapter_id: Option<&str>,
        round: usize,
        user_message: &str,
        tool_results: &[String],
        config: &AIConfig,
    ) -> Option<String> {
        let prompt_manager = get_prompt_manager();

        let results_text = tool_results
            .iter()
            .take(6)
            .map(|item| truncate_chars(item, 1500))
            .collect::<Vec<_>>()
            .join("\n\n");

        let user_prompt = format!(
            "## 用户请求\n{}\n\n## 本轮工具结果\n{}",
            user_message,
            if results_text.is_empty() {
                "（无）".to_string()
            } else {
                results_text
            },
        );

        Self::stream_thinking(
            app,
            session_id,
            book_id,
            chapter_id,
            &format!("round-{}", round),
            &format!("第{}轮反思", round),
            &prompt_manager.think_reflect,
            &user_prompt,
            1200,
            config,
        )
        .await
    }

    /// 生成「用户当前正在编辑的大纲」说明，让 AI 知道“这个大纲/这里”指什么
    async fn describe_outline_focus(
        book_id: &str,
        target: Option<&serde_json::Value>,
    ) -> String {
        let Some(target) = target else {
            return String::new();
        };

        let level = target.get("level").and_then(|v| v.as_str()).unwrap_or("");
        let target_id = target.get("targetId").and_then(|v| v.as_str()).unwrap_or("");

        let Ok(pool) = crate::db::get_pool().await else {
            return String::new();
        };

        let description = match level {
            "book" => format!("书籍级大纲（bookId={}）", book_id),
            "volume" => {
                let title: Option<String> =
                    sqlx::query_scalar("SELECT title FROM volumes WHERE id = ?1")
                        .bind(target_id)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten();
                match title {
                    Some(title) => format!("卷「{}」的大纲（volumeId={}）", title, target_id),
                    None => return String::new(),
                }
            }
            "chapter" => {
                let title: Option<String> =
                    sqlx::query_scalar("SELECT title FROM chapters WHERE id = ?1")
                        .bind(target_id)
                        .fetch_optional(pool)
                        .await
                        .ok()
                        .flatten();
                match title {
                    Some(title) => format!("章节「{}」的大纲（chapterId={}）", title, target_id),
                    None => return String::new(),
                }
            }
            _ => return String::new(),
        };

        format!(
            "## 用户当前正在编辑\n{}\n用户说“这个大纲”“这里”时默认指它；修改时请用 propose_outlines，并带上上面的 id 与对应 level。",
            description
        )
    }

    /// 获取章节列表字符串（按卷分组，只查元数据，不读取正文）
    ///
    /// 只展示「当前章节所在卷」：卷内章节过多时取当前章节前后窗口，
    /// 其余卷用一行提示概括，避免 100 章后把整本书的目录每轮都塞进 prompt。
    async fn get_chapter_list(book_id: &str, focus_chapter_id: Option<&str>) -> anyhow::Result<String> {
        use sqlx::Row;

        let pool = crate::db::get_pool().await?;

        let volume_rows = sqlx::query(
            r#"SELECT id, title FROM volumes WHERE book_id = ?1 ORDER BY "order""#,
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        if volume_rows.is_empty() {
            return Ok("当前书籍没有卷和章节".to_string());
        }

        let chapter_rows = sqlx::query(
            r#"SELECT id, volume_id, title, "order" FROM chapters WHERE book_id = ?1 ORDER BY "order""#,
        )
        .bind(book_id)
        .fetch_all(pool)
        .await?;

        if chapter_rows.is_empty() {
            return Ok("当前书籍没有章节".to_string());
        }

        // 当前章节所属卷
        let focus_volume: Option<String> = match focus_chapter_id {
            Some(cid) => sqlx::query_scalar::<_, String>("SELECT volume_id FROM chapters WHERE id = ?1")
                .bind(cid)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten(),
            None => None,
        };

        // 收集卷与卷内章节
        let mut volumes: Vec<(String, String)> = Vec::new();
        for row in &volume_rows {
            let id: String = row.try_get("id")?;
            let title: String = row.try_get("title")?;
            volumes.push((id, title));
        }

        let mut chapters: Vec<(String, String, String, i32)> = Vec::new(); // (volume_id, id, title, order)
        for row in &chapter_rows {
            let volume_id: String = row.try_get("volume_id").unwrap_or_default();
            let id: String = row.try_get("id")?;
            let title: String = row.try_get("title")?;
            let order: i32 = row.try_get("order").unwrap_or(0);
            chapters.push((volume_id, id, title, order));
        }

        let target_volume = focus_volume
            .filter(|v| volumes.iter().any(|(id, _)| id == v))
            .or_else(|| volumes.last().map(|(id, _)| id.clone()));

        let Some(target_volume) = target_volume else {
            return Ok("当前书籍没有章节".to_string());
        };

        let target_volume_title = volumes
            .iter()
            .find(|(id, _)| id.as_str() == target_volume.as_str())
            .map(|(_, title)| title.clone())
            .unwrap_or_else(|| "当前卷".to_string());

        let mut volume_chapters: Vec<(String, String, i32)> = chapters
            .iter()
            .filter(|(vid, _, _, _)| vid == &target_volume)
            .map(|(_, id, title, order)| (id.clone(), title.clone(), *order))
            .collect();

        let total_in_volume = volume_chapters.len();
        let mut window_note = String::new();

        if total_in_volume > CHAPTER_LIST_WINDOW {
            let focus_order: Option<i32> = match focus_chapter_id {
                Some(cid) => chapters
                    .iter()
                    .find(|(_, id, _, _)| id.as_str() == cid)
                    .map(|(_, _, _, order)| *order),
                None => None,
            };

            if let Some(order) = focus_order {
                let half = CHAPTER_LIST_WINDOW / 2;
                let start = order.saturating_sub(half as i32);
                let end = start + CHAPTER_LIST_WINDOW as i32;
                volume_chapters.retain(|(_, _, o)| *o >= start && *o < end);
                window_note = format!(
                    "（本卷共 {} 章，仅显示当前章节前后各约 {} 章；完整目录可用 list_all_chapters 工具查询）\n",
                    total_in_volume,
                    half
                );
            } else {
                // 没有当前章节时展示本卷最后一页
                let skip = total_in_volume.saturating_sub(CHAPTER_LIST_WINDOW);
                volume_chapters = volume_chapters.into_iter().skip(skip).collect();
                window_note = format!(
                    "（本卷共 {} 章，仅显示最近 {} 章；完整目录可用 list_all_chapters 工具查询）\n",
                    total_in_volume, CHAPTER_LIST_WINDOW
                );
            }
        }

        let mut result = String::from("当前书籍的章节列表（按卷分组，可直接使用章节ID）：\n");
        result.push_str(&format!("\n## {}\n", target_volume_title));
        if !window_note.is_empty() {
            result.push_str(&window_note);
        }
        for (id, title, _) in &volume_chapters {
            result.push_str(&format!("{} (ID: {})\n", title, id));
        }

        let other_volumes = volumes.len().saturating_sub(1);
        if other_volumes > 0 {
            result.push_str(&format!(
                "\n（另有 {} 个卷未展开；需要其它卷的章节时用 list_all_chapters 或 list_chapters_in_volume 工具查询）\n",
                other_volumes
            ));
        }

        Ok(result)
    }

    /// 从消息历史中提取写作上下文
    fn extract_writer_context(messages: &[ChatMessage]) -> String {
        let mut user_parts: Vec<String> = Vec::new();
        let mut guidance_parts: Vec<String> = Vec::new();
        let mut tool_results: Vec<String> = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "user" => {
                    if let Some(content) = &msg.content {
                        // 跳过压缩上下文和工具返回
                        if !content.starts_with("工具返回结果") && !content.starts_with("【历史对话摘要】") {
                            user_parts.push(truncate_chars(content, 2000));
                        }
                    }
                }
                "assistant" => {
                    // 提取决策AI的分析和创作指导
                    if let Some(content) = &msg.content {
                        // 跳过纯工具调用消息（有tool_calls的assistant消息）
                        if msg.tool_calls.is_none() && !content.is_empty() {
                            guidance_parts.push(truncate_chars(content, 2000));
                        }
                    }
                }
                "tool" => {
                    if let Some(content) = &msg.content {
                        // 单条工具结果截断到 4000 字符，避免整章正文把 writer 上下文撑爆
                        tool_results.push(truncate_chars(content, 4000));
                    }
                }
                _ => {}
            }
        }

        let mut ctx = String::new();

        // 只取最近 3 轮用户需求 / 创作指导，避免旧需求覆盖本次任务
        for content in user_parts.iter().rev().take(3).rev() {
            ctx.push_str(&format!("## 用户需求\n{}\n\n", content));
        }
        for content in guidance_parts.iter().rev().take(3).rev() {
            ctx.push_str(&format!("## 创作指导\n{}\n\n", content));
        }

        // 参考资料只保留最近 6 条、总量不超过约 12000 字：
        // 越早的工具结果越可能是已被决策 AI 消化过的中间产物
        let start = tool_results.len().saturating_sub(6);
        let recent_tools = &tool_results[start..];

        if !recent_tools.is_empty() {
            ctx.push_str("## 查询到的参考资料\n");
            let mut used_chars = 0usize;
            for result in recent_tools {
                if used_chars >= 12000 {
                    ctx.push_str("（更多资料已省略）\n");
                    break;
                }
                used_chars += result.chars().count();
                ctx.push_str(&format!("### 资料\n{}\n\n", result));
            }
        }

        if ctx.is_empty() {
            ctx.push_str("无额外上下文");
        }
        ctx
    }

    /// 从角色卡里挑出本次写作上下文里出现的角色，生成紧凑的设定说明
    ///
    /// 写作最容易「吃书」的地方就是人物：性格、外貌、称呼、关系。
    /// 这里不依赖决策 AI 是否查过角色卡，直接按名字/别名匹配后注入。
    async fn build_character_notes(book_id: &str, context: &str) -> String {
        let cards = match crate::services::character_service::list_character_cards(book_id.to_string()).await {
            Ok(cards) => cards,
            Err(e) => {
                log::warn!("[Agent] 读取角色卡失败: {}", e);
                return String::new();
            }
        };

        let mut notes = String::new();
        let mut matched = 0usize;

        for card in cards {
            let aliases: Vec<String> = serde_json::from_str(&card.aliases).unwrap_or_default();
            let mut names = vec![card.name.clone()];
            names.extend(aliases.iter().filter(|a| !a.trim().is_empty()).cloned());

            let hit = names.iter().any(|name| !name.is_empty() && context.contains(name.as_str()));
            if !hit {
                continue;
            }

            let mut parts: Vec<String> = Vec::new();

            if !card.gender.is_empty() || !card.age.is_empty() {
                let age = if card.age.is_empty() { String::new() } else { format!("，{}", card.age) };
                parts.push(format!("{}{}", card.gender, age));
            }
            if !card.personality.is_empty() {
                parts.push(format!("性格：{}", card.personality));
            }
            if !card.appearance.is_empty() {
                parts.push(format!("外貌：{}", card.appearance));
            }
            if !card.background.is_empty() {
                parts.push(format!("背景：{}", card.background));
            }
            if !card.goals.is_empty() {
                parts.push(format!("目标：{}", card.goals));
            }
            if !aliases.is_empty() {
                parts.push(format!("别名：{}", aliases.join("、")));
            }
            if !card.notes.is_empty() {
                parts.push(format!("备注：{}", card.notes));
            }

            // 关系是 JSON 数组，按 snake_case / camelCase 兼容读取
            if let Ok(relations) = serde_json::from_str::<Vec<serde_json::Value>>(&card.relationships) {
                let relation_text = relations
                    .iter()
                    .filter_map(|relation| {
                        let name = relation
                            .get("target_name")
                            .and_then(|v| v.as_str())
                            .or_else(|| relation.get("targetName").and_then(|v| v.as_str()))?;
                        if name.is_empty() {
                            return None;
                        }
                        let kind = relation.get("relationship").and_then(|v| v.as_str()).unwrap_or("");
                        let desc = relation.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        Some(if desc.is_empty() {
                            format!("{}（{}）", name, kind)
                        } else {
                            format!("{}（{}：{}）", name, kind, desc)
                        })
                    })
                    .collect::<Vec<_>>()
                    .join("、");
                if !relation_text.is_empty() {
                    parts.push(format!("关系：{}", relation_text));
                }
            }

            matched += 1;
            notes.push_str(&format!("- {}：{}\n", card.name, parts.join("；")));
        }

        if matched == 0 {
            return String::new();
        }

        format!(
            "## 相关角色设定（必须遵守，不得改动姓名/性格/关系）\n{}",
            notes
        )
    }

    /// 取「上一章」的结尾约 800 字，作为续写衔接参考
    ///
    /// 章节 order 是按卷计算的，跨卷不可比，这里按创建时间取前一章。
    async fn previous_chapter_ending(book_id: &str, chapter_id: &str) -> Option<String> {
        let pool = crate::db::get_pool().await.ok()?;

        let created_at: Option<i64> = sqlx::query_scalar(
            "SELECT created_at FROM chapters WHERE id = ?1 AND book_id = ?2",
        )
        .bind(chapter_id)
        .bind(book_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        let created_at = created_at?;

        let previous_id: Option<String> = sqlx::query_scalar(
            r#"SELECT id FROM chapters
               WHERE book_id = ?1 AND created_at < ?2
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(book_id)
        .bind(created_at)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        let previous_id = previous_id?;

        let content = crate::services::book_service::load_chapter_content(previous_id)
            .await
            .ok()?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }

        let chars: Vec<char> = trimmed.chars().collect();
        let start = chars.len().saturating_sub(800);
        Some(chars[start..].iter().collect())
    }

    /// 流式调用写作AI，发射 ai-write-chunk 事件到前端
    async fn stream_writer_ai(
        app: &AppHandle,
        session_id: &str,
        messages: Vec<ChatMessage>,
        config: AIConfig,
    ) -> anyhow::Result<String> {
        let response = send_ai_message_stream(messages, config).await?;
        let mut stream = response.bytes_stream();
        let mut full_content = String::new();
        // 网络分块可能把一行 data: 从中间切断，必须跨块缓冲
        let mut sse_buffer = String::new();
        let mut stream_error: Option<String> = None;

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    for content in crate::services::ai_service::extract_sse_deltas(&mut sse_buffer, &chunk) {
                        full_content.push_str(&content);
                        let _ = app.emit("ai-write-chunk", serde_json::json!({
                            "sessionId": session_id,
                            "chunk": content,
                        }));
                    }
                }
                Err(e) => {
                    log::error!("[Writer] 流读取错误: {}", e);
                    stream_error = Some(e.to_string());
                    break;
                }
            }
        }

        // 冲刷缓冲区里最后一个没有空行结尾的事件
        for content in crate::services::ai_service::extract_sse_deltas(&mut sse_buffer, b"\n\n") {
            full_content.push_str(&content);
            let _ = app.emit("ai-write-chunk", serde_json::json!({
                "sessionId": session_id,
                "chunk": content,
            }));
        }

        if let Some(error) = stream_error {
            if full_content.trim().is_empty() {
                return Err(anyhow::anyhow!("写作流式响应失败：{}", error));
            }
            // 有部分内容时保留，但明确告知前端本次不完整
            log::warn!(
                "[Writer] 流中断，保留部分内容（{} 字）: {}",
                full_content.chars().count(),
                error
            );
        }

        Ok(full_content)
    }

    /// 生成「批量正文修改」提案：逐条校验能否定位，落库并推送给前端
    ///
    /// 与旧版逐条 propose_line_edit 的区别：
    /// - 一次可以带多条修改，前端一张卡片全选/一键应用
    /// - 后端先用当前正文验证每条 anchor，避免用户点了才发现改不了
    /// - 支持整章替换（replace_all）
    async fn propose_edits(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        chapter_id: &str,
        summary: &str,
        edits: Vec<crate::services::edit_proposal_service::EditItemArgs>,
        replace_all: Option<String>,
    ) -> anyhow::Result<String> {
        if chapter_id.is_empty() {
            return Ok("缺少 chapterId，无法生成修改提案。请先确认目标章节。".to_string());
        }

        let content = crate::services::book_service::load_chapter_content(chapter_id.to_string())
            .await
            .unwrap_or_default();

        let replace_all = replace_all.filter(|s| !s.trim().is_empty());

        let previews = if replace_all.is_none() {
            crate::services::edit_proposal_service::preview_edits(&content, &edits)
        } else {
            Vec::new()
        };

        let mut edits_json = Vec::new();
        for (index, edit) in edits.iter().enumerate() {
            let preview = previews.get(index);
            edits_json.push(serde_json::json!({
                "lineNumber": edit.line_number.unwrap_or(0),
                "originalText": edit.original_text,
                "newText": edit.new_text,
                "reason": edit.reason,
                "matched": preview.map(|p| p.matched).unwrap_or(false),
                "before": preview.map(|p| p.before.clone()).unwrap_or_default(),
                "message": preview.map(|p| p.message.clone()).unwrap_or_default(),
            }));
        }

        let matched = edits_json
            .iter()
            .filter(|item| item["matched"] == serde_json::json!(true))
            .count();

        let message_id = format!(
            "edit_proposal_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            rand::random::<u16>()
        );

        let payload = serde_json::json!({
            "proposalId": message_id,
            "chapterId": chapter_id,
            "summary": summary,
            "replaceAll": replace_all.clone(),
            "content": replace_all.clone().unwrap_or_default(),
            "edits": edits_json,
            "status": "pending",
        });

        if let Ok(pool) = crate::db::get_pool().await {
            let _ = sqlx::query(
                "INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4, 'edit_proposal', ?5, ?6)"
            )
            .bind(&message_id)
            .bind(session_id)
            .bind(book_id)
            .bind(chapter_id)
            .bind(payload.to_string())
            .bind(chrono::Utc::now().timestamp_millis())
            .execute(pool)
            .await;
        }

        let _ = app.emit("ai-edit-proposal", serde_json::json!({
            "sessionId": session_id,
            "bookId": book_id,
            "messageId": message_id,
            "proposal": payload,
        }));

        if replace_all.is_some() {
            Ok("已生成整章替换提案，等待用户确认。".to_string())
        } else {
            Ok(format!(
                "已生成修改提案：共 {} 处，其中 {} 处可直接定位，已展示给用户勾选确认。不要重复提交同样的修改。",
                edits.len(),
                matched
            ))
        }
    }

    /// 生成「批量大纲修改」提案（支持书籍/卷/章节三级）
    async fn propose_outlines(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        summary: &str,
        outlines: Vec<crate::services::outline_service::OutlineChange>,
    ) -> anyhow::Result<String> {
        if outlines.is_empty() {
            return Ok("没有可保存的大纲内容。".to_string());
        }

        let items: Vec<serde_json::Value> = outlines
            .iter()
            .map(|change| {
                serde_json::json!({
                    "level": change.level,
                    "volumeId": change.volume_id.clone().unwrap_or_default(),
                    "chapterId": change.chapter_id.clone().unwrap_or_default(),
                    "outlineType": change.outline_type,
                    "content": change.content,
                    "label": change.label(),
                })
            })
            .collect();

        let message_id = format!(
            "outline_proposal_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            rand::random::<u16>()
        );

        let payload = serde_json::json!({
            "proposalId": message_id,
            "summary": summary,
            "outlines": items,
            "status": "pending",
        });

        if let Ok(pool) = crate::db::get_pool().await {
            let _ = sqlx::query(
                "INSERT INTO chat_messages (id, session_id, book_id, chapter_id, role, content, timestamp) VALUES (?1, ?2, ?3, NULL, 'outline_proposal', ?4, ?5)"
            )
            .bind(&message_id)
            .bind(session_id)
            .bind(book_id)
            .bind(payload.to_string())
            .bind(chrono::Utc::now().timestamp_millis())
            .execute(pool)
            .await;
        }

        let _ = app.emit("ai-outline-proposal", serde_json::json!({
            "sessionId": session_id,
            "bookId": book_id,
            "messageId": message_id,
            "proposal": payload,
        }));

        Ok(format!(
            "已生成大纲提案（{} 项），已展示给用户一次性确认保存。",
            outlines.len()
        ))
    }

    /// 发送直接响应
    async fn emit_direct_response(app: &AppHandle, session_id: &str, response: &str) {
        let event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: response.to_string(),
            is_complete: false,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
            is_error: false,
            error_message: None,
        };
        let _ = app.emit("ai-chat-stream", event);

        let complete_event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: String::new(),
            is_complete: true,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
            is_error: false,
            error_message: None,
        };
        let _ = app.emit("ai-chat-stream", complete_event);
    }

    /// 发送工具调用事件
    async fn emit_tool_event(app: &AppHandle, session_id: &str, tool_name: &str, tool_params: Option<serde_json::Value>) {
        let display_name = match tool_name {
            "get_book_info" => "获取书籍信息",
            "list_all_chapters" => "获取章节列表",
            "query_chapter_summary" => "查询章节摘要",
            "search_chapter_content" => "搜索章节内容",
            "query_chapter_content" => "查询章节内容",
            "list_character_cards" => "获取角色列表",
            "get_character_card" => "获取角色详情",
            "get_outline" => "获取大纲",
            "create_character_card" => "创建角色卡",
            "update_character_card" => "更新角色卡",
            "save_outline" => "生成大纲",
            "propose_outlines" => "提交大纲提案",
            "propose_edits" => "提交修改提案",
            "report_conflict" => "报告设定冲突",
            "write_chapter" => "创作正文",
            "learn_writing_style" => "学习写作风格",
            "propose_line_edit" => "提议行编辑",
            "get_volume_outline" => "获取卷大纲",
            "create_chapter" => "创建章节",
            "ask_user" => "询问用户",
            "get_story_memory" => "获取故事记忆",
            "get_canon" => "查询设定账本",
            "search_canon" => "检索设定账本",
            "get_canon_timeline" => "查询设定时间线",
            "get_character_timeline" => "获取角色时间线",
            "list_chapters_in_volume" => "获取卷内章节",
            _ => tool_name,
        };

        let event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: String::new(),
            is_complete: false,
            is_tool_call: true,
            tool_name: Some(tool_name.to_string()),
            tool_display_name: Some(display_name.to_string()),
            tool_params,
            is_error: false,
            error_message: None,
        };
        let _ = app.emit("ai-chat-stream", event);
    }

    /// 发送阶段事件
    async fn emit_phase_event(app: &AppHandle, session_id: &str, phase: &str) {
        log::info!("[Agent] 进入阶段: {}", phase);
        let message = match phase {
            "intent" => "正在分析意图...",
            "tool" => "正在查询数据...",
            "writing" => "正在创作...",
            "outlining" => "正在生成大纲...",
            "polishing" => "正在润色...",
            _ => "",
        };
        let event = serde_json::json!({
            "type": "phase_start",
            "sessionId": session_id,
            "phase": phase,
            "message": message,
        });
        let _ = app.emit("ai-agent-phase", event);
    }
}

use crate::models::{AIConfig, ChatMessage, StreamEvent, ToolCall, ToolDef, FunctionDef};
use crate::services::ai_service::{send_ai_message, send_ai_message_stream};
use crate::services::prompt_service::get_prompt_manager;
use crate::services::tool_call_service::execute_tool_call;
use futures::StreamExt;
use tauri::{AppHandle, Emitter};
use std::sync::Mutex;
use std::collections::HashMap;

/// 会话状态
#[derive(Debug, Clone)]
struct SessionState {
    messages: Vec<ChatMessage>,
    round_count: u32,
    compressed_context: String,
}

/// 全局会话状态管理
static SESSION_STATES: Mutex<Option<HashMap<String, SessionState>>> = Mutex::new(None);

fn get_session_state(session_id: &str) -> Option<SessionState> {
    let states = SESSION_STATES.lock().ok()?;
    states.as_ref()?.get(session_id).cloned()
}

fn set_session_state(session_id: String, state: SessionState) {
    if let Ok(mut guard) = SESSION_STATES.lock() {
        if guard.is_none() {
            *guard = Some(HashMap::new());
        }
        if let Some(states) = guard.as_mut() {
            states.insert(session_id, state);
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
                name: "list_all_chapters".into(),
                description: "获取当前书籍的所有章节列表（含章节ID和标题）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["bookId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "query_chapter_summary".into(),
                description: "获取指定章节的摘要信息（短摘要+长摘要+标签+角色+事件）".into(),
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
                description: "全文搜索。默认普通匹配，regex=true启用正则表达式。结果太多时考虑用更精确的关键词重新搜索".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chapterId": {"type": "string", "description": "章节ID（可选，不指定则搜索全书）"},
                        "keyword": {"type": "string", "description": "搜索关键字或正则表达式"},
                        "bookId": {"type": "string", "description": "书籍ID（搜索全书时必填）"},
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
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["bookId"]
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
                        "chapterId": {"type": "string", "description": "章节ID"},
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["chapterId", "bookId"]
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
                        "bookId": {"type": "string", "description": "书籍ID"},
                        "name": {"type": "string", "description": "角色名"},
                        "gender": {"type": "string", "description": "性别"},
                        "age": {"type": "string", "description": "年龄"},
                        "appearance": {"type": "string", "description": "外貌描述"},
                        "personality": {"type": "string", "description": "性格"},
                        "background": {"type": "string", "description": "背景故事"},
                        "goals": {"type": "string", "description": "目标"},
                        "notes": {"type": "string", "description": "备注"}
                    },
                    "required": ["bookId", "name"]
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
                name: "save_outline".into(),
                description: "保存大纲（粗纲或细纲）到指定章节".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"},
                        "chapterId": {"type": "string", "description": "章节ID"},
                        "outlineType": {"type": "string", "description": "coarse(粗纲) 或 fine(细纲)"},
                        "content": {"type": "string", "description": "大纲内容"}
                    },
                    "required": ["bookId", "chapterId", "outlineType", "content"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "report_conflict".into(),
                description: "报告一个确认后的设定冲突。必须已查原文核实过再报告，空章节不算。每个冲突单独调用".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "description": {"type": "string", "description": "冲突描述，如'第1章叶铃音是黑发，第5章变成了银发'"},
                        "suggestion": {"type": "string", "description": "修改建议"},
                        "severity": {"type": "string", "description": "严重程度：high/medium/low"}
                    },
                    "required": ["description", "suggestion", "severity"]
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
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["bookId"]
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
        // ===== 故事记忆类 =====
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_story_memory".into(),
                description: "获取小说的全局故事记忆（故事圣经），包括全书梗概、分卷梗概、事件时间线、主角状态、重要角色现状、未解决伏笔、世界观设定。这是AI理解全书剧情的首要入口，在回答剧情、角色、设定相关问题前应优先使用。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"}
                    },
                    "required": ["bookId"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "get_character_timeline".into(),
                description: "获取特定角色在全书中的关键事件时间线和当前状态。用于回答'XX角色做了什么'之类需要了解角色故事弧线的问题。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"},
                        "characterName": {"type": "string", "description": "角色名"}
                    },
                    "required": ["bookId", "characterName"]
                }),
            },
        },
        ToolDef {
            def_type: "function".into(),
            function: FunctionDef {
                name: "list_chapters_in_volume".into(),
                description: "获取指定卷下的所有章节列表（含ID和标题）。浏览具体章节时使用，避免一次性加载全书所有章节。".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "bookId": {"type": "string", "description": "书籍ID"},
                        "volumeId": {"type": "string", "description": "卷ID"}
                    },
                    "required": ["bookId", "volumeId"]
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

        let style_guide = prompt_manager.style_guide;
        let system_prompt = prompt_manager
            .polish_agent
            .replace("{{STYLE_GUIDE}}", style_guide);

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

        Self::stream_ai_response(app, session_id, book_id, messages, config, true).await
    }

    /// 处理普通对话请求 - 使用 function calling
    pub async fn process_chat(
        app: &AppHandle,
        session_id: &str,
        book_id: &str,
        user_message: &str,
        config: AIConfig,
    ) -> anyhow::Result<()> {
        let prompt_manager = get_prompt_manager();

        let mut state = get_session_state(session_id).unwrap_or_else(|| SessionState {
            messages: vec![],
            round_count: 0,
            compressed_context: String::new(),
        });

        // 构建 system prompt（每次都刷新，确保 Story Bible 最新）
        let story_bible_text = crate::services::story_memory_service::get_story_bible_for_prompt(book_id).await;
        let system_prompt = prompt_manager
            .decision_agent
            .replace("{{BOOK_ID}}", book_id)
            .replace("{{STORY_BIBLE}}", &story_bible_text)
            .replace("{{COMPRESSED_CONTEXT}}", &state.compressed_context);

        // 应用后台压缩好的上下文（如果有）
        let had_compressed = !state.compressed_context.is_empty();
        if had_compressed {
            log::info!("[Agent] 应用后台压缩结果，长度: {} 字", state.compressed_context.len());
            state.compressed_context.clear();
            state.messages.clear();
            state.messages.push(ChatMessage::new("system", &system_prompt));
        } else if state.messages.is_empty() {
            state.messages.push(ChatMessage::new("system", &system_prompt));
        } else {
            // 用最新的 system prompt 替换旧的
            if let Some(first) = state.messages.first_mut() {
                if first.role == "system" {
                    first.content = Some(system_prompt);
                }
            }
        }

        state.messages.push(ChatMessage::new("user", user_message));
        state.round_count += 1;

        let tools = build_tools();
        let max_rounds = config.max_rounds.max(3) as usize;

        for round in 0..max_rounds {
            log::info!("[Agent] 决策轮次 {}/{}", round + 1, max_rounds);

            let decision = send_ai_message(
                state.messages.clone(),
                config.clone(),
                Some(&tools),
            ).await?;

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

                for tc in tool_calls {
                    let tool_name = &tc.function.name;
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null);

                    log::info!("[Agent] 执行工具: {} args={}", tool_name, args);

                    Self::emit_tool_event(app, session_id, tool_name, Some(args.clone())).await;

                    // 收集工具摘要数据（前端恢复历史时用 buildToolDisplayText 重建显示文本）
                    tool_call_summary.push(serde_json::json!({
                        "name": tool_name,
                        "chapterId": args.get("chapterId").and_then(|v| v.as_str()).unwrap_or(""),
                        "outlineType": args.get("outlineType").and_then(|v| v.as_str()).unwrap_or(""),
                        "description": args.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                        "startLine": args.get("startLine").and_then(|v| v.as_i64()).unwrap_or(0),
                        "endLine": args.get("endLine").and_then(|v| v.as_i64()).unwrap_or(0),
                    }));

                    // save_outline / write_chapter 特殊处理：不真保存，发事件给前端渲染带按钮的卡片
                    let result = if tool_name == "save_outline" {
                        let _ = app.emit("ai-outline-result", serde_json::json!({
                            "sessionId": session_id,
                            "bookId": book_id,
                            "chapterId": args.get("chapterId").and_then(|v| v.as_str()).unwrap_or(""),
                            "outlineType": args.get("outlineType").and_then(|v| v.as_str()).unwrap_or("coarse"),
                            "content": args.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                        }));
                        "大纲已展示给用户，等待用户确认保存。".to_string()
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
                            // 保存到数据库
                        let conflict_id = format!("conflict_{}", uuid::Uuid::new_v4().to_string().replace('-', "_"));
                        let now = chrono::Utc::now().timestamp();
                        if let Ok(pool) = crate::db::get_pool().await {
                            let _ = sqlx::query(
                                "INSERT INTO detected_conflicts (id, book_id, description, suggestion, detected_at, is_ignored)
                                 VALUES (?1, ?2, ?3, ?4, ?5, 0)"
                            )
                            .bind(&conflict_id)
                            .bind(book_id)
                            .bind(desc)
                            .bind(sug)
                            .bind(now)
                            .execute(pool)
                            .await;
                        }
                        let _ = app.emit("ai-conflict-result", serde_json::json!({
                            "sessionId": session_id,
                            "bookId": book_id,
                            "conflictId": conflict_id,
                            "description": desc,
                            "suggestion": sug,
                            "severity": sev,
                        }));
                        format!("冲突已记录: {}", desc)
                        }
                    } else if tool_name == "write_chapter" {
                        let ch_id = args.get("chapterId").and_then(|v| v.as_str()).unwrap_or("");
                        let w_desc = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        log::info!("[Agent] 触发写作AI: chapterId={}, desc={}", ch_id, w_desc);

                        // 发射开始事件
                        let _ = app.emit("ai-write-start", serde_json::json!({
                            "sessionId": session_id,
                            "bookId": book_id,
                            "chapterId": ch_id,
                            "description": w_desc,
                        }));

                        // 提取上下文（工具返回的章节内容、角色信息等）
                        let context = Self::extract_writer_context(&state.messages);
                        log::info!("[Agent] 写作上下文长度: {} 字", context.len());

                        // 调用写作AI生成内容
                        let prompt_manager = get_prompt_manager();
                        let style_guide = prompt_manager.style_guide;
                        let writer_prompt = prompt_manager.writer_agent
                            .replace("{{STYLE_GUIDE}}", style_guide)
                            .replace("{{CONTEXT}}", &context);

                        let writer_messages = vec![
                            ChatMessage::new("system", &writer_prompt),
                            ChatMessage::new("user", &format!("请根据以上上下文进行创作。任务：{}", w_desc)),
                        ];

                        // 流式调用写作AI
                        match Self::stream_writer_ai(app, session_id, writer_messages, config.clone()).await {
                            Ok(content) => {
                                log::info!("[Agent] 写作AI完成，内容长度: {} 字", content.len());
                                let _ = app.emit("ai-write-result", serde_json::json!({
                                    "sessionId": session_id,
                                    "bookId": book_id,
                                    "chapterId": ch_id,
                                    "content": content,
                                    "description": w_desc,
                                }));
                                format!("创作完成（{}字），内容已展示给用户", content.chars().count())
                            }
                            Err(e) => {
                                log::error!("[Agent] 写作AI失败: {}", e);
                                format!("写作失败: {}", e)
                            }
                        }
                    } else {
                        execute_tool_call(&ToolCall {
                            name: tool_name.clone(),
                            arguments: args,
                        }).await?
                    };

                    tool_results.push(format!("工具 {} 返回: {}", tool_name, result));
                    last_tool_name = tool_name.clone();

                    // 将工具结果加入历史（AI上下文用，不存数据库）
                    state.messages.push(ChatMessage {
                        role: "tool".into(),
                        content: Some(result),
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

                // 持久化工具调用摘要（恢复历史时显示，不含工具返回的长数据）
                if let Err(e) = crate::services::chat_service::save_tool_call_summary(
                    session_id, book_id, &serde_json::to_string(&tool_call_summary).unwrap_or_default(),
                ).await {
                    log::error!("[Agent] 保存工具调用摘要失败: {}", e);
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
                        // 截断长返回，避免压缩输入过大
                        let truncated: String = if content.len() > 300 {
                            format!("{}...（已截断，共{}字）", &content[..300], content.len())
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

        let compress_prompt = prompt_manager.compress_agent;

        let messages = vec![
            ChatMessage::new("system", compress_prompt),
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

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                continue;
                            }

                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                    full_content.push_str(content);

                                    let event = StreamEvent {
                                        session_id: session_id.to_string(),
                                        chunk: content.to_string(),
                                        is_complete: false,
                                        is_tool_call: false,
                                        tool_name: None,
                                        tool_display_name: None,
                                        tool_params: None,
                                    };
                                    let _ = app.emit("ai-chat-stream", event);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[Agent] 流读取错误: {}", e);
                    break;
                }
            }
        }

        let complete_event = StreamEvent {
            session_id: session_id.to_string(),
            chunk: String::new(),
            is_complete: true,
            is_tool_call: false,
            tool_name: None,
            tool_display_name: None,
            tool_params: None,
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

    /// 从消息历史中提取写作上下文
    fn extract_writer_context(messages: &[ChatMessage]) -> String {
        let mut ctx = String::new();
        for msg in messages {
            match msg.role.as_str() {
                "user" => {
                    if let Some(content) = &msg.content {
                        // 跳过压缩上下文和工具返回
                        if !content.starts_with("工具返回结果") && !content.starts_with("【历史对话摘要】") {
                            ctx.push_str(&format!("## 用户需求\n{}\n\n", content));
                        }
                    }
                }
                "tool" => {
                    if let Some(content) = &msg.content {
                        // 截断过长的工具返回
                        if content.len() > 2000 {
                            let safe_len = content.char_indices().nth(2000).map(|(i, _)| i).unwrap_or(content.len());
                            ctx.push_str(&format!("## 查询结果\n{}...（已截断）\n\n", &content[..safe_len]));
                        } else {
                            ctx.push_str(&format!("## 查询结果\n{}\n\n", content));
                        }
                    }
                }
                _ => {}
            }
        }
        if ctx.is_empty() {
            ctx.push_str("无额外上下文");
        }
        ctx
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

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" { continue; }
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                    full_content.push_str(content);
                                    let _ = app.emit("ai-write-chunk", serde_json::json!({
                                        "sessionId": session_id,
                                        "chunk": content,
                                    }));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("[Writer] 流读取错误: {}", e);
                    break;
                }
            }
        }
        Ok(full_content)
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
        };
        let _ = app.emit("ai-chat-stream", complete_event);
    }

    /// 发送工具调用事件
    async fn emit_tool_event(app: &AppHandle, session_id: &str, tool_name: &str, tool_params: Option<serde_json::Value>) {
        let display_name = match tool_name {
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
            "report_conflict" => "报告设定冲突",
            "write_chapter" => "创作正文",
            "learn_writing_style" => "学习写作风格",
            "get_story_memory" => "获取故事记忆",
            "get_character_timeline" => "查询角色时间线",
            "list_chapters_in_volume" => "列出卷内章节",
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

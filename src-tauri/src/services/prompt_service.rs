use std::sync::OnceLock;

/// 提示词缓存
static PROMPT_CACHE: OnceLock<PromptManager> = OnceLock::new();

/// 提示词管理器
pub struct PromptManager {
    /// 核心系统提示词
    pub system_core: &'static str,
    /// 安全规则
    pub safety_guardrails: &'static str,
    /// 风格指南
    pub style_guide: &'static str,
    /// 摘要生成提示词
    pub summary_generation: &'static str,
    /// 工具提示词
    pub tools: &'static [(&'static str, &'static str)],
    /// 风格分析相关提示词
    pub style_prompts: StylePrompts,
}

/// 风格分析提示词
pub struct StylePrompts {
    /// 写作风格分析提示词
    pub analyze_writing_style: &'static str,
    /// 写作风格分析系统提示词
    pub analyze_writing_style_system: &'static str,
    /// 写作风格指南模板
    pub writing_style_guide_template: &'static str,
}

impl PromptManager {
    /// 初始化提示词管理器（编译时嵌入所有提示词）
    pub fn init() -> anyhow::Result<Self> {
        let system_core = include_str!("../../prompts/system_core.md");
        let safety_guardrails = include_str!("../../prompts/safety_guardrails.md");
        let style_guide = include_str!("../../prompts/style_guide.md");
        let summary_generation = include_str!("../../prompts/summary_generation.md");

        // 工具提示词 - 按固定顺序排列，_开头的（格式说明）排在前面
        let tools: &[(&'static str, &'static str)] = &[
            ("_tool_calling_format", include_str!("../../prompts/tools/_tool_calling_format.md")),
            ("tool_chapter_content", include_str!("../../prompts/tools/tool_chapter_content.md")),
            ("tool_chapter_summary", include_str!("../../prompts/tools/tool_chapter_summary.md")),
            ("tool_create_character", include_str!("../../prompts/tools/tool_create_character.md")),
            ("tool_get_character", include_str!("../../prompts/tools/tool_get_character.md")),
            ("tool_get_outline", include_str!("../../prompts/tools/tool_get_outline.md")),
            ("tool_learn_writing_style", include_str!("../../prompts/tools/tool_learn_writing_style.md")),
            ("tool_list_chapters", include_str!("../../prompts/tools/tool_list_chapters.md")),
            ("tool_list_characters", include_str!("../../prompts/tools/tool_list_characters.md")),
            ("tool_save_outline", include_str!("../../prompts/tools/tool_save_outline.md")),
            ("tool_update_character", include_str!("../../prompts/tools/tool_update_character.md")),
        ];

        // 风格分析提示词
        let style_prompts = StylePrompts {
            analyze_writing_style: include_str!("../../prompts/style/analyze_writing_style.md"),
            analyze_writing_style_system: include_str!("../../prompts/style/analyze_writing_style_system.md"),
            writing_style_guide_template: include_str!("../../prompts/style/writing_style_guide_template.md"),
        };

        Ok(PromptManager {
            system_core,
            safety_guardrails,
            style_guide,
            summary_generation,
            tools,
            style_prompts,
        })
    }

    /// 获取完整系统提示词
    pub fn get_full_system_prompt(&self, chapter_list: &str) -> String {
        let mut prompt = String::new();

        prompt.push_str(self.system_core);
        prompt.push_str("\n\n");

        if !self.safety_guardrails.is_empty() {
            prompt.push_str(self.safety_guardrails);
            prompt.push_str("\n\n");
        }

        if !self.style_guide.is_empty() {
            prompt.push_str(self.style_guide);
            prompt.push_str("\n\n");
        }

        if !self.tools.is_empty() {
            prompt.push_str("## 可用工具\n\n");
            for (name, content) in self.tools {
                if name.starts_with('_') {
                    prompt.push_str(content);
                    prompt.push_str("\n\n");
                }
            }
            for (name, content) in self.tools {
                if !name.starts_with('_') {
                    prompt.push_str(content);
                    prompt.push_str("\n\n");
                }
            }
        }

        prompt.push_str("## 当前可用信息 - 章节列表\n\n");
        prompt.push_str(chapter_list);

        prompt
    }

    /// 获取摘要生成提示词
    pub fn get_summary_system_prompt(&self) -> anyhow::Result<&'static str> {
        if self.summary_generation.is_empty() {
            Err(anyhow::anyhow!("摘要生成提示词文件不存在或为空，请检查 prompts/summary_generation.md 文件"))
        } else {
            Ok(self.summary_generation)
        }
    }
}

/// 获取提示词管理器实例
pub fn get_prompt_manager() -> &'static PromptManager {
    PROMPT_CACHE.get_or_init(|| {
        PromptManager::init().unwrap_or_else(|e| {
            log::error!("初始化提示词管理器失败: {}", e);
            PromptManager {
                system_core: "你是一位专业的小说写作助手。",
                safety_guardrails: "",
                style_guide: "",
                summary_generation: "",
                tools: &[],
                style_prompts: StylePrompts {
                    analyze_writing_style: "",
                    analyze_writing_style_system: "",
                    writing_style_guide_template: "",
                },
            }
        })
    })
}

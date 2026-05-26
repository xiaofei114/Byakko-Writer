use std::sync::OnceLock;

/// 提示词缓存
static PROMPT_CACHE: OnceLock<PromptManager> = OnceLock::new();

/// 提示词管理器
pub struct PromptManager {
    /// 风格指南
    pub style_guide: &'static str,
    /// 摘要生成提示词
    pub summary_generation: &'static str,
    /// 风格分析相关提示词
    pub style_prompts: StylePrompts,
    /// Agent提示词 - 决策AI
    pub decision_agent: &'static str,
    /// Agent提示词 - 写作AI（由 write_chapter 触发）
    pub writer_agent: &'static str,
    /// Agent提示词 - 润色AI
    pub polish_agent: &'static str,
    /// Agent提示词 - 压缩AI
    pub compress_agent: &'static str,
    /// Agent提示词 - 冲突检测AI
    pub conflict_detection: &'static str,
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
        let style_guide = include_str!("../../prompts/style_guide.md");
        let summary_generation = include_str!("../../prompts/style/summary_generation.md");

        // 风格分析提示词
        let style_prompts = StylePrompts {
            analyze_writing_style: include_str!("../../prompts/style/analyze_writing_style.md"),
            analyze_writing_style_system: include_str!("../../prompts/style/analyze_writing_style_system.md"),
            writing_style_guide_template: include_str!("../../prompts/style/writing_style_guide_template.md"),
        };

        // Agent提示词
        let decision_agent = include_str!("../../prompts/agents/decision_agent.md");
        let writer_agent = include_str!("../../prompts/agents/writer_agent.md");
        let polish_agent = include_str!("../../prompts/agents/polish_agent.md");
        let compress_agent = include_str!("../../prompts/agents/compress_agent.md");
        let conflict_detection = include_str!("../../prompts/agents/conflict_detection.md");
        Ok(PromptManager {
            style_guide,
            summary_generation,
            style_prompts,
            decision_agent,
            writer_agent,
            polish_agent,
            compress_agent,
            conflict_detection,
        })
    }

    /// 获取摘要生成提示词
    pub fn get_summary_system_prompt(&self) -> anyhow::Result<&'static str> {
        if self.summary_generation.is_empty() {
            Err(anyhow::anyhow!("摘要生成提示词文件不存在或为空"))
        } else {
            Ok(self.summary_generation)
        }
    }
}

/// 获取提示词管理器实例
pub fn get_prompt_manager() -> &'static PromptManager {
    PROMPT_CACHE.get_or_init(|| {
        PromptManager::init().expect("提示词文件加载失败，请检查 src-tauri/prompts/ 目录")
    })
}

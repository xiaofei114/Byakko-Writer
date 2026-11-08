use std::sync::OnceLock;

#[cfg(debug_assertions)]
use std::path::PathBuf;

/// 提示词缓存（进程内只加载一次；修改提示词后重启应用生效）
static PROMPT_CACHE: OnceLock<PromptManager> = OnceLock::new();

/// 提示词管理器
///
/// 提示词在运行时从 `%APPDATA%/byakko-writer/prompts/` 读取；
/// 文件不存在时回退到编译期嵌入的默认版本，因此用户可以直接改文件微调提示词。
pub struct PromptManager {
    /// 风格指南
    pub style_guide: String,
    /// 摘要生成提示词
    pub summary_generation: String,
    /// 风格分析相关提示词
    pub style_prompts: StylePrompts,
    /// Agent提示词 - 决策AI
    pub decision_agent: String,
    /// Agent提示词 - 写作AI（由 write_chapter 触发）
    pub writer_agent: String,
    /// Agent提示词 - 润色AI
    pub polish_agent: String,
    /// Agent提示词 - 压缩AI
    pub compress_agent: String,
    /// Agent提示词 - 冲突检测AI
    pub conflict_detection: String,
    /// 故事圣经 - 分组摘要
    pub story_bible_group: String,
    /// 故事圣经 - 全书汇总
    pub story_bible_update: String,
    /// 设定账本 - 单章抽取
    pub canon_extract: String,
    /// 思考模式 - 内部计划
    pub think_agent: String,
    /// 思考模式 - 每轮工具执行后的反思
    pub think_reflect: String,
}

/// 风格分析提示词
pub struct StylePrompts {
    /// 写作风格分析提示词
    pub analyze_writing_style: String,
    /// 写作风格分析系统提示词
    pub analyze_writing_style_system: String,
    /// 写作风格指南模板
    pub writing_style_guide_template: String,
}

/// 用户可编辑的提示词目录（仅开发模式下启用）
#[cfg(debug_assertions)]
pub fn user_prompts_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join("byakko-writer").join("prompts"))
}

/// 开发模式下项目内的提示词目录（编译期路径，发布版不启用）
///
/// 开发时直接改 `src-tauri/prompts/` 就能生效，避免被 `%APPDATA%` 里的旧副本覆盖。
#[cfg(debug_assertions)]
fn project_prompts_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// 内置默认提示词（编译期嵌入，作为兜底）
struct EmbeddedPrompts;

impl EmbeddedPrompts {
    const STYLE_GUIDE: &'static str = include_str!("../../prompts/style_guide.md");
    const SUMMARY_GENERATION: &'static str = include_str!("../../prompts/style/summary_generation.md");
    const ANALYZE_WRITING_STYLE: &'static str = include_str!("../../prompts/style/analyze_writing_style.md");
    const ANALYZE_WRITING_STYLE_SYSTEM: &'static str = include_str!("../../prompts/style/analyze_writing_style_system.md");
    const WRITING_STYLE_GUIDE_TEMPLATE: &'static str = include_str!("../../prompts/style/writing_style_guide_template.md");
    const DECISION_AGENT: &'static str = include_str!("../../prompts/agents/decision_agent.md");
    const WRITER_AGENT: &'static str = include_str!("../../prompts/agents/writer_agent.md");
    const POLISH_AGENT: &'static str = include_str!("../../prompts/agents/polish_agent.md");
    const COMPRESS_AGENT: &'static str = include_str!("../../prompts/agents/compress_agent.md");
    const CONFLICT_DETECTION: &'static str = include_str!("../../prompts/agents/conflict_detection.md");
    const STORY_BIBLE_GROUP: &'static str = include_str!("../../prompts/agents/story_bible_group.md");
    const STORY_BIBLE_UPDATE: &'static str = include_str!("../../prompts/agents/story_bible_update.md");
    const CANON_EXTRACT: &'static str = include_str!("../../prompts/agents/canon_extract.md");
    const THINK_AGENT: &'static str = include_str!("../../prompts/agents/think_agent.md");
    const THINK_REFLECT: &'static str = include_str!("../../prompts/agents/think_reflect_agent.md");
}

/// 所有提示词相对路径 + 内置内容（仅开发模式用于回写用户目录）
#[cfg(debug_assertions)]
const PROMPT_FILES: &[(&str, &str)] = &[
    ("style_guide.md", EmbeddedPrompts::STYLE_GUIDE),
    ("style/summary_generation.md", EmbeddedPrompts::SUMMARY_GENERATION),
    ("style/analyze_writing_style.md", EmbeddedPrompts::ANALYZE_WRITING_STYLE),
    ("style/analyze_writing_style_system.md", EmbeddedPrompts::ANALYZE_WRITING_STYLE_SYSTEM),
    ("style/writing_style_guide_template.md", EmbeddedPrompts::WRITING_STYLE_GUIDE_TEMPLATE),
    ("agents/decision_agent.md", EmbeddedPrompts::DECISION_AGENT),
    ("agents/writer_agent.md", EmbeddedPrompts::WRITER_AGENT),
    ("agents/polish_agent.md", EmbeddedPrompts::POLISH_AGENT),
    ("agents/compress_agent.md", EmbeddedPrompts::COMPRESS_AGENT),
    ("agents/conflict_detection.md", EmbeddedPrompts::CONFLICT_DETECTION),
    ("agents/story_bible_group.md", EmbeddedPrompts::STORY_BIBLE_GROUP),
    ("agents/story_bible_update.md", EmbeddedPrompts::STORY_BIBLE_UPDATE),
    ("agents/canon_extract.md", EmbeddedPrompts::CANON_EXTRACT),
    ("agents/think_agent.md", EmbeddedPrompts::THINK_AGENT),
    ("agents/think_reflect_agent.md", EmbeddedPrompts::THINK_REFLECT),
];

/// 首次运行时把内置提示词写到用户目录，方便直接编辑（仅开发模式）
#[cfg(debug_assertions)]
fn ensure_user_prompts() {
    // 开发时项目内已有提示词，不再往用户目录写旧副本
    if project_prompts_dir().is_some() {
        return;
    }

    let Some(dir) = user_prompts_dir() else {
        return;
    };

    for (relative, fallback) in PROMPT_FILES {
        let path = dir.join(relative);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, fallback) {
            log::warn!("[Prompt] 写入默认提示词失败 {}: {}", path.display(), e);
        }
    }
}

/// 读取提示词
///
/// 开发模式：优先读用户目录（便于改提示词调试），缺失/为空回退内置默认。
/// 生产构建：直接使用编译期嵌入的提示词，磁盘文件一律不读，避免被外部改动。
fn load_prompt(relative: &str, fallback: &'static str) -> String {
    #[cfg(debug_assertions)]
    {
        // 1. 开发模式：项目内提示词优先（改文件后重启生效）
        if let Some(dir) = project_prompts_dir() {
            match std::fs::read_to_string(dir.join(relative)) {
                Ok(content) if !content.trim().is_empty() => return content,
                Ok(_) => log::warn!("[Prompt] 项目内 {} 为空，尝试用户目录", relative),
                Err(_) => {}
            }
        }

        // 2. 用户目录（兼容之前生成的副本）
        if let Some(dir) = user_prompts_dir() {
            let path = dir.join(relative);
            match std::fs::read_to_string(&path) {
                Ok(content) if !content.trim().is_empty() => return content,
                Ok(_) => log::warn!("[Prompt] {} 为空，使用内置默认", path.display()),
                Err(_) => {}
            }
        }
    }

    #[cfg(not(debug_assertions))]
    let _ = relative;

    fallback.to_string()
}

impl PromptManager {
    /// 初始化提示词管理器（失败回退内置默认）
    pub fn init() -> anyhow::Result<Self> {
        #[cfg(debug_assertions)]
        ensure_user_prompts();

        Ok(PromptManager {
            style_guide: load_prompt("style_guide.md", EmbeddedPrompts::STYLE_GUIDE),
            summary_generation: load_prompt("style/summary_generation.md", EmbeddedPrompts::SUMMARY_GENERATION),
            style_prompts: StylePrompts {
                analyze_writing_style: load_prompt("style/analyze_writing_style.md", EmbeddedPrompts::ANALYZE_WRITING_STYLE),
                analyze_writing_style_system: load_prompt("style/analyze_writing_style_system.md", EmbeddedPrompts::ANALYZE_WRITING_STYLE_SYSTEM),
                writing_style_guide_template: load_prompt("style/writing_style_guide_template.md", EmbeddedPrompts::WRITING_STYLE_GUIDE_TEMPLATE),
            },
            decision_agent: load_prompt("agents/decision_agent.md", EmbeddedPrompts::DECISION_AGENT),
            writer_agent: load_prompt("agents/writer_agent.md", EmbeddedPrompts::WRITER_AGENT),
            polish_agent: load_prompt("agents/polish_agent.md", EmbeddedPrompts::POLISH_AGENT),
            compress_agent: load_prompt("agents/compress_agent.md", EmbeddedPrompts::COMPRESS_AGENT),
            conflict_detection: load_prompt("agents/conflict_detection.md", EmbeddedPrompts::CONFLICT_DETECTION),
            story_bible_group: load_prompt("agents/story_bible_group.md", EmbeddedPrompts::STORY_BIBLE_GROUP),
            story_bible_update: load_prompt("agents/story_bible_update.md", EmbeddedPrompts::STORY_BIBLE_UPDATE),
            canon_extract: load_prompt("agents/canon_extract.md", EmbeddedPrompts::CANON_EXTRACT),
            think_agent: load_prompt("agents/think_agent.md", EmbeddedPrompts::THINK_AGENT),
            think_reflect: load_prompt("agents/think_reflect_agent.md", EmbeddedPrompts::THINK_REFLECT),
        })
    }

    /// 获取摘要生成提示词
    pub fn get_summary_system_prompt(&self) -> anyhow::Result<&str> {
        if self.summary_generation.is_empty() {
            Err(anyhow::anyhow!("摘要生成提示词文件不存在或为空"))
        } else {
            Ok(&self.summary_generation)
        }
    }
}

/// 获取提示词管理器实例
pub fn get_prompt_manager() -> &'static PromptManager {
    PROMPT_CACHE.get_or_init(|| {
        PromptManager::init().expect("提示词加载失败，请检查 src-tauri/prompts/ 与用户提示词目录")
    })
}

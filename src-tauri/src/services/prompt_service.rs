use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// 提示词缓存
static PROMPT_CACHE: OnceLock<PromptManager> = OnceLock::new();

/// 提示词管理器
pub struct PromptManager {
    /// 核心系统提示词
    pub system_core: String,
    /// 安全规则
    pub safety_guardrails: String,
    /// 风格指南
    pub style_guide: String,
    /// 摘要生成提示词
    pub summary_generation: String,
    /// 工具提示词
    pub tools: Vec<(String, String)>,
    /// 风格分析相关提示词
    pub style_prompts: StylePrompts,
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

impl StylePrompts {
    fn new() -> Self {
        Self {
            analyze_writing_style: String::new(),
            analyze_writing_style_system: String::new(),
            writing_style_guide_template: String::new(),
        }
    }
}

impl PromptManager {
    /// 初始化提示词管理器
    pub fn init() -> anyhow::Result<Self> {
        let prompts_dir = get_prompts_dir()?;
        
        // 加载核心提示词
        let system_core = load_prompt_file(&prompts_dir, "system_core.md")?;
        let safety_guardrails = load_prompt_file(&prompts_dir, "safety_guardrails.md")?;
        let style_guide = load_prompt_file(&prompts_dir, "style_guide.md")?;
        let summary_generation = load_prompt_file(&prompts_dir, "summary_generation.md")?;

        // 加载工具提示词
        let tools_dir = prompts_dir.join("tools");
        let mut tools = Vec::new();

        if tools_dir.exists() {
            if let Ok(entries) = fs::read_dir(&tools_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                tools.push((name.to_string(), content));
                            }
                        }
                    }
                }
            }
        }

        // 加载风格分析提示词
        let mut style_prompts = StylePrompts::new();
        let style_dir = prompts_dir.join("style");
        if style_dir.exists() {
            style_prompts.analyze_writing_style = load_prompt_file(&style_dir, "analyze_writing_style.md")?;
            style_prompts.analyze_writing_style_system = load_prompt_file(&style_dir, "analyze_writing_style_system.md")?;
            style_prompts.writing_style_guide_template = load_prompt_file(&style_dir, "writing_style_guide_template.md")?;
        }

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
        
        // 添加核心提示词
        prompt.push_str(&self.system_core);
        prompt.push_str("\n\n");
        
        // 添加安全规则
        if !self.safety_guardrails.is_empty() {
            prompt.push_str(&self.safety_guardrails);
            prompt.push_str("\n\n");
        }
        
        // 添加风格指南
        if !self.style_guide.is_empty() {
            prompt.push_str(&self.style_guide);
            prompt.push_str("\n\n");
        }
        
        // 添加工具说明
        if !self.tools.is_empty() {
            prompt.push_str("## 可用工具\n\n");
            // 首先添加工具调用格式指南（文件名以 _ 开头的优先）
            for (name, content) in &self.tools {
                if name.starts_with('_') {
                    prompt.push_str(content);
                    prompt.push_str("\n\n");
                }
            }
            // 然后添加其他工具说明
            for (name, content) in &self.tools {
                if !name.starts_with('_') {
                    prompt.push_str(content);
                    prompt.push_str("\n\n");
                }
            }
        }
        
        // 添加章节列表
        prompt.push_str("## 当前可用信息 - 章节列表\n\n");
        prompt.push_str(chapter_list);
        
        prompt
    }
    
    /// 获取摘要生成提示词
    pub fn get_summary_system_prompt(&self) -> anyhow::Result<String> {
        if self.summary_generation.is_empty() {
            Err(anyhow::anyhow!("摘要生成提示词文件不存在或为空，请检查 prompts/summary_generation.md 文件"))
        } else {
            Ok(self.summary_generation.clone())
        }
    }
}

/// 获取提示词目录
fn get_prompts_dir() -> anyhow::Result<PathBuf> {
    // 在开发环境中，使用项目目录
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_exe()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        });
    
    Ok(manifest_dir.join("prompts"))
}

/// 加载提示词文件
fn load_prompt_file(prompts_dir: &PathBuf, filename: &str) -> anyhow::Result<String> {
    let file_path = prompts_dir.join(filename);
    
    if file_path.exists() {
        fs::read_to_string(&file_path)
            .map_err(|e| anyhow::anyhow!("读取文件 {} 失败: {}", filename, e))
    } else {
        // 文件不存在时返回空字符串
        Ok(String::new())
    }
}

/// 获取提示词管理器实例
pub fn get_prompt_manager() -> &'static PromptManager {
    PROMPT_CACHE.get_or_init(|| {
        PromptManager::init().unwrap_or_else(|e| {
            log::error!("初始化提示词管理器失败: {}", e);
            // 返回一个空的默认实例
            PromptManager {
                system_core: String::from("你是一位专业的小说写作助手。"),
                safety_guardrails: String::new(),
                style_guide: String::new(),
                summary_generation: String::new(),
                tools: Vec::new(),
                style_prompts: StylePrompts::new(),
            }
        })
    })
}



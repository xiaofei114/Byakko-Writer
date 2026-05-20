use serde::{Deserialize, Serialize};

/// 用户全局写作风格配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserWritingStyle {
    pub id: String,
    /// 风格分析结果（JSON 格式存储详细分析）
    pub style_analysis: String,
    /// 生成的风格提示词
    pub style_prompt: String,
    /// 分析基于的总字数
    pub total_word_count: i64,
    /// 分析基于的章节数
    pub chapter_count: i64,
    /// 最后更新时间
    pub updated_at: i64,
    /// 是否启用
    pub is_enabled: bool,
}

/// 书籍级写作风格配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookWritingStyle {
    pub id: String,
    pub book_id: String,
    /// 风格分析结果（JSON 格式）
    pub style_analysis: String,
    /// 生成的风格提示词
    pub style_prompt: String,
    /// 分析基于的总字数
    pub total_word_count: i64,
    /// 分析基于的章节数
    pub chapter_count: i64,
    /// 最后更新时间
    pub updated_at: i64,
    /// 是否启用（覆盖全局风格）
    pub is_enabled: bool,
    /// 是否继承全局风格
    pub inherit_global: bool,
}

/// 风格分析结果结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleAnalysis {
    /// 语言风格特征
    pub language_style: LanguageStyle,
    /// 叙事特征
    pub narrative_features: NarrativeFeatures,
    /// 对话风格
    pub dialogue_style: DialogueStyle,
    /// 描写偏好
    pub description_preference: DescriptionPreference,
    /// 节奏特点
    pub pacing_style: PacingStyle,
    /// 常用修辞手法
    pub common_rhetoric: Vec<String>,
    /// 典型句式特征
    pub sentence_patterns: Vec<String>,
    /// 词汇偏好
    pub vocabulary_preference: VocabularyPreference,
}

/// 语言风格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStyle {
    /// 整体风格：简洁/华丽/朴实/诗意/冷峻等
    pub overall_tone: String,
    /// 用词特点
    pub word_choice: String,
    /// 句式特点：长短句偏好、整散结合等
    pub sentence_structure: String,
    /// 修辞密度：高/中/低
    pub rhetoric_density: String,
}

/// 叙事特征
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeFeatures {
    /// 叙事视角偏好
    pub perspective: String,
    /// 叙事时序：线性/倒叙/插叙/多线
    pub chronology: String,
    /// 叙事距离：沉浸/疏离/客观
    pub narrative_distance: String,
    /// 心理描写深度
    pub psychological_depth: String,
}

/// 对话风格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueStyle {
    /// 对话密度：高/中/低
    pub dialogue_density: String,
    /// 对话特点：简洁/幽默/正式/口语化等
    pub dialogue_characteristics: String,
    /// 对话标签使用习惯
    pub dialogue_tag_style: String,
    /// 方言/特色用语使用情况
    pub dialect_usage: String,
}

/// 描写偏好
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptionPreference {
    /// 五感使用偏好
    pub sensory_preference: Vec<String>,
    /// 环境描写比重：高/中/低
    pub environment_description_level: String,
    /// 动作描写风格
    pub action_description_style: String,
    /// 心理描写风格
    pub psychological_description_style: String,
}

/// 节奏特点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingStyle {
    /// 整体节奏：紧凑/舒缓/张弛有度
    pub overall_pacing: String,
    /// 段落长度偏好
    pub paragraph_length_preference: String,
    /// 场景切换频率
    pub scene_transition_frequency: String,
}

/// 词汇偏好
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyPreference {
    /// 高频词汇（前20）
    pub high_frequency_words: Vec<String>,
    /// 特色用词
    pub characteristic_words: Vec<String>,
    /// 禁用/少用词汇
    pub avoided_words: Vec<String>,
    /// 词汇难度：通俗/中等/艰深
    pub vocabulary_level: String,
}

/// 风格学习请求参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnStyleParams {
    /// 书籍ID（可选，不传则学习全局风格）
    pub book_id: Option<String>,
    /// 指定分析的章节ID列表（可选，不传则自动选择）
    pub chapter_ids: Option<Vec<String>>,
    /// 是否强制重新学习（覆盖已有风格）
    pub force_relearn: bool,
}

/// 风格学习结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnStyleResult {
    pub success: bool,
    pub message: String,
    pub analyzed_chapters: i32,
    pub total_word_count: i64,
    pub style_prompt_preview: String,
}

/// 风格配置更新参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStyleParams {
    /// 书籍ID（None 表示更新全局风格）
    pub book_id: Option<String>,
    /// 是否启用
    pub is_enabled: Option<bool>,
    /// 是否继承全局风格（仅书籍级有效）
    pub inherit_global: Option<bool>,
    /// 手动编辑的风格提示词（可选）
    pub custom_style_prompt: Option<String>,
}

/// 获取风格提示词结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylePromptResult {
    /// 最终使用的风格提示词
    pub style_prompt: String,
    /// 提示词来源：global/book/none
    pub source: String,
    /// 是否启用了风格学习
    pub is_enabled: bool,
}

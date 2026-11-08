// ==================== 字体配置 ====================

import type { AppConfig } from '../types';

export interface FontOption {
  value: string;
  label: string;
  family: string;
}

export const FONT_OPTIONS: FontOption[] = [
  {
    value: 'system',
    label: '系统默认',
    family: 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
  },
  {
    value: 'LXGWWenKai',
    label: '霞鹜文楷',
    family: '"LXGWWenKai", "Noto Serif SC", "Source Han Serif SC", serif',
  },
  {
    value: 'LXGWWenKaiMono',
    label: '霞鹜文楷 Mono',
    family: '"LXGWWenKaiMono", "LXGWWenKai", monospace',
  },
  {
    value: 'NotoSerifCJK',
    label: '思源宋体',
    family: '"NotoSerifCJK", "Noto Serif SC", "Source Han Serif SC", serif',
  },
  {
    value: 'PingFang',
    label: '苹方',
    family: '"PingFang SC", "Hiragino Sans GB", sans-serif',
  },
  {
    value: 'MicrosoftYaHei',
    label: '微软雅黑',
    family: '"Microsoft YaHei", "PingFang SC", sans-serif',
  },
  {
    value: 'SourceHanSans',
    label: '思源黑体',
    family: '"Source Han Sans SC", "Noto Sans SC", sans-serif',
  },
];

export const FONT_FAMILY_MAP: Record<string, string> = Object.fromEntries(
  FONT_OPTIONS.map((font) => [font.value, font.family])
);

// ==================== 主题配置 ====================

export interface ThemeOption {
  value: 'light' | 'dark' | 'auto';
  label: string;
}

export const THEME_OPTIONS: ThemeOption[] = [
  { value: 'light', label: '浅色' },
  { value: 'dark', label: '深色' },
  { value: 'auto', label: '跟随系统' },
];

// ==================== AI 提供商预设 ====================

export interface AIProviderPreset {
  name: string;
  apiUrl: string;
  model: string;
  help?: string;
  keyUrl: string;
  showApiUrl?: boolean;
}

export const AI_PROVIDER_PRESETS: Record<string, AIProviderPreset> = {
  deepseek: {
    name: 'DeepSeek',
    apiUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    help: 'DeepSeek 官方 API，价格便宜效果好',
    keyUrl: 'https://platform.deepseek.com/',
  },
  openai: {
    name: 'OpenAI',
    apiUrl: 'https://api.openai.com/v1',
    model: 'gpt-3.5-turbo',
    help: 'OpenAI 官方 API，需要海外支付方式',
    keyUrl: 'https://platform.openai.com/api-keys',
  },
  aliyun: {
    name: '阿里云 (通义千问)',
    apiUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen-turbo',
    help: '阿里云灵积平台，国内访问稳定',
    keyUrl: 'https://dashscope.console.aliyun.com/apiKey',
  },
  moonshot: {
    name: '月之暗面 (Kimi)',
    apiUrl: 'https://api.moonshot.cn/v1',
    model: 'moonshot-v1-8k',
    help: 'Kimi 长文本模型，支持超长上下文',
    keyUrl: 'https://platform.moonshot.cn/',
  },
  zhipu: {
    name: '智谱AI (GLM)',
    apiUrl: 'https://open.bigmodel.cn/api/paas/v4',
    model: 'glm-4-flash',
    help: '智谱清言，国内大模型，有免费额度',
    keyUrl: 'https://open.bigmodel.cn/usercenter/apikeys',
  },
  ollama: {
    name: 'Ollama (本地)',
    apiUrl: 'http://localhost:11434/v1',
    model: 'llama3.2',
    help: '本地运行开源模型，无需联网和 API 密钥',
    keyUrl: 'https://ollama.com/',
    showApiUrl: true,
  },
  custom: {
    name: '自定义',
    apiUrl: '',
    model: '',
    help: '其他兼容 OpenAI 格式的 API',
    keyUrl: '',
    showApiUrl: true,
  },
};

export const AI_PROVIDERS = Object.entries(AI_PROVIDER_PRESETS).map(([value, config]) => ({
  value,
  label: config.name,
}));

// ==================== 默认配置 ====================

export const DEFAULT_AI_CONFIG = {
  provider: 'deepseek',
  apiKey: '',
  apiUrl: AI_PROVIDER_PRESETS.deepseek.apiUrl,
  model: AI_PROVIDER_PRESETS.deepseek.model,
  temperature: 0.7,
  maxTokens: 4000,
  maxRounds: 30,
  enableThinking: true,
};

export const DEFAULT_CONFIG: AppConfig = {
  theme: 'light',
  primaryColor: '#3498db',
  fontFamily: 'system',
  fontSize: 16,
  lineHeight: 1.8,
  autoSave: true,
  autoSaveInterval: 30,
  autoStoryMemory: true,
  ai: DEFAULT_AI_CONFIG,
};

// ==================== 工具显示名称映射 ====================

export const TOOL_DISPLAY_NAMES: Record<string, string> = {
  query_chapter_summary: '查询章节摘要',
  search_chapter_content: '搜索章节内容',
  query_chapter_content: '查询章节内容',
  get_outline: '获取大纲',
  get_volume_outline: '获取卷大纲',
  create_chapter: '创建新章节',
  get_character_card: '查询角色详情',
  create_character_card: '创建角色',
  update_character_card: '更新角色卡',
  write_chapter: '创作章节',
  save_outline: '保存大纲',
  propose_edits: '批量修改正文',
  propose_outlines: '批量修改大纲',
  propose_line_edit: '提议行修改',
  ask_user: '询问用户',
  list_all_chapters: '获取书籍章节列表',
  list_character_cards: '获取角色列表',
  get_book_info: '获取书籍信息',
  get_story_memory: '获取故事记忆',
  get_character_timeline: '获取角色时间线',
  list_chapters_in_volume: '获取卷内章节',
  learn_writing_style: '学习写作风格',
  report_conflict: '报告设定冲突',
};

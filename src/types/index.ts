export interface Chapter {
  id: string;
  title: string;
  content: string;
  order: number;
  volumeId: string;
  createdAt: number;
  updatedAt: number;
}

export interface Volume {
  id: string;
  title: string;
  order: number;
  isCollapsed: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface Book {
  id: string;
  title: string;
  author: string;
  description: string;
  volumes: Volume[];
  chapters: Chapter[];
  currentChapterId: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface AIConfig {
  provider: string;
  apiKey: string;
  apiUrl: string;
  model: string;
  temperature: number;
  maxTokens: number;
  maxRounds: number;
}

export interface AppConfig {
  theme: 'light' | 'dark' | 'auto';
  primaryColor: string;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  autoSave: boolean;
  autoSaveInterval: number;
  autoStoryMemory: boolean;
  ai: AIConfig;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
}

export interface RecentBook {
  id: string;
  title: string;
  lastOpenedAt: number;
}

// 角色卡
export interface CharacterCard {
  id: string;
  bookId: string;
  name: string;
  aliases: string[];
  gender: string;
  age: string;
  appearance: string;
  personality: string;
  background: string;
  goals: string;
  relationships: CharacterRelationship[];
  tags: string[];
  notes: string;
  createdAt: number;
  updatedAt: number;
}

export interface CharacterRelationship {
  targetCharacterId: string;
  targetName: string;
  relationship: string;
  description: string;
}

// 章节摘要
export interface ChapterSummary {
  id: string;
  chapterId: string;
  shortSummary: string;
  longSummary: string;
  tags: string[];
  characters: string[];
  locations: string[];
  events: string[];
  // 新增增强字段
  plotProgression?: string;      // 剧情推进点
  emotionalBeats: string[];      // 情感节点
  foreshadowing: string[];       // 伏笔
  unresolvedThreads: string[];   // 未解决线索
  generatedAt: number;
  isConfirmed: boolean;
}

// ==================== AI 聊天类型 ====================

export interface AIChatMessage {
  id: string;
  bookId: string;
  chapterId?: string;
  sessionId: string;
  role: 'user' | 'assistant' | 'system' | 'tool' | 'outline' | 'write' | 'conflict' | 'line_edit' | 'ask_user';
  content: string;
  contextType?: 'normal' | 'rag' | 'summary';
  timestamp: number;
  toolName?: string;
  toolStatus?: 'calling' | 'success' | 'error';
  polishHandled?: boolean; // 润色是否已处理（应用或取消）
  isHandled?: boolean; // 是否已处理（用于line_edit和ask_user）
  handledStatus?: string; // 持久化处理状态（从DB的handled_status列）
}

export interface ChatSession {
  sessionId: string;
  bookId: string;
  chapterId?: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
}

// ==================== 章节快照类型 ====================

export interface ChapterSnapshot {
  id: string;
  book_id: string;
  chapter_id: string;
  chapter_title: string;
  name: string;
  content: string;
  word_count: number;
  created_at: string;
}

export type DiffBlockType = 'equal' | 'insert' | 'delete';

export interface DiffBlock {
  block_type: DiffBlockType;
  old_text: string;
  new_text: string;
}

export interface SnapshotComparison {
  old_snapshot: ChapterSnapshot;
  new_snapshot: ChapterSnapshot;
  diff_blocks: DiffBlock[];
}

// ==================== 角色卡类型 ====================

export interface CharacterRelationship {
  target_character_id: string;
  target_name: string;
  relationship: string;
  description: string;
}

export interface CharacterCard {
  id: string;
  book_id: string;
  name: string;
  aliases: string[];
  gender: string;
  age: string;
  appearance: string;
  personality: string;
  background: string;
  goals: string;
  relationships: CharacterRelationship[];
  tags: string[];
  notes: string;
  created_at: number;
  updated_at: number;
}

// ==================== 大纲类型 ====================

export type OutlineType = 'coarse' | 'fine';

export interface Outline {
  id: string;
  book_id: string;
  volume_id?: string;
  chapter_id?: string;
  outline_type: OutlineType;
  content: string;
  created_at: number;
  updated_at: number;
}

export interface OutlineInfo {
  id: string;
  outline_type: OutlineType;
  has_content: boolean;
  updated_at: number;
}

// 设定冲突检测
export interface DetectedConflict {
  id: string;
  bookId: string;
  description: string;
  suggestion: string;
  severity?: string;
  detectedAt: number;
  isIgnored: number;
  ignoredAt?: number;
}

export interface ConflictCheckResult {
  hasConflicts: boolean;
  totalChecked: number;
  conflicts: DetectedConflict[];
  checkedChapters: number;
  checkedWordCount: number;
}

// ==================== 故事记忆类型 ====================

export interface VolumeSummaryItem {
  title: string;
  summary: string;
}

export interface EventTimelineItem {
  chapter: number;
  title: string;
  event: string;
  impact: string;
  arc: string;
}

export interface KeyCharacterStatus {
  name: string;
  status: string;
  location: string;
}

export interface StoryMemory {
  book_id: string;
  book_summary: string;
  volume_summaries: VolumeSummaryItem[];
  event_timeline: EventTimelineItem[];
  protagonist_status: string;
  key_character_statuses: KeyCharacterStatus[];
  unresolved_threads: string[];
  world_rules: string;
  last_chapter_count: number;
  last_word_count: number;
  updated_at: number;
}

export interface GroupProgress {
  group_index: number;
  start_chapter: number;
  end_chapter: number;
  status: string; // "cached" | "generated" | "error"
  message: string;
}

export interface StoryMemoryUpdateResult {
  success: boolean;
  message: string;
  chapter_count: number;
  total_word_count: number;
  groups: GroupProgress[];
  groups_cached: number;
  groups_generated: number;
}

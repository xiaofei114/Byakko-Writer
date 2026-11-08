# NovelForge AI - 详细产品需求文档 (PRD)

**版本：** V1.1  
**日期：** 2026-05-18  
**状态：** 持续迭代开发中  

---

## 文档结构

本文档按模块细化需求，每个模块包含：
- 功能需求 (Functional Requirements)
- 数据模型 (Data Models)
- 接口设计 (API Design)
- UI/UX 设计 (Interface Design)
- 实现状态 (Implementation Status)

---

## 目录

1. [核心写作与项目管理模块](#1-核心写作与项目管理模块)
2. [AI上下文引擎模块](#2-ai上下文引擎模块)
3. [基础AI功能模块](#3-基础ai功能模块)
4. [对比决策模式What-If模块](#4-对比决策模式what-if模块)
5. [剧情时间轴可视化模块](#5-剧情时间轴可视化模块)
6. [废稿回收站模块](#6-废稿回收站模块)
7. [技术架构设计](#7-技术架构设计)
8. [数据库设计](#8-数据库设计)
9. [开发里程碑](#9-开发里程碑)

---

## 1. 核心写作与项目管理模块

### 1.1 功能需求

#### FR-001 项目结构管理
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-001-1 | 支持创建、编辑、删除书籍 | P0 | 已实现 |
| FR-001-2 | 支持"卷 -> 章"两级层级结构 | P0 | 已实现 |
| FR-001-3 | 支持卷的创建、编辑、删除、排序 | P0 | 已实现 |
| FR-001-4 | 支持章节的创建、编辑、删除、排序 | P0 | 已实现 |
| FR-001-5 | 支持章节在卷之间移动 | P1 | 待实现 |
| FR-001-6 | 支持卷的展开/折叠状态记忆 | P1 | 已实现 |

#### FR-002 编辑器功能
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-002-1 | 富文本编辑器支持基础格式（粗体、斜体、标题） | P0 | 已实现 |
| FR-002-2 | 实时字数统计 | P0 | 已实现 |
| FR-002-3 | 自动保存（默认30秒间隔） | P0 | 已实现 |
| FR-002-4 | 章节切换时自动保存当前章节 | P0 | 已实现 |
| FR-002-5 | 全屏写作模式（无干扰模式） | P1 | 待实现 |
| FR-002-6 | 打字机模式（当前行居中） | P2 | 待实现 |
| FR-002-7 | 专注模式（模糊非当前段落） | P2 | 待实现 |
| FR-002-8 | 查找替换功能 | P1 | 待实现 |

#### FR-003 版本快照
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-003-1 | 手动创建快照 | P0 | 已实现 |
| FR-003-2 | 快照列表查看 | P0 | 已实现 |
| FR-003-3 | 从快照恢复 | P0 | 已实现 |
| FR-003-4 | 快照删除 | P0 | 已实现 |
| FR-003-5 | 自动快照（章节切换/关闭应用时） | P1 | 已实现 |
| FR-003-6 | 快照对比功能 | P2 | 待实现 |
| FR-003-7 | 按章节独立存储快照 | P0 | 已实现 |

### 1.2 数据模型

```typescript
// 书籍
interface Book {
  id: string;
  title: string;
  author: string;
  description: string;
  currentChapterId: string | null;
  createdAt: number;
  updatedAt: number;
}

// 卷
interface Volume {
  id: string;
  bookId: string;
  title: string;
  order: number;
  isCollapsed: boolean;
  createdAt: number;
  updatedAt: number;
}

// 章节
interface Chapter {
  id: string;
  bookId: string;
  volumeId: string;
  title: string;
  order: number;
  content: string;
  wordCount: number;
  createdAt: number;
  updatedAt: number;
}

// 章节快照 (按章节独立存储)
interface ChapterSnapshot {
  id: string;
  bookId: string;
  chapterId: string;
  chapterTitle: string;
  name: string;
  content: string;
  wordCount: number;
  createdAt: string;
}
```

---

## 2. AI上下文引擎模块

### 2.1 功能需求

#### FR-004 章节摘要自动生成
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-004-1 | 手动触发单章摘要生成 | P0 | 已实现 |
| FR-004-2 | 批量生成全书摘要 | P1 | 待实现 |
| FR-004-3 | 短摘要（50字，用于列表展示） | P0 | 已实现 |
| FR-004-4 | 长摘要（500字，用于AI上下文） | P0 | 已实现 |
| FR-004-5 | 摘要编辑与确认 | P1 | 已实现 |
| FR-004-6 | 摘要自动提取关键标签（人物、地点、事件） | P1 | 已实现 |

#### FR-005 角色/设定卡管理
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-005-1 | 手动创建角色卡 | P0 | 待实现 |
| FR-005-2 | 手动创建设定卡（道具、势力、世界观） | P0 | 待实现 |
| FR-005-3 | AI自动提取角色信息 | P1 | 待实现 |
| FR-005-4 | 设定卡关联章节 | P1 | 待实现 |
| FR-005-5 | 设定冲突检测 | P2 | 待实现 |

#### FR-006 向量检索 (RAG)
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-006-1 | 章节内容向量化存储 | P0 | 已弃用 |
| FR-006-2 | 语义检索相关章节 | P0 | 已弃用 |
| FR-006-3 | 检索结果相关性排序 | P1 | 已弃用 |
| FR-006-4 | 检索范围限定（指定卷/章节范围） | P1 | 已弃用 |

> **注意**: 向量检索(RAG)方案已弃用，改用基于章节摘要的工具调用方案。AI通过工具调用主动查询需要的章节信息。

### 2.2 数据模型

```typescript
// 章节摘要
interface ChapterSummary {
  id: string;
  chapterId: string;
  shortSummary: string;
  longSummary: string;
  tags: string[];
  characters: string[];
  locations: string[];
  events: string[];
  generatedAt: number;
  isConfirmed: boolean;
}

// 角色卡
interface CharacterCard {
  id: string;
  bookId: string;
  name: string;
  aliases: string[];
  description: string;
  personality: string;
  background: string;
  goals: string;
  relationships: Relationship[];
  appearance: string;
  tags: string[];
  firstAppearanceChapterId: string;
  createdAt: number;
  updatedAt: number;
}

interface Relationship {
  characterId: string;
  characterName: string;
  relationType: string;
  description: string;
}

// 设定卡
interface SettingCard {
  id: string;
  bookId: string;
  type: 'item' | 'faction' | 'world' | 'custom';
  name: string;
  description: string;
  details: Record<string, string>;
  tags: string[];
  relatedChapterIds: string[];
  createdAt: number;
  updatedAt: number;
}

// 向量索引
interface VectorIndex {
  id: string;
  chapterId: string;
  embedding: number[];
  content: string;
  createdAt: number;
}
```

---

## 3. 基础AI功能模块

### 3.1 功能需求

#### FR-007 AI对话
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-007-1 | 基础AI对话（已接入OpenAI/Claude/DeepSeek等） | P0 | 已实现 |
| FR-007-2 | 上下文感知（注入当前章节内容） | P1 | 已实现 |
| FR-007-3 | 上下文感知（注入相关摘要） | P1 | 已实现 |
| FR-007-4 | 上下文感知（注入角色卡信息） | P1 | 待实现 |
| FR-007-5 | 对话历史保存与会话管理 | P1 | 已实现 |
| FR-007-6 | 多轮对话支持 | P0 | 已实现 |
| FR-007-7 | 流式输出响应 | P0 | 已实现 |
| FR-007-8 | AI工具调用（查询章节摘要/内容/列表） | P0 | 已实现 |

#### FR-008 润色功能
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-008-1 | 选中文本润色 | P0 | 待实现 |
| FR-008-2 | 润色风格选项（华丽/简洁/张力） | P1 | 待实现 |
| FR-008-3 | 润色结果对比显示 | P1 | 待实现 |
| FR-008-4 | 一键替换 | P1 | 待实现 |

#### FR-009 续写功能
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-009-1 | 基于上下文续写 | P0 | 待实现 |
| FR-009-2 | 续写字数控制 | P1 | 待实现 |
| FR-009-3 | 多版本续写供选择 | P2 | 待实现 |

#### FR-010 大纲生成
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-010-1 | 根据主题生成粗纲 | P1 | 待实现 |
| FR-010-2 | 粗纲细化成细纲 | P1 | 待实现 |
| FR-010-3 | 根据细纲生成章节骨架 | P2 | 待实现 |
| FR-010-4 | 大纲导入为实际章节结构 | P2 | 待实现 |

### 3.2 数据模型

```typescript
// AI配置
interface AIConfig {
  provider: 'openai' | 'claude' | 'deepseek' | 'custom';
  apiKey: string;
  apiUrl: string;
  model: string;
  temperature: number;
  maxTokens: number;
}

// 对话消息
interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: number;
  context?: ChatContext;
}

// 对话上下文
interface ChatContext {
  currentChapterId?: string;
  currentChapterTitle?: string;
  currentChapterContent?: string;
  relatedSummaries?: ChapterSummary[];
  relatedCharacters?: CharacterCard[];
  selectedText?: string;
}

// 润色请求
interface PolishRequest {
  text: string;
  style: 'elegant' | 'concise' | 'tense' | 'dialogue';
  keepMeaning: boolean;
}

// 续写请求
interface ContinueRequest {
  chapterId: string;
  contextLength: number;
  targetLength: number;
  direction?: string;
}

// 大纲
interface Outline {
  id: string;
  bookId: string;
  title: string;
  arcs: StoryArc[];
  createdAt: number;
  updatedAt: number;
}

interface StoryArc {
  id: string;
  title: string;
  description: string;
  chapters: OutlineChapter[];
}

interface OutlineChapter {
  id: string;
  title: string;
  summary: string;
  keyEvents: string[];
  characters: string[];
}
```

---

## 4. 对比决策模式(What-If)模块

### 4.1 功能需求

#### FR-011 分支推演
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-011-1 | 选中文本触发分支推演 | P2 | 待实现 |
| FR-011-2 | 生成3种分支（稳健/激进/黑天鹅） | P2 | 待实现 |
| FR-011-3 | 每种分支显示300字预览 | P2 | 待实现 |
| FR-011-4 | 蝴蝶效应分析 | P2 | 待实现 |
| FR-011-5 | 选择分支后替换原文 | P2 | 待实现 |

### 4.2 UI设计

```
+-------------------------------------------------------------+
|  What-If 分支推演                                            |
+-------------------------------------------------------------+
|  原文片段：                                                   |
|  +-----------------------------------------------------+   |
|  | {{selected_text}}                                    |   |
|  +-----------------------------------------------------+   |
+-----------------+-----------------+-------------------------+
|   A分支 (稳健)   |   B分支 (激进)   |   C分支 (黑天鹅)        |
|                 |                 |                         |
|  [剧情梗概]      |  [剧情梗概]      |  [剧情梗概]             |
|  100字描述      |  100字描述      |  100字描述             |
|                 |                 |                         |
|  [300字预览]    |  [300字预览]    |  [300字预览]           |
|  ...            |  ...            |  ...                   |
|                 |                 |                         |
|  蝴蝶效应：      |  蝴蝶效应：      |  蝴蝶效应：             |
|  - 伏笔1激活    |  - 角色A死亡    |  - 世界观改变           |
|  - 结局走向A    |  - 结局走向B    |  - 结局走向C           |
|                 |                 |                         |
|  [选择此分支]   |  [选择此分支]   |  [选择此分支]          |
+-----------------+-----------------+-------------------------+
```

---

## 5. 剧情时间轴可视化模块

### 5.1 功能需求

#### FR-012 时间轴生成
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-012-1 | 从章节摘要解析时间信息 | P2 | 待实现 |
| FR-012-2 | 从章节摘要解析地点信息 | P2 | 待实现 |
| FR-012-3 | 生成可视化时间轴 | P2 | 待实现 |
| FR-012-4 | 时间轴支持缩放/平移 | P2 | 待实现 |
| FR-012-5 | 点击时间轴节点跳转章节 | P2 | 待实现 |

#### FR-013 逻辑冲突检测
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-013-1 | 时间悖论检测 | P2 | 待实现 |
| FR-013-2 | 空间瞬移检测 | P2 | 待实现 |
| FR-013-3 | 人物状态冲突检测 | P2 | 待实现 |
| FR-013-4 | 在时间轴上标红预警 | P2 | 待实现 |

### 5.2 数据模型

```typescript
// 时间轴事件
interface TimelineEvent {
  id: string;
  bookId: string;
  chapterId: string;
  title: string;
  description: string;
  timestamp: string;           // 故事内时间
  location: string;
  characters: string[];
  eventType: 'scene' | 'battle' | 'dialogue' | 'transition';
  order: number;
}

// 冲突检测
interface TimelineConflict {
  id: string;
  bookId: string;
  conflictType: 'time' | 'space' | 'character';
  description: string;
  chapterIds: string[];
  detectedAt: number;
  isResolved: boolean;
}
```

---

## 6. 废稿回收站模块

### 6.1 功能需求

#### FR-014 废稿回收
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-014-1 | 监听编辑器剪切/删除操作 | P2 | 待实现 |
| FR-014-2 | 自动将删除的大段文本存入回收站 | P2 | 待实现 |
| FR-014-3 | 自动打标签（废弃设定/备用金句/待修改） | P2 | 待实现 |
| FR-014-4 | 手动添加废稿 | P2 | 待实现 |

#### FR-015 废稿召回
| 需求ID | 需求描述 | 优先级 | 实现状态 |
|--------|----------|--------|----------|
| FR-015-1 | 废稿库搜索 | P2 | 待实现 |
| FR-015-2 | AI推荐相关废稿 | P2 | 待实现 |
| FR-015-3 | 一键恢复废稿到当前章节 | P2 | 待实现 |
| FR-015-4 | 废稿引用统计 | P3 | 待实现 |

### 6.2 数据模型

```typescript
// 废稿
interface Scrap {
  id: string;
  bookId: string;
  sourceChapterId: string;
  content: string;
  tags: string[];
  category: 'abandoned_plot' | 'golden_line' | 'to_be_revised' | 'other';
  wordCount: number;
  createdAt: number;
  restoredAt?: number;
  restoredToChapterId?: string;
}

// 废稿搜索请求
interface ScrapSearchRequest {
  bookId: string;
  keywords: string;
  categories?: string[];
  dateRange?: [number, number];
}
```

---

## 7. 技术架构设计

### 7.1 整体架构

```
+-------------------------------------------------------------+
|                        前端层 (Frontend)                     |
|  +----------------+  +----------------+  +----------------+ |
|  |   Vue 3        |  |  Element Plus  |  |    Pinia       | |
|  |   (UI框架)     |  |  (组件库)      |  |  (状态管理)    | |
|  +----------------+  +----------------+  +----------------+ |
+-------------------------------------------------------------+
                              |
                              | Tauri IPC
                              v
+-------------------------------------------------------------+
|                        后端层 (Backend)                      |
|  +----------------+  +----------------+  +----------------+ |
|  |    Tauri       |  |   SQLite       |  |   LanceDB      | |
|  |   (Rust)       |  |  (结构化数据)   |  |  (向量数据库)   | |
|  +----------------+  +----------------+  +----------------+ |
|  +----------------+  +----------------+                     |
|  |   AI Service   |  |   File System  |                     |
|  |  (LLM API调用)  |  |  (本地文件存储) |                     |
|  +----------------+  +----------------+                     |
+-------------------------------------------------------------+
```

### 7.2 技术栈对比（初始设计 vs 当前实现）

| 层级 | 初始设计 | 当前实现 | 差异说明 |
|------|----------|----------|----------|
| 桌面端 | Tauri | Tauri | 一致 |
| 前端框架 | React | Vue 3 | 已采用Vue 3 |
| UI组件 | Tailwind CSS | Element Plus | 已采用Element Plus |
| 后端 | Rust + Python | Rust | 纯Rust实现，无需Python |
| 数据库 | SQLite | SQLite | 一致 |
| 向量库 | LanceDB | 未使用 | 已弃用，改用工具调用方案 |
| AI接口 | OpenAI/Claude | 已接入 | 已支持OpenAI/Claude/DeepSeek等 |

### 7.3 上下文管理策略

为确保AI不乱说话，采用 **"System Prompt + Tool Calling"** 策略：

1. **System Prompt 锁定**：明确告知AI角色和约束
2. **基础上下文注入**：
   - User Input (当前指令)
   - Current Chapter (当前章节全文)
   - All Chapter Summaries (所有章节的短摘要)
3. **AI主动查询**：通过工具调用让AI按需获取详细信息
   - `list_all_chapters` - 获取所有章节列表
   - `query_chapter_summary` - 查询指定章节的详细摘要
   - `query_chapter_content` - 查询指定章节的完整内容
4. **流式输出**：AI响应采用SSE流式输出，提升用户体验

### 7.4 模块依赖关系

```
核心写作模块 (基础)
       |
       v
+------+------+------+
|      |      |      |
v      v      v      v
AI对话  润色   续写   大纲生成
       |
       v
AI上下文引擎 (Tool Calling)
       |
       +------+------+
       |      |      |
       v      v      v
     摘要    角色卡  工具调用
       |
       v
+------+------+
|      |      |
v      v      v
What-If 时间轴 废稿回收
```

---

## 8. 数据库设计

### 8.1 现有表结构

```sql
-- 书籍表
CREATE TABLE books (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    current_chapter_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 卷表
CREATE TABLE volumes (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    title TEXT NOT NULL,
    "order" INTEGER NOT NULL,
    is_collapsed INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 章节表
CREATE TABLE chapters (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    volume_id TEXT NOT NULL,
    title TEXT NOT NULL,
    "order" INTEGER NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    word_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (volume_id) REFERENCES volumes(id) ON DELETE CASCADE
);

-- 章节快照表 (按章节独立存储)
CREATE TABLE chapter_snapshots (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    chapter_title TEXT NOT NULL,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    word_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX idx_chapter_snapshots_book_id ON chapter_snapshots(book_id);
CREATE INDEX idx_chapter_snapshots_chapter_id ON chapter_snapshots(chapter_id);
```

### 8.2 新增表结构（待实现）

```sql
-- 章节摘要表
CREATE TABLE chapter_summaries (
    id TEXT PRIMARY KEY,
    chapter_id TEXT NOT NULL UNIQUE,
    short_summary TEXT NOT NULL,
    long_summary TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]', -- JSON数组
    characters TEXT NOT NULL DEFAULT '[]',
    locations TEXT NOT NULL DEFAULT '[]',
    events TEXT NOT NULL DEFAULT '[]',
    generated_at INTEGER NOT NULL,
    is_confirmed INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
);

-- 角色卡表
CREATE TABLE character_cards (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    name TEXT NOT NULL,
    aliases TEXT NOT NULL DEFAULT '[]',
    description TEXT NOT NULL DEFAULT '',
    personality TEXT NOT NULL DEFAULT '',
    background TEXT NOT NULL DEFAULT '',
    goals TEXT NOT NULL DEFAULT '',
    appearance TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    first_appearance_chapter_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 人物关系表
CREATE TABLE character_relationships (
    id TEXT PRIMARY KEY,
    character_id TEXT NOT NULL,
    related_character_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (character_id) REFERENCES character_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (related_character_id) REFERENCES character_cards(id) ON DELETE CASCADE
);

-- 设定卡表
CREATE TABLE setting_cards (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    type TEXT NOT NULL, -- 'item', 'faction', 'world', 'custom'
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    details TEXT NOT NULL DEFAULT '{}', -- JSON对象
    tags TEXT NOT NULL DEFAULT '[]',
    related_chapter_ids TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 设定卡与章节关联表
CREATE TABLE setting_card_chapters (
    setting_card_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    PRIMARY KEY (setting_card_id, chapter_id),
    FOREIGN KEY (setting_card_id) REFERENCES setting_cards(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
);

-- 废稿表
CREATE TABLE scraps (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    source_chapter_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    category TEXT NOT NULL DEFAULT 'other',
    word_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    restored_at INTEGER,
    restored_to_chapter_id TEXT,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (source_chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
);

-- 时间轴事件表
CREATE TABLE timeline_events (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    story_timestamp TEXT,
    location TEXT NOT NULL DEFAULT '',
    characters TEXT NOT NULL DEFAULT '[]',
    event_type TEXT NOT NULL DEFAULT 'scene',
    "order" INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
);

-- 时间轴冲突表
CREATE TABLE timeline_conflicts (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    conflict_type TEXT NOT NULL,
    description TEXT NOT NULL,
    chapter_ids TEXT NOT NULL DEFAULT '[]',
    detected_at INTEGER NOT NULL,
    is_resolved INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 对话历史表
CREATE TABLE chat_histories (
    id TEXT PRIMARY KEY,
    book_id TEXT NOT NULL,
    chapter_id TEXT,
    title TEXT NOT NULL DEFAULT '新对话',
    messages TEXT NOT NULL DEFAULT '[]', -- JSON数组
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX idx_volumes_book_id ON volumes(book_id);
CREATE INDEX idx_chapters_book_id ON chapters(book_id);
CREATE INDEX idx_chapters_volume_id ON chapters(volume_id);
CREATE INDEX idx_chapter_snapshots_book_id ON chapter_snapshots(book_id);
CREATE INDEX idx_chapter_snapshots_chapter_id ON chapter_snapshots(chapter_id);
CREATE INDEX idx_chapter_summaries_chapter_id ON chapter_summaries(chapter_id);
CREATE INDEX idx_character_cards_book_id ON character_cards(book_id);
CREATE INDEX idx_setting_cards_book_id ON setting_cards(book_id);
CREATE INDEX idx_scraps_book_id ON scraps(book_id);
CREATE INDEX idx_timeline_events_book_id ON timeline_events(book_id);
CREATE INDEX idx_chat_histories_book_id ON chat_histories(book_id);
```

---

## 9. 开发里程碑

### 9.1 当前进度总览

| 模块 | 实现度 | 核心功能状态 |
|------|--------|--------------|
| 核心写作与项目管理 | 90% | 基础功能已完成，版本快照已优化 |
| AI上下文引擎 | 60% | 章节摘要已实现，角色卡待开发 |
| 基础AI功能 | 70% | AI对话已完善，工具调用已接入 |
| 对比决策模式 | 0% | 待开发 |
| 剧情时间轴 | 0% | 待开发 |
| 废稿回收站 | 0% | 待开发 |

### 9.2 详细里程碑

#### Phase 1: 核心功能完善 (已完成)
- **周期**: 2周
- **交付物**:
  - Tauri应用框架搭建
  - Vue 3 + Element Plus前端
  - SQLite数据库集成
  - 书籍/卷/章节的CRUD
  - 富文本编辑器集成
  - 自动保存功能
  - 版本快照功能

#### Phase 2: AI上下文引擎 (已完成核心功能)
- **周期**: 2周
- **交付物**:
  - ✅ 章节摘要自动生成（短摘要+长摘要+标签提取）
  - ⏳ 角色卡管理系统（待开发）
  - ⏳ 设定卡管理系统（待开发）
  - ❌ LanceDB向量数据库集成（已弃用）
  - ✅ 工具调用方案替代RAG
- **优先级**: P0
- **依赖**: Phase 1
- **变更说明**: 向量检索方案改为工具调用方案，AI通过工具主动查询章节信息

#### Phase 3: 基础AI功能增强 (已完成核心功能)
- **周期**: 2周
- **交付物**:
  - ✅ AI对话上下文感知（当前章节+所有摘要+工具调用）
  - ✅ 流式输出响应
  - ✅ 对话历史保存与会话管理
  - ⏳ 润色功能（待开发）
  - ⏳ 续写功能（待开发）
  - ⏳ 大纲生成功能（待开发）
- **优先级**: P0
- **依赖**: Phase 2

#### Phase 4: 高级AI功能
- **周期**: 3周
- **交付物**:
  - What-If分支推演
  - 剧情时间轴可视化
  - 逻辑冲突检测
  - 废稿回收站
- **优先级**: P1
- **依赖**: Phase 3

#### Phase 5: 打磨与优化
- **周期**: 2周
- **交付物**:
  - UI/UX优化
  - 性能优化
  - Bug修复
  - Beta测试
- **优先级**: P1
- **依赖**: Phase 4

### 9.3 功能优先级矩阵

| 功能 | 用户价值 | 技术难度 | 优先级 | 建议阶段 |
|------|----------|----------|--------|----------|
| 章节摘要生成 | 高 | 中 | P0 | Phase 2 |
| 角色卡管理 | 高 | 低 | P0 | Phase 2 |
| AI上下文感知 | 高 | 中 | P0 | Phase 3 |
| 润色功能 | 高 | 低 | P0 | Phase 3 |
| 续写功能 | 高 | 中 | P0 | Phase 3 |
| 工具调用 | 高 | 中 | P0 | 已完成 |
| 大纲生成 | 中 | 中 | P1 | Phase 3 |
| What-If模式 | 中 | 高 | P2 | Phase 4 |
| 时间轴可视化 | 中 | 高 | P2 | Phase 4 |
| 废稿回收站 | 低 | 低 | P2 | Phase 4 |
| 全屏写作模式 | 中 | 低 | P1 | Phase 1+ |
| 查找替换 | 中 | 低 | P1 | Phase 1+ |

### 9.4 技术债务与优化项

| 问题 | 影响 | 建议解决方案 | 优先级 |
|------|------|--------------|--------|
| 章节标题编辑有时不保存 | 用户体验 | 优化事件处理逻辑 | P1 |
| 大数据量时卡顿 | 性能 | 虚拟滚动 + 分页加载 | P1 |
| AI对话无历史记录 | 功能缺失 | 增加chat_histories表 | P1 |
| 缺少向量数据库 | AI功能受限 | 集成LanceDB | P0 |
| 编辑器功能有限 | 用户体验 | 评估替换或增强编辑器 | P2 |

---

## 附录

### A. Prompt模板汇总

#### A.1 系统提示词 (System Prompt)
```markdown
你是NovelForge AI，一位专业的小说写作助手。你的职责是帮助作者创作长篇小说。

## 核心原则
1. 绝不擅自修改用户的原文设定
2. 保持人物性格和行为的一致性
3. 尊重已有的世界观和规则
4. 提供建设性的建议，而非替用户做决定

## 当前任务上下文
- 书籍: {{book_title}}
- 当前章节: {{chapter_title}}
- 涉及角色: {{character_names}}
- 相关摘要: {{related_summaries}}
```

#### A.2 章节摘要生成
```markdown
请为以下小说章节生成摘要：

## 章节内容
{{chapter_content}}

## 输出格式 (JSON)
{
  "shortSummary": "50字以内",
  "longSummary": "500字以内",
  "tags": ["标签1", "标签2"],
  "characters": ["角色名"],
  "locations": ["地点"],
  "events": ["事件"]
}
```

#### A.3 角色提取
```markdown
从以下章节内容中提取所有角色信息：

## 章节内容
{{chapter_content}}

## 输出格式 (JSON数组)
[
  {
    "name": "角色名",
    "aliases": ["别名"],
    "description": "角色描述",
    "personality": "性格特点"
  }
]
```

#### A.4 润色
```markdown
请润色以下文本，风格：{{style}}

## 原文
{{text}}

## 要求
- 保持原意
- 提升表现力
- 符合小说风格

直接输出润色后的文本。
```

#### A.5 What-If分支推演
```markdown
为以下情节生成3种不同的发展分支：

## 当前情节
{{selected_text}}

## 分支要求
1. 稳健分支：符合逻辑的正常发展
2. 激进分支：制造冲突和反转
3. 黑天鹅分支：意外事件改变走向

## 输出格式 (JSON)
{
  "branches": [
    {
      "type": "稳健",
      "summary": "100字梗概",
      "preview": "300字预览",
      "butterflyEffect": ["影响1", "影响2"]
    }
  ]
}
```

### B. 术语表

| 术语 | 说明 |
|------|------|
| RAG | Retrieval-Augmented Generation，检索增强生成 |
| OOC | Out of Character，角色行为偏离设定 |
| P0/P1/P2 | 优先级，P0最高 |
| MVP | Minimum Viable Product，最小可行产品 |
| Tauri | 基于Rust的桌面应用框架 |
| LanceDB | 嵌入式向量数据库 |

---

**文档结束**

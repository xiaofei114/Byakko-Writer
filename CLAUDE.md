# CLAUDE.md - 白喵写作 (Byakko Writer) 项目指南

> **重要提示**: 根据项目规则，Claude 不应主动执行以下操作：
> - 不要自动重启应用
> - 不要自动运行项目
> - 不要自动安装依赖
> - 如果需要执行命令，只提供命令语句让用户自己执行

## 项目概述

白喵写作 (Byakko Writer) 是一款基于 Tauri + Vue 3 的桌面端小说写作应用，专为小说创作者设计，集成 AI 辅助功能，帮助作者更高效地创作。

## 技术栈

- **前端**: Vue 3 + TypeScript + Element Plus + Pinia
- **后端**: Rust + Tauri
- **数据库**: SQLite (通过 sqlx 访问)
- **构建工具**: Vite

## 项目结构

```
byakko-writer/
├── src/                    # 前端源码
│   ├── components/         # Vue 组件
│   │   ├── Sidebar.vue     # 侧边栏（卷/章节树）
│   │   ├── WritingArea.vue # 编辑区域
│   │   ├── AIChat.vue      # AI 对话面板（工具卡片/润色/行编辑）
│   │   ├── OutlineEditor.vue    # 独立大纲编辑器组件（当前页面内联实现）
│   │   ├── CharacterCardManager.vue  # 角色卡管理
│   │   ├── SnapshotManager.vue  # 版本快照管理与对比
│   │   ├── QuickReferencePanel.vue   # 速查面板（角色/大纲）
│   │   ├── ConflictSidebar.vue # 设定冲突面板
│   │   ├── CanonLedgerDialog.vue # 设定账本管理（查看/纠错/回填）
│   │   └── SettingsDialog.vue   # 设置面板
│   ├── stores/             # Pinia 状态管理
│   │   ├── book.ts         # 书籍/卷/章/快照/角色卡/大纲
│   │   └── config.ts       # 配置（AI、主题、字体、保存间隔）
│   ├── views/              # 页面视图
│   │   ├── Home.vue        # 首页（书籍列表）
│   │   └── Editor.vue      # 编辑器页面（含大纲编辑模式）
│   ├── api/                # 前端 API 封装
│   │   ├── aiChat.ts       # AI 对话/润色（事件流）
│   │   ├── aiFeatures.ts   # 章节摘要
│   │   └── storyMemory.ts  # 故事记忆
│   ├── config/appConfig.ts # 字体、主题、AI 提供商预设、工具显示名
│   ├── utils/              # 事件总线、错误处理、Markdown 渲染与 HTML 清洗
│   └── types/index.ts      # TypeScript 类型定义
├── src-tauri/              # Tauri 后端
│   ├── prompts/            # AI 提示词（编译期嵌入；仅开发模式读用户目录覆盖）
│   │   ├── agents/         # 决策/写作/润色/压缩/冲突/故事圣经/设定抽取
│   │   └── style/          # 摘要与写作风格分析
│   └── src/
│       ├── main.rs         # 应用入口与命令注册
│       ├── db.rs           # 数据库初始化、建表与迁移
│       ├── models/         # 数据模型（serde）
│       ├── commands/       # Tauri 命令层（薄封装）
│       └── services/       # 业务服务层
│           ├── agent_orchestrator.rs # 决策 AI 编排（工具循环/压缩）
│           ├── chat_service.rs       # 会话与消息持久化
│           ├── tool_call_service.rs  # 工具执行
│           ├── prompt_service.rs     # 提示词加载
│           ├── book_service.rs       # 书籍/卷/章
│           ├── summary_service.rs    # 章节摘要读写
│           ├── summary_generator_service.rs # 摘要批量生成
│           ├── story_memory_service.rs # 故事圣经（分组摘要）
│           ├── conflict_service.rs   # 设定冲突检测
│           ├── style_service.rs      # 写作风格学习
│           ├── auto_style_service.rs # 后台风格/冲突定时任务
│           ├── auto_story_memory.rs  # 后台故事记忆定时任务
│           ├── character_service.rs  # 角色卡
│           ├── outline_service.rs    # 大纲
│           ├── snapshot_service.rs   # 章节快照（每本书独立 SQLite）
│           ├── canon_service.rs      # 设定账本（实体/别名/状态/事件）
│           ├── secret_service.rs     # API Key 加密（Windows DPAPI）
│           └── config_service.rs     # 配置读写
├── package.json
└── README.md
```

## 数据库结构

主库：`%APPDATA%/byakko-writer/data.db`（sqlx，启用外键，删除级联）
快照库：`%APPDATA%/byakko-writer/books/<book_id>/chapter_snapshots.db`（每本书独立）

### 表结构

- **books**: 书籍信息（含 author / description / current_chapter_id）
- **volumes**: 卷信息
- **chapters**: 章节信息（正文直接存库）
- **outlines**: 大纲（书籍/卷/章节三级，coarse+fine）
- **chapter_summaries**: 章节摘要（短/长摘要、标签、角色、事件、伏笔等）
- **chat_sessions** / **chat_messages**: AI 会话与消息（含工具调用摘要、行编辑、ask_user）
- **character_cards**: 角色卡
- **user_writing_styles** / **book_writing_styles**: 全局/书籍级写作风格
- **detected_conflicts**: 设定冲突（含 severity）
- **conflict_check_progress**: 冲突检测进度
- **story_memory** / **story_memory_groups**: 故事圣经与分组缓存
- **canon_entities** / **canon_aliases** / **canon_states** / **canon_events**: 设定账本（实体、别名归一、状态时间线、事件流）

## 常用命令

```bash
# 开发模式
pnpm tauri dev

# 构建生产版本
pnpm tauri build

# 仅构建前端
pnpm build

# 检查 Rust 代码
cd src-tauri && cargo check

# 运行 Rust 单元测试（快照 diff、DPAPI 加解密）
cd src-tauri && cargo test

# 前端类型检查
pnpm exec vue-tsc --noEmit
```

## 开发注意事项

### 添加新的 Tauri 命令

1. 在 `src-tauri/src/services/` 下创建或修改服务文件
2. 在 `src-tauri/src/main.rs` 中导入并注册命令
3. 前端通过 `invoke('command_name', args)` 调用

### 数据库操作

- 使用 `get_pool().await` 获取数据库连接池
- 使用 sqlx 的 query/query_as 执行 SQL
- 所有数据库操作都是异步的
- 数据库迁移在 `src-tauri/src/db.rs` 中管理

### 状态管理

- 使用 Pinia 管理状态
- `book.ts` 管理当前书籍、章节、内容、快照
- `config.ts` 管理应用配置（AI 配置、主题等）

### 样式规范

- 使用 Element Plus 组件
- 主题色通过 CSS 变量 `--el-color-primary` 控制
- 支持深色/浅色主题切换
- 侧边栏和 AI 面板支持拖拽调整宽度

### AI 功能架构

- **工具调用方案**（决策 AI = `prompts/agents/decision_agent.md`，最多 `max_rounds` 轮）
  - 查询类：`get_book_info` / `list_all_chapters` / `query_chapter_summary` / `query_chapter_content` / `search_chapter_content` / `get_outline` / `get_volume_outline` / `list_character_cards` / `get_character_card` / `get_story_memory` / `get_character_timeline` / `list_chapters_in_volume` / `get_canon` / `search_canon`
  - 修改类（一次提交多项，前端一张卡片批量确认）：
    - `propose_edits`：批量正文修改（原文锚定 + 可选整章替换），应用前自动创建快照
    - `propose_outlines`：批量大纲修改（书籍/卷/章节 × 粗纲/细纲）
    - `propose_line_edit` / `save_outline`：旧工具，内部会转成上面的提案（兼容历史提示词）
  - 写入类：`create_character_card` / `update_character_card` / `create_chapter` / `report_conflict`
  - 生成类：`write_chapter`（转交写作 AI）/ `learn_writing_style` / `ask_user`（暂停等待用户答复）
- **流式输出**: AI 响应采用 SSE 流式输出，通过 `ai-chat-stream` / `ai-write-chunk` 等事件推送到前端
- **提示词管理**: 提示词在编译期 `include_str!` 嵌入。**仅开发模式**（`debug_assertions`）会优先读取 `src-tauri/prompts/`（项目内，改完重启生效），其次 `%APPDATA%/byakko-writer/prompts/`；发布版一律使用内置提示词，不读磁盘、不写用户目录
- **设定账本（Canon Ledger）**: 章节摘要落库后自动抽取实体/别名/状态/事件到 `canon_*` 表；AI 用 `get_canon`/`search_canon`/`get_canon_timeline` 精确核对；写作上下文自动注入相关设定卡；改稿 30 秒后自动重抽该章；界面可查看/纠错/一键回填
- **上下文压缩**: 每 3 轮用户消息后台压缩一次历史，压缩结果写入 system prompt 的 `{{COMPRESSED_CONTEXT}}`

## 核心功能实现

### AI 润色功能
1. 用户选中文本，点击润色按钮
2. 发送选中文本 + 润色提示词到 AI
3. AI 返回润色后的文本
4. 显示对比界面，用户选择是否应用

### 大纲生成功能
1. 用户请求生成大纲
2. AI 查询相关章节内容
3. 生成粗纲和细纲
4. 显示生成结果，提供单独保存按钮
5. 保存后触发事件刷新大纲编辑器

### 写作风格学习
1. 后台服务定期分析用户作品
2. 选取代表性章节（开头、中间、结尾）
3. AI 分析语言风格、叙事方式等
4. 存储风格特征；**润色与正文生成（`write_chapter`）都会注入学习结果**

## 常见问题

### 数据库连接失败
- 检查 `%APPDATA%/byakko-writer` 目录是否存在且有写入权限
- 数据库文件会在首次启动时自动创建

### 前端调用后端命令报错
- 检查命令名是否正确
- 检查参数类型是否匹配
- 查看 Rust 编译错误信息

### AI 流式输出中断
- 检查网络连接
- 查看 AI 配置是否正确
- 检查控制台错误日志

## 代码规范

### Rust
- 使用 anyhow 处理错误
- 异步函数使用 `async/await`
- 数据库操作使用 sqlx
- 服务层按功能模块化

### Vue
- 使用 Composition API + `<script setup>`
- 组件名使用 PascalCase
- Props 使用 TypeScript 类型定义
- 事件使用 camelCase

### 类型
- 前后端共享类型定义，保持同步
- 使用 TypeScript 严格模式
- Rust 模型使用 serde 序列化

## 待优化项

- [x] 章节标题编辑有时不保存（改为输入时捕获章节ID+标题快照，防抖写入）
- [x] 长篇「吃书」治理：设定账本 P1 完成（`canon_*` 表 + 自动抽取 + `get_canon`/`search_canon`/`get_canon_timeline` + 写作注入 + 账本管理界面 + 一键回填 + 改稿自动重抽 + 快照回滚联动）
- [ ] 性能优化（大数据量时卡顿；`load_book` 会一次性读取全部章节正文）
- [ ] `AIChat.vue`（约 2300 行）拆分为多个子组件
- [ ] 关闭窗口前用 Tauri `onCloseRequested` 同步保存（当前 `beforeunload` 中的异步快照不可靠）
- [ ] 导出功能（Word/PDF/Markdown）
- [ ] 拼写检查
- [ ] 写作数据统计
- [ ] 云同步

## 更新日志

### 未发布（思考模式）
- **卡住修复**：思考流改为「按内容」判活——45 秒没有新内容（keep-alive 空事件不算）就中止并收尾；请求 90 秒无响应直接跳过；写作/润色流 60 秒无数据也会中止，避免界面永久停在「思考中/创作中」
- **中断可见**：思考被中止时内容末尾标注「（思考中断，已停止）」，不再无限转圈
- **多轮反思**：不再只在开头想一次。**每批工具执行后**自动反思（拿到了什么 / 是否足够 / 有无矛盾 / 下一步），结果注入下一轮 system prompt；前端按「第 N 轮反思」独立分块展示，最多 5 次/轮
- 纯提案/写作类工具批次（`propose_edits` 等）属于收尾动作，不再触发反思，避免白花钱
- **流式思考**：思考计划改为流式请求（`send_ai_message_stream`），边生成边推到前端「思考过程」卡片；支持模型原生 `reasoning_content`（DeepSeek reasoner / Qwen 等）与普通 `content`
- **思考更充分**：提示词升级为「需求拆解 / 信息核对 / 方案对比 / 执行计划 / 自检清单」，目标 400-800 字；上下文加入故事圣经节选与最近 8 条对话
- **修复思考被吞**：思考消息改为按 `start/chunk/done` 复用同一条消息并插在空占位气泡前，不再每条事件新建/覆盖；流式时显示「思考中…」并自动滚动，完成后可折叠
- **上下文**：思考请求会带上当前章节、正在编辑的大纲目标、故事圣经节选、最近 8 条对话
- **可回看**：思考过程以 `role='thinking'` 落库，刷新/切换会话后仍显示；不进入后续对话上下文
- **设置**：设置 → AI → 思考模式（默认开启）；每轮多一次小请求，可关闭

### 未发布（写作风格接入）
- **决策 AI 接入风格**：`decision_agent.md` 增加 10 条「正文写作铁律」；对话时动态注入学到的个人写作风格。此前 `propose_edits` 生成的新文本（AI 改正文的主路径）完全没有风格约束
- **润色显式合并个人风格**：`{{STYLE_GUIDE}}` 现在包含通用指南 + 学到的个人风格（之前个人风格只在 JSON 上下文里，容易被忽略）
- **开发模式提示词优先级**：优先读项目内 `src-tauri/prompts/`，其次 `%APPDATA%/byakko-writer/prompts/`，避免改了项目文件却被旧副本覆盖
- **可验证**：写作 / 润色 / 决策三处都会在日志打印风格指南字数，方便确认是否真的注入

### 未发布（对话体验）
- **修复空白 AI 气泡**：`ask_user` 暂停时后端现在会补发完成事件，前端不再卡在工具轮；并在提案/写作/冲突/行编辑/提问卡片插入前、流式完成时清理空占位气泡
- **输入框上方待回答面板**：`ask_user` 的问题与选项固定在输入框上方（可折叠/展开），支持多个待回答连续点选、面板内「其他…」输入，不用再往上翻找消息

### 未发布（快照落库 + 冲突联动 + 代码复查）
- **快照恢复真正落库**：新增命令 `restore_chapter_snapshot`，恢复前自动备份「恢复前」快照，正文/标题写入数据库，并触发摘要失效、账本重抽、冲突待复核
- **冲突联动**：`detected_conflicts` 增加 `chapter_id` / `needs_review` / `resolved_at`；AI 报告冲突可带章节；章节改稿后相关冲突自动标「可能已修复，待复核」；面板新增「已解决」标记与分区
- **摘要一致性**：整章替换 / 快照回滚后删除旧摘要，自动摘要重新生成，避免摘要与新正文脱节
- **复查修复**：
  - `write_chapter` 缺少 chapterId 时后端直接拒绝（此前会生成无法落库的内容）
  - 工具结果截断后明确告知 AI「已截断，可继续查询」（此前 AI 误以为看全）
  - 账本列表/写作注入改为批量取别名（消除 N+1 查询）
  - 人工改别名时清理旧归属，避免别名挂在两个实体上
  - 抽取结果为空时保留本章旧设定（防止模型抖动清空事实）
  - 冲突检测报告补充可选 `chapterId`，便于后续联动

### 未发布（设定账本管理 + 提示词策略）
- **提示词只读策略**：提示词目录改为仅开发模式启用；发布版固定使用编译期内置提示词，不读磁盘、不写 `%APPDATA%`
- **账本管理界面**：编辑器右侧新增「设定账本」入口，可搜索/按类型筛选、查看实体设定卡、编辑名称/类型/简介/别名、删除实体、删除单条状态/事件
- **一键回填**：界面触发全书逐章重抽（后台执行，进度条实时反馈，并发限制 3）
- **状态时间线工具**：新增 `get_canon_timeline`（某实体在指定章节区间的状态变化与事件）
- **改稿自动重抽**：章节保存 30 秒后自动重抽该章（仅当书里已有账本数据且内容比上次抽取新）；重抽会先删本章旧状态/事件再写新的，避免改稿后残留旧事实
- **账本过期判断**：新增 `canon_extractions` 记录每章抽取时的内容版本；实体加 `manual` 标记，人工改过的简介不会被自动覆盖

### 未发布（长篇连续性与 AI 体验）
- **自动摘要**：章节保存 20 秒后自动为「内容 ≥ 600 字且尚无摘要」的章节生成摘要（防抖、去重、后台串行），摘要随后自动喂给设定账本与故事圣经
- **设定账本 P1**：新增 `canon_entities` / `canon_aliases` / `canon_states` / `canon_events` 四张表；摘要落库后后台抽取单章事实（并发限制 3）；新增工具 `get_canon`（按名/别名查设定卡+状态时间线+最近事件）、`search_canon`（模糊检索）；写作时自动注入相关设定卡
- **大纲上下文透传**：编辑器把当前正在编辑的大纲层级/ID 同步到 store，聊天请求带上 `outlineTarget`，AI 知道“这个大纲/这里”指哪一层
- **决策上下文节流**：工具结果进对话历史时单条截断到 8000 字
- **提示词加载**：提示词编译期嵌入；仅开发模式会优先读 `%APPDATA%/byakko-writer/prompts/`（首次启动自动写入默认文件），发布版固定使用内置提示词

### 未发布（AI 能力优化）
- **流式解析修复**：SSE 改为跨块缓冲按事件解析（`ai_service::extract_sse_deltas`），网络分块切开 `data:` 行时不再丢字/丢句；流结束时冲刷尾部事件
- **写作长度**：`write_chapter` 输出 token 上限提升到至少 8000，润色至少 6000，避免长章节被截断
- **会话恢复**：进程重启后从数据库恢复最近 8 条对话作为 Agent 上下文，AI 不再「失忆」
- **写作上下文增强**：自动注入上下文里出现的角色卡（姓名/别名匹配，含性格/外貌/背景/关系），以及上一章结尾约 800 字，减少续写断层
- **上下文节流**：写作上下文只保留最近 3 轮需求/指导和最近 6 条工具结果（单条 4000 字、总量 12000 字）；`query_chapter_content` 全文超过 12000 字时截断并提示用行号续读；`get_story_memory` 限制 12000 字
- **性能**：`get_book_info` 改为纯 SQL 统计，不再 `load_book` 读取全书正文
- **体验**：描述含「重写/替换」时写作结果默认切换为「替换全文」；缺少章节 ID 时插入会明确报错
- **压缩提示词**：要求专有名词/数字/章节号原样保留，用户偏好与禁忌逐条留下，不得编造

### 未发布（AI 批量修改）
- 新增 `propose_edits`：一次提交多处正文修改，用原文锚定；后端先校验能否定位，前端一张卡片全选 / 一键应用；应用前自动创建「AI 修改前」快照
- 新增 `propose_outlines`：一次提交多层级（书籍/卷/章节）+ 多类型（粗纲/细纲）大纲变更，一张卡片一次性保存
- 旧工具 `propose_line_edit` / `save_outline` 保留兼容，内部统一转成新的提案流程
- 修复「AI 改不了正文」：应用改成后端落库（不再依赖当前打开的是哪一章）
- 新增命令：`apply_edit_proposal` / `apply_outline_proposal`
- 决策提示词改为「修改统一用 propose_edits / propose_outlines，一次提交全部改动，不要逐条调用」

### 未发布（长篇连续性优化）
- `search_chapter_content`：自动展开角色别名（小动/林兄 → 林动）、按相关度排序、返回章节ID+行号+上下文片段，并限制最多 30 处，避免一次塞爆上下文
- `query_chapter_summary`：补返回剧情推进 / 情感节点 / **伏笔** / **未解决线索**（此前存了但没给 AI 看）
- 决策 context 瘦身：章节列表只注入「当前卷 + 前后约 20 章」，故事圣经按 6000 字预算裁剪（设定类优先、时间线先丢），并注入当前章节用于裁剪
- 决策提示词补充「按预算裁剪、查不到不要编、核对连续性」的规则
- 设定账本（Canon Ledger）设计稿完成：`需求文档/设定账本设计方案.md`

### 未发布（维护性修复）
- 新建书籍时会自动创建「第一卷」「第一章」，不再出现建书失败
- 快照对比接口与前端对齐（返回 `old_snapshot` / `new_snapshot` / `diff_blocks`），并改为真正的 LCS 行级 diff
- 修复上下文压缩后会丢掉用户最新一条消息的问题
- API Key 改为 Windows DPAPI 加密存储，读取历史明文时自动迁移
- 收紧 WebView CSP，AI 生成的 Markdown 渲染前做 HTML 白名单清洗
- 快照数据库改为按书籍复用连接池，删除书籍/卷/章节时同步清理快照
- 学习到的写作风格会注入正文生成（此前只影响润色）
- 「自动更新故事记忆」开关真正生效；后台冲突检测结果会在界面提示
- 建章不再触发全书摘要批量生成；摘要生成并发限制为 4
- 修复中文截断可能 panic、会话状态无上限、`create_chapter` 工具 SQL 列名错误等问题

### v0.0.5-beta
- 基础写作功能
- AI 对话与润色
- 大纲生成（粗纲/细纲）
- 角色卡管理
- 版本快照与对比
- 写作风格学习
- 深色/浅色主题

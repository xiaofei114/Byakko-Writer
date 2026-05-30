<div align="center">

<img src="src-tauri/icons/128x128.png" alt="白喵写作" width="128" height="128">

# 白喵写作

**Byakko Writer** — 专为小说创作者设计的 AI 辅助写作工具

[![Version](https://img.shields.io/badge/version-0.0.5-beta/badge.svg)](https://github.com/xiaofei114/Byakko-Writer/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.5-4FC08D?logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-1.75-000000?logo=rust)](https://www.rust-lang.org)

[下载](https://github.com/xiaofei114/Byakko-Writer/releases) • [问题反馈](../../issues)

</div>

---

## 特性

<table>
<tr>
<td width="50%">

### 专业写作体验
- 多书籍、多卷、多章节管理
- 富文本编辑器，专注写作
- 自动保存，版本快照
- 深色/浅色主题切换

</td>
<td width="50%">

### AI 智能辅助
- 实时 AI 对话，获取创作建议
- 智能润色，提升文笔
- 自动生成粗纲/细纲
- 角色卡管理
- 写作风格学习

</td>
</tr>
</table>

## 快速开始

### 下载安装

前往 [Releases](https://github.com/xiaofei114/Byakko-Writer/releases) 页面下载对应平台的安装包。

**支持平台：**
- Windows 10/11 (x64)
- macOS 11+ (Intel/Apple Silicon)
- Linux (AppImage)

### 从源码构建

**环境要求：**
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/) 8+
- [Rust](https://www.rust-lang.org/) 1.75+

```bash
# 克隆仓库
git clone https://github.com/xiaofei114/Byakko-Writer.git
cd byakko-writer

# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建生产版本
pnpm tauri build
```

## 使用指南

### 创建书籍

1. 启动应用，点击首页的「新建书籍」
2. 输入书名、作者等信息
3. 系统会自动创建第一章，点击即可开始写作

### 配置 AI

1. 点击右上角设置图标
2. 在「AI 设置」中配置 API 密钥
3. 支持 OpenAI 兼容接口（如 OpenRouter、SiliconFlow 等）

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl + S` | 保存 |
| `Ctrl + Z` | 撤销 |
| `Ctrl + Shift + Z` | 重做 |
| `Ctrl + B` | 粗体 |
| `Ctrl + I` | 斜体 |

## 技术架构

```
┌─────────────────────────────────────────┐
│              前端 (Frontend)              │
│  ┌─────────┐ ┌─────────┐ ┌────────────┐ │
│  │  Vue 3  │ │Element+ │ │   Quill    │ │
│  └─────────┘ └─────────┘ └────────────┘ │
│  ┌─────────┐ ┌─────────┐ ┌────────────┐ │
│  │  Pinia  │ │ Tauri API│ │   Vite     │ │
│  └─────────┘ └─────────┘ └────────────┘ │
└─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────┐
│              后端 (Backend)               │
│  ┌─────────┐ ┌─────────┐ ┌────────────┐ │
│  │  Tauri  │ │   Rust  │ │    SQLx    │ │
│  └─────────┘ └─────────┘ └────────────┘ │
│  ┌─────────┐ ┌─────────┐ ┌────────────┐ │
│  │  Reqwest│ │  Tokio  │ │  SQLite    │ │
│  └─────────┘ └─────────┘ └────────────┘ │
└─────────────────────────────────────────┘
```

## 项目结构

```
byakko-writer/
├── src/                      # 前端源码
│   ├── components/           # Vue 组件
│   │   ├── Sidebar.vue       # 侧边栏（卷/章节树）
│   │   ├── WritingArea.vue   # 写作区域
│   │   ├── AIChat.vue        # AI 对话面板
│   │   └── ...
│   ├── stores/               # Pinia 状态管理
│   ├── views/                # 页面视图
│   └── api/                  # API 封装
├── src-tauri/                # Tauri 后端
│   ├── src/
│   │   ├── main.rs           # 应用入口
│   │   ├── services/         # 业务服务
│   │   ├── models/           # 数据模型
│   │   └── db.rs             # 数据库管理
│   ├── prompts/              # AI 提示词
│   └── icons/                # 应用图标
├── docs/                     # 文档
└── package.json
```

## 数据存储

应用数据存储在系统标准目录：

| 平台 | 路径 |
|------|------|
| Windows | `%APPDATA%\byakko-writer\` |
| macOS | `~/Library/Application Support/byakko-writer/` |
| Linux | `~/.local/share/byakko-writer/` |

包含文件：
- `data.db` - SQLite 数据库（书籍、章节、大纲等）
- `config.json` - 应用配置
- `books/` - 书籍快照数据

## 路线图

- [x] 基础写作功能
- [x] AI 对话与润色
- [x] 大纲生成（粗纲/细纲）
- [x] 角色卡管理
- [x] 版本快照与对比
- [x] 深色/浅色主题
- [ ] 导出功能（Word/PDF/Markdown）
- [ ] 拼写检查
- [ ] 写作数据统计
- [ ] 云同步
- [ ] 协作写作

## 贡献

欢迎提交 Issue 和 Pull Request！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 [MIT](LICENSE) 许可证。

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Vue.js](https://vuejs.org/) - 前端框架
- [Element Plus](https://element-plus.org/) - UI 组件库
- [Quill](https://quilljs.com/) - 富文本编辑器

---

<div align="center">

**[⬆ 回到顶部](#白喵写作)**

Made with ❤️ by [xiaofei114](https://github.com/xiaofei114)

</div>

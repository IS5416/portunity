# 技术栈调研

**领域：** Windows 端口管理桌面工具（双前端：Tauri GUI + Ratatui TUI）
**调研日期：** 2026-07-26
**置信度：** HIGH

## 推荐技术栈

### 核心框架

| 技术 | 版本 | 用途 | 理由 |
|------|------|------|------|
| Rust | edition 2024 (1.85+) | 系统语言 | 项目约束要求；与 Win32 互操作的零成本抽象 |
| Tauri | 2.11.3 | 桌面 GUI 框架 | 成熟的 v2 稳定版，IPC、插件系统、托盘支持。截至 2026 年 6 月最新版 |
| Svelte | 5.25.8 (SvelteKit 2.20.4) | GUI 前端框架 | 编译输出，无虚拟 DOM 运行时，最小打包体积。Tauri 官方推荐搭配 |
| Ratatui | 0.30.2 | 终端 TUI 框架 | Rust TUI 行业标准。v0.30 重构为子 crate 架构 |
| Tokio | 1.50.0 | 异步运行时 | 事件驱动非阻塞 I/O。Rust 异步事实标准。多线程工作窃取调度器 + Windows IOCP |

### Windows 系统 API

| 技术 | 版本 | 用途 | 理由 |
|------|------|------|------|
| windows (windows-rs) | 0.73 | 官方 Win32 API 绑定 | 微软维护。从 Windows metadata 自动生成 Rust 绑定。覆盖 IP Helper、Process API、ETW、WFP。替代废弃的 winapi |
| sysinfo | 0.39.3 | 跨平台系统信息 | 进程枚举、CPU/内存统计。v0.39+ 新增 `Process::kill_and_wait()` |
| ferrisetw | 1.2.0 | ETW 事件消费者 | 安全的 Rust ETW 抽象。用于实时网络事件订阅。自 2024 年 6 月稳定 |
| windows-wfp | 0.2.1 | Windows 过滤平台（防火墙） | 自动 DOS→NT 路径转换（WFP 关键需求）、RAII 引擎管理、builder 模式 FilterRule API |
| tray-icon | 0.24.1 | 系统托盘集成 | Tauri 内部使用：`tauri = { features = ["tray-icon"] }` |

### 数据库

| 技术 | 版本 | 用途 | 理由 |
|------|------|------|------|
| rusqlite | 0.40.1 | 核心库 SQLite 访问 | 直接、同步、最小依赖。用于 port-core 内的历史、收藏、标签、设置。bundled 功能编译自带 SQLite |
| tauri-plugin-sql | 2.4.0 | GUI 前端 SQLite | Tauri 官方插件。内部使用 sqlx。仅用于 GUI 前端自身查询；核心逻辑直接用 rusqlite |

### 序列化

| 技术 | 版本 | 用途 |
|------|------|------|
| serde | 1.0.229 | 通用序列化框架，8.15 亿+ 下载量 |
| serde_json | 1.0.149 | JSON 序列化（Tauri IPC 格式） |

### 错误处理与可观测性

| 技术 | 版本 | 用途 |
|------|------|------|
| thiserror | 2.0.17 | 库错误类型 — `#[derive(Error)]` 精确可匹配的错误枚举 |
| anyhow | 1.0.102 | 应用错误传播 — `anyhow::Result<T>`、`.context()` |
| tracing | 0.1+ | 结构化日志，基于 Span 的可观测性 |
| tracing-subscriber | 0.3+ | 日志输出格式化 |

### TUI 生态

| 技术 | 版本 | 用途 |
|------|------|------|
| crossterm | 0.29.0 | 终端后端（Ratatui 0.30.x 要求） |
| ratatui-textarea | 0.9.1 | 多行文本编辑控件 |
| tui-logger | 0.12+ | TUI 日志显示控件 |
| eddacraft-tui | 0.4.0 | 预构建主题组件库（DataTable、Tree、ProgressBar、Spinner） |

### 国际化

| 技术 | 版本 | 用途 |
|------|------|------|
| fluent-i18n | 0.1.0-rc.0 | 声明式 i18n 宏。`i18n!("locales")` + `t!("key")`。线程安全语言环境切换 |
| unic-langid | 0.9+ | 类型安全语言标签 |

### 开发工具

- **rustup** — 安装 `stable-msvc` 用于 Windows 原生编译
- **cargo-tauri** — `cargo install tauri-cli --version "^2"`
- **Node.js 22 LTS** — SvelteKit/Vite 构建需要
- **pnpm** — 更快更省空间的包管理器

## 备选方案对比

| 类别 | 推荐 | 替代方案 | 为何不用 |
|------|------|----------|----------|
| GUI 框架 | Tauri v2 | Electron | 10 倍大打包体积，Chromium 开销 |
| GUI 框架 | Tauri v2 | egui | 立即模式 GUI 不适合精致桌面应用 |
| TUI 框架 | Ratatui | tui（原版） | 已归档/废弃 |
| TUI 框架 | Ratatui | cursive | 灵活性差，回调式，生态小 |
| Win32 绑定 | windows-rs | winapi | 已废弃 |
| 数据库 | rusqlite | sqlx | 异步开销对本地 SQLite 不必要。rusqlite 更快（2 万次查询 0.069s vs 0.402s） |
| 数据库 | rusqlite | sled | 小众嵌入式 DB。SQLite 通用工具链、Tauri 插件支持 |
| 防火墙 | windows-wfp | wfp (dlon/wfp-rs) | 缺少路径转换、事件监控 |
| i18n | fluent-i18n | rust-i18n | YAML 基，缺少 ICU 消息格式（无复数规则） |
| i18n | fluent-i18n | i18n-embed | 设置更重。fluent-i18n 宏 API 更简单 |

## 不应使用的

| 避免 | 原因 | 替代 |
|------|------|------|
| `winapi` crate | 已废弃，微软官方背书 windows-rs | `windows` 0.73 |
| `procfs` crate | 仅 Linux，Windows 无法编译 | `sysinfo` + `windows` |
| `sqlx` 在核心库 | 异步连接池对本地 SQLite 不必要 | `rusqlite` 0.40 (bundled) |
| 原版 `tui` crate | 已归档不维护 | `ratatui` 0.30 |
| `env_logger` | 无 span 支持，无结构化字段 | `tracing` + `tracing-subscriber` |
| `rust-i18n` | 缺少 ICU 消息格式 | `fluent-i18n` |

## 各子项目技术栈模式

**port-core（共享核心库）：**
- 用 `thiserror` 定义错误类型（消费者可匹配）
- 直接用 `rusqlite`（前端无关）
- 依赖 `windows` crate 访问 Win32 API、`sysinfo` 获取进程信息
- 暴露 async API（tokio），内部在 Windows API 同步调用处保持简洁
- 绝不依赖 Tauri、Ratatui 或任何前端 crate

**Tauri GUI 前端：**
- GUI 端查询用 `tauri-plugin-sql`（连接同一 SQLite DB）
- 前后端通过 Tauri `invoke` IPC 通信
- SvelteKit + `@sveltejs/adapter-static` 的 SPA 模式
- 启用 `tray-icon` 和 `image-png` features

**Ratatui TUI 前端：**
- 用 `crossterm` 0.29 后端（非 termion — Windows 目标）
- 用 `clap` 解析 CLI 参数（`--theme`, `--locale`, `--refresh-interval`）
- 用 `eddacraft-tui` 预构建组件
- 从 JSON/TOML 配置文件加载主题

## 版本兼容性

| 包 A | 兼容 | 说明 |
|------|------|------|
| ratatui 0.30.2 | crossterm 0.29.0 | 必须配对 |
| tauri 2.11.3 | tray-icon 0.24.1 | 打包内置 |
| tauri 2.11.3 | tauri-plugin-sql 2.4.0 | v2.x 插件族兼容 |
| tokio 1.50.0 | Rust 1.71+ (MSRV) | |
| windows 0.73 | Rust 1.70+ | |
| sysinfo 0.39.3 | Rust 1.95+ (MSRV) | |

---
*技术栈调研：Windows 端口管理工具（Tauri v2 + Ratatui 双前端）*
*调研日期：2026-07-26*

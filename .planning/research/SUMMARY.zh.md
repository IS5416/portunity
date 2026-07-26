# 调研总结

**项目：** Portunity
**领域：** Windows 端口管理桌面工具（双前端：Tauri GUI + Ratatui TUI）
**调研日期：** 2026-07-26
**置信度：** MEDIUM-HIGH（技术栈+陷阱为高；功能+架构来自社区/竞品分析，中等）

## 执行摘要

Portunity 是一款 Windows 原生端口管理工具，双前端（Tauri 桌面 GUI + Ratatui 终端 TUI），共享 Rust 核心库。这不是传统 MVP —— 项目范围要求全功能 v1 发布，涵盖端口枚举、进程管理、流量监控、防火墙集成和智能标注等 20+ 功能。

推荐方案是自底向上构建：先基础数据类型和存储，再核心 Windows API 能力（扫描、进程管理），然后 TUI 作为快速迭代验证前端，接着事件驱动监控（ETW），Tauri GUI，最后高级功能（防火墙 CRUD、分面搜索、导出）。15 个领域特定陷阱已被记录，其中 5 个（PID 重用、缓冲区重试、双栈枚举、异步阻塞、工作区错误配置）是正确性关键，必须在各自阶段处理。

## 关键发现

### 推荐技术栈

Rust 单体仓库工作区，三个 crate：port-core（共享库）、port-tui（Ratatui 二进制）、port-gui/src-tauri（Tauri v2 二进制 + Svelte 前端）。

**核心技术：**
- Rust edition 2024 (1.85+) — 零成本 Win32 互操作
- Tauri 2.11.3 + Svelte 5.25.8 — WebView2 渲染桌面 GUI，编译型 Svelte 输出
- Ratatui 0.30.2 + crossterm 0.29 — Elm Architecture 终端 TUI
- windows (windows-rs) 0.73 — 微软官方 Win32 API 绑定
- rusqlite 0.40.1 (bundled) — WAL 模式 SQLite，双前端并发访问
- ferrisetw 1.2.0 — 安全 Rust ETW 消费者
- windows-wfp 0.2.1 — 自动 DOS→NT 路径转换的防火墙管理
- tokio 1.50 — Windows IOCP 异步运行时

### 预期功能

**基本盘：** 实时端口列表+进程映射、协议状态颜色编码、可排序、管理员提权、复制导出、自动刷新、进程终止
**差异化功能：** 自动标注端口、自定义标签、分面搜索、智能终止+白名单、端口历史、流量统计、防火墙 CRUD、ETW 事件刷新、JSON/CSV 导出、双界面、收藏
**明确不做：** TCP 主动扫描、自动杀进程、原始包捕获、远程 HTTP 服务器、服务管理、Linux/macOS 构建

### 架构方案

四层系统：前端（GUI + TUI）→ IPC/适配器（Tauri Commands + TUI 事件循环）→ 核心（port-core 共享库，trait 平台抽象）→ OS（Windows API）。

关键组件：
1. **PortScanner** — 双栈 TCP/UDP 枚举，含缓冲区重试和端口字节序转换
2. **ProcessManager** — HANDLE 包裹的 ProcessHandle 结构体，防止 PID 重用竞态
3. **FirewallManager** — 通过 INetFwPolicy2 COM API 的 CRUD 操作
4. **TrafficMonitor** — ETW Kernel-Network 事件订阅 + 2s 轮询兜底
5. **EventBus** — tokio::sync::broadcast 解耦生产者和消费者
6. **DataStore** — WAL 模式 rusqlite，单写连接 + 读取池

### 关键陷阱

1. **PID 重用竞态** — 存储 ProcessHandle（PID + HANDLE + 创建时间），绝不从裸 PID 重新派生句柄
2. **GetExtendedTcpTable 缓冲区重试** — 二次调用模式是必须的，带加倍缓冲区的重试循环
3. **IPv4/IPv6 双栈遗漏** — 始终以 AF_INET 和 AF_INET6 调用
4. **异步线程上的阻塞 Win32 调用** — 每个 Win32 调用包在 `tokio::task::spawn_blocking` 中
5. **单一 Cargo 工作区** — 从第一天起建立单个根工作区，避免 Tauri 单独工作区

## 路线图建议

6 个阶段，单个里程碑：

| 阶段 | 名称 | 交付物 | 关键陷阱防范 |
|------|------|--------|-------------|
| 1 | 基础 | 工作区结构、数据模型、SQLite schema、TOML 配置、i18n | 工作区错误、WAL 遗漏 |
| 2 | 核心引擎 | 双栈端口扫描器、HANDLE 安全进程管理、智能终止、白名单 | PID 重用、缓冲区重试、双栈、字节序、分配器不匹配 |
| 3 | TUI 前端 | Ratatui Elm Architecture、Tab 界面、虚拟化端口表格、搜索栏 | 大表格渲染、主线程阻塞 |
| 4 | 事件系统+监控 | EventBus、ETW 订阅、流量计数、端口历史 SQLite 日志 | ETW 会话残留、回调阻塞、PID 不准 |
| 5 | GUI 前端 | Tauri 命令注册、Svelte 组件、系统托盘、分面搜索 UI | Mutex 跨 await、COM 泄漏、前端业务逻辑 |
| 6 | 高级功能 | 防火墙 CRUD、自动标签、收藏、导出、中文界面、主题预设 | COM 泄漏、防火墙操作无权限检查 |

**排序理由：** 基础优先（一切前提）→ 核心先于前端（最硬最难）→ TUI 先于 GUI（零工具链复杂度验证 API）→ 事件系统在 TUI 之后（TUI 可视化 ETW 事件流）→ GUI 在事件系统之后（需要 EventBus 流式数据）→ 高级功能最后（需要稳定核心）

**需深入调研的阶段：** Phase 4（ETW 事件 schema）、Phase 5（Tauri v2 托盘能力、Svelte 5 IPC）、Phase 6（windows-wfp API 完整性、SQLite FTS5 分面查询）

**模式明确、无需额外调研：** Phase 1（Rust 工作区+SQLite）、Phase 2（IP Helper API，微软文档详尽）、Phase 3（Ratatui+Elm Architecture，Rust TUI 事实标准）

## 置信度评估

| 领域 | 置信度 | 说明 |
|------|--------|------|
| 技术栈 | 高 | 所有版本已验证 crates.io/docs.rs（2026 年 7 月）。兼容性矩阵已验证 |
| 功能 | 中 | 跨 6+ 竞品工具交叉对比，无原始用户访谈 |
| 架构 | 中 | 基于 Ratatui/Tauri 官方文档和实际 Rust 工作区项目 |
| 陷阱 | 高 | 15 个陷阱来自官方文档、GitHub issues、真实调试经验 |

**总体：** MEDIUM-HIGH — 技术栈和陷阱研究扎实；功能和架构信息充分但 Phase 2 的原型验证会有帮助。

## 待补缺口

- **ETW 事件 schema 细节** — Phase 4 规划时通过 spike 捕获原始 ETW 事件验证
- **windows-wfp API 覆盖面** — Phase 6 规划时验证
- **Svelte 5 + Tauri IPC 负载性能** — Phase 5 执行时基准测试
- **双前端 SQLite 并发边缘情况** — Phase 1 并发访问集成测试
- **无用户调研** — 所有功能优先级基于竞品分析。通过发布后反馈进入 v1.1

---
*调研完成：2026-07-26*
*可进入路线图：是*

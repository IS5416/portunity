# 阶段 1：TUI 端口查看器 - 上下文

**收集时间：** 2026-07-26
**状态：** 等待规划

<domain>
## 阶段边界

交付一个可工作的终端应用程序（`port-tui`），启动后显示一个实时、可排序、可过滤的所有 TCP 和 UDP 端口表格，附带所属进程详情，按连接状态进行颜色编码。启动时自动检测管理员权限，支持一键触发提权。

用户流程：启动 → 查看概览标签页（显示摘要统计）→ 切换到端口标签页查看完整表格 → 按任意列排序 → 按 `/` 进行模糊搜索 → 按 `f` 应用组合过滤器 → 按 `r` 手动刷新 → 按 `a` 提升至管理员权限。
</domain>

<decisions>
## 实现决策

### 扫描器实现
- **D-01 — 缓冲区重试策略：** 对 `GetExtendedTcpTable`/`GetExtendedUdpTable` 的缓冲区分配使用指数增长策略。起始 16KB，遇到 `ERROR_INSUFFICIENT_BUFFER` 时翻倍，最多重试 3 次。如果所有重试均失败，保留上次成功的扫描数据可见，并在状态栏显示错误。
- **D-02 — 双栈枚举：** 内部分别调用 AF_INET 和 AF_INET6 API，将结果合并为统一的端口列表。在地址显示中标注 IP 版本行（`::1:8080` vs `0.0.0.0:8080`）。用户看到的是一个完整的视图。
- **D-03 — 错误恢复：** 扫描完全失败时，保留上次成功的数据在表格中。状态栏显示红色错误提示"按 r 重试"。绝不在临时性故障时清空表格。
- **D-04 — TCP/UDP 并发：** 使用 `tokio::join!` 同时扫描 TCP 和 UDP 表。合并并返回组合结果。实际耗时 = max(TCP扫描时间, UDP扫描时间)。
- **D-05 — 性能方案：** 优先简单实现 — 并发扫描 + 批量进程解析 + 指数重试。不做预先优化。实现后用 criterion 基准测试衡量。目标：1000 个端口 <500ms。

### 管理员提权
- **D-06 — 提权机制：** 启动时通过 `IsUserAnAdmin()` 检测非管理员状态。用户按下 `a` 时，使用 `ShellExecuteExW` 配合 `runas` 动词以管理员身份重新启动 `port-tui.exe`。旧进程在触发提权后退出。 — **Reversibility:** costly — 改变了进程生命周期契约；TUI 二进制文件的所有调用方都必须处理自重启。
- **D-07 — UAC 拒绝处理：** 如果用户在 UAC 提示上点击"否"，应用继续以非管理员模式运行。状态栏显示"需要管理员权限 — 按 a 提权"。用户可以随时重试提权。无模态对话框，无强制退出。
- **D-08 — 重启时的状态传递：** 不向提权后的进程传递任何状态。新进程在启动时执行一次全新的完整扫描。扫描通常在 500ms 内完成，使过渡几乎无感知。
- **D-09 — 检测时机：** 启动时检查一次管理员状态。在状态栏显示持久指示器："Admin ✓"（管理员）或"需要管理员权限 — 按 a 提权"（非管理员）。非管理员模式显示所有端口；系统拥有的进程详情显示"—"并使用 dim 修饰符。

### TUI 事件循环
- **D-10 — 事件模型：** crossterm 的 `event::poll(Duration)` 带超时处理键盘/鼠标事件。超时触发周期性任务（自动刷新检查）。入口点使用 `#[tokio::main]` 默认多线程运行时；crossterm `Terminal` 在主线程上运行。
- **D-11 — 自动刷新：** 5 秒间隔后台扫描。结果静默更新表格，不中断用户交互。按 `r` 键手动刷新时触发即时扫描并显示旋转指示器。
- **D-12 — TEA + 异步桥接：** 扫描在生成的 tokio 任务中运行。结果通过 `tokio::sync::mpsc::unbounded_channel` 发送到主线程。主事件循环在每个 tick 执行 `try_recv`，将结果包装为 `Message::ScanComplete(Vec<Connection>)` 并传递给 TEA `update()` 函数。 — **Reversibility:** costly — 更改通道类型或消息形状会影响扫描器生成端和主循环接收端两处。
- **D-13 — 运行时架构：** 在二进制入口点使用单个 `#[tokio::main]`。多线程调度器（默认）。Windows API 调用放在 `spawn_blocking` 内部以避免阻塞异步运行时。crossterm `Terminal` 在主线程上构造和驱动。

### 初始启动体验
- **D-14 — 默认标签页：** 应用在概览标签页（标签 1）打开 — 摘要统计 + 前 10 端口 + 管理员状态卡片。用户一眼看到系统状态，然后导航到端口标签页查看详情。
- **D-15 — 首次渲染：** 立即渲染完整框架布局（标签栏 + 状态栏 + 页脚），内容区域显示旋转指示器和"正在扫描端口..."。无空白屏幕延迟。启动后一个 tick 内首帧可见。
- **D-16 — 进程名称解析：** 端口扫描返回后，收集所有唯一 PID，通过 `sysinfo::System::refresh_processes()` 批量解析进程名称，按 PID 缓存。同一 PID 出现在多个端口时命中缓存。避免逐连接的 `OpenProcess` 调用。

### Claude 的自由裁量

没有领域被推迟给 Claude — 全部 16 项决策均由用户明确选择。
</decisions>

<canonical_refs>
## 规范参考

**下游代理在规划或实现之前必须阅读这些内容。**

### 规划文档
- `.planning/ROADMAP.md` — 阶段 1 范围、成功标准、需求列表、陷阱覆盖
- `.planning/REQUIREMENTS.md` — 完整 v1 需求（SCAN-01 至 TUI-08）、可追溯性矩阵
- `.planning/STATE.md` — 累积决策、工作空间结构、WAL 模式、TEA 架构、async-first API
- `.planning/phases/01-tui-port-viewer/01-UI-SPEC.md` — **已批准的设计契约：** 布局（4 区域网格）、语义颜色槽（One Dark，12 槽）、键盘层级（L0-L3）、组件清单（概览 + 端口 + 3 占位）、文案契约（所有空/错误/加载状态）

### 代码库（现有骨架）
- `port-core/src/lib.rs` — 模块结构、错误类型、平台抽象（`#[cfg(target_os = "windows")]`）
- `port-core/src/models/port.rs` — `Port`、`Protocol`、`PortState` 枚举
- `port-core/src/models/connection.rs` — `Connection`、`HistoryEntry`、`TrafficStats`、`FirewallRule`、`Favorite`
- `port-core/src/models/process.rs` — `ProcessInfo` 结构体
- `port-core/src/scanner/mod.rs` — `PortScanner` trait（已定义，未实现）
- `port-tui/src/main.rs` — 占位入口点
- `Cargo.toml` — 工作空间定义（3 个成员）、依赖项（thiserror、chrono、serde、tokio）

### 调研（项目级别）
- `.planning/research/ARCHITECTURE.md` — 平台抽象设计、trait 边界、crate 依赖图
- `.planning/research/PITFALLS.md` — #10（单一工作空间）、#12（WAL 模式）、#14（分配器不匹配）
</canonical_refs>

<code_context>
## 现有代码洞察

### 可复用资产
- **`port-core::models` 模块：** 完全定义的数据模型 — `Port`、`Connection`、`ProcessInfo`、`PortState`、`Protocol`。阶段 1 无需修改模型。
- **`PortScanner` trait：** 在 `port-core/src/scanner/mod.rs` 中定义，包含 `scan()` 和 `scan_process(pid)` 签名。已准备好进行 Windows 实现。
- **`Error` 枚举：** 在 `port-core/src/lib.rs` 中定义 — `Platform`、`NotFound`、`PermissionDenied`、`Io` 变体覆盖了扫描器故障模式。
- **工作空间结构：** 3 成员 Cargo 工作空间已配置完成。`port-tui` 有自己的 `Cargo.toml`。

### 既定模式
- **基于 Trait 的平台抽象：** Windows 实现在 `#[cfg(target_os = "windows")]` 后面。Linux/macOS 目前编译报错。
- **async-first API：** 根据 STATE.md 决策，所有 Win32 调用包装在 `spawn_blocking` 中。公共 API 返回异步，内部在 Windows API 是同步的地方保持同步。
- **Edition 2024：** 工作空间使用 Rust edition 2024，resolver = "3"。

### 集成点
- **`port-core` → `port-tui`：** TUI 二进制依赖 `port-core` 获取模型和扫描器。不需要 IPC — 直接的 Rust 函数调用。
- **扫描器 → TEA：** 扫描结果通过 `tokio::sync::mpsc` 通道 → `Message::ScanComplete` → `update()` → 视图渲染。
- **TUI → 配置：** 应用数据目录中的 TOML 配置文件（CORE-05）。阶段 1 需要管理员状态，暂无设置 UI。
</code_context>

<specifics>
## 具体想法

- 用户希望状态栏显示"实时 · N 端口 · Admin ✓/需要 · HH:MM:SS"模式（在 UI-SPEC 文案契约中定义）
- 用户拒绝：固定帧率循环、逐连接进程解析、启动画面、提权时状态传递
- 用户选择简单优先：重启时全新扫描、不做预先优化、实现后进行基准测试
</specifics>

<deferred>
## 推迟的想法

无 — 讨论保持在阶段 1 范围内。
</deferred>

---

*阶段：1-tui-port-viewer*
*上下文收集时间：2026-07-26*

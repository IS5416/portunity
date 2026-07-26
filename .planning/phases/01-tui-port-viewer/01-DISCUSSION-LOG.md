# Phase 1: TUI Port Viewer - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-26
**Phase:** 01-tui-port-viewer
**Areas discussed:** 扫描器实现策略, 管理员提权 UX, TUI 事件循环设计, 首次启动体验

---

## 扫描器实现策略

### 缓冲区重试策略

| Option | Description | Selected |
|--------|-------------|----------|
| 重试 + 指数增长 | 初始 16KB，失败翻倍，最多 3 次。上限后缓存上次成功数据 | ✓ |
| 一次分配超大缓冲区 | 直接 256KB 覆盖极端场景 | |
| 先查询大小再分配 | 两次系统调用，内存精确 | |

**User's choice:** 重试 + 指数增长
**Notes:** 稳健且简单，平衡内存和可靠性

### IPv4/IPv6 双栈

| Option | Description | Selected |
|--------|-------------|----------|
| 合并扫描，统一展示 | 内部分别调用，合并列表，标注 IP 版本 | ✓ |
| 分离展示，Tab 切换 | IPv4/IPv6 两个子视图 | |
| 仅 IPv4（初期） | Phase 1 暂不处理 IPv6 | |

**User's choice:** 合并扫描，统一展示
**Notes:** 用户看到完整视图，不需要关心底层差异

### 扫描失败容错

| Option | Description | Selected |
|--------|-------------|----------|
| 保留上次数据 + 警告 | 缓存成功数据，状态栏显示错误 | ✓ |
| 清空表格 + 错误状态 | 空状态页面 + 重试按钮 | |
| 仅状态栏提示 | 数据不变，仅状态栏提示 | |

**User's choice:** 保留上次数据 + 警告
**Notes:** 用户不会丢失已有视图

### TCP/UDP 并发扫描

| Option | Description | Selected |
|--------|-------------|----------|
| 并发扫描 + 合并 | tokio::join! 同时扫描 TCP/UDP | ✓ |
| 顺序扫描 | 先 TCP 后 UDP | |
| 可配置 | 默认并行，可切换顺序 | |

**User's choice:** 并发扫描 + 合并
**Notes:** 总耗时 = max(TCP, UDP)，更快

---

## 管理员提权 UX

### 提权机制

| Option | Description | Selected |
|--------|-------------|----------|
| runas 重新启动自身 | ShellExecuteEx + runas，旧进程退出 | ✓ |
| COM 提权（UAC Moniker） | 独立进程执行高权限操作 | |
| 启动时要求管理员 | 提示用户手动以管理员运行 | |

**User's choice:** runas 重新启动自身
**Notes:** Windows 标准做法

### UAC 拒绝处理

| Option | Description | Selected |
|--------|-------------|----------|
| 继续非管理员模式 | 正常运行，状态栏提示，可重试 | ✓ |
| 退出应用 | 提权失败直接退出 | |
| 弹窗提示 | 模态说明后继续 | |

**User's choice:** 继续非管理员模式
**Notes:** 不中断用户工作流

### 状态传递

| Option | Description | Selected |
|--------|-------------|----------|
| 不传递，重新扫描 | 新进程全量扫描，<500ms | ✓ |
| 通过临时文件传递 | JSON 序列化到临时文件 | |
| 通过 SQLite 传递 | 写入 DB，新进程读取 | |

**User's choice:** 不传递，重新扫描
**Notes:** 最简实现，扫描速度快无需缓存

### 检测时机

| Option | Description | Selected |
|--------|-------------|----------|
| 启动时检测 + 状态栏提示 | IsUserAnAdmin() 检查，持久提示 | ✓ |
| 首次需要详情时提示 | 选中系统进程时检测 | |
| 始终不提示 | 静默运行，用户自行判断 | |

**User's choice:** 启动时检测 + 状态栏提示
**Notes:** 不阻塞，用户看到端口列表后自行决定

---

## TUI 事件循环设计

### 事件驱动模型

| Option | Description | Selected |
|--------|-------------|----------|
| 事件驱动 + 定时刷新 | event::poll(timeout)，tokio 多线程 | ✓ |
| 固定帧率 tick | 60fps 循环 | |
| 纯事件驱动，无定时 | 仅按键时重绘 | |

**User's choice:** 事件驱动 + 定时刷新
**Notes:** Ratatui 社区标准做法，省 CPU

### 自动刷新间隔

| Option | Description | Selected |
|--------|-------------|----------|
| 5 秒自动 + 手动即时 | 每 5 秒后台扫描，'r' 即时触发 | ✓ |
| 2 秒自动 + 手动即时 | 接近实时 | |
| 仅手动刷新 | 完全用户控制 | |

**User's choice:** 5 秒自动 + 手动即时
**Notes:** 平衡实时性和 CPU 消耗

### TEA + async 桥接

| Option | Description | Selected |
|--------|-------------|----------|
| Channel → Message | mpsc channel，Message::ScanComplete | ✓ |
| update() 中直接 spawn | spawn_blocking 在 update 中 | |
| 共享状态 + Mutex | Arc<Mutex<AppState>> | |

**User's choice:** Channel → Message
**Notes:** 标准 TEA + async 模式，单向数据流

### tokio 运行时架构

| Option | Description | Selected |
|--------|-------------|----------|
| tokio 主线程 + crossterm | #[tokio::main] 多线程，主线程处理事件 | ✓ |
| 独立线程 + channel | std::thread + std::sync::mpsc | |
| tokio 单线程运行时 | current_thread flavor | |

**User's choice:** tokio 主线程 + crossterm
**Notes:** 利用 IOCP 优势，后续 ETW 集成兼容

---

## 首次启动体验

### 默认标签页

| Option | Description | Selected |
|--------|-------------|----------|
| Overview（标签 1） | 统计摘要 + Top 10 | ✓ |
| Ports（标签 2） | 完整端口表 | |

**User's choice:** Overview（标签 1）
**Notes:** 先概览后细节，新用户友好

### 首次渲染

| Option | Description | Selected |
|--------|-------------|----------|
| 骨架布局 + spinner | 框架立即可见，spinner 表示工作中 | ✓ |
| 空白屏幕 → 突然出现 | 等扫描完成后一次性渲染 | |
| ASCII 启动画面 | Logo + Loading | |

**User's choice:** 骨架布局 + spinner
**Notes:** 响应感强，框架立即可见

### 进程名解析策略

| Option | Description | Selected |
|--------|-------------|----------|
| 批量解析 + 缓存 | sysinfo 批量查询，PID 缓存 | ✓ |
| 惰性解析 | 先展示 PID，进程名延迟加载 | |
| 每连接逐次调用 | 每个连接单独 OpenProcess | |

**User's choice:** 批量解析 + 缓存
**Notes:** sysinfo 已在依赖中，批量查询更高效

### 性能策略

| Option | Description | Selected |
|--------|-------------|----------|
| 简单实现 + 事后测量 | 先写最简路径，criterion 基准测试 | ✓ |
| 预设性能上限 | 预设高性能参数 | |
| 增量渲染 | 分批返回，边扫边渲染 | |

**User's choice:** 简单实现 + 事后测量
**Notes:** Phase 1 端口数通常 <500，超标概率低

---

## Claude's Discretion

All 16 decisions were explicitly selected by the user. No areas deferred to Claude.

## Deferred Ideas

None — discussion stayed within Phase 1 scope.

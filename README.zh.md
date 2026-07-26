# Portunity

**Port + Opportunity** — 高性能 Windows 端口管理工具。

查找、分类、管理活跃端口及其所属进程。自信 kill。监控流量。管理防火墙规则。零摩擦操作。

## 为什么

端口 3000 被占用了。你打开 `netstat`，翻找 PID，复制，`taskkill`。每天如此。

Portunity 替代这个流程。一键看清一切，一键执行操作。

## 功能

- **多维度搜索** — 按应用名、端口范围、协议类型、进程属性任意组合过滤
- **智能终止** — 先优雅终止（SIGTERM），进程不响应再强制杀死（SIGKILL）
- **端口历史** — SQLite 存储的时间线：哪个进程何时占用了哪个端口
- **流量监控** — 基于 ETW 的每端口、每进程收发字节统计
- **防火墙规则** — 在应用内查看、创建、删除、启禁 Windows 防火墙规则
- **进程详情** — 可执行文件路径、启动时间、命令行参数、数字签名
- **收藏 + 标签** — 收藏常用端口，打标签（"我的 dev server"、"数据库"）
- **导出** — JSON/CSV 格式，分享给同事或贴到 bug 报告
- **主题** — One Dark、Dracula、Solarized、Nord、Monokai、High Contrast 等
- **国际化** — 中英文切换，模块化扩展更多语言

## 双前端

| | GUI | TUI |
|---|---|---|
| **技术** | Tauri v2 + Svelte | Ratatui |
| **适合** | 日常使用、系统托盘、鼠标 | 速度、纯键盘、SSH、tmux |
| **托盘** | 悬浮面板 + 快捷操作 | 不适用 |
| **主题** | 内置 | `t` 键切换 |

两者共享同一个 `port-core` Rust 库 — 数据一致，逻辑一致。

## 快速开始

```bash
# 构建全部
cargo build --release

# 终端 TUI
cargo run --bin port-tui

# 桌面 GUI（需要 Node.js 构建 Svelte 前端）
cd port-gui && npm install && cargo tauri dev
```

## 平台

Windows 10/11 为主。平台抽象层已为 Linux 和 macOS 预留扩展点。

## 许可

MIT

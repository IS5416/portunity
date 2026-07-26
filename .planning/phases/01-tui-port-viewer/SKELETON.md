# Portunity Walking Skeleton

**Plan 01-01 deliverable.** Documents the architectural shape proven by the walking skeleton.

## Architecture Proven

```
port-tui (TEA event loop)
  │
  │  tokio::sync::mpsc::unbounded_channel<Message>
  │
  ▼
port-core::windows::WindowsPortScanner
  │
  │  #[async_trait] PortScanner::scan()
  │
  ▼
port-core::scanner::tcp::scan_tcp()
  │
  │  tokio::task::spawn_blocking(|| { ... })
  │
  ▼
GetExtendedTcpTable (iphlpapi.dll)
  + sysinfo::System (process names)
```

## Crate Dependency Graph

```
port-core
  ├── windows 0.62 (iphlpapi, WinSock, Threading, ToolHelp, Security, ProcessStatus, Foundation)
  ├── sysinfo 0.39
  ├── tokio 1.53 (full)
  ├── rusqlite 0.40 (bundled)
  ├── async-trait 0.1
  ├── serde + serde_json
  ├── toml 0.8
  ├── thiserror 2
  ├── chrono 0.4
  └── tracing 0.1

port-tui
  ├── port-core (path dependency)
  ├── ratatui 0.30 (core + crossterm + widgets + macros)
  ├── crossterm 0.29
  ├── tokio 1.53 (full)
  ├── clap 4.6
  ├── anyhow 1.0
  ├── chrono 0.4
  ├── tracing 0.1
  └── tracing-subscriber 0.3

port-gui (placeholder — not yet implemented)
  └── port-core (path dependency)
```

## Key Module Map

```
port-core/src/
  lib.rs        → module declarations, Error enum, Result alias
  scanner.rs    → PortScanner trait (async, Send+Sync)
  scanner/
    tcp.rs      → scan_tcp(), GetExtendedTcpTable wrapper
  windows.rs    → WindowsPortScanner (impl PortScanner)
  store.rs      → storage module declarations
  store/
    connection.rs → init_db(), default_db_path(), WAL mode
  config.rs     → config module declarations
  config/
    settings.rs → AppSettings, load/save TOML
  models/
    port.rs     → Port, Protocol, PortState
    process.rs  → ProcessInfo
    connection.rs → Connection, HistoryEntry, TrafficStats, FirewallRule, Favorite
    filter.rs   → Filter

port-tui/src/
  main.rs       → #[tokio::main], terminal setup, TEA event loop
  app.rs        → App state struct
  message.rs    → Message enum
  update.rs     → update(&mut App, Message)
  theme.rs      → Theme struct, One Dark palette
  components/
    mod.rs      → Component trait
    ports.rs    → PortsComponent (ratatui::widgets::Table)
```

## Data Flow (End-to-End)

1. **Launch:** `#[tokio::main]` starts → crossterm raw mode + alternate screen
2. **Initial scan:** `spawn_scan()` → `tokio::spawn` → `WindowsPortScanner::scan()` → `scan_tcp()` inside `spawn_blocking`
3. **Result delivery:** `UnboundedSender::send(Message::ScanComplete(connections))` → main loop `rx.try_recv()`
4. **State update:** `update(&mut app, ScanComplete(connections))` → sets `app.ports`, clears `app.scanning`
5. **Render:** `terminal.draw(|f| render_app(f, &app, &theme))` → `PortsComponent::render()` → ratatui Table
6. **Keyboard:** `event::poll(200ms)` → `event::read()` → map to Message → `update()` → next frame
7. **Exit:** `q` or `Esc` → `should_quit=true` → break loop → disable_raw_mode + LeaveAlternateScreen

## Windows API Integration

| API | Module | Purpose | Pattern |
|-----|--------|---------|---------|
| `GetExtendedTcpTable` | scanner/tcp.rs | TCP port enumeration | Two-call buffer: first gets size (ERROR_INSUFFICIENT_BUFFER), second gets data. Retry on table growth. |
| `ntohs` | scanner/tcp.rs | Port byte order | Network byte order (big-endian) → host byte order. Applied to dwLocalPort, dwRemotePort. |
| `sysinfo::System` | scanner/tcp.rs | Process name resolution | Batch resolve: collect unique PIDs, System::new_all(), cache by PID. No per-connection OpenProcess. |

## TEA Architecture Decisions

| Decision | Rationale |
|----------|-----------|
| `tokio::sync::mpsc::unbounded_channel` | Unbounded in tracer — simple, no backpressure needed for single on-demand scan. Plan 02 upgrades to bounded. |
| `event::poll(200ms)` | Poll with timeout — responsive to keyboard while allowing async drain. No busy-wait. |
| Single `render_app()` | Full frame redraw each iteration. crossterm double-buffering prevents flicker. |
| `PortsComponent` stateless | All state in `App`. Components receive read-only references. No local widget state yet. |
| `scan_spawned` flag | Prevents double-spawn of scan tasks when scanning flag is set during update. |

## SQLite Configuration

| Setting | Value | Purpose |
|---------|-------|---------|
| `journal_mode` | WAL | Concurrent reads during writes — needed for dual-frontend later |
| `busy_timeout` | 5000ms | Graceful retry under contention |
| `schema_version` | 1 | Forward-compatibility marker in settings table |
| DB path | `%APPDATA%/Portunity/portunity.db` | Per-user, inherits directory ACLs |

## Config

| Setting | Default | Purpose |
|---------|---------|---------|
| `admin_detected` | false | Tracks whether current session has admin privileges |
| `schema_version` | 1 | Forward-compatibility marker |
| Config path | `%APPDATA%/Portunity/settings.toml` | Created with defaults on first run |

## Version Deviations

The plan specified versions that differ from actual crates.io availability:

| Crate | Plan Version | Actual Version | Impact |
|-------|-------------|----------------|--------|
| windows | 0.73 | 0.62 | API differences: GetExtendedTcpTable returns u32, MIB_TCP_STATE is newtype, ntohs is unsafe |
| tokio | 1.50 | 1.53 | Fully compatible — minor version bump |

## Known Limitations (Tracer Scope)

- **TCP only** — UDP scanning is Plan 02 (D-02)
- **IPv4 only** — AF_INET6 dual-stack enumeration is Plan 02 (Pitfall #3)
- **No filtering/search** — Plan 02
- **No sorting** — Plan 02
- **No admin elevation** — Plan 02
- **No auto-refresh** — Plan 02 (5-second interval per D-11)
- **Basic Table widget** — Plan 02 upgrades to VirtualTable (Pitfall #13)
- **No process details panel** — Plan 02
- **Tab bar is static** — tab switching is Plan 02
- **Only 2 keyboard shortcuts** — full keyboard layers (L0-L3) in Plan 02

---
*Skeleton documented: 2026-07-26*
*Plan: 01-01*

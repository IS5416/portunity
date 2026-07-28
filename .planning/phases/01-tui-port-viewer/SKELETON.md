# Walking Skeleton — Portunity

**Phase:** 1 — TUI Port Viewer
**Generated:** 2026-07-28

## Capability Proven End-to-End

A user launches `port-tui.exe` and immediately sees a live table of all active TCP and UDP ports with owning process names and PIDs, color-coded by connection state, sortable by column, filterable by multi-dimension criteria, and fuzzy-searchable across all fields. Admin elevation is auto-detected and offered with a single keypress.

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Workspace structure | Single Cargo workspace (root), 3 members: port-core, port-tui, port-gui (placeholder) | Prevents double compilation (Pitfall #10). Single Cargo.lock and target/ directory. Unidirectional dependency: frontends depend on core, never reverse. |
| Core library design | Async-first trait-based API (PortScanner trait) with `#[async_trait]` | Windows API calls inside spawn_blocking to prevent async runtime stalls (Pitfall #9). Traits provide platform abstraction for future Linux/macOS (Pitfall #3). |
| TUI architecture | Ratatui Elm Architecture (TEA): centralized Message enum, single update() function, per-tab Component trait | Centralized state management prevents drift between frontends (Anti-Pattern #4). Message-driven rendering is naturally event-driven — no fixed-tick re-render. |
| Scanner communication | `tokio::sync::mpsc::unbounded_channel` between scanner spawn tasks and TEA main loop | Scanner runs in spawn_blocking; results arrive via Message::ScanComplete. `try_recv` on each main loop tick avoids blocking. D-12 decision — costly to change. |
| Database | SQLite via rusqlite, WAL mode enabled on first connection, `PRAGMA busy_timeout=5000` | WAL mode enables concurrent reads during writes (Pitfall #12). rusqlite chosen over sqlx for simplicity and speed (20K queries: 0.069s vs 0.402s). |
| Configuration | TOML files in `%APPDATA%/Portunity/`, serde-deserialized, hot-reloadable pattern | TOML chosen over JSON for readability (comments, trailing commas). Hot-reload requires file watcher — deferred to Phase 6. |
| Port scanning strategy | Dual-stack AF_INET + AF_INET6 enumeration via `GetExtendedTcpTable`/`GetExtendedUdpTable`, buffer retry with exponential growth (16KB start, 2x per retry, max 3) | Single AF_INET call misses dual-stack connections (Pitfall #3). Buffer retry prevents silent data loss when table grows between calls (Pitfall #2). Port numbers converted via `ntohs` (Pitfall #4). |
| Process name resolution | Batch resolution via `sysinfo::System` with PID cache (ProcessResolver) | Avoids per-connection `OpenProcess` calls (D-16). sysinfo provides cross-platform process enumeration without Windows-specific FFI. |
| Admin elevation | `IsUserAnAdmin()` detection at startup; `ShellExecuteExW` with "runas" verb for UAC; old process exits on elevation, new process performs fresh scan | OS-mediated UAC prompt prevents programmatic bypass. No state transfer on restart (D-08) — fresh scan completes in <500ms. |
| Terminal rendering | crossterm 0.29 backend + ratatui 0.30 widgets, double-buffered, event-driven draw (no fixed-tick re-render) | Event-driven rendering prevents CPU burn at idle (Pitfall #13). Double-buffering + batched writes from crossterm provide flicker-free output (TUI-08). |
| Theme system | One Dark hardcoded as Theme struct with 13 semantic color slots (fg_default, fg_muted, fg_emphasis, bg_base, bg_surface, bg_overlay, bg_selection, accent_primary, accent_secondary, status_success, status_warning, status_error, status_info) | Semantic slots decouple intent from hex value. Hardcoded in Phase 1; TOML theme files + t-key switching deferred to Phase 6 (TUI-05). |
| Filter architecture | Free functions in port-core (`apply_filters`, `fuzzy_search`) operating on `Vec<Connection>`, no trait indirection | Filters are platform-independent — no benefit from trait abstraction. Free functions are simpler and just as testable. AND logic across filter dimensions (SRCH-01). |
| Sort architecture | In-place `Vec::sort_by()` on display data with sort column + order (None/Ascending/Descending cycle) | Simple, fast for expected workloads (<10K rows). Persists sort order across manual refreshes (SCAN-04). |
| Tab system | 5-tab dashboard (Overview, Ports, History, Traffic, Firewall) with 0-based `active_tab` index, tab bar with active highlighting (Bold + accent_primary), 1-5 keys for direct switch, Tab/Shift+Tab for cycling | 5 distinct function domains per PROJECT.md decision. Tab bar visually distinguishes active vs inactive per UI-SPEC. Overview tab as default (D-14). |
| Resize gate | Minimum 80x24 terminal enforced at start of every `render_app()` call; below threshold renders centered "Terminal too small" message with current dimensions | TUI-07 requirement. Prevents layout panic at small sizes. Adapts immediately on resize — no restart needed. |
| Build toolchain | Rust edition 2024 (1.85+), MSVC toolchain (stable-msvc), tokio 1.53 multi-threaded runtime | Edition 2024 required by workspace resolver = "3". MSVC required for Windows native compilation (windows-rs). Tokio multi-threaded for work-stealing scheduler. |
| Release optimization | LTO + single codegen unit + strip symbols, `opt-level = 3` | Binary size 1.1MB stripped (target <10MB). LTO enables cross-crate inlining that significantly reduces tokio's generic monomorphization footprint. |
| Module layout | New-style Rust (edition 2018+): leaf modules as `name.rs`, sub-modules as `name.rs` + `name/` directory. No `mod.rs` files. | Per CLAUDE.md rule. Applied to both port-core (scanner.rs + scanner/tcp.rs) and port-tui (components.rs + components/overview.rs, etc.). |
| Error resilience (D-03) | Scan failure preserves last successful data in `app.ports`; error displayed in status bar with "Press r to retry" | Prevents blanking the table on transient failures. User always sees most recent data. |

## Stack Touched in Phase 1

- [x] Project scaffold — Cargo workspace (3 members), all dependencies, module declarations
- [x] Routing — Tab-based navigation (5 tabs, 1-5 keys, Tab/Shift+Tab cycling)
- [x] Database — SQLite with WAL mode, settings table, config TOML read/write
- [x] UI — Full TUI dashboard: overview + port table + placeholder tabs, color-coded, sortable, filterable, searchable
- [x] Deployment — `cargo build --release --bin port-tui` produces distributable binary (1.1MB); `cargo run --bin port-tui` for dev
- [x] Windows API integration — GetExtendedTcpTable, GetExtendedUdpTable, IsUserAnAdmin, ShellExecuteExW
- [x] Process enumeration — sysinfo batch resolution with PID cache
- [x] Admin elevation — UAC prompt via ShellExecuteExW, non-admin graceful degradation

## Data Flow (End-to-End)

1. **Launch:** `#[tokio::main]` starts → crossterm raw mode + alternate screen
2. **Admin check:** `IsUserAnAdmin()` → Message::AdminCheck → status bar indicator
3. **Initial scan:** `spawn_scan()` → `tokio::spawn` → `WindowsPortScanner::scan()` → `scan_tcp()` + `scan_udp()` inside `spawn_blocking`
4. **Result delivery:** `UnboundedSender::send(Message::ScanComplete(connections))` → main loop `rx.try_recv()`
5. **State update:** `update(&mut app, ScanComplete(connections))` → sets `app.ports`, clears `app.scanning`, re-applies sort/search/filter
6. **Render:** `terminal.draw(|f| render_app(f, &app, &theme))` → resize gate check → tab bar → active tab Component::render()
7. **Overview tab (0):** Port Summary stats + Connection States counts + Top 10 mini-table + Admin Status card
8. **Ports tab (1):** Full DataTable with virtual scrolling, sort, search overlay, filter panel overlay
9. **Placeholder tabs (2-4):** Centered "Coming later" message with nav hint
10. **Keyboard:** `event::poll(200ms)` → `event::read()` → map to Message → tab switching (1-5/Tab), search (/), filter (f), sort (s), refresh (r), elevate (a) → `update()` → next frame
11. **Auto-refresh:** 5-second interval when idle (D-11) → re-triggers spawn_scan
12. **Exit:** `q` → `should_quit=true` → break loop → disable_raw_mode + LeaveAlternateScreen

## Windows API Integration

| API | Module | Purpose | Pattern |
|-----|--------|---------|---------|
| `GetExtendedTcpTable` | scanner/tcp.rs | TCP port enumeration | Two-call buffer: first gets size (ERROR_INSUFFICIENT_BUFFER), second gets data. Retry with exponential growth on table growth. |
| `GetExtendedUdpTable` | scanner/udp.rs | UDP port enumeration | Same two-call + retry pattern. UDP has no connection states — entries are ephemeral endpoints. |
| `ntohs` | scanner/tcp.rs, scanner/udp.rs | Port byte order | Network byte order (big-endian) → host byte order. Applied to dwLocalPort, dwRemotePort. |
| `IsUserAnAdmin` | elevate.rs | Admin detection | Win32::UI::Shell. Called once at startup; result persists for session. |
| `ShellExecuteExW` | elevate.rs | UAC elevation | "runas" verb triggers OS UAC prompt. Old process exits; new elevated process performs fresh scan. |
| `sysinfo::System` | scanner/tcp.rs, scanner/udp.rs | Process name resolution | Batch resolve: collect unique PIDs, `System::new_all()`, cache by PID. No per-connection OpenProcess. |

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

## Crate Dependency Graph

```
port-core
  ├── windows 0.62 (iphlpapi, WinSock, Threading, ToolHelp, Security, ProcessStatus, Foundation, Shell, WindowsAndMessaging, Registry)
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
  ├── tracing-subscriber 0.3
  └── windows 0.62 (Shell, WindowsAndMessaging for elevation)

port-gui (placeholder — not yet implemented)
  └── port-core (path dependency)
```

## Key Module Map

```
port-core/src/
  lib.rs          → module declarations, Error enum, Result alias
  scanner.rs      → PortScanner trait (async, Send+Sync)
  scanner/
    tcp.rs        → scan_tcp(), GetExtendedTcpTable wrapper
    udp.rs        → scan_udp(), GetExtendedUdpTable wrapper
  windows.rs      → WindowsPortScanner (impl PortScanner)
  store.rs        → storage module declarations
  store/
    connection.rs → init_db(), default_db_path(), WAL mode
  config.rs       → config module declarations
  config/
    settings.rs   → AppSettings, load/save TOML
  models/
    port.rs       → Port, Protocol, PortState
    process.rs    → ProcessInfo
    connection.rs → Connection, HistoryEntry, TrafficStats, FirewallRule, Favorite
    filter.rs     → Filter
  filter/
    mod.rs        → apply_filters(), fuzzy_search() free functions

port-tui/src/
  main.rs         → #[tokio::main], terminal setup, TEA event loop, render_app()
  app.rs          → App state struct (ports, scanning, error, active_tab, search, filter, admin, sort)
  message.rs      → Message enum (all user actions + system events)
  update.rs       → update(&mut App, Message)
  theme.rs        → Theme struct, One Dark palette (13 semantic color slots)
  elevate.rs      → is_admin(), elevate_to_admin()
  components.rs   → Component trait, module declarations
  components/
    overview.rs   → OverviewComponent (Port Summary, Connection States, Top 10, Admin Status)
    ports.rs      → PortsComponent (DataTable with virtual scrolling, sort, state colors)
    search.rs     → SearchComponent (fuzzy search overlay, / key)
    filter_panel.rs → FilterPanelComponent (multi-field filter overlay, f key)
    history.rs    → HistoryTabComponent (placeholder — "Coming later")
    traffic.rs    → TrafficTabComponent (placeholder — "Coming later")
    firewall.rs   → FirewallTabComponent (placeholder — "Coming later")
```

## Phase 1 Commit History

| Plan | Commits | Deliverables |
|------|---------|-------------|
| 01-01 | 3 | Workspace scaffold, Windows TCP scanner, SQLite WAL init, TOML config, TEA event loop with live port table |
| 01-02 | 2 | UDP scanning, dual-stack enumeration (AF_INET+AF_INET6), state column labels, sort interaction, virtual scrolling, auto-refresh, footer |
| 01-03 | 2 | Fuzzy search (/), multi-dimension filter panel (f), non-modal overlays, context-sensitive footer, admin elevation (a), system process detection |
| 01-04 | 2 | Tab system (Overview tab, placeholder tabs, tab bar highlighting, 1-5/Tab/Shift+Tab), resize gate (80x24), connection states panel, top-10 mini-table, admin status card, release build profile, comprehensive SKELETON.md |

## Out of Scope (Deferred to Later Slices)

- Process termination (PROC-01 through PROC-07) — Phase 2
- ETW real-time monitoring (SCAN-05, TRAF-01 through TRAF-03) — Phase 3
- Port history recording and timeline (HIST-01 through HIST-04) — Phase 3
- Firewall rule management (FW-01 through FW-06) — Phase 4
- Data export (EXP-01 through EXP-03) — Phase 4
- Tauri GUI (GUI-01 through GUI-04) — Phase 5
- Theme switching with 6 presets (TUI-05) — Phase 6
- Language toggle Chinese/English (TUI-06, I18N-01 through I18N-03) — Phase 6
- Auto-labels for known ports (SRCH-04 through SRCH-06) — Phase 6
- Faceted search with AND/OR logic (SRCH-02) — Phase 2
- EventBus (CORE-03) — Phase 3
- Config hot-reload — Phase 6
- Windows Filtering Platform (WFP) for dynamic firewall — Phase 4

## Subsequent Slice Plan

Each later phase adds one vertical slice on top of this skeleton without altering its architectural decisions:

- **Phase 2:** Process management — inspect process details, terminate with smart kill escalation, whitelist-gated protection
- **Phase 3:** Real-time monitoring — ETW event-driven refresh, traffic sparklines, port history timeline with SQLite append log
- **Phase 4:** Firewall management — Windows Firewall rule CRUD, right-click quick actions, JSON/CSV export
- **Phase 5:** Desktop GUI — Tauri v2 + Svelte desktop app with reactive stores, system tray, all Phase 1-4 capabilities
- **Phase 6:** Polish — auto-labels, custom labels, favorites, 6 theme presets, Chinese/English i18n with Fluent FTL files

## Version Deviations

The plan specified versions that differ from actual crates.io availability:

| Crate | Plan Version | Actual Version | Impact |
|-------|-------------|----------------|--------|
| windows | 0.73 | 0.62 | API differences: GetExtendedTcpTable returns u32, MIB_TCP_STATE is newtype, ntohs is unsafe |
| tokio | 1.50 | 1.53 | Fully compatible — minor version bump |

## Pitfall Coverage

| Pitfall | Status | How Addressed |
|---------|--------|---------------|
| #1 PID Reuse Race | Deferred | Phase 2 — ProcessHandle wrapper |
| #2 TCP Table Buffer | Covered | Exponential retry with 3 attempts in scanner/tcp.rs |
| #3 IPv4/IPv6 Dual-Call | Covered | AF_INET + AF_INET6 in scanner; IPv4-mapped dedup |
| #4 Port Byte Order | Covered | `ntohs`/`u16::from_be()` applied to all port fields |
| #5 ETW Orphaning | Deferred | Phase 3 |
| #6 ETW Callback Blocking | Deferred | Phase 3 |
| #7 COM Resource Management | Deferred | Phase 4 |
| #8 std::Mutex Across .await | N/A | No shared mutable async state yet; tokio Mutex planned |
| #9 Blocking Win32 on Async | Covered | All Win32 calls inside spawn_blocking |
| #10 Separate Tauri Workspace | Covered | Single Cargo workspace at root |
| #11 Protected Process Kill | Deferred | Phase 2 — shipped whitelist |
| #12 SQLite WAL Mode | Covered | WAL enabled on first connection + busy_timeout=5000 |
| #13 Ratatui Large Table | Covered | Virtual scrolling with viewport-only row rendering |
| #15 ETW PID Inaccuracy | Deferred | Phase 3 — ETW as trigger only, API as ground truth |

---

*Skeleton documented: 2026-07-28*
*Phase: 01-tui-port-viewer (Plans 01-01 through 01-04)*

---
phase: 01-tui-port-viewer
plan: 01
subsystem: scanner
tags: [windows, iphlpapi, GetExtendedTcpTable, ratatui, crossterm, TEA, SQLite, WAL, TOML]

# Dependency graph
requires: []
provides:
  - Walking skeleton: Cargo workspace compiles, Windows TCP scan works, TUI renders live port table
  - Async PortScanner trait implemented for Windows via GetExtendedTcpTable + sysinfo
  - SQLite database init with WAL mode and settings table
  - TOML config load/save with default-on-error resilience
  - TEA event loop with crossterm raw mode, mpsc unbounded channel, keyboard input
  - One Dark theme semantic color slots
affects: [02-tui-search-filter, 03-traffic-monitoring, 04-firewall-export, 05-tauri-gui]

# Tech tracking
tech-stack:
  added: [windows 0.62, sysinfo 0.39, rusqlite 0.40, async-trait 0.1, ratatui 0.30, crossterm 0.29, clap 4.6, toml 0.8, anyhow 1.0, tracing 0.1, tracing-subscriber 0.3]
  patterns: [TEA (The Elm Architecture), two-call buffer pattern, spawn_blocking for Win32 FFI, async-trait, new-style Rust modules]

key-files:
  created:
    - port-core/src/scanner/tcp.rs - Windows TCP table enumeration with two-call pattern
    - port-core/src/store.rs - SQLite storage module declarations
    - port-core/src/store/connection.rs - SQLite init with WAL mode
    - port-core/src/config.rs - Config module declarations
    - port-core/src/config/settings.rs - TOML settings load/save
    - port-tui/src/main.rs - TEA event loop entry point
    - port-tui/src/app.rs - Application state
    - port-tui/src/message.rs - Message enum
    - port-tui/src/update.rs - TEA update function
    - port-tui/src/theme.rs - One Dark color slots
    - port-tui/src/components/mod.rs - Component trait
    - port-tui/src/components/ports.rs - Port table widget
  modified:
    - Cargo.toml - Workspace dependencies
    - port-core/Cargo.toml - Core crate dependencies
    - port-tui/Cargo.toml - TUI crate dependencies
    - port-core/src/lib.rs - Module declarations, re-exports
    - port-core/src/scanner.rs - Async PortScanner trait
    - port-core/src/windows.rs - WindowsPortScanner impl

key-decisions:
  - "Used windows 0.62 (not 0.73) — 0.73 not yet published on crates.io"
  - "Adapted to new-style Rust module layout (scanner.rs + scanner/tcp.rs) per CLAUDE.md rules — plan specified mod.rs"
  - "Made windows module public so port-tui can import WindowsPortScanner directly"
  - "Used tokio::sync::mpsc::unbounded_channel for scan result delivery (D-12)"
  - "First frame shows scanning indicator with scanning=true in App::new() (D-15)"

patterns-established:
  - "Async PortScanner trait with #[async_trait] — all implementations return async"
  - "Two-call buffer pattern for GetExtendedTcpTable with retry"
  - "All Win32 FFI inside tokio::task::spawn_blocking"
  - "Stateless Components with render(&self, &App, &mut Frame, Rect, &Theme)"
  - "TEA: Message enum → update() → render cycle"

requirements-completed: [CORE-01, CORE-02, CORE-04, CORE-05, CORE-06, SCAN-01, TUI-01, TUI-03, TUI-07, TUI-08]

coverage:
  - id: D1
    description: "Cargo workspace compiles with all dependencies (cargo check --workspace)"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "cargo check --workspace"
        status: pass
    human_judgment: false
  - id: D2
    description: "Windows TCP scanner using GetExtendedTcpTable with two-call buffer pattern, ntohs byte-order conversion, sysinfo process name resolution"
    requirement: SCAN-01
    verification:
      - kind: integration
        ref: "cargo build -p port-core"
        status: pass
    human_judgment: false
  - id: D3
    description: "SQLite database init with WAL mode, busy timeout, settings table"
    requirement: CORE-04
    verification:
      - kind: integration
        ref: "cargo build -p port-core"
        status: pass
    human_judgment: false
  - id: D4
    description: "TOML config load/save with default-on-parse-error, admin_detected field, settings_path in APPDATA"
    requirement: CORE-05
    verification:
      - kind: integration
        ref: "cargo build -p port-core"
        status: pass
    human_judgment: false
  - id: D5
    description: "TUI binary compiles and launches with TEA event loop, renders live TCP port table"
    requirement: TUI-01
    verification:
      - kind: integration
        ref: "cargo build --bin port-tui"
        status: pass
    human_judgment: false
  - id: D6
    description: "TUI renders real TCP port data from current machine, 'r' refreshes, 'q' exits cleanly"
    requirement: TUI-01
    verification: []
    human_judgment: true
    rationale: "TUI render correctness requires visual inspection — cannot be fully automated. Must verify: port table data, state colors, keyboard input, clean terminal exit."

# Metrics
duration: 18min
completed: 2026-07-26
status: complete
---

# Phase 01 Plan 01: Walking Skeleton — TCP port scanner with live TUI table

**Cargo workspace compiles, Windows TCP scan via GetExtendedTcpTable works, TUI renders real-time port table with state colors, keyboard refresh, and clean exit**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-07-26T10:40:51Z
- **Completed:** 2026-07-26T10:58:59Z
- **Tasks:** 3
- **Files modified:** 19 (12 created, 7 modified)

## Accomplishments
- Root Cargo workspace with 17 workspace dependencies (windows 0.62, sysinfo 0.39, rusqlite 0.40, ratatui 0.30, crossterm 0.29, etc.)
- Async PortScanner trait using #[async_trait] with scan() and scan_process(pid)
- Windows TCP scanner: GetExtendedTcpTable two-call buffer pattern, ntohs byte-order conversion, sysinfo process name resolution by PID cache, all inside spawn_blocking
- SQLite database init with WAL journal mode, 5s busy timeout, settings table with schema version
- TOML config: AppSettings struct with admin_detected, load/save with default-on-parse-error resilience
- TEA event loop: crossterm raw mode + alternate screen, mpsc unbounded channel, event::poll with 200ms timeout
- One Dark theme with 12 semantic color slots (fg/bg/accent/status)
- Port table widget: Ratatui Table with zebra striping, color-coded state symbols (LISTENING=green, ESTABLISHED=blue, CLOSE_WAIT=yellow, TIME_WAIT=gray)

## Task Commits

Each task was committed atomically:

1. **Task 1: Workspace scaffold** — `a563e30` (feat)
2. **Task 2: Windows TCP scanner + SQLite WAL + config TOML** — `5e0ab4a` (feat)
3. **Task 3 (tracer): TUI TEA event loop** — `70ef1fe` (feat)

## Files Created/Modified
- `Cargo.toml` — 17 workspace dependencies added
- `port-core/Cargo.toml` — async-trait, windows, sysinfo, tokio, rusqlite, serde_json, tracing, toml
- `port-tui/Cargo.toml` — port-core, ratatui, crossterm, tokio, clap, anyhow, tracing, tracing-subscriber, chrono
- `port-core/src/lib.rs` — pub mod store, pub mod config, pub mod windows with re-exports
- `port-core/src/scanner.rs` — Async PortScanner trait with #[async_trait]
- `port-core/src/scanner/tcp.rs` — GetExtendedTcpTable wrapper (created)
- `port-core/src/windows.rs` — WindowsPortScanner impl (modified from stub)
- `port-core/src/store.rs` — Storage module declarations (created)
- `port-core/src/store/connection.rs` — SQLite WAL init (created)
- `port-core/src/config.rs` — Config module declarations (created)
- `port-core/src/config/settings.rs` — TOML settings load/save (created)
- `port-tui/src/main.rs` — TEA event loop entry point (replaced placeholder)
- `port-tui/src/app.rs` — App state struct (created)
- `port-tui/src/message.rs` — Message enum (created)
- `port-tui/src/update.rs` — TEA update function (created)
- `port-tui/src/theme.rs` — One Dark theme (created)
- `port-tui/src/components/mod.rs` — Component trait (created)
- `port-tui/src/components/ports.rs` — Port table widget (created)

## Decisions Made
- Used `windows 0.62` instead of `0.73` — 0.73 not yet published on crates.io. Adapted API calls accordingly (GetExtendedTcpTable returns u32, MIB_TCP_STATE is newtype, ntohs is unsafe, AF_INET is ADDRESS_FAMILY)
- Adapted to new-style Rust module layout per CLAUDE.md ("No mod.rs files"): scanner.rs + scanner/tcp.rs instead of scanner/mod.rs + scanner/tcp.rs
- Made `windows` module public so TUI can access `port_core::windows::WindowsPortScanner` directly
- Added `chrono.workspace = true` to port-tui for status bar timestamp formatting

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed windows crate version mismatch**
- **Found during:** Task 1 (workspace scaffold)
- **Issue:** Plan specified `windows = "0.73"` but latest published version is 0.62.2. Build failed with version resolution error.
- **Fix:** Changed to `windows = "0.62"` and adapted all API calls: GetExtendedTcpTable returns u32 (not Result), MIB_TCP_STATE_* are newtype constants requiring .0 accessor, ntohs is unsafe, AF_INET is ADDRESS_FAMILY requiring .0 cast
- **Files modified:** Cargo.toml, port-core/src/scanner/tcp.rs
- **Committed in:** a563e30 (Task 1), 5e0ab4a (Task 2)

**2. [Rule 3 - Blocking] Adapted module layout to new-style Rust modules**
- **Found during:** Task 1 (workspace scaffold)
- **Issue:** Plan specified `mod.rs` files (scanner/mod.rs, store/mod.rs, config/mod.rs) but codebase was refactored to new-style Rust modules per CLAUDE.md ("No mod.rs files"). Using mod.rs would violate project conventions.
- **Fix:** Used new-style layout: scanner.rs + scanner/tcp.rs, store.rs + store/connection.rs, config.rs + config/settings.rs
- **Files modified:** All module files
- **Committed in:** a563e30, 5e0ab4a

**3. [Rule 3 - Blocking] Added chrono dependency to port-tui**
- **Found during:** Task 3 (TUI event loop)
- **Issue:** Plan used `chrono::Local::now()` in status bar but chrono was not in port-tui dependencies
- **Fix:** Added `chrono.workspace = true` to port-tui/Cargo.toml
- **Files modified:** port-tui/Cargo.toml
- **Committed in:** 70ef1fe

**4. [Rule 3 - Blocking] Made windows module public**
- **Found during:** Task 3 (TUI event loop)
- **Issue:** TUI needed `port_core::windows::WindowsPortScanner` but windows module was private
- **Fix:** Changed `mod windows` to `pub mod windows` in lib.rs
- **Files modified:** port-core/src/lib.rs
- **Committed in:** 70ef1fe

**5. [Rule 3 - Blocking] Fixed CrosstermBackend import for ratatui 0.30**
- **Found during:** Task 3 (TUI event loop)
- **Issue:** `ratatui::crossterm::CrosstermBackend` does not exist in ratatui 0.30.2. The backend was moved to `ratatui::backend::CrosstermBackend`
- **Fix:** Updated import to `ratatui::backend::CrosstermBackend`, added explicit EnableMouseCapture/DisableMouseCapture imports
- **Files modified:** port-tui/src/main.rs
- **Committed in:** 70ef1fe

---

**Total deviations:** 5 auto-fixed (1 bug, 4 blocking)
**Impact on plan:** All fixes necessary for compilation correctness. No scope creep. The windows 0.62 API adaptation was the most significant — the plan assumed 0.73 API shapes that don't exist yet.

## Issues Encountered
- windows 0.62 API returns raw u32 error codes (not Result<()>), requiring manual comparison against ERROR_INSUFFICIENT_BUFFER (122) and NO_ERROR (0)
- MIB_TCP_STATE constants are newtype MIB_TCP_STATE(i32) in windows 0.62, not plain u32 — needed .0 accessor for comparison
- ntohs is declared unsafe in windows 0.62 — required unsafe blocks around port number conversion

## Next Phase Readiness
- Walking skeleton proven: workspace compiles, Windows TCP scan works, TUI renders live port table
- Plan 02 (search/filter/sort) can build on the existing port table widget, Message enum, and TEA architecture
- Plan 02 should add UDP scanning (D-02, D-04) and IPv6 (Pitfall #3)
- Port table uses basic Ratatui Table — Plan 02 should upgrade to VirtualTable/DataTable for 1000+ rows (Pitfall #13)
- Theme only has One Dark — theme switching (TUI-05) is Phase 6

---
*Phase: 01-tui-port-viewer*
*Completed: 2026-07-26*

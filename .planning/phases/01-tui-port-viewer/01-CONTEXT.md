# Phase 1: TUI Port Viewer - Context

**Gathered:** 2026-07-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver a working terminal application (`port-tui`) that launches to a live, sortable, filterable table of all TCP and UDP ports with owning process details, color-coded by connection state. Admin elevation is auto-detected on startup with a one-key trigger.

Users: launch → see Overview tab with summary stats → switch to Ports tab for the full table → sort by any column → press `/` to fuzzy search → press `f` to apply combined filters → press `r` to manually refresh → press `a` to elevate to admin.
</domain>

<decisions>
## Implementation Decisions

### Scanner Implementation
- **D-01 — Buffer retry strategy:** Use exponential growth for `GetExtendedTcpTable`/`GetExtendedUdpTable` buffer allocation. Start at 16KB, double on `ERROR_INSUFFICIENT_BUFFER`, max 3 retries. If all retries fail, keep last successful scan data visible and show error in status bar.
- **D-02 — Dual-stack enumeration:** Internally call both AF_INET and AF_INET6 APIs, merge results into a single unified port list. Annotate rows with IP version in the address display (`::1:8080` vs `0.0.0.0:8080`). User sees one complete view.
- **D-03 — Error resilience:** On complete scan failure, preserve last successful data in the table. Status bar shows red error with "Press r to retry." Never blank the table on transient failures.
- **D-04 — TCP/UDP concurrency:** Use `tokio::join!` to scan TCP and UDP tables simultaneously. Merge and return combined results. Wall-clock time = max(TCP_scan, UDP_scan).
- **D-05 — Performance approach:** Simple implementation first — concurrent scans + batch process resolution + exponential retry. No pre-optimization. Measure with criterion benchmarks after implementation. Target: <500ms for 1000 ports.

### Admin Elevation
- **D-06 — Elevation mechanism:** Detect non-admin at startup via `IsUserAnAdmin()`. On user pressing `a`, use `ShellExecuteExW` with `runas` verb to relaunch `port-tui.exe` as administrator. Old process exits after triggering elevation. — **Reversibility:** costly — changes the process lifecycle contract; all callers of the TUI binary must handle self-restart.
- **D-07 — UAC denial handling:** If user clicks "No" on the UAC prompt, the app continues running in non-admin mode. Status bar shows "Admin needed — press a to elevate." User can retry elevation at any time. No modal dialogs, no forced exit.
- **D-08 — State transfer on restart:** No state is transferred to the elevated process. The new process performs a fresh full scan on startup. Scan typically completes in <500ms, making the transition imperceptible.
- **D-09 — Detection timing:** Check admin status once at startup. Show persistent status bar indicator: "Admin ✓" (admin) or "Admin needed — press a to elevate" (non-admin). Non-admin mode shows all ports; system-owned process details display "—" with dim modifier.

### TUI Event Loop
- **D-10 — Event model:** crossterm `event::poll(Duration)` with timeout for keyboard/mouse events. Timeout triggers periodic tasks (auto-refresh check). `#[tokio::main]` with default multi-threaded runtime on main; crossterm Terminal lives on the main thread.
- **D-11 — Auto-refresh:** 5-second interval background scan. Results silently update the table without interrupting user interaction. Manual refresh on `r` key triggers immediate scan with spinner indicator.
- **D-12 — TEA + async bridge:** Scanning runs in spawned tokio tasks. Results sent via `tokio::sync::mpsc::unbounded_channel` to the main thread. Main event loop does `try_recv` on each tick, wrapping results in `Message::ScanComplete(Vec<Connection>)` and passing to the TEA `update()` function. — **Reversibility:** costly — changing the channel type or message shape affects both the scanner spawn sites and the main loop receiver.
- **D-13 — Runtime architecture:** Single `#[tokio::main]` at the binary entry point. Multi-threaded scheduler (default). Windows API calls inside `spawn_blocking` to avoid blocking the async runtime. crossterm `Terminal` constructed and driven on the main thread.

### Initial Launch Experience
- **D-14 — Default tab:** App opens on Overview tab (Tab 1) — summary stats + top 10 ports + admin status card. Users see system state at a glance, then navigate to Ports tab for detail.
- **D-15 — First render:** Immediately render the full frame layout (tab bar + status bar + footer) with a spinner and "Scanning ports..." in the content area. No blank-screen delay. First frame visible within one tick of startup.
- **D-16 — Process name resolution:** After port scan returns, collect all unique PIDs, batch-resolve process names via `sysinfo::System::refresh_processes()`, cache by PID. Same PID appearing on multiple ports hits the cache. Avoid per-connection `OpenProcess` calls.

### Claude's Discretion

No areas were deferred to Claude — all 16 decisions were explicitly selected by the user.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Planning Artifacts
- `.planning/ROADMAP.md` — Phase 1 scope, success criteria, requirements list, pitfall coverage
- `.planning/REQUIREMENTS.md` — Full v1 requirements (SCAN-01 through TUI-08), traceability matrix
- `.planning/STATE.md` — Accumulated decisions, workspace structure, WAL mode, TEA architecture, async-first API
- `.planning/phases/01-tui-port-viewer/01-UI-SPEC.md` — **Approved design contract:** layout (4-region grid), semantic color slots (One Dark, 12 slots), keyboard layers (L0-L3), widget inventory (Overview + Ports + 3 placeholders), copywriting contract (all empty/error/loading states)

### Codebase (existing scaffold)
- `port-core/src/lib.rs` — Module structure, error types, platform abstraction (`#[cfg(target_os = "windows")]`)
- `port-core/src/models/port.rs` — `Port`, `Protocol`, `PortState` enums
- `port-core/src/models/connection.rs` — `Connection`, `HistoryEntry`, `TrafficStats`, `FirewallRule`, `Favorite`
- `port-core/src/models/process.rs` — `ProcessInfo` struct
- `port-core/src/scanner/mod.rs` — `PortScanner` trait (defined, not implemented)
- `port-tui/src/main.rs` — Placeholder entry point
- `Cargo.toml` — Workspace definition (3 members), dependencies (thiserror, chrono, serde, tokio)

### Research (project-level)
- `.planning/research/ARCHITECTURE.md` — Platform abstraction design, trait boundaries, crate dependency graph
- `.planning/research/PITFALLS.md` — #10 (single workspace), #12 (WAL mode), #14 (allocator mismatch)
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`port-core::models` module:** Fully defined data model — `Port`, `Connection`, `ProcessInfo`, `PortState`, `Protocol`. No model changes needed for Phase 1.
- **`PortScanner` trait:** Defined in `port-core/src/scanner/mod.rs` with `scan()` and `scan_process(pid)` signatures. Ready for Windows implementation.
- **`Error` enum:** Defined in `port-core/src/lib.rs` — `Platform`, `NotFound`, `PermissionDenied`, `Io` variants cover scanner failure modes.
- **Workspace structure:** 3-member Cargo workspace already configured. `port-tui` has its own `Cargo.toml`.

### Established Patterns
- **Trait-based platform abstraction:** Windows implementation behind `#[cfg(target_os = "windows")]`. Linux/macOS compile error for now.
- **async-first API:** All Win32 calls wrapped in `spawn_blocking` per STATE.md decision. Public API returns async, internals sync where Windows APIs are synchronous.
- **Edition 2024:** Workspace uses Rust edition 2024 with resolver = "3".

### Integration Points
- **`port-core` → `port-tui`:** TUI binary depends on `port-core` for models and scanner. IPC not needed — direct Rust function calls.
- **Scanner → TEA:** Scan results flow through `tokio::sync::mpsc` channel → `Message::ScanComplete` → `update()` → view render.
- **TUI → Config:** TOML config in app data directory (CORE-05). Phase 1 needs admin status, no settings UI yet.
</code_context>

<specifics>
## Specific Ideas

- User wants the status bar to show "Live · N ports · Admin ✓/needed · HH:MM:SS" pattern (defined in UI-SPEC Copywriting Contract)
- User rejected: fixed-frame-rate loop, per-connection process resolution, splash screen, state transfer on elevation
- User chose simplicity-first: fresh scan on restart, no pre-optimization, benchmark after implementation
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 1 scope.
</deferred>

---

*Phase: 1-tui-port-viewer*
*Context gathered: 2026-07-26*

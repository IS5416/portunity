---
phase: 01-tui-port-viewer
plan: 03
subsystem: filter-engine, search-ui, filter-ui, admin-elevation
tags: [filtering, fuzzy-search, admin-elevation, uac, tui-overlay]
requires:
  - 01-01
  - 01-02
provides:
  - apply_filters
  - fuzzy_search
  - search-overlay
  - filter-panel-overlay
  - admin-elevation
  - non-admin-graceful-degradation
affects:
  - port-core/filter
  - port-tui/main
  - port-tui/components
  - port-tui/update
tech-stack:
  added:
    - windows 0.62 (Win32_UI_Shell, Win32_UI_WindowsAndMessaging, Win32_System_Registry)
  patterns:
    - Free functions over traits for filter engine (no platform variants)
    - TEA message dispatch: mode-aware keyboard routing (search > filter > default)
    - Overlay rendering with ratatui Clear widget for non-modal overlays
    - spawn_blocking for ShellExecuteExW (Win32 blocking call)
    - Vec-based Filter dimensions: AND across dimensions, OR within Vec
key-files:
  created:
    - port-tui/src/components/search.rs
    - port-tui/src/components/filter_panel.rs
    - port-tui/src/elevate.rs
  modified:
    - port-core/src/filter.rs
    - port-core/src/lib.rs
    - port-tui/src/app.rs
    - port-tui/src/main.rs
    - port-tui/src/message.rs
    - port-tui/src/update.rs
    - port-tui/src/components.rs
    - port-tui/src/components/ports.rs
    - Cargo.toml
    - port-tui/Cargo.toml
decisions:
  - Filter struct uses Vec fields (protocols, process_names, pids, states) vs plan-assumed Option fields; adapted engine to OR-within-Vec logic naturally
  - Filter engine uses free functions (apply_filters, fuzzy_search) not a trait — no platform variants exist for filtering
  - Protocol matching in filter includes Tcp6/Udp6 sub-variants (tcp matches Tcp+Tcp6)
  - System process detection uses PID<1000 OR hardcoded name set (pid < 1000 catches most, name list handles edge cases like svchost.exe with high PID)
  - SHELLEXECUTEINFOW struct uses Default::default() in windows-rs 0.62 (union Anonymous field instead of flat hMonitor)
  - ShellExecuteExW returns windows_core::Result<()> in 0.62 (not BOOL)
  - SW_SHOW used as literal 5i32 since SHOW_WINDOW_CMD newtype in 0.62
  - Windows API used in both port-core (scanner) and port-tui (elevate); workspace features extended for both
metrics:
  duration: ~15min
  completed: "2026-07-28T10:22:33Z"
  tasks: 2
  files: 12
status: complete
---

# Phase 01 Plan 03: Interactive Filtering, Search, and Admin Elevation Summary

Added interactive fuzzy search, combined multi-dimension filtering, and admin elevation to the TUI port viewer. Users can press `/` for real-time fuzzy search across all port fields, press `f` for combined dimension filters, and press `a` to trigger UAC elevation. Non-admin users see all ports but with dimmed process details for system-owned processes.

## Tasks Completed

### Task 1: Filter engine in port-core + search/filter UI in TUI

**Commit:** `66e332f`

- Replaced `port-core/src/filter.rs` stub with real free functions: `apply_filters()` and `fuzzy_search()`
- `apply_filters` supports 5 filter dimensions with AND logic across dimensions, OR logic within Vec dimensions
- `fuzzy_search` performs case-insensitive substring match across concatenated port/process/PID/protocol/state fields
- Added 8 unit tests for filter engine (port range, protocol, process name, combined AND, fuzzy search multi-field, case-insensitive, empty query, empty filter)
- Extended `App` with `filtered_ports`, `search_query`, `search_active`, `filter_active`, `filter_focused_field`
- Extended `Message` enum with `SearchInput/Backspace/Clear/Activate/Deactivate/CursorLeft/CursorRight` and `FilterActivate/Deactivate/UpdateField/Apply/TabField` variants
- Extended `update()` with search recalc, filter field parsing, sort-on-display-data, and ScanComplete re-application
- Created `SearchComponent` — 3-row overlay with `/` prompt, cursor, and help hint using Clear widget
- Created `FilterPanelComponent` — 5-row overlay with tab-cycling fields (PortMin, PortMax, ProcessName, Pid, Protocol, State)
- Updated `main.rs` keyboard dispatch: mode-aware routing (search mode traps printable chars + Backspace + Enter + Esc; filter mode traps printable chars + Tab + Enter + Esc; all other keys pass through)
- Updated `PortsComponent` to render `app.display_data()` with search/filter empty states per UI-SPEC copywriting
- Context-sensitive status bar (search: "Search: {query} · {M} results", filter: "Filtered: {M} of {N} ports")
- Context-sensitive footer (default: shows `[/]Search [f]Filter`, search: `[Esc]Cancel [Enter]Confirm`, filter: `[Esc]Cancel [Tab]Next field [Enter]Apply`)

### Task 2: Admin elevation — detection, UAC relaunch, non-admin graceful degradation

**Commit:** `22a7ced`

- Created `port-tui/src/elevate.rs` with `is_admin()` (via IsUserAnAdmin) and `elevate_to_admin()` (via ShellExecuteExW runas)
- D-06: ShellExecuteExW with "runas" verb triggers Windows UAC consent prompt
- D-07: On UAC decline (ERROR_CANCELLED=1223), function returns Ok(()), app continues in non-admin mode
- D-08: On elevation approval, `std::process::exit(0)` immediately; new elevated process performs fresh scan
- D-09: Admin check runs once at startup via IsUserAnAdmin
- Added `is_admin`, `admin_check_done`, `elevating` fields to App
- Added `AdminCheck(bool)`, `ElevateRequest`, `ElevateDeclined` messages
- Startup admin check: `elevate::is_admin()` called after terminal init but before initial scan; result sent as AdminCheck message
- `a` key: triggers `ElevateRequest` when not admin, check done, and no overlay active
- ElevateRequest handled in main loop: `spawn_blocking` calls `elevate_to_admin()`; on decline, sends `ElevateDeclined`
- `elevating` flag prevents double-elevation (T-03-03 mitigation)
- Status bar: `Admin ✓` in green (admin) or `Admin needed — press a to elevate` in yellow (non-admin); hidden until check completes to prevent flicker
- Footer: `[a]Elevate` shown when not admin (admin check done); removed when admin; search/filter footers take priority
- System process dimming: when non-admin, process names dimmed for system processes (PID<1000 or name in known set: svchost, services, lsass, winlogon, csrss, smss, wininit, System, etc.)
- Extended workspace `windows` features: `Win32_UI_Shell`, `Win32_UI_WindowsAndMessaging`, `Win32_System_Registry`
- Added `windows.workspace = true` to port-tui/Cargo.toml

## Deviations from Plan

### Adapted to existing code

**1. Filter struct shape mismatch**
- **Found during:** Task 1 Step 1
- **Issue:** Plan assumed Filter struct with singular Option fields (`process_name: Option<String>`, `pid: Option<u32>`). Actual struct uses Vec fields (`process_names: Vec<String>`, `pids: Vec<u32>`, `protocols: Vec<Protocol>`, `states: Vec<PortState>`)
- **Fix:** Implemented `apply_filters` with AND logic across dimensions, OR logic within Vec dimensions. Empty Vec treated as pass-all. More powerful than plan's single-value-per-dimension design.
- **Files modified:** `port-core/src/filter.rs`

**2. No existing filter.rs — created from scratch**
- **Found during:** Task 1 Step 1
- **Issue:** Plan said to "replace existing FilterEngine trait stub" but `port-core/src/filter.rs` content was a trait stub, not a file that should be replaced. The file existed but with only a trait definition.
- **Fix:** Replaced the trait with free functions (no platform variants needed for filtering). Changed from trait to plain functions per plan's own guidance.

**3. Windows API v0.62 compatibility**
- **Found during:** Task 2 build
- **Issue:** Plan assumed windows-rs v0.73. Actual v0.62 has different API shapes: `ShellExecuteExW` returns `windows_core::Result<()>`, `SHELLEXECUTEINFOW` uses `Anonymous` union, `IsUserAnAdmin` in `Win32::UI::Shell` not `Win32::Security`, `SW_SHOW` needs `Win32_UI_WindowsAndMessaging` feature
- **Fix:** Rewrote elevate.rs for v0.62: used `success.is_ok()` instead of `.as_bool()`, `Default::default()` for SEI struct, `5i32` literal for SW_SHOW, qualified path for IsUserAnAdmin. Added 3 extra windows features to workspace.
- **Files modified:** `port-tui/src/elevate.rs`, `Cargo.toml`

### Auto-fixed Issues

**1. [Rule 1 - Bug] FilterField missing PartialEq derive**
- **Found during:** Task 1 build
- **Issue:** FilterPanelComponent compared `FilterField` values with `==` but the enum lacked `PartialEq` derive
- **Fix:** Added `#[derive(PartialEq, Eq)]` to `FilterField`
- **Commit:** `66e332f`

**2. [Rule 3 - Blocking] Rc<[Rect]> not iterable in filter_panel.rs**
- **Found during:** Task 1 build
- **Issue:** `&panel_area` where `panel_area: Rc<[Rect]>` — the reference to Rc doesn't implement IntoIterator in Ratatui 0.30
- **Fix:** Changed to `panel_area.iter()` which deref-coerces to `[Rect]` slice iterator
- **Commit:** `66e332f`

**3. [Rule 1 - Bug] Temporary value dropped while borrowed in filter_panel.rs**
- **Found during:** Task 1 build
- **Issue:** `&format!("{}-{}", ...)` created temporary that was freed while spans referencing it were still alive
- **Fix:** Bound formatted strings to local `let` bindings before passing to `build_field_row`
- **Commit:** `66e332f`

## Known Stubs

| File | Line | Description |
|------|------|-------------|
| port-core/src/filter.rs (favorite_only filter) | ~72 | `favorite_only` filter is pass-all stub — Phase 6 integrates with favorites database |
| port-tui/src/elevate.rs (D-08 state transfer) | n/a | No state on elevation — by design (D-08); fresh scan on restart |

## Threat Flags

None. Existing threat model (T-03-01 through T-03-03) covers the implemented surface. All mitigations are in place:
- T-03-01: UAC prompt OS-mediated, process exit prevents stale handles
- T-03-02: PID<1000 heuristic is cosmetic only (dimming, not access control)
- T-03-03: `elevating` flag prevents concurrent elevation requests

## Self-Check: PASSED

All created files exist, both commits verified in git log.

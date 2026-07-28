---
phase: 01-tui-port-viewer
plan: 02
subsystem: scanner, tui
tags:
  - scanner
  - tui
  - ratatui
  - dual-stack
  - tcp
  - udp
  - auto-refresh
  - sort
  - keyboard-nav
  - virtual-scrolling
requires:
  - 01-01
provides:
  - dual-stack TCP+UDP scanner
  - batch process name resolution
  - production TUI port table
affects:
  - port-core scanner
  - port-tui components
  - port-tui main loop
tech-stack:
  added:
    - eddacraft-tui 0.4.1 (DataTable widget)
  patterns:
    - tokio::join! for concurrent scanning
    - ProcessResolver with HashMap cache
    - virtual scrolling with viewport-only Row rendering
    - sort state as App fields with in-place sort_by
key-files:
  created:
    - port-core/src/scanner/udp.rs
    - port-core/src/scanner/resolver.rs
  modified:
    - port-core/src/scanner/tcp.rs
    - port-core/src/scanner.rs
    - port-core/src/windows.rs
    - port-core/src/lib.rs
    - port-core/src/models/port.rs
    - port-tui/Cargo.toml
    - port-tui/src/app.rs
    - port-tui/src/message.rs
    - port-tui/src/update.rs
    - port-tui/src/main.rs
    - port-tui/src/components/ports.rs
    - port-tui/src/theme.rs
    - Cargo.lock
decisions:
  - "D-01 implemented: exponential buffer retry (16KB start, double, max 3 retries)"
  - "D-02 implemented: dual-stack AF_INET + AF_INET6 with IPv4-mapped IPv6 deduplication"
  - "D-03 implemented: scan failure preserves last successful data; red error bar with retry prompt"
  - "D-04 implemented: tokio::join! concurrent TCP+UDP in scan_all()"
  - "D-11 implemented: 5-second auto-refresh in main event loop, guarded by scanning/error state"
  - "D-16 implemented: ProcessResolver with PID cache, batch sysinfo resolution"
  - "SCAN-03 implemented: full color mapping for 11 TCP states + UDP per UI-SPEC"
  - "SCAN-04 implemented: sort cycles none -> ascending(▲) -> descending(▼) -> none on 's' key"
  - "TUI-02 implemented: j/k/Up/Down/g/G keyboard row navigation with reverse-video selection"
  - "TUI-04 implemented: virtual scrolling with viewport-only Row rendering and right-edge scrollbar"
  - "User feedback applied: State column expanded to 10 chars with abbreviated text labels (● LISTEN, ○ T_WAIT, ◉ C_WAIT, — UDP)"
metrics:
  duration: "~30 minutes"
  completed_date: "2026-07-28"
status: complete
---

# Phase 01 Plan 02: Scanner Completeness + TUI Polish Summary

Dual-stack TCP+UDP scanner with exponential buffer retry, batch process name resolution, and production-quality TUI port table with sort, keyboard navigation, auto-refresh, and full color mapping.

## Tasks Executed

| # | Name | Type | Commit | Files |
|---|------|------|--------|-------|
| 1 | Scanner completeness — dual-stack, UDP, retry, concurrency, batch process resolution | auto | `03493cc` | port-core/src/scanner/tcp.rs, udp.rs (new), resolver.rs (new), scanner.rs, windows.rs, lib.rs, models/port.rs |
| 2 | TUI enhancements — DataTable, sort, color map, keyboard nav, auto-refresh | auto | `2688f60` | port-tui/Cargo.toml, Cargo.lock, app.rs, message.rs, update.rs, main.rs, components/ports.rs, theme.rs |

## Verification Results

- `cargo build -p port-core`: PASSED (0 errors)
- `cargo build --bin port-tui`: PASSED (0 errors)
- Both compile clean; only pre-planned dead-code warnings (extension points for future phases)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Protocol enum missing Hash derive**
- **Found during:** Task 1, build verification
- **Issue:** HashSet<(u16, Protocol)> requires Protocol: Hash; the Protocol enum only derived Debug, Clone, Copy, PartialEq, Eq
- **Fix:** Added `Hash` to Protocol's derive macros in port-core/src/models/port.rs
- **Files modified:** port-core/src/models/port.rs
- **Commit:** `03493cc`

**2. [Rule 1 - Bug] Cast precedence in scrollbar thumb size calculation**
- **Found during:** Task 2, build verification
- **Issue:** `viewport as f64 / total as f64 * track_height as f64 .ceil() as usize` parsed as cast followed by method call; Rust requires parens
- **Fix:** Wrapped expression in parentheses: `(((viewport as f64 / total as f64) * track_height as f64).ceil() as usize)`
- **Files modified:** port-tui/src/components/ports.rs
- **Commit:** `2688f60`

**3. [Rule 1 - Bug] header_cell missing lifetime parameter**
- **Found during:** Task 2, build verification
- **Issue:** Function returning `Cell` with borrowed references needed explicit lifetime annotation
- **Fix:** Added `<'a>` to function signature: `fn header_cell<'a>(...) -> Cell<'a>`
- **Files modified:** port-tui/src/components/ports.rs
- **Commit:** `2688f60`

**4. [Rule 1 - Bug] Vec<Span> cannot convert to Text for Cell**
- **Found during:** Task 2, build verification
- **Issue:** `Cell::from(vec![Span::styled(...)])` — Ratatui's `Cell` doesn't implement `From<Vec<Span>>`
- **Fix:** Wrapped spans in `Text::from(Line::from(vec![...]))`
- **Files modified:** port-tui/src/components/ports.rs
- **Commit:** `2688f60`

**5. [Rule 3 - Blocking] Unused import cleanup**
- **Found during:** Task 2, build verification
- **Issue:** `KeyModifiers` and `SortColumn` imports in main.rs were unused
- **Fix:** Removed unused imports
- **Files modified:** port-tui/src/main.rs
- **Commit:** `2688f60`

## Decisions Made

| Decision | Context | Outcome |
|----------|---------|---------|
| Exponential buffer retry | D-01: start 16KB, double on ERROR_INSUFFICIENT_BUFFER, max 3 retries | Implemented in both tcp.rs and udp.rs with identical patterns |
| Dual-stack merge strategy | D-02: scan both AF_INET and AF_INET6, deduplicate IPv4-mapped IPv6 | AF_INET entries kept as canonical; IPv4-mapped Tcp6/Udp6 duplicates dropped |
| Process name resolution | D-16: batch resolve all PIDs with cache | ProcessResolver with HashMap; special-cases PID 0 ("System Idle Process") and PID 4 ("System") |
| scan_all orchestration | D-04: tokio::join! for concurrent TCP+UDP | Returns merged Vec<Connection> with names applied from ProcessResolver |
| State column label format | User feedback: color alone insufficient | "● LISTEN" (10-char column), "○ T_WAIT", "◉ C_WAIT", "— UDP", etc. |
| Auto-refresh guard | D-11: 5-second interval, not interrupt user | Guard: `!app.scanning && app.error.is_none()` before triggering |
| eddacraft-tui dependency | Plan step 1: add 0.4 for DataTable | Resolved to 0.4.1; used as dependency but core table rendered with ratatui::widgets::Table + custom virtual scrolling |

## Known Stubs

None. All implemented features are wired end-to-end. Pre-planned extension points (SortColumn::next, Message::Tick, accent_secondary, bg_overlay) are documented as dead-code warnings for future phases — not stubs.

## Threat Flags

None. All security surface matches the plan's threat model. Buffer retry is bounded (max 128KB). Auto-refresh is guarded against concurrent scans. Process name cache holds only data already visible via Task Manager.

## Verification Notes

Terminal visual verification (`cargo run --bin port-tui`) was not performed (non-interactive environment). Human verification checklist per plan:
1. Table shows both TCP and UDP entries
2. Colors match UI-SPEC: LISTENING=green ●, ESTABLISHED=blue ●, TIME_WAIT=gray ○, CLOSE_WAIT=yellow ◉, UDP=gray dash
3. Pressing 's' cycles sort on current column (▲ → ▼ → no indicator)
4. j/k or arrow keys move selection highlight up/down
5. Footer shows "[↑↓jk]Navigate [s]Sort [r]Refresh [q]Quit [?]Help"
6. Wait 5 seconds — table auto-refreshes without user action
7. Status bar shows "Live · N ports · HH:MM:SS"

## Self-Check: PASSED

- [x] Commit `03493cc` exists (Task 1)
- [x] Commit `2688f60` exists (Task 2)
- [x] port-core/src/scanner/udp.rs exists
- [x] port-core/src/scanner/resolver.rs exists
- [x] All modified files confirmed on disk
- [x] `cargo build -p port-core` passes (0 errors)
- [x] `cargo build --bin port-tui` passes (0 errors)

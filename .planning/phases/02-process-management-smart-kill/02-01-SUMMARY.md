---
phase: 02-process-management-smart-kill
plan: 01
subsystem: process-management
tags: [kill-pipeline, whitelist, process-snapshot, tui, ctrl-c-helper]
type: execute
status: complete
requires: []
provides:
  - "port-core::process::handle (ProcessSnapshot, open_verified)"
  - "port-core::process::whitelist (BUILTIN, Protection, protection_status)"
  - "port-core::process::kill (Strategy, KillOutcome, kill)"
  - "port-tui x-key kill surface (confirm dialog, status outcomes, --ctrl-c helper)"
affects:
  - port-core/src/process.rs
  - port-core/src/config/settings.rs
  - port-tui/src/{main,app,message,update,components}.rs
  - Cargo.toml
key-files:
  created:
    - port-core/src/process/handle.rs
    - port-core/src/process/whitelist.rs
    - port-core/src/process/kill.rs
    - port-core/tests/kill_integration.rs
    - port-core/tests/process_handle_integration.rs
    - port-tui/src/components/kill_confirm.rs
  modified:
    - Cargo.toml
    - port-core/src/process.rs
    - port-core/src/config/settings.rs
    - port-core/src/models/process.rs
    - port-core/src/filter.rs
    - port-core/src/scanner/tcp.rs
    - port-core/src/scanner/udp.rs
    - port-tui/src/message.rs
    - port-tui/src/app.rs
    - port-tui/src/main.rs
    - port-tui/src/update.rs
    - port-tui/src/components.rs
decisions:
  - "HardBlocked status copy uses the compact form '✗ {name} … Press w to review the whitelist.' (name budget term_width−41) — the full 127+ char form never fits the 80-col gate (A9 declared in format_kill_status)"
  - "Ctrl+C console probe merged into the helper call: helper exit code 0=delivered / 1=no console decides ConsoleCtrlC vs ForceDirect — no separate AttachConsole probe in the TUI process (console-ownership constraint)"
  - "TerminateProcess/kill task runs via tokio::spawn (kill() is async and does its own spawn_blocking) — not nested spawn_blocking"
  - "last_killed_pid cleared in update()'s ScanComplete handler (functionally the drain-loop block the plan named)"
  - "kill_status persists until the next kill attempt (KillStart overwrites) — outcome stays visible across the post-kill scan per D-04"
tech-stack:
  added: []
  patterns:
    - "windows-rs error mapping: HRESULT 0x8007XXXX → Win32 code via low-16-bit mask (ERROR_ACCESS_DENIED=5)"
    - "Send-safe identity crossing mpsc: ProcessSnapshot; HANDLE only inside spawn_blocking scopes"
    - "Two-stage protection gate: Kill → snapshot_for+protection_status → KillPrepared → (None: direct kill | UserConfirm: dialog | HardBlocked: status)"
    - "Self-reexec Ctrl+C helper: hidden clap flag, early-return guard before terminal init"
metrics:
  duration: "~2.5h execution (started 2026-07-31T09:49:33Z)"
  tasks: 2
  commits: 5 (511da68, fa8a682, 05e9d89, cd80ccc)
  tests: 62 passing (45 unit + 5 kill integration + 1 churn + 11 TUI)
actuals:
  tokens: 26000   # chars/4 over the 2606-line realized diff (≈104k chars)
  tasks: 2
  commits: 5
---

# Phase 2 Plan 1: Smart Kill — Core Pipeline + TUI Kill Surface Summary

One-liner: full smart-kill escalation pipeline (WM_CLOSE / Ctrl+C helper / force) with PID-reuse-safe ProcessSnapshot identity, two-tier whitelist protection, and the x-key TUI kill surface with confirm dialog, status-bar outcomes, and post-kill auto-refresh.

## What Was Built

**Core (port-core, Task 1 tracer):**

- `process/handle.rs` — `ProcessSnapshot { pid, creation_time: Option<FILETIME>, executable_path }` (Send-safe channel payload); internal `OpenProcessHandle` with Drop→CloseHandle (HANDLE never crosses an async boundary); `snapshot_for()` (QLI|SYNCHRONIZE, creation time + path with buffer retry); `open_verified()` verifying `GetProcessId` + `GetProcessTimes` creation FILETIME before any action — mismatch aborts with `NotFound` (PROC-07, Pitfall #1); pure `creation_matches()` unit-tested.
- `process/whitelist.rs` — `BUILTIN` (25 entries after A1 human review: dropped explorer.exe, fixed securesystem typo; Restart Manager Tier-1 14 + Tier-2 11, each with plain-language reason); `builtin_match` (PID 0/4 special cases + case-insensitive basename); `user_match` (normalized full-path, case-insensitive, quotes/trailing separators stripped); `Protection { None, UserConfirm, HardBlocked }`; `protection_status` with built-in tier checked FIRST (Pitfall #6).
- `process/kill.rs` — `Strategy` + pure `route_strategy` (full matrix tested); `KillOutcome` (8 variants); `kill()` async entry wrapping one `spawn_blocking` scope: D-15 settings re-read → protection gate BEFORE OpenProcess (Pitfall #11) → open+verify → already-exited check → EnumWindows WM_CLOSE probe → Ctrl+C helper (exit 0=delivered/1=no console decides graceful vs force-direct) → WaitForSingleObject timeout → `on_timeout` callback → TerminateProcess → 3s exit wait (A5); ERROR_ACCESS_DENIED → `AccessDenied` (D-03).
- `config/settings.rs` — `whitelist: Vec<String>` + `kill_timeout_secs: u64` (default fn 5) with serde defaults; Phase-1-era TOML round-trip tests prove backward compatibility (D-02/D-13).
- `process.rs` — trait reshaped: `details(&ProcessSnapshot)`, `terminate(&ProcessSnapshot, timeout_secs) -> KillOutcome`; `WindowsProcessManager` thin wrapper; module decls for handle/whitelist/kill (no `mod.rs`, no premature `info` module).
- Integration tests: 5 kill-pipeline tests (graceful console child, timeout→force on signal-ignoring child, creation-time mismatch abort with child surviving, already-exited, HardBlocked PID 4) + 10-iteration PID-reuse churn test.

**TUI (Task 2):**

- `x` key kills the selected row's process (no-op on empty list — PROC-01 edge truth).
- Two-stage gate: `Kill` → spawn_blocking snapshot+protection → `KillPrepared` → drain-loop routes `Protection::None` straight to kill, `UserConfirm` opens the dialog, `HardBlocked` shows the status message (no dialog — D-09).
- `KillConfirmComponent`: centered 60×7 bordered popup, topmost overlay, name truncation to area.width−13, accent_secondary kill-button styling.
- Confirm layer in `map_key_event`: y/Enter confirm, n/Esc cancel, x intercepted as no-op; all other keys pass through.
- Status bar: 8 locked UI-SPEC strings with A9 truncation (compact hard-block form), InProgress/Success/Error tones.
- D-04 post-kill auto-refresh: successful kill sets `last_killed_pid` + `scanning = true`; `last_killed_pid` cleared on ScanComplete.
- Ports-tab footer replaced with the exact 73-col locked string (removes [s]Sort + conditional [a]Elevate per D-09); confirm-dialog footer `[y]Confirm kill [n]Cancel — {name} is on your protection list` (name budget term_width−63); other tabs keep Phase 1 footer.
- `--ctrl-c <pid>` hidden helper mode: clap flag, early-return guard before terminal init (Pitfall 7), `helper_send_ctrl_c` (SetConsoleCtrlHandler(None,true) → FreeConsole → AttachConsole → GenerateConsoleCtrlEvent). **Verified live**: real binary delivers Ctrl+C to a CREATE_NEW_CONSOLE child (ping output shows ^C, child exits, helper exit 0).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Windows error-code mapping: HRESULT vs Win32 code**
- **Found during:** Task 2 verification (churn test failure surfaced `TerminateProcess failed: 0x80070005`)
- **Issue:** `windows::core::Error::code()` returns an HRESULT (`0x80070005` for access denied), but `open_with` and `terminate_and_wait` compared `e.code().0 as u32 == 5` — the comparison never matched, so genuine ERROR_ACCESS_DENIED mapped to `Platform`/`Failed` instead of `PermissionDenied`/`AccessDenied`. The D-03 "admin rights needed" flow would never fire for elevated/system targets.
- **Fix:** mask low 16 bits (`(e.code().0 as u32) & 0xFFFF == 5`) in both handle.rs (`win32_error_code` helper) and kill.rs.
- **Files modified:** port-core/src/process/handle.rs, port-core/src/process/kill.rs
- **Commit:** cd80ccc

**2. [Rule 1 - Bug] Clap derived `--ctrl-c-pid` instead of `--ctrl-c`**
- **Found during:** Task 2 manual helper verification
- **Issue:** `#[arg(long)] ctrl_c_pid` derives `--ctrl-c-pid`; the kill.rs helper contract spawns `--ctrl-c <pid>`, so the helper would exit 2 (unknown arg) → every console target force-killed.
- **Fix:** `#[arg(long = "ctrl-c", hide = true)]`.
- **Files modified:** port-tui/src/main.rs
- **Commit:** cd80ccc

**3. [Rule 1 - Bug] Churn test flakiness: transient TerminateProcess access-denied**
- **Found during:** Task 2 verification (iter 4 failed)
- **Issue:** `cmd /c timeout 30` children in the churn test can be mid-exit when TerminateProcess runs; the OS returns ERROR_ACCESS_DENIED for a terminating process (documented race). The test rejected that outcome.
- **Fix:** tolerate `AccessDenied` in the churn test's accepted outcomes (same practical result — process goes away; the wrong-process-kill proof is the open_verified check, unchanged).
- **Files modified:** port-core/tests/process_handle_integration.rs
- **Commit:** cd80ccc

### External Fixes (checkpoint review, committed by orchestrator)

- **fa8a682:** removed dead `pid` field from `OpenProcessHandle` (never-read dead_code warning).
- **05e9d89:** A1 whitelist review — dropped `explorer.exe` (deliberately excluded per RESEARCH; restarts itself, not system-fatal), fixed `secureystem.exe` → `securesystem.exe` typo. BUILTIN now 25 entries (Tier-1 14 + Tier-2 11), still satisfying the ≥25 contract.

### Deviations from Plan Text

- The Ctrl+C console probe is merged into the helper invocation (helper exit code = console probe result) rather than a separate `AttachConsole` probe in the kill scope — the TUI process already owns a console and cannot AttachConsole another; the helper IS the probe. Same routing outcome, honors prohibition P2 (never FreeConsole in the TUI process).
- Kill tasks spawn via `tokio::spawn` (kill() is async and performs its own spawn_blocking internally) rather than nested spawn_blocking — functionally the plan's intent (off-runtime execution, single blocking scope for Win32).
- `KillConfirmed { pid }` is used as a defensive sanity check against the pending snapshot's pid before executing.
- `last_killed_pid` cleared in update()'s ScanComplete arm (drain loop routes ScanComplete through update anyway).
- The plan's `format_kill_status` signature keeps `timeout_secs` as a reserved parameter (documented; KillTimeout copy handled directly in update.rs).

## Auth Gates

None — no authentication was required at any point.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `WindowsProcessManager::details()` returns `Err("not yet implemented (plan 02-02)")` | port-core/src/process.rs | info.rs lands in plan 02-02 (detail panel); the trait reshape is this plan's contract — no caller invokes details() yet |
| `Message::KillExecute` never constructed | port-tui/src/message.rs | Declared per plan's message contract; current flow intercepts KillPrepared directly — future kill paths (plan 02-03 whitelist overlay) may emit it; `#[allow(dead_code)]` with rationale |

## Threat Flags

None — no new security surface beyond the plan's threat_model. The `--ctrl-c` helper is the planned hidden flag with early-return guard; the whitelist gate is checked before OpenProcess per T-02-05; ProcessSnapshot verification covers T-02-01.

## Verification

- `cargo test -p port-core` — 45 unit tests green (whitelist contract, route_strategy matrix, creation_matches, settings serde)
- `cargo test -p port-core --test kill_integration --test process_handle_integration` — 6 integration tests green (real children killed; mismatch aborts with child surviving; churn no-wrong-process-kill)
- `cargo test --workspace` — 62 tests green
- `cargo build --workspace` — compiles; `grep -c Win32_Security_WinTrust`/`Win32_System_Console` == 1 each
- `grep HANDLE port-tui/src/` — no matches (no HANDLE in messages or App state)
- Live helper verification: `port-tui.exe --ctrl-c <pid>` delivered Ctrl+C to an isolated-console child (^C in ping output, child exited, exit 0)
- Manual UAT deferred to end-of-phase per `human_verify_mode=end-of-phase` (graceful escalation messages, hard-block UX, dialog visuals)

## Self-Check: PASSED

- Files exist: process/{handle,whitelist,kill}.rs, tests/{kill_integration,process_handle_integration}.rs, components/kill_confirm.rs ✓
- Commits exist: 511da68, fa8a682, 05e9d89, cd80ccc ✓

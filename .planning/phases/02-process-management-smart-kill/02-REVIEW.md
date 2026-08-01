---
phase: 02-process-management-smart-kill
reviewed: 2026-08-01T00:00:00Z
depth: standard
files_reviewed: 24
files_reviewed_list:
  - Cargo.toml
  - port-core/src/config/settings.rs
  - port-core/src/filter.rs
  - port-core/src/models/process.rs
  - port-core/src/process.rs
  - port-core/src/process/handle.rs
  - port-core/src/process/info.rs
  - port-core/src/process/kill.rs
  - port-core/src/process/whitelist.rs
  - port-core/src/scanner.rs
  - port-core/src/scanner/tcp.rs
  - port-core/src/scanner/udp.rs
  - port-core/tests/kill_integration.rs
  - port-core/tests/process_handle_integration.rs
  - port-tui/src/app.rs
  - port-tui/src/components.rs
  - port-tui/src/components/detail_panel.rs
  - port-tui/src/components/help.rs
  - port-tui/src/components/kill_confirm.rs
  - port-tui/src/components/ports.rs
  - port-tui/src/components/whitelist_overlay.rs
  - port-tui/src/main.rs
  - port-tui/src/message.rs
  - port-tui/src/update.rs
findings:
  critical: 1
  warning: 5
  info: 5
  total: 11
status: issues_found
---

# Phase 2: Code Review Report

**Reviewed:** 2026-08-01
**Depth:** standard
**Files Reviewed:** 24
**Status:** issues_found

## Summary

Reviewed the Phase 2 process-management implementation: smart-kill escalation pipeline (WM_CLOSE → Ctrl+C → force), PID-reuse identity verification, two-tier whitelist protection, process detail fetchers, and the TUI kill/whitelist/detail/help overlays. The core identity-verification design (creation-time FILETIME check in `open_verified`) is sound and well-tested; the kill pipeline ordering (protection gate before OpenProcess, fresh settings re-read) follows the plan's pitfall list correctly. However, one critical event-loop bug bricks the TUI after any scan error, the graceful-signal paths (WM_CLOSE, Ctrl+C helper) are keyed by raw PID rather than the verified handle (the exact PID-reuse class this phase set out to prevent), and there is an unsafe 2x buffer-size mismatch in `QueryFullProcessImageNameW` usage. Several smaller race conditions and dead-code/consistency defects are listed below.

## Critical Issues

### CR-01: TUI permanently stuck in "Scanning..." after any scan error — refresh never recovers

**File:** `port-tui/src/main.rs:548-659`
**Issue:** The `scan_spawned` guard is only reset to `false` in the `Message::ScanComplete` drain arm (line 549). `Message::ScanError` falls through to `other => update(app, other)` (line 650), which sets `app.scanning = false` but leaves `scan_spawned == true`. The spawn guard at line 656 — `if app.scanning && !*scan_spawned { spawn_scan(...) }` — then never fires again, and the auto-refresh block (line 662) requires `!app.scanning`, so it is also blocked once `scanning` is re-set. Trace: initial scan fails (or a non-admin user presses `a` and elevation fails — line 260 sends `ScanError`), error is shown, user presses `r` → `Refresh` sets `scanning = true` → guard sees `scan_spawned == true` → no scan is ever spawned → the app shows "Scanning... 0 found so far" forever; every subsequent `r` is a no-op. Only a restart recovers. This is reachable through normal use (transient IP Helper failure, UAC denial).
**Fix:** Reset the flag wherever a scan lifecycle ends, e.g. add an explicit drain arm:
```rust
Message::ScanError(e) => {
    *scan_spawned = false;
    update(app, Message::ScanError(e));
}
```

## Warnings

### WR-01: QueryFullProcessImageNameW declared buffer size is 2x the actual allocation (unsafe)

**File:** `port-core/src/process/handle.rs:200-203`
**Issue:** `QueryFullProcessImageNameW`'s `lpdwSize` is in characters (WCHARs) for the W variant. The code allocates `vec![0u16; capacity / 2]` (16,384 WCHARs for a 32 KiB capacity) but declares `size = capacity` (32,768 chars). The API believes the buffer is twice as large and will happily write up to 32,768 WCHARs into a 16,384-WCHAR heap buffer — a heap overflow if the path exceeds ~16K characters (possible with extended-length paths; Windows allows 32,767-char paths). Practically unreachable for real executable paths, but this is a genuine memory-corruption defect in unsafe code — the declared capacity must match the allocation.
**Fix:**
```rust
let mut buf: Vec<u16> = vec![0u16; capacity / 2];
let mut size: u32 = buf.len() as u32; // characters, not bytes
```

### WR-02: Kill TOCTOU — WM_CLOSE / Ctrl+C targeted by raw PID, not the verified handle

**File:** `port-core/src/process/kill.rs:169-180`
**Issue:** `open_verified()` establishes identity on the HANDLE (creation-time verified), but the graceful-signal probe `has_visible_window_for_pid(pid)` / `post_wm_close_to_pid(pid)` and `spawn_ctrl_c_helper(pid)` then re-key by raw PID. If the target exits and the PID is recycled between `open_verified` and the probe, WM_CLOSE is posted to an innocent process's window (could close an app with unsaved data) and the Ctrl+C helper broadcasts into the wrong console. This is exactly the wrong-process class PROC-07/Pitfall #1 exists to prevent — the verification is applied to the handle while the side effects are keyed to the PID. The race window is microseconds, but the consequence is real, and the phase explicitly targets PID-reuse safety.
**Fix:** Before posting, re-verify that the probed window's PID still maps to the verified process — e.g. check `GetProcessId(handle.handle) == window_pid` inside the enum callback (pass the handle in the LPARAM context), and/or re-check creation time immediately before dispatch; on mismatch, fall through to the force path instead of signaling an unknown process.

### WR-03: Ctrl+C helper re-executes `current_exe` — breaks port-core's frontend-agnostic contract

**File:** `port-core/src/process/kill.rs:363-378`
**Issue:** `spawn_ctrl_c_helper` does `Command::new(current_exe).arg("--ctrl-c")`. The `--ctrl-c` helper mode exists only in `port-tui/src/main.rs:65-68`. When the kill pipeline is used from any other binary (the Tauri GUI, or tests), the spawned "helper" is the full application with the flag ignored: `status()` then blocks until that app exits — for the GUI that means the graceful path hangs for the entire timeout and force-kills every console process. This couples port-core (declared frontend-agnostic per CLAUDE.md) to a specific binary implementing an undocumented flag.
**Fix:** Move the helper into port-core itself (a small `fn deliver_ctrl_c(pid) -> bool` behind a cfg(windows) module) and have `kill_blocking` call it via a re-exec of `current_exe` only as a fallback, or gate the graceful-Ctrl+C path on a trait/capability the binary declares.

### WR-04: Concurrent whitelist add/delete can clobber settings via stale working copies

**File:** `port-tui/src/main.rs:385-489`
**Issue:** Both `WhitelistAdd` and `WhitelistDeleteSelected` closures clone `app.whitelist_settings` at intercept time, then `save_settings` unconditionally writes the whole file. Two saves in flight (delete spawns save task 1, add spawns save task 2 from a clone taken after the delete mutated the working copy) can land out of order — if task 1's write lands last, the newly added entry is lost from disk while the UI shows it (or vice versa). The window is small (file writes are fast) but the file is not locked and there is no ordering guarantee on the blocking pool. Same class: two rapid adds with different paths both see "not a duplicate" and last-write-wins loses one entry.
**Fix:** Serialize settings writes — re-read `load_settings()` inside the blocking closure immediately before save and merge onto that fresh copy, or route all saves through a single async task with an ordered queue.

### WR-05: Helper reports "delivered" even when GenerateConsoleCtrlEvent fails

**File:** `port-tui/src/main.rs:183-189`
**Issue:** `GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` result is discarded (`let _ = ...`), then the helper unconditionally returns 0 ("delivered"). `kill_blocking` (kill.rs:179) treats exit 0 as "graceful signal dispatched" and waits the full timeout, when in fact nothing was delivered. Additionally, the broadcast reaches every process on the target's console — including the TUI itself if the target is the TUI's own process or shares its console, terminating the TUI via the default Ctrl+C handler. The former is a truthful-status defect; the latter is a documented caveat but deserves a guard.
**Fix:** Return the event result: `if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_ok() { 0 } else { 1 }`. Optionally, refuse to signal a console containing the caller's own process group.

### WR-06: TCP6 remote addresses never displayed — `format_ipv6` built but unused

**File:** `port-core/src/scanner/tcp.rs:173-248, 333`
**Issue:** `connection_from_tcp6_row` hardcodes `remote_address: None` while `connection_from_tcp6_row`'s IPv4 counterpart populates it. The RFC 5952 `format_ipv6` helper (marked `#[allow(dead_code)]`) is never called, so TCP6 rows show "—" for the remote address while TCP4 rows show it — an inconsistent remote-column across the table. Either wire the IPv6 remote address through `format_ipv6` (see IN-02 for the bug inside it) or drop the dead code deliberately and document the gap.
**Fix:** Populate `remote_address: Some(format_ipv6(&row.ucRemoteAddr))` when `dwRemotePort != 0`, reusing the IPv4 row's None-when-zero logic.

## Info

### IN-01: `route_strategy` is dead in production

**File:** `port-core/src/process/kill.rs:82-88`
**Issue:** `kill_blocking` inlines the strategy branch (`if has_visible_windows ... else spawn_ctrl_c_helper(pid) == 0`) instead of calling `route_strategy`, so the pure function exists only for tests. Keep it as the single decision point — call it from `kill_blocking` — so the documented pipeline (PROC-02) and the code cannot drift.

### IN-02: `format_ipv6` leading-compression bug (in dead code)

**File:** `port-core/src/scanner/tcp.rs:240-242`
**Issue:** When the longest zero run starts at group 0 (e.g. `::1`), `result` is already `"::1"`, but `if best_start == 0 { result = format!(":{}", result); }` prepends another colon → `":::1"` (invalid RFC 5952). The `best_start == 0 && best_len == 8` case is handled by the early return, so this branch is always wrong. If WR-06 wires the function in, fix the branch to `if best_start == 0 && best_len < 8 { /* already correct */ }` — i.e., delete the prepend.

### IN-03: `merge_scan_results` collapses same-port connections to one arbitrary row

**File:** `port-tui/src/update.rs:506-535`
**Issue:** The merge key is `(port.number, protocol)` — a server with many established connections on one local port collapses to a single row after the first refresh (last row in the new list wins via HashMap insert). The table shows N rows on the first scan and 1 row for the same port afterwards, and which connection survives is arbitrary. Pre-existing (present at the base commit) but worth flagging while the phase touches this code: merge key should include the connection identity (pid/remote) or the table should dedupe at scan time.

### IN-04: Doc/code drift — "kill re-captures snapshot fresh" vs. pending snapshot reuse

**File:** `port-core/src/process.rs:58-62`, `port-tui/src/main.rs:338`
**Issue:** The `details()` doc claims the kill "re-captures it fresh via snapshot_for(pid) at kill time," but the TUI executes kills with the keypress-time snapshot stored in `pending_kill_snapshot`. This is actually safe — `open_verified` creation-time checks the stale identity — but only when `creation_time` is `Some`; the None case (GetProcessTimes failure) silently degrades to PID-only verification. Align the docs with the implementation, and consider re-capturing `snapshot_for(pid)` at KillConfirmed when the pending snapshot's `creation_time` is None.

### IN-05: `kill_timeout_secs * 1000` can truncate

**File:** `port-core/src/process/kill.rs:191-193`
**Issue:** `timeout_secs * 1000` cast to `u32` for `WaitForSingleObject`: a hand-edited `settings.toml` with a timeout > ~49 days wraps, producing an immediate wait timeout → instant force-kill. Settings-controlled and absurd in practice; clamp with `.min(WAIT_INFINITE.saturating_sub(1) as u64)` or saturate at `u32::MAX`.

### IN-06: `KillExecute` message variant is dead by declaration

**File:** `port-tui/src/message.rs:201-207`
**Issue:** Self-acknowledged (`#[allow(dead_code)]`) — no producer exists. Fine as a contract placeholder, but if the plan's message contract no longer needs it, remove it rather than shipping dead variants with allowed warnings.

---

_Reviewed: 2026-08-01_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

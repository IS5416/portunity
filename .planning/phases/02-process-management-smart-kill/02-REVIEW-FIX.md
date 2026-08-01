---
phase: 02-process-management-smart-kill
fixed_at: 2026-08-01T12:18:48Z
review_path: .planning/phases/02-process-management-smart-kill/02-REVIEW.md
iteration: 1
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 2: Code Review Fix Report

**Fixed at:** 2026-08-01T12:18:48Z
**Source review:** `.planning/phases/02-process-management-smart-kill/02-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 7 (1 Critical, 6 Warning)
- Fixed: 7
- Skipped: 0

**Test results (`cargo test --workspace`, after all fixes):**

| Suite | Result |
|-------|--------|
| port-core lib (incl. new `format_ipv6` / TCP6 tests) | 83 passed, 0 failed |
| port-core `ctrl_c_helper` bin | builds clean, 0 tests |
| `tests/kill_integration.rs` (incl. graceful console kill) | 5 passed, 0 failed |
| `tests/process_handle_integration.rs` (PID-churn no wrong-process kill) | 1 passed, 0 failed |
| port-tui | 14 passed, 0 failed |
| port-gui | 0 tests |
| Doc-tests | 0 passed |

No warnings introduced (edition-2024 `unsafe_op_in_unsafe_fn` warnings from the first WR-02 pass were fixed in a follow-up commit). The graceful-console integration test now completes via the port-core `ctrl_c_helper` binary (sibling lookup in `target/debug`), finishing in ~2s instead of relying on the 5s timeout-then-force fallback.

## Fixed Issues

### CR-01: TUI permanently stuck in "Scanning..." after any scan error

**Files modified:** `port-tui/src/main.rs`
**Commit:** `df07353`
**Applied fix:** Added an explicit `Message::ScanError(e)` drain arm in `run_event_loop` that resets `*scan_spawned = false` before forwarding to `update()` (which clears `scanning` and surfaces the error). The spawn guard at the loop bottom can now fire again on the next Refresh; previously ScanError fell through to `other =>`, leaving `scan_spawned == true` and every subsequent refresh a no-op until restart. Reachable via transient IP Helper failure or UAC denial (`a` key).

### WR-01: QueryFullProcessImageNameW declared buffer size 2x the allocation

**File:** `port-core/src/process/handle.rs`
**Commit:** `1a0538b`
**Applied fix:** `lpdwSize` is in WCHARs for the W variant; the declared size now comes from the actual allocation (`let mut size: u32 = buf.len() as u32;`) instead of the byte capacity. The API can no longer believe a 16,384-WCHAR buffer is 32,768 chars — the heap-overflow window for extended-length paths is closed. `buf.truncate(size)` after success is unchanged (size is characters either way).

### WR-02: Kill TOCTOU — WM_CLOSE / Ctrl+C targeted by raw PID, not the verified handle

**File:** `port-core/src/process/kill.rs`
**Commits:** `381921c` (+ `4e6c422` edition-2024 unsafe-block follow-up)
**Applied fix:** The window probes (`has_visible_window`, `post_wm_close`) are now keyed on the VERIFIED handle instead of the raw PID. The `EnumWindows` callbacks compare each window's PID against `GetProcessId(handle)` inside the LPARAM context: the open handle keeps the process object alive, so the numeric PID cannot be recycled, and any matching window is guaranteed to belong to the verified process. WM_CLOSE can never be posted to a PID-reused impostor; on mismatch the window is simply not matched and the pipeline falls through (no graceful channel → force path on the verified handle). Note: the numeric PID also cannot be recycled while the handle is open, so the Ctrl+C helper's `AttachConsole(pid)` targets the verified process or fails cleanly.

### WR-03: Ctrl+C helper re-executes `current_exe` — breaks frontend-agnostic contract

**Files:** `port-core/src/process/kill.rs`, `port-core/src/bin/ctrl_c_helper.rs` (new)
**Commit:** `866c6d5`
**Applied fix:** The helper moved INTO port-core as its own binary (`ctrl_c_helper`, auto-discovered from `src/bin/`). `spawn_ctrl_c_helper` now prefers the helper binary as a sibling of the calling exe (candidates: same dir, then parent dir — the latter covers integration tests running from `target/debug/deps/`), falling back to the `current_exe --ctrl-c` re-exec only for single-crate dev builds (`cargo run -p port-tui`) where the helper is not built. Any binary linking port-core (TUI, GUI, tests) now gets a correct helper with a documented exit-code contract (0 = delivered, 1 = no console, 2 = delivery failed). The helper logic (incl. the WR-05 truthful-status fix) is duplicated in the TUI's `--ctrl-c` mode, which remains as the fallback path.

### WR-04: Concurrent whitelist add/delete can clobber settings via stale working copies

**File:** `port-tui/src/main.rs`
**Commit:** `bb82c66`
**Applied fix:** Both `WhitelistAdd` and `WhitelistDeleteSelected` blocking closures now re-read `load_settings()` FRESH immediately before saving and merge their single logical mutation onto that copy (add: dedupe-check then push; delete: remove by path, case-insensitive, instead of by index). Out-of-order saves can no longer lose entries — every writer starts from the latest on-disk state, so the last write always includes all prior mutations. UI working copy remains authoritative for rendering (delete still mutates it first; adds land via `WhitelistSaved`).

### WR-05: Helper reports "delivered" even when GenerateConsoleCtrlEvent fails

**File:** `port-tui/src/main.rs` (+ the port-core helper created in WR-03)
**Commit:** `cedef5f`
**Applied fix:** The TUI fallback `helper_send_ctrl_c` now returns `0` only when `GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)` succeeds, `2` on event failure, `1` on attach failure — the pipeline force-kills instead of waiting the full graceful timeout for a signal never sent. The port-core `ctrl_c_helper` binary implements the same truthful status. (The "broadcast hits the caller's own console" caveat is documented in both helpers; the kill pipeline's `WaitForSingleObject` timeout remains the arbiter.)

### WR-06: TCP6 remote addresses never displayed

**File:** `port-core/src/scanner/tcp.rs`
**Commits:** `e4cdb4e` (+ `6b9aad7` test byte-order fix)
**Applied fix:** `connection_from_tcp6_row` now populates `remote_address` — `Some(format_ipv6(&row.ucRemoteAddr))` when `dwRemotePort != 0`, `None` for listen rows — mirroring the IPv4 row. `format_ipv6` lost its `#[allow(dead_code)]`. Wiring it in required fixing the IN-02 bug inside it: the `best_start == 0` branch that prepended another colon (producing invalid `:::…` output) was deleted — a run starting at group 0 already yields the leading `::` from the compression push. Added 8 unit tests: 6 `format_ipv6` cases (loopback `::1` regression, all-zeros, leading/trailing/middle runs, no compression, IPv4-mapped) and 2 `connection_from_tcp6_row` cases (remote address populated when port set, None when port zero). All pass.

## Skipped Issues

None — all in-scope findings fixed.

---

_Fixed: 2026-08-01T12:18:48Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

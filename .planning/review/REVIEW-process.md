# Read-Only Review: port-core Process-Management Layer

**Scope:** `process.rs`, `process/{handle,kill,info,whitelist}.rs`, `bin/ctrl_c_helper.rs`, `tests/{kill_integration,process_handle_integration}.rs`, `config/settings.rs` (whitelist/kill surface), plus the TUI surfaces the layer drives (`port-tui/src/main.rs`, `update.rs`, `message.rs`) where needed to verify claimed fixes.
**Mode:** read-only — no files modified.

## Summary

- The core safety design is sound and the **seven claimed fixes (CR-01 + WR-01..WR-06) are all actually present** in the code and correctly implemented. WR-01 (buffer-size chars-vs-bytes), WR-02 (handle-anchored window probes) and WR-05 (truthful Ctrl+C status) are the load-bearing ones and are done right.
- **No CRITICAL / memory-safety issue remains** in the in-scope port-core layer: the `QueryFullProcessImageNameW` heap-overflow (WR-01) is closed, all HANDLEs are RAII-closed, and the ntdll command-line reader is defensively bounds-checked against its own allocation.
- The main residual is a **WARNING: PID-only degradation when the snapshot's `creation_time` is `None`**. `open_verified` (handle.rs:158-177) silently skips the creation-time check in that case, reducing PROC-07 protection to a bare PID check against a keypress-time snapshot (IN-04's residual). Rarely reachable, but it is the exact wrong-process class this phase exists to prevent.
- One consistency defect the context notes understate: `kill_blocking` inlines the strategy branch and never calls the pure `route_strategy` (IN-01 confirmed still open), and the two graceful branches (`WmClose`, `ConsoleCtrlC`) share no enumeration/problem-avoidance structure, producing two real costs detailed below.
- All five prior INFO items (IN-01, IN-04, IN-05, IN-06, plus IN-02/IN-03 which landed with WR-06) are either confirmed still present or resolved as documented; only the doc-fix claim in IN-04 and the two dead-code items need action.

## Findings table

| Severity | File:Line | Issue | Suggested fix | Why-matter-now |
|----------|-----------|-------|---------------|----------------|
| WARNING | `port-core/src/process/handle.rs:158-177` | When `snapshot.creation_time == None`, `open_verified` skips the FILETIME check and only does the trivial PID check (`GetProcessId` on a handle we just opened by that very PID always matches, handle.rs:149-155). The PID "belt" is tautological — only creation time is meaningful. `creation_time` goes `None` when `GetProcessTimes` fails at snapshot capture (handle.rs:125-129) or for pre-Phase-2 rows, so PROC-07 degrades to PID-only in those cases. | At KillConfirmed, re-capture `snapshot_for(pid)` (or re-run `GetProcessTimes` on the held handle) when the keypress snapshot's `creation_time` is `None`; alternatively fail the kill with `NotFound` rather than proceed PID-only. | Closes the silent worst-case of the PID-reuse guarantee the whole layer is designed around. Reachable via a transient `GetProcessTimes` failure between the row render and the kill. |
| WARNING | `port-core/src/process/kill.rs:196-198` | `timeout_ms = timeout_secs * 1000` then `as u32` for `WaitForSingleObject`. `timeout_secs` is `u64` from `settings.toml`; a hand-edited value above ~4.29e9 wraps to a small `u32`, making the wait return `WAIT_TIMEOUT` immediately → silent instant force-kill instead of a graceful window. (Prior IN-05.) | Clamp/saturate: `let ms = timeout_secs.saturating_mul(1000).min(u32::MAX as u64) as u32;` | Missed in the FIX pass; cheap correctness hardening on attacker/user-editable input in a layer whose whole premise is "don't force-kill before the graceful timeout." |
| INFO | `port-core/src/process/kill.rs:174-185` | `route_strategy` (kill.rs:82-88) is still **not** called by `kill_blocking`; the branch is inlined (`if has_visible_windows … else spawn_ctrl_c_helper(pid) == 0`). The pure function exists only for unit tests. (Prior IN-01, confirmed still open.) | Call `route_strategy(has_visible_windows, has_console)` once and drive dispatch off the returned `Strategy`, consuming the probed `has_console` result. | Keeps the documented pipeline (PROC-02) and the code from drifting; the inlined version already behaves identically, so this is cheap and zero-risk. |
| INFO | `port-core/src/process/kill.rs:174, 289-360` | **Two full `EnumWindows` passes per GUI kill** (`has_visible_window` then `post_wm_close`), each walking every top-level window on the desktop. For a console-only process the enumeration is walked once and discarded before the Ctrl+C helper is even tried. | Merge probe+post into a single enumeration that records the first matching visible window and whether any was found; the second pass only re-posts if the first found one (it virtually always does). | Per-kill latency on large desktops; trivial to combine and clearly in scope of the pending strategy-refactor from the item above. |
| INFO | `port-tui/src/main.rs:278-336` + `port-core/src/process/kill.rs:116-138` | Protection + settings are computed **twice per kill**: once at keypress in the TUI (`load_settings` + `protection_status`, main.rs:307-327) to decide the confirm dialog, then again inside `kill_blocking` (kill.rs:116-138) to make the authoritative gate. Both re-read `settings.toml` fresh. | Keep the authoritative gate in `kill_blocking`; have the TUI's keypress path skip the file re-read and reuse the last settings, or expose a single `(prot, settings)` helper. | Redundant file read + normalize per kill; the two reads can even disagrees in a race (the dialog shows `UserConfirm` while `kill_blocking` hard-blocks). |
| INFO | `port-tui/src/main.rs:612-614, 636-646` + `process/info.rs:331` | The `signature_cache` is per-PID and **cleared on every scan** (every ~5 s AUTO_REFRESH). The currently-open detail panel therefore re-runs `WinVerifyTrustEx` (10–100 ms, A2) on **every scan** even though the file is unchanged. | Key the cache by `(path, file-modified-time)` and only evict on path/pid change instead of blanket-clear per scan; or skip re-verify while the same process is displayed. | WinVerifyTrust is one of the most expensive per-view operations; the current lifecycle rebuilds it constantly for a single static file. |
| INFO | `port-core/src/process/whitelist.rs:257-271` vs `130-218` | Two distinct normalization code paths: `normalize_path` (used by `user_match`) strips single **and** double quotes and trailing slashes; `normalize_inner` (used for stored entries) strips only double quotes + resolves 8.3 + rejects non-absolute. Minor semantic skew (e.g. a single-quoted stored value normalizes differently between the two). | Reuse one normalization core for both compare-time and store-time, or document the divergence. | Low impact (stored entries are already long-form absolute), but the duplication invites future drift exactly where path-matching correctness lives. |
| INFO | `port-tui/src/main.rs:172-199` + `port-core/src/bin/ctrl_c_helper.rs` | The Ctrl+C delivery logic (AttachConsole/FreeConsole/GenerateConsoleCtrlEvent/SetConsoleCtrlHandler) is **duplicated** in the TUI's `--ctrl-c` fallback and in the port-core helper binary (WR-03 retained both by design). Two copies can drift (e.g. a future exit-code change). | Since port-core now ships the helper binary as primary, consider deleting the TUI `--ctrl-c` mode entirely (and the `helper_send_ctrl_c` + CLI arg) or have the TUI re-exec the helper too. | Goal-4 structure item; the duplication is currently correct and both implement WR-05 truthfully, but only one should be the source of truth. |
| INFO | `port-tui/src/message.rs:206-207` | `Message::KillExecute` is still dead-by-declaration (`#[allow(dead_code)]`, no producer). (Prior IN-06, confirmed.) | Remove it or wire it as the actual kill trigger (it would be the natural home for the re-verify-at-confirm from the first WARNING). | Dead contract variant shipped with an allowed warning. |
| INFO | `port-core/src/process/info.rs:247-283` | `query_command_line` defensive bounds are correct today, but `offset + claimed_chars` (line 277) could in principle overflow `usize` with a maximally-corrupt returned `Buffer` pointer (offset near `usize::MAX` from a pointer just before the allocation). Unreachable with OS-controlled class-60 output, so purely a hardening nit. | Use `checked_add` / `offset.checked_add(claimed_chars).is_some_and(|e| e <= buffer.len())`. | Negligible now; cheap insurance in unsafe FFI code. |

## Verified-fixes checklist

All **seven** claimed fixes are present in the code. Details with citations:

| Fix | Status | Evidence |
|-----|--------|----------|
| **CR-01** (TUI stuck "Scanning…" after scan error) | ✅ Fixed | `port-tui/main.rs:655-662` — explicit `Message::ScanError(e)` arm resets `*scan_spawned = false` before forwarding to `update()`. Commit `df07353`. |
| **WR-01** (QueryFullProcessImageNameW size chars vs bytes) | ✅ Fixed | `handle.rs:205-206` — `let mut size: u32 = buf.len() as u32;` from the actual `buf.len()` (char count), not byte capacity. Commit `1a0538b`. Retry/truncate consistent. |
| **WR-02** (WM_CLOSE/Ctrl+C keyed on raw PID) | ✅ Fixed | `kill.rs:174,254-360` — probes and post now hold the **verified handle** in the LPARAM context and compare `GetWindowThreadProcessId` against `GetProcessId(ctx.handle)` inside the callback; handle keeps the process object alive so the PID cannot be recycled. Commits `381921c` + `4e6c422`. |
| **WR-03** (Ctrl+C helper re-executes `current_exe`) | ✅ Fixed | `bin/ctrl_c_helper.rs` (new port-core binary); `kill.rs:389-418` prefers the sibling helper exe (same dir, then parent for `target/debug/deps`), re-exec fallback only as last resort. Commit `866c6d5`. |
| **WR-04** (concurrent whitelist add/delete clobber) | ✅ Fixed | `main.rs:406-422` (add) and `503-528` (delete) both re-read `load_settings()` FRESH inside the blocking closure and merge a single logical mutation (add: dedupe+push; delete: remove by path case-insensitively) onto that copy. Commit `bb82c66`. |
| **WR-05** (helper reports "delivered" on event failure) | ✅ Fixed | `bin/ctrl_c_helper.rs:56-64` and `main.rs:186-196` — `GenerateConsoleCtrlEvent(...).is_ok()` gates exit `0`; returns `2` on failure. Commit `cedef5f`. |
| **WR-06** (TCP6 remote addresses never displayed) | ✅ Fixed | `scanner/tcp.rs:318` — `Some(format_ipv6(&row.ucRemoteAddr))` when `dwRemotePort != 0`; IN-02's `:::` bug removed. Commit `e4cdb4e` (+ `6b9aad7`). *(Outside the port-core process files, verified for completeness.)* |

No claimed fix is missing or incomplete.

## Optimization shortlist (only as part of upcoming work)

These are not worth a standalone commit — fold them into the inevitable strategy-refactor / detail-panel work:

1. **Combine the two `EnumWindows` passes** into one (see Findings) — per-GUI-kill latency on large desktops.
2. **Stop rebuilding the signature verdict every scan** — key `signature_cache` by `(path, mtime)` instead of clearing per scan (main.rs:612-614); WinVerifyTrust is the single most expensive per-view op in the layer.
3. **Drop the redundant keypress settings-read/protection computation** — let `kill_blocking`'s authoritative gate be the only gate, or add a shared helper (main.rs:307-327 vs kill.rs:116-138).
4. **Wire `route_strategy`** as the single decision point (IN-01) and, if touching the pipeline, move the console probe (`spawn_ctrl_c_helper` exit code) out from behind the always-run window enumeration so console-first processes skip the desktop walk.
5. If the TUI fallback re-exec is kept, **collapse the duplicated Ctrl+C logic** to one copy (see Findings).

## Suggested follow-up commits

1. `fix(02): fail or re-verify kill when snapshot creation_time is None` — closes the PID-only degradation (handles the IN-04 residual properly; consider re-capturing via `snapshot_for` at KillConfirmed).
2. `fix(02): saturate kill timeout before u32 wait` — handles the WR-of-IN-05 overflow (kill.rs:196).
3. `refactor(02): drive kill dispatch from route_strategy and merge WM_CLOSE probe+post` — eliminates dead-code drift and the double enumeration.
4. `perf(02): key signature cache by path+mtime` — stops re-verifying a static file every scan.
5. `chore(02): remove dead Message::KillExecute or adopt it as the kill trigger`.

## Notes on context claims

- The context's "all tests pass (lib 83, kill integration 5, churn 1)" matches `02-REVIEW-FIX.md` and is consistent with the verified fixes; not re-run (read-only session).
- The prior-review summary that "02-REVIEW-FIX.md claims all were fixed" is **accurate** — all seven are verifiably in the tree.
- The context's IN-04 description ("doc/impl drift about snapshot freshness") is **still half-true**: the doc's claim that the kill "re-captures it fresh via `snapshot_for(pid)` at kill time" exactly matches the current TUI impl, which re-captures at keypress (`Kill`, main.rs:287) — not at KillConfirmed. The genuine residual is the `creation_time: None` case (WARNING above), not the doc itself.
- IN-01, IN-05, IN-06 are confirmed **still present** (not fixed); IN-02 was fixed as part of WR-06; IN-03 (merge collapsing) is in the TUI update layer, out of this scope.

_Read-only review — no workspace files modified._

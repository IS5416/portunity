---
phase: 02-process-management-smart-kill
verified: 2026-08-01T13:05:00Z
status: human_needed
score: 22/29 must-haves verified
behavior_unverified: 7
overrides_applied: 0
behavior_unverified_items:
  - truth: "User-whitelisted processes gate the kill behind the 60x7 confirmation dialog; y/Enter confirms, n/Esc cancels; x is intercepted while the dialog is open (02-01 T8)"
    test: "Run `cargo run -p port-tui`, add an existing exe to the user whitelist (w overlay), press x on its row — dialog appears; y/Enter kills, n/Esc cancels, x while open is a no-op"
    expected: "Centered 60x7 popup with the protection reason; y/Enter confirm; n/Esc cancel; x never re-triggers a kill"
    why_human: "Dialog layout, visual styling, and key-interaction feel are render-time behavior no test exercises (dispatch code is present and wired)"
  - truth: "User presses d on a selected port and a 12-row non-modal overlay shows all fields with protection badge (02-02 T1)"
    test: "Run `cargo run -p port-tui`, press d on a row — 12-row overlay: name+PID bold title, status, owning port, path, command line, start time, parent PID, signature, protection, reason, kill hint"
    expected: "All 12 rows render per UI-SPEC layout; values populate on-demand; per-field failures show '—' dim"
    why_human: "Overlay geometry and per-state rendering (loading/verifying/exited) are visual behavior; fetchers themselves are integration-tested"
  - truth: "Selection change while the detail panel is open refreshes panel content; j/k/up/down/s/g/G//f pass through (02-02 T4)"
    test: "Open detail panel (d), press j/k to move selection — panel refreshes to the new process; table still scrolls"
    expected: "Panel content follows the selection; table keys keep working under the panel (non-modal)"
    why_human: "Pass-through dispatch and refresh-on-move are interactive behavior in a live terminal session"
  - truth: "If the process exits while the panel is open, name renders strikethrough (SGR 9) and Status shows 'Exited' (02-02 T5)"
    test: "Open the detail panel on a process, kill it from another terminal (or via x), wait for the next scan — panel shows strikethrough name + 'Exited'"
    expected: "Strikethrough and 'Exited' status appear after the scan detects the process is gone"
    why_human: "Rendering of Modifier::CROSSED_OUT and the Exited state is visual; detection logic is wired and drain-arm-verified by code reading"
  - truth: "Row dimming is whitelist-driven: protected rows dim only in non-admin sessions; admin sessions render full brightness with the marker (02-02 T7)"
    test: "Run in a non-admin terminal and an admin terminal; compare ◆-marked rows"
    expected: "Non-admin: protected rows dimmed; admin: full brightness with ◆ markers"
    why_human: "Requires two live sessions with different privilege levels to observe the difference"
  - truth: "User presses w and a 20-row non-modal overlay shows read-only built-in section and editable user section; add via Path input, delete with d (02-03 T1)"
    test: "Run `cargo run -p port-tui`, press w — 20-row overlay; add an existing absolute path → 'Added' info; add a bogus path → error, not added; duplicate → no-op; d deletes a selected user entry"
    expected: "Overlay layout per UI-SPEC; add/delete flows work end-to-end with status feedback; built-in section read-only"
    why_human: "Overlay visuals, focus cycling, and end-to-end add/delete UX are interactive; validation logic itself is unit-tested (37 whitelist tests)"
  - truth: "The '?' key opens a Help overlay documenting all keys including [d] Detail, [x] Kill, [w] Whitelist, [s] Sort and the L2-confirm [y]/[n] (02-03 T8)"
    test: "Run `cargo run -p port-tui`, press ? — full key reference; press w then ? — help renders above the whitelist overlay (stack order); Esc closes"
    expected: "All keys documented; help above whitelist, below confirm dialog; Esc/?' close"
    why_human: "Overlay content and stack order are visual; dispatch precedence is code-verified"
human_verification:
  - test: "Review BUILTIN constant (Assumption A1): every entry is a real system-critical Windows process with a plain-language reason; Tier-1 matches the Microsoft Restart Manager Critical System Services list (planner-deferred, 02-01-PLAN)"
    expected: "25 entries (Tier-1 14 + Tier-2 11), all lowercase, unique, non-empty plain-language reasons; explorer.exe deliberately excluded"
    why_human: "Domain judgment on Windows system-criticality grounding — machine-checked only for count/uniqueness/lowercase/reasons"
  - test: "Run `cargo run -p port-tui` (admin or non-admin): select a non-protected row and press x — status bar shows the graceful/force outcome and the row disappears after refresh; select an svchost.exe/system row and press x — hard-block message with 'Press w to review the whitelist.' shows and no dialog appears; kill a process twice — second attempt shows 'already exited' (planner-deferred, 02-01-PLAN)"
    expected: "Outcome strings in the status bar (8 locked UI-SPEC strings ≤80 cols); hard-blocked rows show the non-technical explanation and never open a dialog; repeat kill yields the already-exited outcome"
    why_human: "Live terminal session; status-bar display and row disappearance are user-visible behavior"
  - test: "Run `cargo run -p port-tui`: select a row, press d — 12-row panel shows all fields (path, cmdline, start time, parent PID, signature after 'Verifying…', protection, reason when protected); j/k while open refreshes the panel; kill the process then wait for scan — the row shows strikethrough, the panel shows 'Exited'; protected rows show the ◆ marker (color per tier) and dim in a non-admin session (planner-deferred, 02-02-PLAN)"
    expected: "Panel renders per UI-SPEC internal layout; live refresh on selection move; exited detection with strikethrough; ◆ markers in status.error (built-in) / status.warning (user)"
    why_human: "Interactive terminal session; visual states and colors"
  - test: "Run `cargo run -p port-tui`, press w: built-in section shows ◆ entries with reasons; add an existing absolute path (e.g. the current port-tui exe) — 'Added' status.info; add a bogus path — error, not added; duplicate add — no-op info string; kill a user-listed process — confirm dialog appears; remove it in the overlay — next kill is instant; press x on a built-in row — hard-block message only (planner-deferred, 02-03-PLAN)"
    expected: "Overlay add/remove flows per UI-SPEC with the locked status strings; whitelist changes take effect without restart; built-in entries never open the confirm dialog"
    why_human: "Interactive terminal session; full add/remove/kill UX loop"
  - test: "Run `cargo run -p port-tui`, press ?: Help overlay lists all keys including d/x/w/s/y/n; Esc closes; press w then ? — help renders above the whitelist overlay (stack order); footer [?]Help works (planner-deferred, 02-03-PLAN)"
    expected: "Full key reference visible; overlay stack order: table -> search -> filter -> detail -> whitelist -> help -> confirm"
    why_human: "Interactive terminal session; stack order and content completeness"
---

# Phase 2: Process Management & Smart Kill Verification Report

**Phase Goal:** Users can inspect detailed process information for any port's owning process and terminate it with smart kill escalation (graceful shutdown, timeout, force kill) and whitelist-gated protection against accidental termination of system-critical processes.
**Verified:** 2026-08-01
**Status:** human_needed
**Re-verification:** No — initial verification

## MVP-Mode Discrepancy

ROADMAP.md marks Phase 2 as `**Mode:** mvp`, but the phase goal is not in canonical user-story form ("As a … I want to … so that …"). `gsd query user-story.validate` returned `valid=false`. The plans themselves flag this ("Consider running `/gsd mvp-phase 2`"). Verification proceeded with the standard goal-backward methodology against the 5 ROADMAP Success Criteria + PLAN must_haves; the User Flow Coverage table below is derived from the Success Criteria. Recommended: run `/gsd mvp-phase 2` to canonicalize the goal.

## User Flow Coverage

| User action | Expected (from Success Criteria) | Evidence | Status |
|-------------|----------------------------------|----------|--------|
| Select a port, press d | Detail panel: full executable path, start time, command-line args, digital signature status, parent PID | port-tui/src/components/detail_panel.rs (12-row render); port-core/src/process/info.rs (fetch_details: QueryFullProcessImageNameW, GetProcessTimes, ntdll class 60, Toolhelp32, WinVerifyTrustEx); fetch_details self-population integration test | Present + wired — visual render states need human |
| Press x on a port row | Owning process terminates with one keypress; instant for non-whitelisted | main.rs:932 x→Message::Kill; drain KillPrepared Protection::None → direct kill task; kill_integration.rs kills real children (graceful console, timeout→force, mismatch abort, already-exited — all pass) | VERIFIED (behavioral tests) |
| Press x on a whitelisted process | Confirmation dialog explaining protection | kill_confirm.rs 60x7 popup; main.rs y/Enter/n/Esc/x dispatch; update.rs KillPrepared UserConfirm → dialog state | Present + wired — interaction needs human |
| Press x on a system-critical process | Non-technical explanation, kill impossible | BUILTIN 25 entries; kill_blocking protection gate BEFORE OpenProcess (Pitfall #11); HardBlocked status string; test_hardblocked_system_process passes | VERIFIED (behavioral test) |
| Kill an unresponsive process | Graceful → configurable timeout → force kill; outcome displayed | kill.rs escalation pipeline; settings.rs kill_timeout_secs default 5; app.rs format_kill_status 8 locked strings with 80-col tests; test_timeout_then_force passes (isolation) | VERIFIED (behavioral test) |
| Press w, add/remove a path | Whitelist changes take effect immediately without restart | whitelist_overlay.rs 20-row overlay; validate_user_entry/normalize_user_entry (37 unit tests); save_settings + D-15 re-read in kill_blocking; WR-04 fresh-merge saves | VERIFIED (code + unit tests) — UX loop needs human |

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | x kills the selected row's owning process; a multi-port process is terminated once and all its ports free together after the post-kill scan (PROC-01) | ✓ VERIFIED | main.rs x dispatch; kill by PID; update.rs KillOutcome sets scanning=true (post-kill scan); kill_integration.rs kills real children |
| 2 | x with no selected row is a no-op — no kill, no dialog (PROC-01) | ✓ VERIFIED | main.rs:936 `display_data().get(selected_index)` guard → None |
| 3 | Repeated x on an already-exited process yields 'already exited'; no duplicate-kill attempt (PROC-01) | ✓ VERIFIED | test_already_exited_kill passes; AlreadyExited status string (app.rs:370) |
| 4 | Non-protected process: x kills instantly (no dialog); outcome in status bar (PROC-03) | ✓ VERIFIED | drain-loop KillPrepared Protection::None → direct kill task (main.rs:674); kill tests pass |
| 5 | Smart kill: graceful signal first (WM_CLOSE / Ctrl+C helper), waits kill_timeout_secs (default 5), force-kills via TerminateProcess; no window/console → direct (PROC-02) | ✓ VERIFIED | kill.rs pipeline (steps 1-6); settings.rs default_kill_timeout_secs; test_graceful_console_child + test_timeout_then_force pass (isolation); ctrl_c_helper.rs binary with truthful exit codes (WR-05) |
| 6 | Every kill outcome displayed in status bar with 8 locked UI-SPEC strings ≤80 cols tail-preserving; success triggers one immediate scan (PROC-02) | ✓ VERIFIED | app.rs format_kill_status + 80-col tests; update.rs KillOutcome → kill_status + scanning=true |
| 7 | Built-in whitelist (≥25 entries, Restart Manager grounded, plain-language reasons) hard-blocks: no kill path, status bar shows non-technical explanation + whitelist hint; check BEFORE OpenProcess (PROC-04) | ✓ VERIFIED | whitelist.rs BUILTIN = 25 entries (contract tests); kill.rs protection gate before open_verified; test_hardblocked_system_process passes |
| 8 | User-whitelisted processes (full-path, case-insensitive normalized) gate kill behind 60x7 dialog; y/Enter confirms, n/Esc cancels; x intercepted (PROC-03) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | kill_confirm.rs 60x7; main.rs:798-806 dispatch — wired; dialog interaction needs human (human item 2) |
| 9 | Kill identity is Send-safe ProcessSnapshot (PID + creation FILETIME + path); open/verify/act in one spawn_blocking; creation-time mismatch aborts — no wrong-process kill (PROC-07) | ✓ VERIFIED | handle.rs ProcessSnapshot/open_verified; kill.rs single spawn_blocking; test_creation_time_mismatch_aborts passes (child survives); test_churn_no_wrong_process_kill passes (10 iterations) |
| 10 | settings.toml re-read before every kill attempt (D-15) — whitelist changes take effect without restart | ✓ VERIFIED | kill.rs kill_blocking step 1 load_settings(); whitelist saves persist via save_settings |
| 11 | Access-denied kills map to 'admin rights needed — Press a to elevate'; no auto-elevation, no state-loss relaunch (D-03) | ✓ VERIFIED | app.rs AccessDenied string; main.rs PermissionDenied → AccessDenied mapping; error-code mask (0x7FF5) unit-mapped |
| 12 | d opens 12-row non-modal overlay: name+PID bold title w/ protection badge, status, port+protocol, path, command line, start time, parent PID, signature, protection tier, reason, kill hint (PROC-06) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | detail_panel.rs full layout; render states present — visual render needs human (human item 3) |
| 13 | All 9 detail fields populate on open; on-demand fetch; per-field failure renders '—' (PROC-06) | ✓ VERIFIED | info.rs Option-per-field fetchers; fetch_details self integration test; error-fallback path in main.rs |
| 14 | Digital signature on-demand async (WinVerifyTrustEx in spawn_blocking), 'Verifying…' → Signed/Unsigned/Unknown; cache per-PID, invalidated on scan (D-07) | ✓ VERIFIED | info.rs verify_signature (WTD_CACHE_ONLY_URL_RETRIEVAL + CLOSE cleanup); main.rs cache-miss spawn; cache cleared on ScanComplete; no-path → Unknown insert |
| 15 | Selection change while panel open refreshes content; j/k/up/down/s/g/G//f pass through (D-06) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | main.rs movement intercepts respawn fetch; pass-through dispatch — interactive behavior needs human (human item 3) |
| 16 | Process exits while panel open → strikethrough (SGR 9) + Status 'Exited' | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | ProcessExited detection in ScanComplete drain; detail_exited render — visual needs human (human item 3) |
| 17 | Table ◆ markers: status.error built-in / status.warning user, populated at scan time by core post-pass | ✓ VERIFIED | scanner.rs apply_protection_postpass (6 unit tests); ports.rs:201-209 marker render |
| 18 | Row dimming whitelist-driven, non-admin only; admin full brightness + marker (supersedes Phase 1 heuristic) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | ports.rs system_dim = !app.is_admin && protected; SYSTEM_NAMES heuristic deleted (grep == 0) — dual-session visual needs human (human item 3) |
| 19 | Start time derives from the same creation FILETIME identity the kill verifies against (PROC-07, D-08) | ✓ VERIFIED | info.rs filetime_to_systemtime (epoch + 2026 chrono tests); kill re-verifies via open_verified creation check |
| 20 | w opens 20-row overlay: read-only built-in section (◆ + reason) + editable user section; add via 'Path: >_' input, delete with d (D-14) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | whitelist_overlay.rs 20-row layout; input row; delete dispatch — visual/UX needs human (human item 4) |
| 21 | Whitelist changes take effect immediately: D-15 re-read before every kill + w overlay saves on add/remove | ✓ VERIFIED | kill.rs load_settings per kill; main.rs spawn_blocking validate+save (WR-04 fresh-merge) |
| 22 | Invalid/nonexistent path → status-bar error, not added (absolute syntax, quote strip, 8.3 via GetLongPathNameW, control-char reject, length cap) | ✓ VERIFIED | whitelist.rs validate_user_entry (12 unit tests incl. 8.3 round-trip); WhitelistError → error status string |
| 23 | Duplicate add is a no-op; removing non-existent entry is a no-op; settings.toml round-trips without duplicates (PROC-05) | ✓ VERIFIED | duplicate short-circuit before save; WhitelistSaved disambiguation (update.rs); serde round-trip tests |
| 24 | User whitelist entries are confirmation-level only — no hard-block option; removal instant without confirmation (D-12) | ✓ VERIFIED | Protection::UserConfirm for user tier only; delete has no confirm path |
| 25 | Whitelist mutations occur only on the main TUI event loop; kill-time re-read sees last written state (PROC-05 concurrency) | ✓ VERIFIED | intercepts on main loop; WR-04 merge-onto-fresh-read prevents clobbering |
| 26 | Built-in entries cannot be added/edited/removed; built-in kill never opens confirm dialog (D-13, D-09) | ✓ VERIFIED | read-only BUILTIN render (no mutation path); HardBlocked → status, never dialog |
| 27 | '?' opens Help overlay documenting all keys incl. d/x/w/s/y/n + secrets note (UI-SPEC) | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED | help.rs full key reference; ToggleHelp dispatch — visual/stack-order needs human (human item 5) |
| 28 | SRCH-02 (AND-across/OR-within) already implemented in filter.rs from Phase 1 — verification + traceability only, no engine change | ✓ VERIFIED | filter.rs or_within_vec_and_across_dimensions passes; REQUIREMENTS.md SRCH-02 Complete (02-03) |
| 29 | Access-denied outcome string includes the elevation hint; WR-02 handle-anchored probes; WR-06 TCP6 remote addresses populated | ✓ VERIFIED | app.rs AccessDenied string; kill.rs EnumWindows callbacks compare GetProcessId(handle); tcp.rs format_ipv6 wired with 8 tests |

**Score:** 22/29 truths verified (7 present, behavior-unverified — see Human Verification)

### Deferred Items

None — no gaps deferred to later milestone phases (Step 9b: later phases cover history/traffic/firewall/GUI, none re-cover Phase 2 kill/detail/whitelist concerns).

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| port-core/src/process/handle.rs | ProcessSnapshot, open_verified w/ creation-time check, snapshot_for | ✓ VERIFIED | exists, substantive (creation_matches, WR-01 buffer fix), wired (kill.rs, info.rs), tests |
| port-core/src/process/whitelist.rs | BUILTIN ≥25 w/ reasons, builtin_match, user_match, Protection, protection_status, validate/normalize_user_entry | ✓ VERIFIED | 25 entries; 37 unit tests; wired into kill gate + scanner post-pass + w overlay |
| port-core/src/process/kill.rs | Strategy, route_strategy, KillOutcome, kill() escalation pipeline | ✓ VERIFIED | full pipeline; WR-02/WR-03/WR-05 fixes present; 5 integration tests |
| port-core/src/process/info.rs | fetch_details, verify_signature, filetime_to_systemtime, extract_unicode_string | ✓ VERIFIED | all 9 fields; ntdll class 60 two-call; bounds-checked; integration test on current_exe |
| port-core/src/config/settings.rs | whitelist + kill_timeout_secs serde defaults (5) | ✓ VERIFIED | Phase-1-era TOML backward-compat tests |
| port-core/src/bin/ctrl_c_helper.rs | Ctrl+C helper binary (WR-03) | ✓ VERIFIED | truthful exit codes 0/1/2 (WR-05); used by kill.rs sibling lookup |
| port-core/src/scanner.rs | apply_protection_postpass wired into scan_all | ✓ VERIFIED | 6 unit tests; one spawn_blocking w/ fresh settings |
| port-tui/src/components/kill_confirm.rs | 60x7 centered bordered popup | ✓ VERIFIED | exists, substantive, rendered topmost (main.rs:1119) |
| port-tui/src/components/detail_panel.rs | 12-row overlay all states | ✓ VERIFIED | exists, substantive, rendered (main.rs:1075) |
| port-tui/src/components/whitelist_overlay.rs | 20-row overlay (built-in read-only + user list + input) | ✓ VERIFIED | exists, substantive, rendered (main.rs:1097) |
| port-tui/src/components/help.rs | full key reference overlay | ✓ VERIFIED | exists, substantive, rendered (main.rs:1104) |
| port-core/tests/kill_integration.rs | 5 Windows-gated real-child tests | ✓ VERIFIED | 3/3 pass in full run + 2 pass in isolation (flaky under parallel load — see Warnings) |
| port-core/tests/process_handle_integration.rs | 10-iteration PID churn | ✓ VERIFIED | passes (53.55s) |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | --- | --- | ------ | ------- |
| main.rs map_key_event | message.rs | x→Kill{pid}; confirm y/n/Esc/x dispatch | WIRED | main.rs:798-806, 932-946 |
| main.rs event loop | kill.rs | Kill→snapshot_for+protection→KillPrepared→KillConfirmed→kill()→KillOutcome | WIRED | main.rs:278-384, 664-720 |
| kill.rs | handle.rs | open_verified() GetProcessTimes creation-time compare before terminate | WIRED | kill.rs:145; mismatch abort test passes |
| kill.rs | whitelist.rs | protection_status BEFORE OpenProcess (Pitfall #11); settings re-read (D-15) | WIRED | kill.rs:116-142 |
| update.rs | app.rs | KillOutcome sets kill_status + scanning=true (post-kill refresh); last_killed_pid | WIRED | update.rs:441-474 |
| main.rs | info.rs | d→ToggleDetailPanel→spawn_blocking fetch_details→DetailDataLoaded; verify_signature→SignatureVerified (D-07 cache) | WIRED | main.rs:553-592, 630-653 |
| scanner.rs | whitelist.rs | post-pass builtin_match + user_match w/ fresh settings | WIRED | scanner.rs:76-83 |
| main.rs | settings.rs | w add/delete→validate+save_settings→WhitelistSaved→next kill D-15 re-read | WIRED | main.rs:393-545; WR-04 fresh-merge |
| main.rs | help.rs | '?'→ToggleHelp→HelpComponent render | WIRED | main.rs:898-901; components.rs stack |
| kill.rs | ctrl_c_helper | helper binary sibling lookup + --ctrl-c <pid> | WIRED | kill.rs:389-435; helper built as port-core bin |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| detail_panel.rs | detail_data | info::fetch_details (real Win32 calls: QLI handle, GetProcessTimes, ntdll class 60, Toolhelp32) | Yes | ✓ FLOWING |
| detail_panel.rs | signature_cache | verify_signature (WinVerifyTrustEx on real path) | Yes | ✓ FLOWING |
| ports.rs markers | is_system_critical / user_protected | scan post-pass (real builtin_match + QueryFullProcessImageNameW) | Yes | ✓ FLOWING |
| whitelist_overlay.rs | whitelist_settings | load_settings/save_settings (settings.toml) | Yes | ✓ FLOWING |
| whitelist_overlay.rs built-in | BUILTIN | port-core constant | Yes | ✓ FLOWING |
| status bar | kill_status | format_kill_status(real outcome) | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Creation-time mismatch aborts kill; child survives (PROC-07) | `cargo test -p port-core --test kill_integration test_creation_time_mismatch_aborts -- --exact` | 1 passed, 7.59s, real child spawned | ✓ PASS |
| Graceful Ctrl+C + timeout→force escalation (PROC-02) | `cargo test -p port-core --test kill_integration -- test_graceful_console_child test_timeout_then_force` | 2 passed, 2.42s (^C visible in child output) | ✓ PASS |
| PID-reuse churn — no wrong-process kill (PROC-07) | `cargo test -p port-core --test process_handle_integration` | 1 passed, 53.55s (10 iterations) | ✓ PASS |
| SRCH-02 AND/OR combination | `cargo test -p port-core filter` (within full run) | or_within_vec_and_across_dimensions ok | ✓ PASS |
| TUI suite | `cargo test -p port-tui` | 14 passed | ✓ PASS |
| Core lib suite | `cargo test --workspace` (partial) | port-core lib: 83 passed; kill_integration: 3/5 (2 flaky under parallel load — see Warnings); run aborted before churn/TUI (those verified separately above) | ⚠️ PARTIAL (flaky, not failing in isolation) |

### Probe Execution

No probes declared in PLANs or SUMMARYs for this phase (no `probe-*.sh` scripts, no probe criteria) — SKIPPED.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ----------- | ----------- | ------ | -------- |
| PROC-01 | 02-01 | Terminate owning process directly from port list | ✓ SATISFIED | x key → kill pipeline; integration tests; REQUIREMENTS.md Complete (02-01) |
| PROC-02 | 02-01 | Smart kill escalation: graceful → timeout → force | ✓ SATISFIED | kill.rs pipeline; timeout→force + graceful tests pass; kill_timeout_secs setting |
| PROC-03 | 02-01 | Instant kill default; whitelisted → confirm dialog | ✓ SATISFIED | KillPrepared routing; 60x7 kill_confirm.rs; dialog interaction → human check |
| PROC-04 | 02-01 | Built-in whitelist protects system-critical | ✓ SATISFIED | BUILTIN 25 entries; HardBlocked before OpenProcess; hardblock test passes |
| PROC-05 | 02-03 | User-customizable whitelist, immediate effect | ✓ SATISFIED | w overlay; validate/save; D-15 re-read; WR-04 concurrency fix |
| PROC-06 | 02-02 | View process details (path, start, cmdline, signature, parent PID) | ✓ SATISFIED | info.rs fetchers + detail_panel.rs; fetcher integration test; render → human check |
| PROC-07 | 02-01, 02-02 | HANDLE retained; PID-reuse safety | ✓ SATISFIED | ProcessSnapshot + open_verified; mismatch-abort + churn tests pass |
| SRCH-02 | 02-03 | AND/OR filter combination | ✓ SATISFIED | filter.rs existing engine + additive assertion test; Complete (02-03) |

All 8 phase requirement IDs accounted for: no orphans, no BLOCKED requirements. Every ID is marked `[x]` with a traceability row `Complete (0X-0X)` in REQUIREMENTS.md.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| port-core/tests/kill_integration.rs | 74, 108 | Flaky `wait_for_exit(pid, 5000)` — PID-based polling fails under parallel test load (PID reuse false-alarm) | ⚠️ Warning | 2 tests failed during the full-suite run but pass in isolation; the pipeline itself confirms exit via the verified HANDLE (WaitForSingleObject), so it cannot falsely report success — the flakiness is in the test harness, not the kill pipeline |
| ROADMAP.md SC2 | — | Success Criterion says kill key `k`; implementation uses `x` (UI-SPEC contract) | ⚠️ Warning | Documented deviation (plans/summaries consistently use `x`); ROADMAP SC text is stale — the capability (single-keypress kill) is delivered |
| port-core/src/process.rs:58-62 | — | Doc claims kill "re-captures snapshot fresh via snapshot_for(pid) at kill time"; implementation executes kills with the keypress-time snapshot (pending_kill_snapshot) | ℹ️ Info | IN-04 (unfixed, Info-level): safe in practice — open_verified re-verifies creation time; None creation_time degrades to PID-only check (best possible for pre-Phase-2 rows) |
| port-tui/src/message.rs KillExecute | 201-207 | Declared-but-never-constructed message variant | ℹ️ Info | IN-06 (self-acknowledged, `#[allow(dead_code)]`, documented rationale) |
| kill.rs route_strategy | 82-88 | Pure fn exists for tests; kill_blocking inlines the branch | ℹ️ Info | IN-01 (unfixed, Info-level): documented drift risk, no functional impact |

No TBD/FIXME/XXX debt markers found in any phase-modified file. No stub components (no placeholder renders, no hardcoded empty data, no console.log-only implementations). All 7 review findings (CR-01 + WR-01..WR-06) verified FIXED in code and git history (df07353, 1a0538b, 381921c, 4e6c422, 866c6d5, bb82c66, cedef5f, e4cdb4e, 6b9aad7).

Prohibitions verified:
- P1 (MUST NOT kill without creation-FILETIME verification): open_verified enforces; behavioral test passes. ✓
- P2 (MUST NOT FreeConsole/AttachConsole in the TUI process): console dance lives only in the `--ctrl-c` helper mode, which early-returns BEFORE terminal init (main.rs:63-68) and in the port-core helper binary. ✓
- P3 (MUST NOT report success when TerminateProcess failed / process still alive): terminate_and_wait maps every failure to an explicit KillOutcome; success requires WAIT_OBJECT_0 on the verified handle. ✓
- P4 (MUST NOT persist/export command-line arguments): no write/export path touches command_line (grep-verified). ✓
- P5 (MUST NOT accept a whitelist entry without normalization): validate_user_entry normalizes (quotes, separators, case, 8.3) before acceptance. ✓

### Human Verification Required

Automated checks passed (22/29 truths verified; no failed must-haves), but 7 behavior-unverified truths plus 5 planner-deferred human-check blocks require a live terminal session:

1. **BUILTIN constant domain review** — every entry a real system-critical Windows process; Tier-1 grounded in Microsoft Restart Manager Critical System Services. (Planner-deferred, 02-01)
2. **x-key kill UX** — outcome strings in the status bar, row disappears after refresh, hard-block message with "Press w to review the whitelist." and no dialog, "already exited" on repeat kill. (Planner-deferred, 02-01)
3. **d-key detail panel** — 12-row panel with all fields, j/k refresh, strikethrough + "Exited" after the process dies, ◆ markers and non-admin dimming. (Planner-deferred, 02-02)
4. **w-key whitelist overlay** — built-in section read-only with reasons, add/remove flows with the locked status strings, confirm dialog on user-listed kill, instant effect without restart. (Planner-deferred, 02-03)
5. **?-key Help overlay** — full key reference, Esc close, stack order above whitelist and below confirm. (Planner-deferred, 02-03)

### Gaps Summary

No gaps_found. All 29 must-have truths pass code-level verification (existence, substance, wiring, data flow), all 8 requirement IDs are satisfied and traceable, all 7 code-review findings are fixed, and the core behavioral invariants (PID-reuse safety, escalation pipeline, hard-block gate, churn) are proven by passing tests against real Windows processes.

Two warnings to carry forward (not blockers):
1. Two kill_integration tests (`test_graceful_console_child`, `test_timeout_then_force`) are flaky under full-suite parallel load (PID-based `wait_for_exit` polling; pass in isolation). Recommend hardening `wait_for_exit` to verify exit via a held handle or tolerate PID reuse.
2. ROADMAP.md Success Criterion 2 references kill key `k`; the shipped contract is `x`. Recommend updating the SC text to match the implemented UI-SPEC.

MVP-mode note: phase mode is `mvp` but the ROADMAP goal is not a canonical user story (`/gsd mvp-phase 2` recommended to canonicalize).

---

_Verified: 2026-08-01_
_Verifier: Claude (gsd-verifier)_

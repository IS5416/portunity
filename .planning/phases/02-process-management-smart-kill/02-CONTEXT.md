# Phase 2: Process Management & Smart Kill - Context

**Gathered:** 2026-07-31
**Status:** Ready for planning

<domain>
## Phase Boundary

Users inspect detailed process information for any port's owning process (full executable path, start time, command-line arguments, digital signature status, parent PID) via a detail panel, and terminate the owning process with smart kill escalation (graceful shutdown → configurable timeout → force kill). A built-in whitelist hard-blocks termination of ~30 system-critical processes with clear non-technical explanations; a user-editable whitelist (by executable path) gates termination behind a confirmation dialog. Whitelist changes take effect immediately without restart.

Users: select a port → press `d` for detail panel → press `x` to kill → graceful attempt → 5s timeout → force kill → status bar shows outcome → table auto-refreshes. Protected processes: clear explanation, cannot be killed (built-in) or confirmation dialog (user whitelist). Whitelist managed via `w` overlay.

</domain>

<decisions>
## Implementation Decisions

### Kill Trigger & Escalation
- **D-01 — Kill key is `x`:** `k` is already bound to MoveUp (vim-style, `port-tui/src/main.rs:281`). `x` follows lazygit-style TUI convention, single key, no modifier, adjacent to hjkl cluster. — **Reversibility:** reversible — key mapping is local to one handler.
- **D-02 — Graceful shutdown timeout: 5s default, configurable:** New `kill_timeout_secs` setting in settings.toml. Covers most graceful-close scenarios; user gets fast feedback when a port is freed.
- **D-03 — Admin-required kills: prompt + `a` key elevation, no auto-elevate:** When OpenProcess/TerminateProcess fails with access denied, status bar shows a clear message plus "press a to elevate" hint. No automatic UAC relaunch with `--kill-pid` — Phase 1 D-08 already rejected state transfer; an auto-restart would discard the user's browsing state, and UAC denial after relaunch leaves the old session gone. — **Reversibility:** costly — adding auto-elevate later requires a `--kill-pid` CLI arg, a relaunch protocol, and a handoff path between processes.
- **D-04 — Kill outcome via status bar + auto table refresh:** Success/failure with reason shown in the existing status bar (red bar for errors, normal color for info). After a successful kill, immediately trigger one scan so the freed port disappears from the table. No new toast component.

### Detail Panel
- **D-05 — Detail panel is a soft non-modal overlay:** Same interaction pattern as Phase 1 search/filter overlays (search.rs, filter_panel.rs). Does not squeeze the port table — critical at the 80x24 minimum terminal. Long paths/command lines may occupy most of the screen.
- **D-06 — `d` key toggles the panel:** Open/close via `d` (detail semantics, currently unbound). While open, changing the selected row refreshes panel content. Closed = zero interference.
- **D-07 — Digital signature: on-demand async + cache:** When the panel opens, run WinVerifyTrust in `spawn_blocking`, show "verifying..." then result. Cache in memory until the process disappears or next scan. Full pre-fetch at scan time is infeasible: 1000 processes × 10–100ms per signature blows the <500ms scan budget. Signature check failure (access denied) shows "unknown".
- **D-08 — Detail data fetched on panel open + cached:** Path (QueryFullProcessImageNameW), command line, start time, parent PID pulled once when the panel opens; per-process cost <5ms. Cache invalidated on next scan. Start time reuses the `ProcessHandle` creation time (PROC-07, locked in STATE.md). No batch pre-fetch at scan time.

### Protection Semantics
- **D-09 — Built-in whitelist = hard block; user whitelist = confirmation:** System-critical built-ins (smss.exe, csrss.exe, wininit.exe, services.exe, lsass.exe, svchost.exe, winlogon.exe, System, Idle, etc.) cannot be killed at all. Attempt shows a clear non-technical explanation ("this would crash the system") plus a hint that the entry can be reviewed in the whitelist settings. User-added entries gate the kill behind a confirmation dialog (user may proceed). — **Reversibility:** costly — switching to a unified confirm-and-kill model later changes the kill flow, copy, and tests; the hard-block is the safety floor per success criteria #4.
- **D-10 — Match semantics: built-in by basename, user entries by full path:** Built-in list matches process basename (smss.exe); user entries (PROC-05) match full executable path so same-name different-path instances (multiple node.exe) are distinguishable. System (PID 4) and Idle (PID 0) have no path — special-cased by PID. — **Reversibility:** costly — path matching on user entries is a stored-data contract; changing to basename later would silently protect the wrong instances.
- **D-11 — Confirmation dialog is a non-modal overlay:** Shows process name, the protection reason in plain language, and [Confirm kill] / [Cancel]. Same overlay family as search/filter/detail; table interaction stays live.
- **D-12 — User whitelist entries are confirmation-level only:** No hard-block option for user entries — PROC-05 scope is add/remove only. Simpler model: user whitelist = "kills need my confirmation".

### Whitelist Storage & UI
- **D-13 — Storage: extend settings.toml, built-in list hardcoded in port-core:** `AppSettings.whitelist: Vec<String>` (user path entries) appended to the existing TOML at `%APPDATA%/Portunity/settings.toml` (CORE-05, schema_version already present for forward-compat). The ~30-entry built-in list is hardcoded in port-core source (name + PID special cases), not user-editable. — **Reversibility:** costly — moving user entries to a different store later requires a settings.toml migration plus GUI adaptation in Phase 5/6.
- **D-14 — Whitelist management UI: `w` key overlay:** Non-modal overlay showing a read-only built-in section (what is protected and why) + an editable user section (list, add by path input, delete selected). No new tab — TUI-01 fixes 5 tabs; Phase 6 settings center reuses the same data structure. Input entry reuses the search-bar input pattern.
- **D-15 — Instant effect: re-read settings.toml before every kill attempt:** Cost <1ms per kill (kill is low-frequency). No file watcher dependency (notify crate not needed); a save in the `w` overlay takes effect on the next kill attempt immediately.

### Claude's Discretion
No areas deferred — all decisions explicitly selected by the user. Implementation mechanics left to researcher/planner: WM_CLOSE detection for GUI processes (EnumWindows by PID), console Ctrl+C delivery (AttachConsole/GenerateConsoleCtrlEvent — known flaky, needs fallback), graceful-vs-force routing for processes with neither GUI nor console (force directly), and `ProcessManager` trait reshaping for `ProcessHandle` + kill strategy parameters.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Planning Artifacts
- `.planning/ROADMAP.md` — Phase 2 scope, success criteria #1–5, requirements list (PROC-01…07, SRCH-02), pitfall coverage #1 (PID reuse), #2 (buffer retry), #3 (dual-stack), #4 (byte order), #11 (protected process)
- `.planning/REQUIREMENTS.md` — PROC-01 through PROC-07 full text; SRCH-02 (already implemented in Phase 1 — verify and mark traceability)
- `.planning/STATE.md` — Locked decisions: ProcessHandle (PID + HANDLE + creation time) over bare PID; config TOML hot-reload (CORE-05); async-first API with spawn_blocking; Phase 1 heuristic for system process dimming (to be replaced by real protection)
- `.planning/phases/01-tui-port-viewer/01-CONTEXT.md` — Phase 1 decisions: D-06/D-07/D-08 (elevation model — no state transfer), D-10/D-12/D-13 (TEA event loop, Message flow), soft non-modal overlay pattern (search/filter)
- `.planning/phases/01-tui-port-viewer/01-UI-SPEC.md` — Approved design contract: 4-region grid layout, keyboard layers (L0–L3 — `x`/`d`/`w` need registration), copywriting contract (error/empty/loading states)

### Research (project-level)
- `.planning/research/PITFALLS.md` — #1 (PID reuse — ProcessHandle rationale), #11 (protected processes), #9 (async blocking — spawn_blocking discipline)
- `.planning/research/ARCHITECTURE.md` — Platform abstraction design, trait boundaries (ProcessManager trait lives here), crate dependency graph

### Codebase
- `port-core/src/process.rs` — `ProcessManager` trait stub (`details(pid)`, `terminate(pid, force)`); signatures must evolve for ProcessHandle + kill strategy
- `port-core/src/models/process.rs` — `ProcessInfo` model with all PROC-06 fields (path, command line, start time, is_signed, is_system_critical, parent_pid)
- `port-core/src/config/settings.rs` — `AppSettings` TOML load/save at `%APPDATA%/Portunity/settings.toml`; extend with `whitelist` + `kill_timeout_secs`
- `port-core/src/lib.rs` — `Error` enum (Platform, NotFound, PermissionDenied, Io); kill failure mapping
- `port-tui/src/main.rs` — Key handling; `k` currently bound to MoveUp (line 281) — the conflict D-01 resolves
- `port-tui/src/app.rs` — `selected_index` state, status bar, tab dispatch
- `port-tui/src/update.rs` — TEA `update()`; new Messages: Kill, ToggleDetailPanel, ToggleWhitelistOverlay
- `port-tui/src/elevate.rs` — Existing ShellExecuteExW runas elevation (`a` key) — D-03 prompt path
- `port-tui/src/components/search.rs`, `filter_panel.rs` — Soft overlay pattern to replicate for detail panel, confirm dialog, whitelist overlay

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`ProcessManager` trait stub** (`port-core/src/process.rs`): Shape for the full implementation; needs `details` extension and `terminate` reshaping (strategy: graceful/force; handle-based per PROC-07).
- **`ProcessInfo` model** (`port-core/src/models/process.rs`): All PROC-06 fields already modeled — no model changes needed, only population.
- **`AppSettings` TOML** (`port-core/src/config/settings.rs`): Load/save with `schema_version`; add `whitelist: Vec<String>` and `kill_timeout_secs` with serde defaults — backward compatible.
- **Soft overlay pattern** (search.rs, filter_panel.rs): Direct template for detail panel, kill confirmation, and `w` whitelist overlay.
- **Status bar** (app.rs): Kill outcome display per D-04.
- **Elevation** (elevate.rs): `a`-key runas relaunch already exists — D-03 prompt points here.
- **Phase 1 system-process heuristic** (ports.rs ~line 363): PID<1000 or known-name dimming; superseded by the real whitelist in Phase 2 (keep cosmetic dimming, drive it from whitelist membership).

### Established Patterns
- **async-first API with spawn_blocking**: All Win32 calls (OpenProcess, QueryFullProcessImageNameW, WinVerifyTrust, EnumWindows, GenerateConsoleCtrlEvent) wrapped in spawn_blocking — never on the async runtime.
- **Non-modal soft overlays**: Search/filter precedent — detail panel, confirm dialog, whitelist overlay follow the same keyboard-interruptible style.
- **TEA message flow**: Results via `tokio::sync::mpsc` → `Message::Xxx` → `update()` → render. Kill outcome, detail fetch, signature result all flow this way.
- **ProcessHandle**: PID + HANDLE + creation time retained from OpenProcess (STATE.md) — kill validates creation time before terminating (Pitfall #1).

### Integration Points
- **main.rs:281**: `k` → MoveUp conflict — register `x`, `d`, `w` in the keyboard layers.
- **update.rs**: New messages: Kill, KillConfirmed, ToggleDetailPanel, DetailDataLoaded, ToggleWhitelistOverlay, WhitelistSaved, etc.
- **app.rs**: `selected_index` is the kill target and detail-panel data source; status bar shows outcomes.
- **config/settings.rs**: whitelist + kill_timeout_secs fields; save_settings called by the `w` overlay.
- **scanner/resolver.rs**: PID → name cache already exists; details() fetches the richer set on panel open (D-08).
- **filter.rs**: `system_only` filter uses `is_system_critical` — Phase 2 whitelist membership should keep this field truthful.

</code_context>

<specifics>
## Specific Ideas

- User discussed in Chinese but wants all artifacts English-only (CLAUDE.md rule updated 2026-07-31; `.planning/**/*.zh.md` gitignored).
- User selected the recommended option for every question — defaults matter: `x` kill key, 5s timeout, prompt-and-elevate, status bar outcome, overlay panel, `d` toggle, on-demand signature, open-time fetch, hard-block built-ins, basename/path matching split, non-modal confirm, confirmation-only user entries, settings.toml storage, `w` overlay, re-read-before-kill.
- No specific third-party references — open to standard Windows API approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within Phase 2 scope.

</deferred>

---

*Phase: 2-process-management-smart-kill*
*Context gathered: 2026-07-31*

# Phase 2: Process Management & Smart Kill — Research

**Researched:** 2026-07-31
**Domain:** Windows process management (Win32 process APIs, graceful termination, whitelist protection)
**Confidence:** MEDIUM

## Summary

Phase 2 implements the full process management stack in `port-core`: a `ProcessHandle`-safe process model (PROC-07), on-demand detail acquisition (path, command line, start time, parent PID, digital signature — PROC-06), smart kill escalation (WM_CLOSE → Ctrl+C → TerminateProcess with configurable timeout — PROC-02), and a two-tier whitelist (hard-blocked built-ins + confirmation-gated user entries — PROC-03/04/05), surfaced through three new TUI overlays (detail panel, confirm dialog, whitelist overlay) per the approved 02-UI-SPEC.

All Win32 API signatures were verified against the actual `windows` crate 0.62.2 source in the local cargo registry (the workspace already depends on 0.62, not 0.73 — STATE.md decision). Two crate feature flags must be added (`Win32_Security_WinTrust`, `Win32_System_Console`); **zero new crates** are needed. The single most load-bearing finding: `windows::Win32::Foundation::HANDLE` is `!Send` (wraps `*mut c_void`, no Send/Sync impls) — so OS handles must never cross async channel boundaries; a pure-data `ProcessSnapshot` (PID + FILETIME creation time + path) is the object that crosses the TEA channel, and handles are opened/verified/acted upon inside single `spawn_blocking` scopes.

Three research refinements to the CONTEXT assumptions deserve planner attention: (1) command line retrieval via `NtQueryInformationProcess` class 60 needs **no `ReadProcessMemory` and no `PROCESS_VM_READ`** — only `PROCESS_QUERY_LIMITED_INFORMATION`, and works against elevated processes (psutil/wazuh production pattern); (2) console Ctrl+C delivery (AttachConsole/GenerateConsoleCtrlEvent) is genuinely flaky for a console-subsystem app like the TUI — `FreeConsole()` permanently destroys the caller's own console — so the robust design is a **self-reexec helper process** (`port-tui --ctrl-c <pid>` with `CREATE_NO_WINDOW`), which keeps the TUI's terminal attachment untouched; (3) the built-in whitelist content should follow Microsoft's official Restart Manager "Critical System Services" list, which is smaller than a naive guess (~14 core entries); the "~30" target is reached by adding session infrastructure and security processes (conhost, spoolsv, MsMpEng, etc.) — the exact constant is a human-verifiable code artifact (Assumption A1).

**Primary recommendation:** Build `port-core` process management as four sub-modules (`process.rs` + `process/{handle,info,kill,whitelist}.rs` per the scanner.rs precedent), keep every Win32 call inside `spawn_blocking`, hold `HANDLE`s only within blocking scopes, and route graceful-kill strategies as: visible windows → `WM_CLOSE`; else console (probe via `AttachConsole`) → Ctrl+C helper; else → force immediately.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Kill Trigger & Escalation
- **D-01 — Kill key is `x`:** `k` is already bound to MoveUp (vim-style, `port-tui/src/main.rs:281`). `x` follows lazygit-style TUI convention, single key, no modifier, adjacent to hjkl cluster. — **Reversibility:** reversible — key mapping is local to one handler.
- **D-02 — Graceful shutdown timeout: 5s default, configurable:** New `kill_timeout_secs` setting in settings.toml. Covers most graceful-close scenarios; user gets fast feedback when a port is freed.
- **D-03 — Admin-required kills: prompt + `a` key elevation, no auto-elevate:** When OpenProcess/TerminateProcess fails with access denied, status bar shows a clear message plus "press a to elevate" hint. No automatic UAC relaunch with `--kill-pid` — Phase 1 D-08 already rejected state transfer; an auto-restart would discard the user's browsing state, and UAC denial after relaunch leaves the old session gone. — **Reversibility:** costly — adding auto-elevate later requires a `--kill-pid` CLI arg, a relaunch protocol, and a handoff path between processes.
- **D-04 — Kill outcome via status bar + auto table refresh:** Success/failure with reason shown in the existing status bar (red bar for errors, normal color for info). After a successful kill, immediately trigger one scan so the freed port disappears from the table. No new toast component.

#### Detail Panel
- **D-05 — Detail panel is a soft non-modal overlay:** Same interaction pattern as Phase 1 search/filter overlays (search.rs, filter_panel.rs). Does not squeeze the port table — critical at the 80x24 minimum terminal. Long paths/command lines may occupy most of the screen.
- **D-06 — `d` key toggles the panel:** Open/close via `d` (detail semantics, currently unbound). While open, changing the selected row refreshes panel content. Closed = zero interference.
- **D-07 — Digital signature: on-demand async + cache:** When the panel opens, run WinVerifyTrust in `spawn_blocking`, show "verifying..." then result. Cache in memory until the process disappears or next scan. Full pre-fetch at scan time is infeasible: 1000 processes × 10–100ms per signature blows the <500ms scan budget. Signature check failure (access denied) shows "unknown".
- **D-08 — Detail data fetched on panel open + cached:** Path (QueryFullProcessImageNameW), command line, start time, parent PID pulled once when the panel opens; per-process cost <5ms. Cache invalidated on next scan. Start time reuses the `ProcessHandle` creation time (PROC-07, locked in STATE.md). No batch pre-fetch at scan time.

#### Protection Semantics
- **D-09 — Built-in whitelist = hard block; user whitelist = confirmation:** System-critical built-ins (smss.exe, csrss.exe, wininit.exe, services.exe, lsass.exe, svchost.exe, winlogon.exe, System, Idle, etc.) cannot be killed at all. Attempt shows a clear non-technical explanation ("this would crash the system") plus a hint that the entry can be reviewed in the whitelist settings. User-added entries gate the kill behind a confirmation dialog (user may proceed). — **Reversibility:** costly — switching to a unified confirm-and-kill model later changes the kill flow, copy, and tests; the hard-block is the safety floor per success criteria #4.
- **D-10 — Match semantics: built-in by basename, user entries by full path:** Built-in list matches process basename (smss.exe); user entries (PROC-05) match full executable path so same-name different-path instances (multiple node.exe) are distinguishable. System (PID 4) and Idle (PID 0) have no path — special-cased by PID. — **Reversibility:** costly — path matching on user entries is a stored-data contract; changing to basename later would silently protect the wrong instances.
- **D-11 — Confirmation dialog is a non-modal overlay:** Shows process name, the protection reason in plain language, and [Confirm kill] / [Cancel]. Same overlay family as search/filter/detail; table interaction stays live.
- **D-12 — User whitelist entries are confirmation-level only:** No hard-block option for user entries — PROC-05 scope is add/remove only. Simpler model: user whitelist = "kills need my confirmation".

#### Whitelist Storage & UI
- **D-13 — Storage: extend settings.toml, built-in list hardcoded in port-core:** `AppSettings.whitelist: Vec<String>` (user path entries) appended to the existing TOML at `%APPDATA%/Portunity/settings.toml` (CORE-05, schema_version already present for forward-compat). The ~30-entry built-in list is hardcoded in port-core source (name + PID special cases), not user-editable. — **Reversibility:** costly — moving user entries to a different store later requires a settings.toml migration plus GUI adaptation in Phase 5/6.
- **D-14 — Whitelist management UI: `w` key overlay:** Non-modal overlay showing a read-only built-in section (what is protected and why) + an editable user section (list, add by path input, delete selected). No new tab — TUI-01 fixes 5 tabs; Phase 6 settings center reuses the same data structure. Input entry reuses the search-bar input pattern.
- **D-15 — Instant effect: re-read settings.toml before every kill attempt:** Cost <1ms per kill (kill is low-frequency). No file watcher dependency (notify crate not needed); a save in the `w` overlay takes effect on the next kill attempt immediately.

### Claude's Discretion
No areas deferred — all decisions explicitly selected by the user. Implementation mechanics left to researcher/planner: WM_CLOSE detection for GUI processes (EnumWindows by PID), console Ctrl+C delivery (AttachConsole/GenerateConsoleCtrlEvent — known flaky, needs fallback), graceful-vs-force routing for processes with neither GUI nor console (force directly), and `ProcessManager` trait reshaping for `ProcessHandle` + kill strategy parameters.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within Phase 2 scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROC-01 | Terminate the process owning a selected port directly from the port list | Kill flow (§Architecture Patterns: P3 Kill Escalation Pipeline); `x` key per D-01; status bar outcome per D-04; access-denied → elevation hint per D-03 |
| PROC-02 | Smart kill: graceful first (WM_CLOSE for GUI, Ctrl+C for console), configurable timeout, force-kill | WM_CLOSE routing via EnumWindows/GetWindowThreadProcessId/PostMessageW; Ctrl+C via self-reexec helper (`--ctrl-c <pid>`, CREATE_NO_WINDOW); WaitForSingleObject(handle, `kill_timeout_secs`×1000); TerminateProcess fallback; force-direct when neither GUI nor console |
| PROC-03 | Instant kill default; whitelisted processes get confirmation dialog | `Protection` enum (None/UserConfirm/HardBlocked) computed in port-core; TUI renders confirm dialog only for UserConfirm; hard-block renders no dialog (D-09) |
| PROC-04 | Built-in whitelist protects ~30 system-critical processes | Content grounded in Microsoft Restart Manager "Critical System Services" list + session/security processes (Assumption A1); basename match + PID 0/4 special case (D-10); checked BEFORE OpenProcess (Pitfall #11) |
| PROC-05 | User-customizable whitelist by executable path | `AppSettings.whitelist: Vec<String>` with serde defaults (backward compatible); case-insensitive full-path match with normalization; add/remove in `w` overlay; re-read before every kill (D-15) |
| PROC-06 | View process details: full path, start time, command line, digital signature, parent PID | QueryFullProcessImageNameW (path); GetProcessTimes FILETIME (start time, 100ns precision — also serves PID-reuse verification); NtQueryInformationProcess class 60 (command line, needs only QUERY_LIMITED_INFORMATION); Toolhelp32 snapshot (parent PID); WinVerifyTrustEx (signature, spawn_blocking + cache per D-07) |
| PROC-07 | Process HANDLE retained from OpenProcess; PID never re-derived after storage | `ProcessSnapshot` (PID + FILETIME creation time + path) is the Send-safe cache object; HANDLE opened + `GetProcessId`/`GetProcessTimes` verified + acted on within one `spawn_blocking` scope (HANDLE is `!Send` in windows-rs 0.62 — verified) |
| SRCH-02 | Combine multiple filter dimensions with AND/OR logic | **Already implemented in Phase 1** (`port-core/src/filter.rs` `apply_filters`: AND across dimensions, OR within Vec fields). Phase 2 work = verification + traceability update only — no new implementation |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Process detail acquisition (path/cmdline/start/parent/signature) | port-core (core library) | — | All Win32 access lives in port-core per CORE-01/02; frontends must never import windows-rs (Anti-Pattern #2) |
| Kill escalation routing + execution | port-core | — | Business logic (strategy selection, whitelist check, timeout) is frontend-agnostic; both TUI and future GUI consume the same `kill()` |
| Whitelist matching + protection status | port-core | — | `Protection` enum computed in core; filter.rs `system_only` reads `is_system_critical` which must stay truthful (CONTEXT integration point) |
| Whitelist storage (settings.toml) | port-core config layer | — | Extends existing `AppSettings` load/save (CORE-05); no new store |
| Whitelist membership markers (◆, dimming) | port-core (data) → TUI (render) | — | Core populates `is_system_critical` + `user_protected` at scan time; TUI renders markers/dimming from the model (ports.rs) |
| Detail panel / confirm dialog / whitelist overlay UI | TUI (client) | — | Pure rendering + key dispatch per 02-UI-SPEC; state lives in `App` |
| Kill outcome + status bar messages | TUI (client) | — | Message flow: core result → `Message::KillOutcome` → status bar (D-04) |
| Elevation on access denied | TUI (client, `a` key) | — | Existing elevate.rs ShellExecuteExW runas reused (D-03) — no new elevation code |

## Standard Stack

### Core

No new third-party crates. Phase 2 is built on the Win32 API surface of the **already-locked `windows` crate 0.62.2** (workspace dep — STATE.md decision; verified in local cargo registry). Two feature flags must be added to the workspace `Cargo.toml`.

| Feature Flag (workspace Cargo.toml) | APIs Enabled | Status |
|---------|-------------|--------|
| `Win32_System_Threading` | `OpenProcess`, `TerminateProcess`, `QueryFullProcessImageNameW`, `GetProcessTimes`, `GetProcessId`, `GetExitCodeProcess`, `WaitForSingleObject`, `ReadProcessMemory` | Already enabled ✓ |
| `Win32_UI_WindowsAndMessaging` | `EnumWindows`, `GetWindowThreadProcessId`, `PostMessageW`, `WM_CLOSE` | Already enabled ✓ |
| `Win32_System_Diagnostics_ToolHelp` | `CreateToolhelp32Snapshot`, `Process32FirstW`, `Process32NextW`, `PROCESSENTRY32W` | Already enabled ✓ |
| `Win32_Foundation` | `HANDLE`, `FILETIME`, `HWND`, `LPARAM`, `WPARAM`, `NTSTATUS` | Already enabled ✓ |
| `Win32_Security_WinTrust` | `WinVerifyTrust` / `WinVerifyTrustEx`, `WINTRUST_DATA`, `WINTRUST_FILE_INFO`, `WINTRUST_ACTION_GENERIC_VERIFY_V2` | **ADD — not enabled** |
| `Win32_System_Console` | `AttachConsole`, `FreeConsole`, `GenerateConsoleCtrlEvent`, `SetConsoleCtrlHandler`, `CTRL_C_EVENT` | **ADD — not enabled** |

**Version verification:** `windows` 0.62.2 present in local cargo registry at `~/.cargo/registry/src/index.crates.io-*/windows-0.62.2` (7 yrs old, 4.8M weekly downloads, Microsoft-maintained — legitimacy OK). Do NOT upgrade to 0.73 (STATE.md: v0.73 not on crates.io in July 2026; API differences).

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio::task::spawn_blocking` | 1.x (locked) | All Win32 calls off the async runtime (Pitfall #9) | Every OpenProcess/TerminateProcess/EnumWindows/WinVerifyTrust/Toolhelp32 call — non-negotiable |
| `std::process::Command` | std | Spawn the Ctrl+C helper (self-reexec `port-tui --ctrl-c <pid>` with `CREATE_NO_WINDOW` = 0x08000000) | Console-subsystem targets without visible windows |
| `clap` (port-tui dep) | 4.6 (locked) | Register `--ctrl-c <pid>` hidden flag for the helper mode | Helper re-reexec arg parsing |
| `toml` (port-core dep) | 0.8 (locked) | settings.toml serialization for `whitelist` + `kill_timeout_secs` | Existing config.rs pattern, serde defaults |
| `sysinfo` (port-core dep) | 0.39 (locked) | Fallback only: process existence checks, name resolution | NOT for start time or parent PID — seconds precision is too coarse for PID-reuse verification (verified in registry source: `start_time() -> u64` seconds) |
| `chrono` (locked) | 0.4 | FILETIME → `SystemTime` → display formatting | Start time display, existing dep |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| NtQueryInformationProcess class 60 (command line) | PEB walk + `ReadProcessMemory` | PEB walk needs `PROCESS_VM_READ` (+`PROCESS_QUERY_INFORMATION`), fails on elevated/protected processes, crash-prone with multi-level pointer derefs. Class 60 needs only `PROCESS_QUERY_LIMITED_INFORMATION` and works elevated (psutil/wazuh production). Undocumented but stable since Win 8.1 |
| NtQueryInformationProcess class 60 | WMI `Win32_Process::CommandLine` | WMI returns null for non-admin callers, needs WMI service, 10–100ms latency. Class 60 is in-process, ~0.1ms |
| Self-reexec helper for Ctrl+C | In-process `FreeConsole`/`AttachConsole` dance | In-process dance permanently destroys the TUI's own console (`FreeConsole` refcount → 0; re-attach fails ERROR_GEN_FAILURE 31 — documented). Helper process costs ~10–50ms spawn but never touches the TUI's terminal |
| Self-reexec helper | `windows-kill`/`ctrlc-windows` external binary | Third-party binary install + version skew + legitimacy burden. Self-reexec is zero-dependency and uses clap already present |
| `WinVerifyTrustEx` (typed) | `WinVerifyTrust` (raw `*mut c_void`) | Typed binding is safe Rust; raw variant needs manual cast of `WINTRUST_DATA` |

**Installation:**
```bash
# No new packages. Only add two feature flags to [workspace.dependencies] windows entry:
#   "Win32_Security_WinTrust",
#   "Win32_System_Console",
cargo build --workspace   # verify compilation
```

## Package Legitimacy Audit

> Phase 2 installs **zero new external packages** — the UI-SPEC Registry Safety section verifies this ("No new crates"). The only dependency change is adding two feature flags to the existing `windows` crate entry. Audit of the one crate whose API surface expands:

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| windows 0.62.2 | crates.io | 7 yrs (2019-01) | 4.84M/wk | github.com/microsoft/windows-rs | OK | Approved — already locked in workspace; add features `Win32_Security_WinTrust`, `Win32_System_Console` |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
**New crates proposed:** none. All other phase dependencies (tokio, sysinfo, clap, toml, chrono) are already locked workspace deps.

## Architecture Patterns

### System Architecture Diagram

```
┌───────────────────────────────  TUI CLIENT (port-tui)  ───────────────────────────────┐
│  keys: d / x / w / y / n / Esc / j / k / Tab                                           │
│  ┌────────────────── main.rs map_key_event ──────────────────┐                         │
│  │  overlay state machine: search ▸ filter ▸ detail ▸ wl ▸ confirm                      │
│  └──────────────┬──────────────────────┬─────────────────────┘                         │
│                 ▼                      ▼                                               │
│  update.rs (Message mutations)   App state (selection, overlays, caches)              │
│  ▲                               │ ▲                          │                       │
│  │ mpsc channel (Send-safe)      │ DetailDataLoaded /         │ KillOutcome           │
│  │ ProcessSnapshot data only     │ SignatureVerified          │ (status bar, D-04)    │
│  └───────────────────────────────┼────────────────────────────┼───────────────────────┘
│                                  │                            │
└──────────────────────────────────┼────────────────────────────┼─────────────────────────┘
                                   │ spawn_blocking             │ spawn_blocking
┌──────────────────────────────────▼────────────────────────────▼────────────────────────┐
│                          CORE LAYER (port-core::process)                                 │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────────┐  ┌──────────────────────┐     │
│  │ handle.rs    │  │ info.rs        │  │ kill.rs        │  │ whitelist.rs         │     │
│  │ ProcessSnap- │→ │ path: QueryFull│→ │ 1. re-read     │  │ BUILTIN: ~30 entries │     │
│  │ shot (pid,   │  │  ProcessImage- │  │    settings    │  │  (name + reason)     │     │
│  │  FILETIME,   │  │  NameW         │  │ 2. built-in    │  │ PID 0/4 special case │     │
│  │  path)       │  │ cmdline: Nt-   │  │    check       │  │ user_match(path)     │     │
│  │ HANDLE held  │  │  QueryInfo cls│ │ 3. OpenProcess  │  │ Protection enum      │     │
│  │  only inside │  │  60            │  │    (QLI|TERM|  │  │  (None|UserConfirm|  │     │
│  │  spawn_block-│  │ start: Get-    │  │    SYNCH)      │  │   HardBlocked)       │     │
│  │  ing scope   │  │  ProcessTimes  │  │ 4. verify      │  └──────────────────────┘     │
│  │  (!Send!)    │  │ parent: Tool-  │  │    creation    │  ▲                            │
│  └──────────────┘  │  help32        │  │    time        │  │ settings.toml re-read      │
│                    │ signature:     │  │ 5. route:      │  │ (D-15, <1ms)               │
│                    │  WinVerifyTrust│  │    windows→    │  └──────────────────────┘     │
│                    │  (async, cache)│  │    WM_CLOSE    │                               │
│                    └────────────────┘  │    else console│  ┌──────────────────────┐     │
│                                        │      → --ctrl-c│  │ config/settings.rs  │     │
│                                        │      helper    │  │ whitelist: Vec<Str> │     │
│                                        │    else force  │  │ kill_timeout_secs=5 │     │
│                                        │ 6. WaitForSin- │  └──────────────────────┘     │
│                                        │    gleObject   │                               │
│                                        │ 7. timeout→    │  ┌──────────────────────┐     │
│                                        │    Terminate-  │  │ scanner (tcp/udp)    │     │
│                                        │    Process     │  │ populates is_system_ │     │
│                                        │ 8. AccessDenied│  │ critical + user_     │     │
│                                        │    → D-03 msg  │  │ protected at scan    │     │
│                                        └────────────────┘  └──────────────────────┘     │
└──────────────────────────────────────────────────────────────────────────────────────────┘
        ▲ Ctrl+C helper (self-reexec): SetConsoleCtrlHandler(NULL,TRUE) → FreeConsole
        │   → AttachConsole(pid) → GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) → exit
┌───────┴───────────────────────────────  OS (Windows 10/11)  ────────────────────────────┐
│  kernel32 (OpenProcess/TerminateProcess/QueryFullProcessImageNameW/GetProcessTimes/     │
│    WaitForSingleObject/AttachConsole/GenerateConsoleCtrlEvent/ReadProcessMemory)        │
│  user32 (EnumWindows/GetWindowThreadProcessId/PostMessageW)  ntdll (NtQueryInformation- │
│    Process)  wintrust (WinVerifyTrustEx)  toolhelp (CreateToolhelp32Snapshot)           │
└──────────────────────────────────────────────────────────────────────────────────────────┘

Primary use case trace: user selects port → d (panel) → detail fetch (spawn_blocking, <5ms)
→ x (kill) → protection check → dialog or straight to kill → graceful attempt → 5s wait →
force → KillOutcome → status bar + auto-refresh scan → freed port disappears.
```

### Recommended Project Structure

```
port-core/src/
├── process.rs                  # ProcessManager trait (reshaped) + module decls
└── process/
    ├── handle.rs               # ProcessSnapshot (Send-safe), open+verify+drop helper (HANDLE !Send)
    ├── info.rs                 # path / cmdline / start time / parent PID / signature fetchers
    ├── kill.rs                 # KillTarget, KillOutcome, escalation pipeline
    └── whitelist.rs            # BUILTIN constant, builtin_match, user_match, Protection
port-tui/src/
├── main.rs                     # key layers d/x/w + y/n confirm; --ctrl-c <pid> helper mode (clap)
├── app.rs                      # detail cache, confirm state, whitelist overlay state, settings copy
├── message.rs                  # Kill/KillConfirmed/KillOutcome/ToggleDetailPanel/DetailDataLoaded/
│                               #   SignatureVerified/ToggleWhitelistOverlay/Whitelist* /ProcessExited
├── update.rs                   # new Message handlers
└── components/
    ├── detail_panel.rs         # NEW — 12-row Clear-over overlay (per 02-UI-SPEC)
    ├── kill_confirm.rs         # NEW — 60×7 centered popup
    ├── whitelist_overlay.rs    # NEW — 20-row overlay, built-in list + user list + input
    └── ports.rs                # MODIFIED — ◆ marker, whitelist-driven dimming, strikethrough
```

Module layout follows the CLAUDE.md new-style rule and the existing `scanner.rs` + `scanner/` precedent: leaf module `process.rs` becomes the parent with `pub mod handle/info/kill/whitelist` (no `mod.rs` files).

### Pattern 1: ProcessSnapshot — Send-safe Process Identity

**What:** PID-reuse safety (Pitfall #1, PROC-07) requires a stable process identity that survives async boundaries. Raw `HANDLE` is `!Send` in windows-rs 0.62 (verified: `pub struct HANDLE(pub *mut c_void)`, no Send/Sync impls) so it cannot cross the TEA mpsc channel or App state. The Send-safe identity is a pure-data snapshot; the handle is opened, verified, and closed inside one blocking scope.

**When to use:** Every cross-boundary process reference (detail cache, kill target, signature cache key).

**Example:**
```rust
// process/handle.rs — pure data, Send, crosses mpsc channel
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub creation_time: Option<windows::Win32::Foundation::FILETIME>,
    pub executable_path: Option<String>,
}

// Internal only — never stored in App state or channel payloads
struct OpenProcessHandle { pid: u32, handle: windows::Win32::Foundation::HANDLE }
impl Drop for OpenProcessHandle { /* CloseHandle on drop */ }

// In one spawn_blocking scope: open → verify → act → drop
fn verify_and_kill(snapshot: &ProcessSnapshot) -> Result<()> {
    let h = open_handle(snapshot.pid)?;             // QLI | TERMINATE | SYNCHRONIZE
    if GetProcessId(h.handle) != snapshot.pid { return Err(Error::NotFound("pid reused".into())); }
    if let Some(ct) = snapshot.creation_time {
        let mut now = FILETIME::default();
        unsafe { GetProcessTimes(h.handle, &mut now, ... )?; }
        if now != ct { return Err(Error::NotFound("process replaced (creation time mismatch)".into())); }
    }
    unsafe { TerminateProcess(h.handle, 1)?; }
    Ok(())
}
```
*(Signatures from windows-rs 0.62.2 registry source — [VERIFIED])*

### Pattern 2: Kill Escalation Pipeline (kill.rs)

**What:** Ordered strategy: whitelist gate → open+verify → graceful attempt → timeout wait → force. The 5s timeout only starts after a graceful signal was actually dispatched; processes with neither GUI windows nor a console are force-killed directly (Claude's Discretion item — no pointless 5s wait).

**When to use:** The single `kill()` entry point — TUI and future GUI both call it.

**Example (routing logic, pure — unit-testable):**
```rust
pub enum KillOutcome {
    Graceful,           // WM_CLOSE or Ctrl+C caused exit within timeout
    ForceKilled,        // graceful timed out → TerminateProcess → exited
    Direct,             // no GUI window, no console → terminated immediately
    AlreadyExited,      // GetExitCodeProcess != STILL_ACTIVE before attempt
    AccessDenied,       // ERROR_ACCESS_DENIED → D-03 "Press a to elevate"
    HardBlocked(&'static str), // built-in whitelist reason — kill() never reached
    Failed(String),     // other failures with reason
}

// route decision — pure function over probes, unit-testable without Windows
pub fn route_strategy(has_visible_windows: bool, has_console: bool) -> Strategy {
    match (has_visible_windows, has_console) {
        (true, _)    => Strategy::WmClose,
        (false, true) => Strategy::ConsoleCtrlC,
        (false, false) => Strategy::ForceDirect,
    }
}
```

Execution (all in one `spawn_blocking`):
1. Re-read settings.toml (D-15) → `Protection` check (built-in by name/PID; user by full path) → `HardBlocked` short-circuits before any `OpenProcess` (Pitfall #11).
2. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, pid)` — verified constants 0x1000|0x1|0x100000 ([VERIFIED: registry source]).
3. Verify: `GetProcessId` + `GetProcessTimes` creation FILETIME against the snapshot (Pitfall #1).
4. `EnumWindows` by PID → any visible top-level window → `PostMessageW(hwnd, WM_CLOSE, 0, 0)` (fire-and-forget; UIPI silently blocks cross-integrity — that becomes the timeout→force→AccessDenied path, correctly surfacing D-03).
5. No windows → console probe: `AttachConsole(pid)` succeeds? → FreeConsole → spawn `port-tui --ctrl-c <pid>` helper (CREATE_NO_WINDOW), wait for helper exit code (0 = delivered).
6. Neither → `Strategy::ForceDirect`.
7. `WaitForSingleObject(handle, kill_timeout_secs * 1000)` → `WAIT_OBJECT_0` = graceful success; `WAIT_TIMEOUT` → `TerminateProcess(handle, 1)` → `WaitForSingleObject(handle, 3000)` ([ASSUMED: 3s post-terminate wait]).
8. Map `ERROR_ACCESS_DENIED` → `KillOutcome::AccessDenied` (D-03 message).

### Pattern 3: Ctrl+C Helper Process (`--ctrl-c <pid>`)

**What:** Console graceful shutdown cannot be done safely in-process from a console-subsystem app: `FreeConsole()` destroys the caller's own console (refcount → 0; re-attach fails ERROR_GEN_FAILURE 31 — documented in multiple sources). The robust pattern (used by ctrlc-windows, windows-kill, CtrlC.exe) is a short-lived helper that does the console dance and exits.

**When to use:** Every console-subsystem target without visible windows.

**Example:**
```rust
// port-tui/src/main.rs — clap arg: #[arg(long, hide = true)] ctrl_c_pid: Option<u32>
// Helper mode (spawned by port-core::kill with CREATE_NO_WINDOW):
unsafe {
    let _ = SetConsoleCtrlHandler(None, true);        // ignore CTRL_C in helper
    let _ = FreeConsole();                             // detach helper's hidden console
    let attached = AttachConsole(ctrl_c_pid).is_ok();  // attach to target's console
    if attached {
        let _ = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0); // broadcast to that console
    }
    // exit code: 0 = delivered, 1 = no console (caller force-kills directly)
}
```
`port-core` spawns it: `Command::new(current_exe).arg("--ctrl-c").arg(pid).creation_flags(0x08000000 /*CREATE_NO_WINDOW*/).status()`. Caveats to document in code: (a) the event broadcasts to ALL processes on the target's console (sender ignores it via the NULL handler); (b) processes created with `CREATE_NEW_PROCESS_GROUP` ignore Ctrl+C; (c) a pending `ReadConsole` may not be interrupted — "delivered" does not guarantee "exited"; the `WaitForSingleObject` timeout is the real arbiter. *(Pattern [CITED: Stack Overflow / ctrlc-windows / windows-kill]; signatures [VERIFIED: registry source])*

### Pattern 4: Scan-Time Protection Markers

**What:** The `◆` marker (UI-SPEC) and truthful `is_system_critical` require whitelist membership at render time, but user entries match full paths while the scan only produces names. Targeted resolution: for each unique PID whose basename matches any user-entry basename, run `QueryFullProcessImageNameW` (≈1ms, `PROCESS_QUERY_LIMITED_INFORMATION` works non-elevated) and full-path match. Only same-basename processes pay the cost.

**When to use:** In the scan pipeline (scanner.rs `scan_all` post-pass, inside the existing `spawn_blocking`).

**Example:**
```rust
// scanner.rs scan_all() post-pass (pseudocode)
let user_entry_basenames: HashSet<String> = settings.whitelist.iter()
    .filter_map(|p| Path::new(p).file_name().and_then(|f| f.to_str()))
    .map(|s| s.to_lowercase()).collect();
for conn in &mut conns {
    if let Some(bn) = process_basename(&conn.process.name) {
        if builtin_match(conn.process.pid, bn).is_some() {
            conn.process.is_system_critical = true;
        } else if user_entry_basenames.contains(&bn.to_lowercase()) {
            if let Some(path) = query_full_path(conn.process.pid) {   // spawn_blocking scope
                conn.process.user_protected = user_match(&path, &settings.whitelist);
            }
        }
    }
}
```
**Model delta:** `ProcessInfo` gains `pub user_protected: bool` (default false). CONTEXT says "no model changes needed, only population" — that statement covers the PROC-06 detail fields; the marker field is a deliberate, minimal addition (see Assumption A4) required to distinguish the built-in ◆ (error color) from the user ◆ (warning color) at render time.

### Anti-Patterns to Avoid

- **Opening with `PROCESS_ALL_ACCESS`:** Unnecessary privilege escalation, flagged by AV/EDR. Use exactly `QUERY_LIMITED_INFORMATION | TERMINATE | SYNCHRONIZE` (PITFALLS.md Security Mistakes).
- **Holding `HANDLE` in App state or sending it over the mpsc channel:** Compile error (`!Send`) in the wrong place; use `ProcessSnapshot` instead.
- **Calling any Win32 process API outside `spawn_blocking`:** Runtime stall (Pitfall #9). This includes `EnumWindows` and `WinVerifyTrust`.
- **Killing without the whitelist check, or swallowing `TerminateProcess` errors:** Pitfall #11 — protected processes silently fail; always check the return and map outcomes.
- **Case-sensitive path comparison:** Windows paths are case-insensitive — normalize + `eq_ignore_ascii_case`.
- **Deriving kill identity from a scan-time PID alone:** The snapshot's creation FILETIME is the reuse check (Pitfall #1); scanning between `OpenProcess` and `TerminateProcess` without verification is the classic wrong-process kill.
- **Doing the `FreeConsole` dance in the TUI process:** Destroys the terminal attachment (ERROR_GEN_FAILURE 31); always the helper process.
- **Waiting the full timeout when no graceful signal was sent:** Force-direct for windowless/consoleless targets (D-02 intent: 5s is for graceful close, not blanket latency).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PID-reuse-safe process references | Raw `u32` PIDs + re-`OpenProcess` | `ProcessSnapshot` + creation-time verification (Pattern 1) | Windows recycles PIDs in milliseconds; no zombie state (Pitfall #1) |
| Graceful GUI close | Window-tree walking / WM_QUIT heuristics | `EnumWindows` + `GetWindowThreadProcessId` + `PostMessageW(WM_CLOSE)` | Documented cooperative-close channel (Task Manager, STAF, Sysinternals pattern); WM_QUIT is undocumented-in-practice and loses app cleanup |
| Console Ctrl+C delivery | In-process `AttachConsole`/`FreeConsole` dance | Self-reexec helper `--ctrl-c <pid>` (CREATE_NO_WINDOW) | `FreeConsole` permanently destroys the sender's own console; helper isolates the risk (ctrlc-windows/windows-kill production pattern) |
| Command line extraction | PEB structure walking + `ReadProcessMemory` | `NtQueryInformationProcess` class 60 (two-call) | No `PROCESS_VM_READ`, no multi-level pointer derefs, works against elevated processes (psutil/wazuh); PEB walk crashes and fails on protected processes |
| Digital signature check | Hand-parse PE security directory | `WinVerifyTrustEx` + `WINTRUST_ACTION_GENERIC_VERIFY_V2` | Catalog fallback, chain building, revocation logic are OS-owned; hand-rolled parsers misreport (TRUST_E_NOSIGNATURE on corrupt directory) |
| 8.3 / short-path normalization | Manual `C:\PROGRA~1` resolution | `GetLongPathNameW` on user entries at load | Short names break full-path matching; OS API is authoritative |
| Whitelist persistence | New config file / DB table | Extend `settings.toml` (`AppSettings.whitelist` + serde defaults) | CORE-05 locked; schema_version present; backward compatible (D-13) |

**Key insight:** Every hard problem in this phase (PID reuse, console signaling, command lines, signatures) has a canonical Win32 API or production-proven pattern. The Windows API is the ecosystem — custom solutions here are worse because the OS enforces access rights, integrity levels, and console semantics that user code cannot replicate.

## Runtime State Inventory

> Phase 2 reshapes the `ProcessManager` trait and supersedes the Phase 1 system-process heuristic — a source-level refactor. No persisted/runtime state is renamed or migrated.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — settings.toml gains new fields via serde defaults; existing files parse unchanged (defaults apply) | None |
| Live service config | None — no external services involved | None |
| OS-registered state | None — no scheduled tasks/services/registrations created or renamed | None |
| Secrets/env vars | None — no env var or secret names change; command-line display of processes is a new feature (see Security Domain — display-only, no persistence) | None |
| Build artifacts | None — no package/artifact renames; `ProcessManager` trait shape changes are source-only | None |

**Canonical question answered:** After every source file is updated, no runtime system retains a stale string — this phase neither renames nor migrates anything; the only "superseded" artifact (ports.rs `SYSTEM_NAMES` heuristic) is deleted in favor of whitelist-driven logic.

## Common Pitfalls

### Pitfall 1: Killing the wrong process via PID reuse
**What goes wrong:** `OpenProcess(pid)` at kill time resolves to a different process than the one the user selected.
**Why it happens:** PIDs are recycled immediately on exit; the detail-panel open and the kill are separate operations.
**How to avoid:** The detail fetch captures `creation_time` (GetProcessTimes FILETIME, 100ns precision) into the `ProcessSnapshot`; the kill re-opens, compares `GetProcessTimes` against the snapshot, and aborts on mismatch — all inside one `spawn_blocking`.
**Warning signs:** Intermittent "wrong process died" reports; `GetProcessId(handle) != pid` in logs.

### Pitfall 2: TUI console destroyed by the Ctrl+C dance
**What goes wrong:** In-process `FreeConsole()` → `AttachConsole(target)` → `GenerateConsoleCtrlEvent` → `FreeConsole()` → `AttachConsole(ATTACH_PARENT)` sequence leaves the TUI with no console (ERROR_GEN_FAILURE 31); terminal freezes; rendering stops.
**Why it happens:** `FreeConsole` decrements the console refcount; the TUI's own console is destroyed; re-attach to the parent console is not guaranteed (documented failure in VS debugger console, yori, etc.).
**How to avoid:** Always the helper-process pattern (`--ctrl-c <pid>` with `CREATE_NO_WINDOW`). Never `FreeConsole` in the TUI process.
**Warning signs:** Kill attempt followed by frozen terminal; app still runs but output stops.

### Pitfall 3: WM_CLOSE silently blocked across integrity levels
**What goes wrong:** Non-admin user presses `x` on an elevated process; `PostMessageW` returns success but the target never closes; timeout → `TerminateProcess` → `ERROR_ACCESS_DENIED`.
**Why it happens:** UIPI silently drops cross-IL messages; `PostMessageW` does not fail loudly.
**How to avoid:** Accept as the designed D-03 flow — the eventual AccessDenied outcome shows "admin rights needed — press a to elevate". Don't attempt to pre-detect integrity level (fragile); let the pipeline terminate in the correct message.
**Warning signs:** 5s wait then force-kill failure on elevated targets when running non-admin.

### Pitfall 4: "Delivered" Ctrl+C ≠ "exited"
**What goes wrong:** `GenerateConsoleCtrlEvent` returns OK but the target keeps running: it was created with `CREATE_NEW_PROCESS_GROUP` (ignores Ctrl+C), or has a thread blocked in `ReadConsole`, or handles the event and continues.
**Why it happens:** Ctrl+C is a cooperative event; the helper exit code only proves delivery.
**How to avoid:** The `WaitForSingleObject(handle, kill_timeout_secs*1000)` is the arbiter; on `WAIT_TIMEOUT` proceed to `TerminateProcess`. The UI copy already covers this ("Graceful close timed out — force killing").
**Warning signs:** Graceful message shown but process persists for the full timeout.

### Pitfall 5: Signature verification stalling the scan
**What goes wrong:** WinVerifyTrust costs 10–100ms per file (chain building, CRL checks); any batch pre-fetch blows the <500ms scan budget.
**Why it happens:** Temptation to populate `is_signed` at scan time.
**How to avoid:** Strictly on-demand per D-07: spawn_blocking on panel open, cache by PID in memory, "Verifying…" state, invalidate on process disappearance. Also call with `WTD_STATEACTION_CLOSE` on a second `WinVerifyTrustEx` call to release resources.
**Warning signs:** Scan duration creeping toward seconds; signature rows showing stale values.

### Pitfall 6: Whitelist match drift (basename vs path)
**What goes wrong:** Built-in matching by basename catches `C:\windows\svchost.exe` but the user adds `C:\Program Files\Foo\svchost.exe` to the user list — the two tiers must not collide or double-gate.
**Why it happens:** Same basename in both tiers; or case/8.3 path variants breaking the user-list match.
**How to avoid:** Check built-in first (name/PID) — user-tier lookup only for non-built-in basenames; normalize user paths (trim quotes/whitespace, case-insensitive compare, `GetLongPathNameW` for 8.3). D-10 semantics are a stored-data contract — tests must lock them.
**Warning signs:** A process that is both built-in and user-listed showing two markers; user entry "not working" for a copied Task Manager path (8.3 variant).

### Pitfall 7: `--ctrl-c` helper mode activating in normal runs
**What goes wrong:** The hidden clap flag `--ctrl-c <pid>` is parsed by the normal event loop; a typo or stray arg makes the TUI enter helper mode unexpectedly (and the main loop would otherwise also start the UI).
**Why it happens:** No early-return guard for the helper mode.
**How to avoid:** At the top of `main()`, if `ctrl_c_pid` is `Some`, run the helper routine and `std::process::exit(code)` before terminal init / raw mode / event loop. Unit-test the exit-code contract.
**Warning signs:** App launches into a blank screen when launched with extra args.

## Code Examples

Verified patterns from official sources and the local windows-rs 0.62.2 registry source:

### 1. Open, verify creation time, terminate (windows-rs 0.62 signatures)
```rust
// Source: windows-rs 0.62.2 registry source — signatures [VERIFIED]
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::Threading::{
    OpenProcess, GetProcessTimes, GetProcessId, TerminateProcess, WaitForSingleObject,
    PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE,
};
use windows::Win32::Foundation::WAIT_EVENT;

// signature check (from registry):
//   OpenProcess(dwdesiredaccess: PROCESS_ACCESS_RIGHTS, binherithandle: bool, dwprocessid: u32) -> Result<HANDLE>
//   TerminateProcess(hprocess: HANDLE, uexitcode: u32) -> Result<()>
//   GetProcessTimes(hprocess: HANDLE, ... *mut FILETIME x4) -> Result<()>
//   GetProcessId(process: HANDLE) -> u32
//   WaitForSingleObject(hhandle: HANDLE, dwmilliseconds: u32) -> WAIT_EVENT
const RIGHTS: PROCESS_ACCESS_RIGHTS =
    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE;
```

### 2. WM_CLOSE routing (EnumWindows by PID)
```rust
// Source: windows-rs 0.62 signatures [VERIFIED]; pattern [CITED: STAF, Task Manager methodology]
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

// windows-rs 0.62:
//   EnumWindows(lpenumfunc: WNDENUMPROC, lparam: LPARAM) -> Result<()>
//   GetWindowThreadProcessId(hwnd: HWND, lpdwprocessid: Option<*mut u32>) -> u32
//   PostMessageW(hwnd: Option<HWND>, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Result<()>
// WM_CLOSE: u32 = 16 (verified constant)

// WNDENUMPROC = unsafe extern "system" fn(HWND, LPARAM) -> BOOL
// collect windows whose GetWindowThreadProcessId == target pid; prefer visible ones;
// PostMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0)) — fire-and-forget.
// Caveats: caller's desktop only; UIPI blocks cross-IL silently; console windows
// are owned by conhost, not the target process.
```

### 3. Command line via NtQueryInformationProcess class 60
```rust
// Source: psutil PR #1398 / wazuh PR #34727 pattern [CITED]; class 60 stable Win 8.1+
// Requires only PROCESS_QUERY_LIMITED_INFORMATION. NOT in the windows crate — manual link.
#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQueryInformationProcess(
        processhandle: *mut core::ffi::c_void,
        processinformationclass: u32,
        processinformation: *mut core::ffi::c_void,
        processinformationlength: u32,
        returnlength: *mut u32,
    ) -> i32; // NTSTATUS
}
const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004;

// Two-call pattern:
//   1st: buffer=null, len=0 → STATUS_INFO_LENGTH_MISMATCH, returnlength = needed size
//   2nd: allocate returnlength, call again → buffer holds UNICODE_STRING whose
//        Buffer points into the caller's returned buffer (Win 8.1+); copy
//        min(Length, alloc) wide chars defensively (bounds-check Buffer against the
//        allocation before dereferencing).
// Failure (access denied) → field renders "—" per UI-SPEC.
```

### 4. Digital signature verification
```rust
// Source: Mozilla reference + codesigned crate [CITED]; windows-rs 0.62 module [VERIFIED]
use windows::Win32::Security::WinTrust::{
    WinVerifyTrustEx, WINTRUST_DATA, WINTRUST_DATA_STATE_ACTION, WINTRUST_DATA_UNION_CHOICE,
    WINTRUST_DATA_UI_CHOICE, WINTRUST_DATA_REVOCATION_CHECKS, WINTRUST_FILE_INFO,
    WINTRUST_ACTION_GENERIC_VERIFY_V2,
};
// WINTRUST_DATA { cbStruct, dwUIChoice: WTD_UI_NONE(2), fdwRevocationChecks: WTD_REVOKE_NONE(0),
//                 dwUnionChoice: WTD_CHOICE_FILE(1), pFile: &WINTRUST_FILE_INFO, ... }
// WINTRUST_FILE_INFO { cbStruct, pcwszFilePath: PCWSTR (wide path), hFile, pgKnownSubject }
// 0 => Signed; 0x800B0100 (TRUST_E_NOSIGNATURE) => Unsigned; other => Unknown
// Resource cleanup: call WinVerifyTrustEx again with dwStateAction = WTD_STATEACTION_CLOSE.
// Run in spawn_blocking (10–100ms typical) + cache per D-07.
```

### 5. Parent PID via Toolhelp32
```rust
// Source: docs.rs examples (faithe, vibesurfer) [CITED]; signatures [VERIFIED: registry]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::Foundation::CloseHandle;
// CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) -> Result<HANDLE>
// MUST set entry.dwSize = size_of::<PROCESSENTRY32W>() before Process32FirstW.
// Loop Process32NextW until Err; entry.th32ParentProcessID / entry.szExeFile ([u16;260]).
// CloseHandle(snapshot) on completion — snapshot handle must not leak (PITFALLS.md gotcha).
```

### 6. FILETIME → SystemTime (start time)
```rust
// FILETIME = 100ns intervals since 1601-01-01 UTC (two u32 fields)
let ft_u64 = ((dwHighDateTime as u64) << 32) | dwLowDateTime as u64;
let unix_secs = (ft_u64 / 10_000_000).saturating_sub(11_644_473_600); // 1601→1970 offset
let st = std::time::UNIX_EPOCH + std::time::Duration::from_secs(unix_secs);
// format via chrono: "09:41:12 31-Jul-2026" per UI-SPEC detail row 5
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| PEB walk + `ReadProcessMemory` for command lines | `NtQueryInformationProcess(ProcessCommandLineInformation=60)` | Win 8.1 / Server 2012 R2 (psutil adopted 2019, PR #1398) | No `PROCESS_VM_READ` needed; works against elevated processes; simpler + safer |
| `winapi` crate for Win32 interop | `windows` crate 0.62 (Microsoft official) | 2019–2023 migration | RAII `HANDLE` wrappers, `Result` returns, feature-gated modules (STATE.md locked 0.62) |
| `wfp` / raw WFP for firewall | (Phase 4 concern) | — | — |
| In-process console dance for Ctrl+C | Helper-process pattern (ctrlc-windows 2019, windows-kill) | ~2019 | Eliminates console destruction/zombie bugs; standard for library authors |

**Deprecated/outdated:**
- `PROCESS_ALL_ACCESS` on `OpenProcess`: AV/EDR flagging; use minimal rights (PITFALLS.md Security Mistakes).
- In-process `FreeConsole`/`AttachConsole` ctrl-c utilities: documented console-corruption failure modes.
- PEB-based command line extraction: crash-prone, access-denied on elevated/protected targets.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Built-in whitelist content: Microsoft Restart Manager core list (smss, csrss, wininit, winlogon, services, lsass, System/PID4, svchost, logonui) + dwm, fontdrvhost, Memory Compression, Idle/PID0, Registry + second tier (lsaiso, sessionenv, conhost, spoolsv, audiodg, MsMpEng, SgrmBroker) ≈ 26–30 entries | Whitelist / Pattern 4 | The exact constant is user-visible safety policy — planner should add a `checkpoint:human-verify` on the final list; wrong list = over- or under-protection |
| A2 | WinVerifyTrust costs 10–100ms/file | Pitfall 5 | If faster (<10ms) the D-07 on-demand design is still correct; if slower, signature row must keep "Verifying…" until timeout/error handling |
| A3 | `--ctrl-c` helper is the right delivery mechanism vs in-process dance | Pattern 3 | If in-process worked reliably on Windows 11, helper adds ~50ms; the risk asymmetry (destroyed terminal) strongly favors helper |
| A4 | `ProcessInfo` gains `user_protected: bool` field (deviation from CONTEXT "no model changes") | Pattern 4 | CONTEXT statement covered PROC-06 detail fields; the marker field is required to render ◆-warning at scan time without per-row OpenProcess at render. If rejected, marker must be computed in TUI render (per-row path lookup — violates render-on-demand) |
| A5 | 3s post-TerminateProcess wait before declaring failure | Pattern 2 | TerminateProcess is async; too short → false "failed" for slow teardown; too long → UI lag. 3s is a reasonable default |
| A6 | NtQueryInformationProcess class 60 stability on Win 10/11 | Pattern 1 / Code Example 3 | Undocumented API; psutil/wazuh production usage since 2019 with no breakage. If a future Windows changes it, fallback = PEB walk (needs PROCESS_VM_READ) or "—" rendering |
| A7 | Console detection via `AttachConsole` probe (attach → success = has console → FreeConsole) is reliable and side-effect-free for the TUI | Pattern 2 | Probe is done in the helper or a blocking scope; AttachConsole on a console-less process returns ERROR_INVALID_PARAMETER (87) — low risk |
| A8 | `WTD_CACHE_ONLY_URL_RETRIEVAL` flag choice for WinVerifyTrustEx (offline cache-first) | Code Example 4 | Blocks online revocation lookup; for a port tool, offline-first is correct (no hang on network). If revocation matters, use default flags |
| A9 | Status-bar overflow: hard-block message (127 chars + name) needs truncation preserving "Press w to review the whitelist" tail | (UI-SPEC ⚠ unresolved) | Planner MUST declare the truncation rule per UI-SPEC UI Considerations (tail-anchored, `…`); all 8 kill-outcome strings must fit ≤80 cols |

## Open Questions (RESOLVED)

1. **Built-in whitelist exact membership (Assumption A1).**
   - What we know: Microsoft's official critical-services list (smss/csrss/wininit/winlogon/services/lsass/System/svchost-RPCSS) + session/security additions; svchost.exe covers all instances (can't distinguish RPCSS-hosting instances by name alone).
   - What's unclear: Whether to include debatable entries (spoolsv, audiodg, explorer.exe — restarts itself; MsMpEng — PPL-protected anyway) to reach "~30".
   - Recommendation: ship Tier-1 (official, ~14) + Tier-2 session/security (~12–16) = 26–30; keep each with a plain-language reason (UI-SPEC row format); human-verify the constant at execution (checkpoint), document the reasoning for each entry in the source.
   - **RESOLVED:** Tier-1 + Tier-2 = 26–30 entries with plain-language reasons, shipped as `BUILTIN` in plan 02-01 Task 1 step 5; the final constant is human-verified at execution via the Task 1 human-check.

2. **User whitelist entry validation strictness (UI-SPEC backstop: "invalid/nonexistent path → error, not added").**
   - What we know: Entries are full paths; matching is normalized case-insensitive full-path compare.
   - What's unclear: Should an add require file-existence at add time? (A path may be valid later — e.g., app being updated; but the UI-SPEC backstop says nonexistent → error.)
   - Recommendation: require absolute-path syntax (drive/UNC prefix), strip quotes, resolve 8.3 via GetLongPathNameW when the file exists, and reject nonexistent paths per the approved UI-SPEC backstop (consistent, predictable).
   - **RESOLVED:** absolute-path syntax required, quotes stripped, 8.3 resolved via GetLongPathNameW when the file exists, nonexistent paths rejected per the UI-SPEC backstop — implemented as `validate_user_entry`/`normalize_user_entry` in plan 02-03 Task 1.

3. **Signature "Signed" vs "Trusted" semantics.**
   - What we know: WinVerifyTrust GENERIC_VERIFY_V2 with WTD_REVOKE_NONE returns 0 for any valid signature chain (including self-signed certs in the root store); TRUST_E_NOSIGNATURE for unsigned.
   - What's unclear: Whether "Signed" should mean "any signature" or "signed by a trusted publisher" (requires additional cert-chain trust check).
   - Recommendation: "any valid signature" (0) is the pragmatic D-07 scope; document the limitation in the detail panel copy — do not attempt trusted-publisher semantics this phase.
   - **RESOLVED:** "any valid signature" (WinVerifyTrust 0) is the D-07 scope; the limitation is documented in the detail-panel copy — implemented in plan 02-02 Task 1 (`verify_signature` scope comment).

4. **`svchost.exe` hard-block granularity.**
   - What we know: The requirement names svchost.exe as protected; all svchost instances share the name (basename match protects all).
   - What's unclear: A user might legitimately want to kill one svchost instance hosting a dev service; the hard-block forbids it entirely (no per-instance override exists in the model).
   - Recommendation: accept the coarse rule per D-09 (safety floor; the whitelist UI documents "all svchost instances"); note as a future refinement candidate (per-service protection) — do not build in Phase 2.
   - **RESOLVED:** coarse name-based rule accepted per D-09 (safety floor); the Help overlay note documents that all instances of a built-in name (e.g. svchost.exe) are protected (plan 02-03 Task 2); per-service protection remains a future refinement candidate — not built in Phase 2.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Windows 10/11 (Win32 APIs: kernel32/user32/ntdll/wintrust/toolhelp) | All Phase 2 features | ✓ | Windows 11 Pro (dev machine) | — |
| Rust toolchain + cargo | Build/test | ✓ | (workspace builds today) | — |
| Interactive desktop session | WM_CLOSE / Ctrl+C manual UAT | ✓ | — | Automated tests spawn real child processes instead |
| Elevated session | Killing elevated/system processes (D-03 path) | ✓ (via `a` key UAC) | — | Access-denied flow is the designed non-admin path |
| External services (WMI service etc.) | None — all APIs are in-process | N/A | — | — |

**Missing dependencies with no fallback:** none — this phase is entirely code/config changes over OS-provided APIs.
**Missing dependencies with fallback:** none.

## Validation Architecture

> `workflow.nyquist_validation: true` (config.json) — section required.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`#[cfg(test)]` + `tests/` integration dir) — no new test deps |
| Config file | none — standard cargo layout (Phase 1 precedent: inline `#[cfg(test)]` in `port-core/src/filter.rs`) |
| Quick run command | `cargo test -p port-core` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROC-01 | Kill owning process from port list | integration (spawn real child `cmd.exe /c ping -t 127.0.0.1`, kill via core API, assert exit) + manual UAT | `cargo test -p port-core --test kill_integration` | ❌ Wave 0 |
| PROC-02 | Escalation routing + timeout + force | unit: `route_strategy` pure fn (windows/console/neither → WmClose/CtrlC/ForceDirect); integration: WM_CLOSE on a real GUI-ish child (spawn `notepad`-free approach: any process with a window), timeout→force on a signal-ignoring child | `cargo test -p port-core kill::strategy` | ❌ Wave 0 |
| PROC-03 | Instant default vs confirm gate | unit: `protection_status` matrix (built-in / user-listed / none → HardBlocked / UserConfirm / None); assert no dialog for None at TUI level (manual UAT) | `cargo test -p port-core process::whitelist` | ❌ Wave 0 |
| PROC-04 | Built-in hard block (~30 entries) | unit: list completeness (≥25 entries, lowercase names, unique), PID 0/4 special case, basename match, reason present per entry | `cargo test -p port-core process::whitelist` | ❌ Wave 0 |
| PROC-05 | User whitelist add/remove + path match | unit: settings serde round-trip (defaults on old TOML), path normalization (quotes, case, 8.3 via mock), full-path vs basename distinction | `cargo test -p port-core config::settings` | ❌ Wave 0 |
| PROC-06 | Detail fields (path/start/cmdline/signature/parent) | unit: FILETIME→SystemTime conversion, UNICODE_STRING extraction (defensive bounds), cmdline parse; integration: details() on current process (`std::env::current_exe`) asserting all fields populate | `cargo test -p port-core process::info` | ❌ Wave 0 |
| PROC-07 | Handle retention + creation-time verification | unit: verify logic (pure FILETIME compare, GetProcessId mismatch); integration: rapid spawn/kill churn (10 iterations of spawn+kill different processes with reused PIDs where possible), assert no wrong-process kill | `cargo test -p port-core --test process_handle_integration` | ❌ Wave 0 |
| SRCH-02 | AND/OR filter combination | existing filter.rs tests cover AND-across/OR-within (Phase 1) — no new tests; traceability update only | `cargo test -p port-core filter` (existing) | ✅ exists |
| TUI surfaces (UI-SPEC) | Overlay render states, key mapping | manual UAT (end-of-phase human verify per config) — Ratatui rendering has no headless test infra in Phase 1 | manual | — |

### Sampling Rate
- **Per task commit:** `cargo test -p port-core`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd-verify-work`; UAT per human_verify_mode (end-of-phase)

### Wave 0 Gaps
- [ ] `port-core/src/process/whitelist.rs` tests — built-in list contract (PROC-04), `user_match` normalization (PROC-05)
- [ ] `port-core/src/process/kill.rs` tests — `route_strategy` pure routing, outcome mapping (PROC-02)
- [ ] `port-core/src/process/handle.rs` tests — FILETIME verification logic (PROC-07)
- [ ] `port-core/src/process/info.rs` tests — FILETIME→SystemTime, UNICODE_STRING extraction (PROC-06)
- [ ] `port-core/tests/kill_integration.rs` — spawn real child processes, exercise WM_CLOSE/Ctrl+C/force against them (Windows-gated, interactive-session)
- [ ] `port-core/src/config/settings.rs` tests — serde defaults for `whitelist` + `kill_timeout_secs` on a Phase-1-era TOML fixture (backward compat proof)
- [ ] No framework install needed — cargo test built-in

## Security Domain

> `security_enforcement: true` (config.json), ASVS Level 1.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Local single-user tool, no auth |
| V3 Session Management | no | No sessions |
| V4 Access Control | partial | Whitelist is a local safety policy, not an access-control boundary; enforcement is in-process only (D-09 hard block + confirmation) |
| V5 Input Validation | yes | Whitelist path input: absolute-path syntax check, control-char rejection, length cap, quote trimming (settings.rs + whitelist overlay add path); no shell interpolation anywhere (Ctrl+C helper takes a numeric PID only) |
| V6 Cryptography | no | No crypto; signature verification is display-only (WinVerifyTrust), never an authorization gate |

### Known Threat Patterns for {Windows process management stack}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| PID reuse → wrong process killed | Spoofing / Integrity | `ProcessSnapshot` creation-time verification inside one spawn_blocking scope (Pitfall #1, PROC-07) |
| Protected-process kill attempt silently failing | — | Built-in whitelist check BEFORE OpenProcess; every TerminateProcess result mapped to a KillOutcome (Pitfall #11); never swallow errors |
| UIPI bypass attempts (non-admin killing elevated) | Elevation of Privilege | Impossible by design — cross-IL PostMessage/TerminateProcess is OS-blocked; the D-03 AccessDenied flow is the sanctioned path (user-initiated `a` elevation) |
| Whitelist bypass via path trickery (8.3 names, case, trailing `\`) | Tampering | Normalized case-insensitive full-path compare; GetLongPathNameW for 8.3; strip quotes/trailing separators (user_match tests lock the contract) |
| Command-line secrets exposed in detail panel | Information Disclosure | Inherent to PROC-06 (user-initiated local view only, no persistence, no export); note in Help overlay copy that command lines may contain secrets (PITFALLS.md Security Mistakes) — no redaction this phase |
| `--ctrl-c <pid>` helper abuse (arbitrary pid via CLI) | Spoofing | The helper only generates a console event — it cannot kill; worst case it signals an unrelated console process; hidden flag, numeric pid parsing, exit-code contract |
| Malware masquerading as system processes | Spoofing | Built-in whitelist matches by name/PID only (D-10 locked); defense-in-depth: PPL on csrss/lsass etc. blocks termination anyway; document in whitelist overlay that name-based matching is heuristic for the non-PPL subset |

## Sources

### Primary (HIGH confidence)
- **windows-rs 0.62.2 local cargo registry source** (`~/.cargo/registry/src/index.crates.io-*/windows-0.62.2/`) — verified signatures: OpenProcess/TerminateProcess/QueryFullProcessImageNameW/GetProcessTimes/GetProcessId/WaitForSingleObject/GetExitCodeProcess (Win32::System::Threading); EnumWindows/GetWindowThreadProcessId/PostMessageW/WM_CLOSE (Win32::UI::WindowsAndMessaging); CreateToolhelp32Snapshot/Process32FirstW/Process32NextW/PROCESSENTRY32W/TH32CS_SNAPPROCESS (Win32::System::Diagnostics::ToolHelp); AttachConsole/FreeConsole/GenerateConsoleCtrlEvent/SetConsoleCtrlHandler/CTRL_C_EVENT (Win32::System::Console); WinVerifyTrust/WinVerifyTrustEx/WINTRUST_DATA/WINTRUST_FILE_INFO/WINTRUST_ACTION_GENERIC_VERIFY_V2 (Win32::Security::WinTrust); access-right constants; HANDLE `!Send` (repr(transparent) `*mut c_void`, no Send/Sync impls); FILETIME struct
- **sysinfo 0.39.6 registry source** — `start_time() -> u64` seconds precision (too coarse for PID-reuse verification), parent() exists (fallback)
- **Microsoft Learn — Critical System Services (Restart Manager)** — https://learn.microsoft.com/en-us/windows/win32/rstmgr/critical-system-services — canonical "never kill" process list
- **02-CONTEXT.md / 02-UI-SPEC.md / REQUIREMENTS.md / ROADMAP.md / STATE.md / PITFALLS.md / ARCHITECTURE.md** — project-locked decisions, UI contract, pitfall mandates
- **Codebase inspection** — port-core/src/process.rs (trait stub), models/process.rs (ProcessInfo), config/settings.rs (AppSettings), scanner (resolver/tcp/udp, module layout), port-tui (main.rs key map, app.rs state, update.rs, elevate.rs, components, theme.rs)

### Secondary (MEDIUM confidence)
- Stack Overflow: "GenerateConsoleCtrlEvent won't shutdown console application" + ctrlc-windows (thefrontside) — console signal flakiness and helper pattern
- Stack Overflow / psutil PR #1398 (giampaolo/psutil) + wazuh PR #34727 — NtQueryInformationProcess class 60 command line
- docs.rs source views (faithe, vibesurfer) — Toolhelp32 usage pattern (dwSize init, null-terminator handling)
- Mozilla Bugzilla #1816848 / codesigned crate — WinVerifyTrust data-structure setup, WTD_STATEACTION_CLOSE cleanup
- habitat/core os/process/windows.rs — OpenProcess minimal-rights pattern (QUERY_LIMITED|TERMINATE|SYNCHRONIZE)
- apache kogito-benchmarks CtrlC.exe — console signal helper reference

### Tertiary (LOW confidence — flagged for validation)
- General web results on WM_CLOSE best practice (STAF automation docs, python-win32 mailing lists) — corroborate the EnumWindows pattern but no authoritative single source
- zhihu/superuser process-list articles — supplementary for the extended whitelist (Assumption A1, human-verify at execution)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all API signatures verified against local windows-rs 0.62.2 source; zero new crates
- Architecture: MEDIUM — patterns grounded in verified APIs + cited production practice; helper-process and snapshot designs are recommendations under the locked decisions
- Pitfalls: MEDIUM — console-dance and PID-reuse pitfalls documented by multiple independent sources; runtime behavior (UIPI, PPL) is OS-enforced and well-established

**Research date:** 2026-07-31
**Valid until:** 2026-08-30 (stable Win32 APIs; windows-rs 0.62 pinned by STATE.md — re-verify only if the crate is upgraded)

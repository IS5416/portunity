---
phase: 02-process-management-smart-kill
plan: 02
subsystem: process-management
tags: [detail-panel, process-info, signature-verify, protection-markers, tui]
type: execute
status: complete
requires:
  - "02-01 (ProcessSnapshot, whitelist, kill pipeline, x-key surface)"
provides:
  - "port-core::process::info (fetch_details, verify_signature, filetime_to_systemtime, extract_unicode_string)"
  - "port-core::scanner::apply_protection_postpass (scan-time ◆ markers)"
  - "port-tui d-key detail panel (12-row overlay, signature cache, exited detection)"
  - "port-tui ◆ markers + whitelist dimming + strikethrough in ports table"
affects:
  - port-core/src/process.rs
  - port-core/src/process/handle.rs
  - port-core/src/process/info.rs
  - port-core/src/scanner.rs
  - port-tui/src/{main,app,message,update,components}.rs
  - port-tui/src/components/{detail_panel,ports}.rs
  - Cargo.toml
key-files:
  created:
    - port-core/src/process/info.rs
    - port-tui/src/components/detail_panel.rs
  modified:
    - port-core/src/process.rs
    - port-core/src/process/handle.rs
    - port-core/src/scanner.rs
    - port-tui/src/main.rs
    - port-tui/src/app.rs
    - port-tui/src/message.rs
    - port-tui/src/update.rs
    - port-tui/src/components.rs
    - port-tui/src/components/ports.rs
    - Cargo.toml
decisions:
  - "Scan-time markers via one spawn_blocking post-pass with a path_for closure — the marker logic is unit-testable without live Windows processes (5 pure tests); production wiring passes info::query_full_path"
  - "No-path signature rows insert None into the cache — 'Unknown' renders instead of a stuck 'Verifying…' (Rule 2 add during Task 2)"
  - "fetch_details fills name from the resolved path basename; empty name falls back to the caller's row name at the TUI layer (spawn_detail_fetch)"
  - "ProcessManager::details() now delegates to info::fetch_details — resolves the plan 02-01 Known Stub; snapshot creation-time is NOT reused (D-08: panel never caches the FILETIME)"
  - "WinVerifyTrustEx data structs are gated behind Win32_Security_Cryptography in windows-rs 0.62.2 — added the feature flag (zero new crates, T-02-SC); the WinTrust feature alone does not enable WINTRUST_DATA"
  - "DetailDataLoaded drain special-case gates on detail_pid match — a stale fetch result (selection moved on) is dropped, never rendered over newer data"
tech-stack:
  added:
    - "windows feature Win32_Security_Cryptography (enables WINTRUST_DATA/WINTRUST_FILE_INFO/WinVerifyTrustEx)"
  patterns:
    - "Manual ntdll FFI (#[link(name = 'ntdll')]) for NtQueryInformationProcess class 60 — two-call, read_unaligned UNICODE_STRING, bounds-check Buffer against the allocation before dereference (T-02-09)"
    - "Per-field Option fetchers render '—' — never a whole-panel Err (UI-SPEC Detail Panel error state)"
    - "Signature verification: drain-loop cache-miss spawn (update() cannot spawn), per-PID cache cleared on ScanComplete (D-07, T-02-07)"
    - "ProcessExited detection in the ScanComplete drain arm: detail_pid absent from the new connection list"
    - "Right-segment-preserving path truncation (…\\dir\\name.exe) + U+2026 right-truncation for command line (UI-SPEC overflow rules)"
metrics:
  duration: "~1.5h execution (started 2026-08-01T02:14:02Z)"
  tasks: 2
  commits: 4 (a382549, 70949b2, 86714a0, fe48047)
  tests: 76 passing (59 unit + 5 kill integration + 1 churn + 11 TUI)
actuals:
  tokens: 18000   # chars/4 over the ~71.9k-char realized diff
  tasks: 2
  commits: 4
---

# Phase 2 Plan 2: Process Detail Inspection — Core Fetchers + Detail Panel + Protection Markers Summary

One-liner: full PROC-06 detail stack — port-core fetchers (path, command line via ntdll class 60, start time via creation FILETIME, parent PID via Toolhelp32, digital signature via WinVerifyTrustEx) with per-field failure tolerance, a scan-time protection marker post-pass feeding the ◆ table markers, and the d-key 12-row detail panel overlay with on-demand signature verification and exited-process strikethrough.

## What Was Built

**Core (port-core, Task 1 — TDD RED a382549 / GREEN 70949b2):**

- `process/info.rs` — per-field Option fetchers (a failure renders "—", never a whole-panel error):
  - `filetime_to_systemtime(FILETIME)` — pure helper; epoch anchor test (116444736000000000 → 1970-01-01T00:00:00Z) + 2026 round-trip through the chrono `%H:%M:%S %d-%b-%Y` display ("09:41:12 31-Jul-2026").
  - `extract_unicode_string(&[u16], u32)` — defensive bounds: claimed length beyond the allocation → `None` (T-02-09, no OOB read); trailing NULs trimmed, embedded NULs preserved.
  - `fetch_details(pid)` — one `spawn_blocking` scope (Pitfall #9): QLI handle → `QueryFullProcessImageNameW` path (buffer retry) → `GetProcessTimes` creation FILETIME → start time (the same identity the kill re-verifies fresh via `snapshot_for(pid)` at kill time, D-08/PROC-07 — the panel never caches the FILETIME) → `NtQueryInformationProcess` class 60 command line (manual ntdll link, two-call pattern, `read_unaligned` UNICODE_STRING with bounds-check-before-dereference, works elevated with QLI-only) → Toolhelp32 parent PID (`dwSize` set before `Process32FirstW`, snapshot handle RAII-closed) → protection markers via `protection_status` (built-in first, Pitfall #6).
  - `verify_signature(path)` — WinVerifyTrustEx with `WTD_CACHE_ONLY_URL_RETRIEVAL` (A8 offline cache-first), 3-way verdict (0 → Signed, TRUST_E_NOSIGNATURE → Unsigned, other → Unknown), `WTD_STATEACTION_CLOSE` cleanup (Pitfall 5). Scope comment documents "Signed" = any valid chain, not trusted-publisher semantics (Open Question 3).
- `scanner.rs` — `apply_protection_postpass(conns, settings, path_for)`: built-in basename → `is_system_critical`; user-entry basename → full-path query → `user_match` → `user_protected`. Wired into `scan_all()` in one spawn_blocking scope with fresh `load_settings()` (D-15); the closure keeps the marker logic pure-testable (5 tests). Keeps `filter.rs`'s `system_only` truthful and feeds the ◆ markers at render time (O(1) per row).
- `process.rs` — `pub mod info` + re-exports; `WindowsProcessManager::details()` now delegates to `info::fetch_details` (resolves the 02-01 Known Stub, committed separately as fe48047).
- `handle.rs` — `open_with`/`query_full_process_image_name` widened to `pub(crate)` for info.rs reuse (no duplicated Win32 code).
- `Cargo.toml` — workspace windows features += `Win32_Security_Cryptography` (WINTRUST_DATA and friends are gated behind it in 0.62.2; zero new crates, T-02-SC).

**TUI (Task 2, 86714a0):**

- `d` key (Ports tab) toggles the 12-row top-anchored Clear-over detail panel; `d`/`Esc` close while open, everything else (j/k/up/down/r/s/g/G//f/x) passes through to the live table (D-06 pass-through per UI-SPEC).
- `DetailPanelComponent` — full UI-SPEC internal layout: `{name} — PID {pid}` Bold title with protection badge (`[PROTECTED]` status.error / `[CONFIRM]` status.warning) and trailing `[Esc]`; Status Running|Exited; Owning port from the selected row; Executable path (tail-preserving `…\dir\name.exe` truncation); Command line (U+2026 right-truncate); Start time (chrono format); Parent PID; Signature (Verifying…/Signed/Unsigned/Unknown); Protection; Reason (only when protected, UI-SPEC copy verbatim); accent_secondary kill hint (instant / confirmation required / system-critical); U+2500 bottom border. States: no-selection copy, loading (name/PID immediate, fields "Loading details…"), exited (strikethrough SGR 9 + Status "Exited"), per-field "—" dim.
- Fetch flow (D-08): `ToggleDetailPanel` and movement intercepts spawn `fetch_details` (row ProcessInfo is the error fallback — name/PID/markers kept, detail fields "—"); drain-loop `DetailDataLoaded` arm gates on `detail_pid` match (stale results dropped) and fires the signature verification on cache miss.
- Signature (D-07): per-PID cache in `App::signature_cache`, populated on `SignatureVerified`, cleared on every `ScanComplete` (drain arm); no-path rows insert `None` → "Unknown" (never stuck on "Verifying…").
- `ProcessExited { pid }` fires from the ScanComplete drain arm when `detail_pid` leaves the new connection list → strikethrough + "Exited".
- `ports.rs` — ◆ marker (U+25C6) in `status.error` (built-in) / `status.warning` (user); protected rows dim only when non-admin; `Modifier::CROSSED_OUT` on the process cell when `pid == last_killed_pid` (pending scan removal). `SYSTEM_NAMES` const + `is_system_process()` heuristic deleted (superseded — `grep -c SYSTEM_NAMES == 0`).
- Detail footer: `[Esc]Close [j/k]Next port [x]Kill [r]Refresh  —  detail for {name}` with name budget `term_width − 66` (UI-SPEC footer table).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Test-data encoding error in extract_unicode_string RED tests**
- **Found during:** Task 1 GREEN verification
- **Issue:** The RED test arrays encoded byte-array representations (`[97, 0, 98, 0, 0, 0]`) into `u16` buffers — the wide-string layout for L"ab" is `[97, 98, 0]`, not `[97, 0, 98, 0, 0, 0]`. Three tests asserted wrong expectations against the correct implementation.
- **Fix:** Corrected the test buffers to true wide-char arrays (L"ab" + terminator, L"a\0b" + terminator).
- **Files modified:** port-core/src/process/info.rs (tests)
- **Commit:** 70949b2 (part of GREEN)

**2. [Rule 2 - Missing functionality] No-path signature rows stuck on "Verifying…"**
- **Found during:** Task 2 implementation review
- **Issue:** A `DetailDataLoaded` with `executable_path: None` never triggered the signature spawn, leaving the cache empty and the Signature row on "Verifying…" forever.
- **Fix:** The drain arm inserts `None` into the cache for pathless rows → renders "Unknown" (UI-SPEC Unknown state).
- **Files modified:** port-tui/src/main.rs
- **Commit:** 86714a0

**3. [Rule 3 - Blocking] WinTrust structs gated behind an unlisted windows feature**
- **Found during:** Task 1 GREEN compile
- **Issue:** `WINTRUST_DATA`, `WINTRUST_FILE_INFO`, `WinVerifyTrustEx` in windows-rs 0.62.2 are `#[cfg(feature = "Win32_Security_Cryptography")]`-gated; the workspace enabled `Win32_Security_WinTrust` but not the Cryptography feature, so the imports failed to resolve. RESEARCH's "two feature flags" list did not name this one.
- **Fix:** Added `Win32_Security_Cryptography` to the workspace `windows` entry — a feature flag on the already-audited crate (zero new crates, T-02-SC posture unchanged).
- **Files modified:** Cargo.toml
- **Commit:** 70949b2

### Deviations from Plan Text

- `user_protected` model field + all 5 struct-literal constructor sites were already shipped in plan 02-01 (the field landed with the kill pipeline) — plan 02-02 Task 1 steps 3-4 were no-ops; the SUMMARY documents this rather than re-editing identical code.
- Task 1 TDD: the scanner post-pass tests were folded into the RED commit (the plan's TDD gate is per-task; the post-pass behavior belongs to Task 1).
- `ProcessManager::details()` wiring (fe48047) — the plan did not list it, but the 02-01 Known Stub explicitly deferred it to "info.rs lands in plan 02-02"; leaving it stubbed would strand a documented contract.
- The plan's "spawn_blocking { fetch_details(pid) }" wording is implemented as `tokio::spawn(async … fetch_details(pid).await)` — fetch_details is itself async and owns its spawn_blocking scope; nesting a blocking scope around an await would deadlock the worker thread.

## Auth Gates

None — no authentication was required at any point.

## Known Stubs

None — the two plan 02-01 stubs (`WindowsProcessManager::details()`, now wired to `info::fetch_details`; `Message::KillExecute`, still declared-for-future-use by design with `#[allow(dead_code)]`) are either resolved or intentional. No placeholder data sources, no empty-value rendering stubs in the detail panel.

## Threat Flags

None — no security surface beyond the plan's threat_model. The ntdll manual link (T-02-SC) is a `#[link]` to the OS, not a new crate; the WinTrust feature flag addition is configuration-only; command lines remain display-only with no persistence (prohibition P4 — verified: no write path touches `command_line` or `executable_path` outside the in-memory panel render).

## Verification

- `cargo test -p port-core process::info` — 8 tests green (FILETIME epoch + 2026 chrono display, UNICODE_STRING bounds ×4, verify_signature verdict on current_exe, fetch_details populates self — real Win32 calls against the test binary)
- `cargo test -p port-core filter` — 8 tests green (SRCH-02 untouched)
- `cargo test -p port-core scanner` — 5 post-pass tests green (builtin marker, user marker, builtin-wins-over-user Pitfall #6, PID 4, unmatched-unchanged, same-name-different-path)
- `cargo test --workspace` — 76 tests green (59 unit + 5 kill integration + 1 churn + 11 TUI); the "Unrecognized option: 'ctrl-c'" stderr lines are the expected 02-01 helper-binary noise
- `cargo build --workspace` — clean, zero warnings
- `grep -c 'SYSTEM_NAMES\|is_system_process' port-tui/src/components/ports.rs` == 0 (heuristic deleted)
- TDD gate compliance: `test(02-02)` RED commit a382549 precedes `feat(02-02)` GREEN commit 70949b2 in git log
- Manual UAT deferred to end-of-phase per `human_verify_mode=end-of-phase` (panel render states, d/Esc key mapping, ◆ marker colors, non-admin dimming, exited strikethrough — interactive terminal session required)

## Self-Check: PASSED

- Files exist: port-core/src/process/info.rs, port-tui/src/components/detail_panel.rs ✓
- Commits exist: a382549 (RED), 70949b2 (GREEN), 86714a0 (TUI), fe48047 (details wiring) ✓
- 76 workspace tests green; zero warnings; SYSTEM_NAMES grep == 0 ✓

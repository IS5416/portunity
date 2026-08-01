---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: in_progress
stopped_at: Completed 02-02-PLAN.md (process detail inspection)
last_updated: "2026-08-01T02:32:15.968Z"
progress:
  total_phases: 2
  completed_phases: 1
  total_plans: 7
  completed_plans: 6
current_phase_name: Process Management & Smart Kill
---

# Project State: Portunity

**Project:** Windows Port Management Tool
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.
**Current Focus:** Phase 2 — Process Management & Smart Kill

## Current Position

| Field | Value |
|-------|-------|
| **Milestone** | 1 |
| **Current Phase** | 2 — Process Management & Smart Kill (1/3 plans) |
| **Current Plan** | 02-02 complete (detail fetchers + panel + markers). Next: 02-03 (whitelist + Help overlay) |
| **Phase Status** | In progress |
| **Progress** | `[██░░░░░░░░░░░░░░░░]` 1/6 phases complete |

## Accumulated Context

### Decisions Made

| Decision | Phase | Rationale |
|----------|-------|-----------|
| Single Cargo workspace (root) | 1 | Prevents double compilation (Pitfall #10); three members: port-core, port-tui, port-gui/src-tauri |
| SQLite WAL mode from first connection | 1 | Required for dual-frontend concurrent access (Pitfall #12) |
| `port-core` async-first API (all Win32 calls wrapped in `spawn_blocking`) | 1 | Prevents async runtime stalls (Pitfall #9) |
| TUI before GUI build order | 1, 5 | TUI validates core API surface with zero Tauri/Svelte toolchain complexity; if API is awkward, discovered in Phase 3 not Phase 5 |
| ProcessHandle struct (PID + HANDLE + creation time) over bare PID | 2 | Prevents PID reuse race condition killing wrong process (Pitfall #1) |
| ETW as change-notification trigger only; `GetExtendedTcpTable` as ground truth | 3 | ETW PID has ~30% inaccuracy rate (Pitfall #15); polling APIs provide authoritative data |
| Instant kill default + whitelist-gated confirmation | 2 | Most kills are intentional (dev stopping own server); whitelist protects system processes |
| Tab-based widget dashboard (TEA architecture) | 1 | 5 distinct function domains need dedicated canvas; TEA provides centralized state management |
| Trait-based platform abstraction in port-core | 1 | Linux/macOS are explicit extension points (out of scope for v1) |
| All business logic exclusively in port-core | All | Both frontends are thin adapters; anti-pattern prevention (ARCHITECTURE.md Anti-Pattern #1) |
| windows crate v0.62 (not v0.73) | 1 | v0.73 not yet published on crates.io (July 2026). API differences: GetExtendedTcpTable returns u32 error codes, MIB_TCP_STATE is newtype, ntohs is unsafe |
| New-style Rust module layout | 1 | Per CLAUDE.md rule ("No mod.rs files"). scanner.rs + scanner/tcp.rs (not scanner/mod.rs). Reverted plan's mod.rs references. |
| Plan 01-01 walking skeleton complete | 1 | Workspace compiles, Windows TCP scan works, TUI renders live port table with state colors. 3 commits: a563e30, 5e0ab4a, 70ef1fe. |
| Exponential buffer retry (D-01) | 1 | GetExtendedTcpTable/GetExtendedUdpTable: start 16KB, double on ERROR_INSUFFICIENT_BUFFER, max 3 retries, max 128KB |
| Dual-stack enumeration (D-02) | 1 | AF_INET + AF_INET6 tables merged into unified view; IPv4-mapped IPv6 duplicates dropped, AF_INET kept canonical |
| Error resilience (D-03) | 1 | Scan failure preserves last successful data; red error bar with "Press r to retry" |
| Concurrent TCP+UDP scanning (D-04) | 1 | tokio::join! in scan_all() runs TCP and UDP simultaneously; wall-clock = max(TCP, UDP) |
| Batch process name resolution (D-16) | 1 | ProcessResolver with HashMap<u32, String> cache; batch-resolved via sysinfo after scan |
| Full connection state color map (SCAN-03) | 1 | 11 TCP states + UDP mapped to semantic color slots per UI-SPEC One Dark palette |
| Sort cycle interaction (SCAN-04) | 1 | 's' key toggles: none → ascending(▲) → descending(▼) → none on current column |
| Virtual scrolling (TUI-04) | 1 | Viewport-only Row rendering with right-edge scrollbar (█ thumb, │ track) |
| Auto-refresh (D-11) | 1 | 5-second background scan; guarded: only when not scanning and no error active |
| State column text labels | 1 | User feedback: "● LISTEN", "○ T_WAIT", "◉ C_WAIT", "— UDP" — color + text, not color alone |
| Filter engine: free functions over trait | 1 | Filtering has no platform-specific variants; free functions are simpler and avoid unnecessary abstraction |
| Filter: Vec-based dimensions with AND/OR logic | 1 | AND across dimensions, OR within Vec fields. More powerful than single-value design while matching existing Filter struct |
| Fuzzy search: simple substring | 1 | Concatenated field substring match (not Levenshtein). Full fuzzy (typo-tolerant) deferred to Phase 6 with search history/ranking |
| Non-modal search/filter overlays | 1 | Search bar and filter panel are soft overlays, not blocking modals. Keyboard navigation of port table continues while overlays are visible |
| Admin elevation: ShellExecuteExW runas | 1 | D-06: Triggers UAC prompt. Old process exits immediately (D-08: no state transfer). User decline continues in non-admin (D-07) |
| System process detection heuristic | 1 | PID < 1000 OR name in known set. Cosmetic dimming only — not access control. Full whitelist in Phase 2 |
| windows-rs v0.62 API adaptation for elevation | 1 | ShellExecuteExW returns Result<()>, SHELLEXECUTEINFOW uses Anonymous union, IsUserAnAdmin in Win32::UI::Shell, SW_SHOW needs WindowsAndMessaging feature |
| ProcessSnapshot (PID + creation FILETIME + path) over bare PID for kill identity | 2 | HANDLE is !Send — pure-data snapshot crosses the mpsc channel; handle opened+verified+acted on inside one spawn_blocking scope (PROC-07, Pitfall #1) |
| BUILTIN whitelist = 25 entries (Restart Manager Tier-1 14 + Tier-2 11) | 2 | A1 human-verified at checkpoint: explorer.exe deliberately excluded, securesystem typo fixed. HardBlocked tier checked BEFORE OpenProcess (Pitfall #11) |
| Two-tier protection: HardBlocked (built-in) / UserConfirm (user path whitelist) | 2 | D-09/D-10: built-in by basename+PID, user by normalized full path; built-in wins when both match (Pitfall #6) |
| Ctrl+C helper self-reexec (`--ctrl-c <pid>`, hidden clap flag) | 2 | D-02/Pitfall 7: helper does the FreeConsole/AttachConsole dance — never the TUI process; exit 0=delivered/1=no console; live-verified delivers Ctrl+C to CREATE_NEW_CONSOLE child |
| Win32 error mapping: HRESULT 0x8007XXXX → low-16-bit code | 2 | windows-rs Error.code() is HRESULT; masking low 16 bits is required for ERROR_ACCESS_DENIED (5) → AccessDenied (D-03) |
| HardBlocked status copy: compact form at ≤80 cols | 2 | A9 declared: "✗ {name} … Press w to review the whitelist." (name budget term_width−41) — full 127+ char form never fits the 80-col gate |
| Plan 02-01 smart kill complete | 2 | Tracer 511da68 + fixes fa8a682/05e9d89 + TUI cd80ccc. 62 tests green. x-key kill, confirm dialog, status outcomes, auto-refresh, whitelist gate |
| 5-tab dashboard with tab bar highlighting | 1 | active_tab: usize (0-based, 0=Overview default per D-14), per-tab Component trait dispatch, tab bar: Bold+accent_primary bg for active, Dim+fg_muted for inactive |
| Resize gate: 80x24 minimum terminal | 1 | frame.area() checked at start of render_app(); below threshold renders centered "Terminal too small" message, hides normal layout (TUI-07) |
| Placeholder tab pattern | 1 | History/Traffic/Firewall tabs render centered "Coming later" message with "Press 1 or 2 to view active tabs" nav hint; content deferred to Phases 3/4 |
| Release build profile | 1 | LTO + single codegen unit + strip symbols + opt-level=3; binary size: 1.1MB (target <10MB) |
| SKELETON.md architectural record | 1 | 227-line comprehensive document covering all 20+ Phase 1 decisions, data flow, crate graph, module map, pitfall coverage, out-of-scope map, subsequent slice plan |

| Plan 02-02 detail inspection complete | 2 | PROC-06 detail fetchers (path via QueryFullProcessImageNameW, cmdline via ntdll class 60 two-call, start via creation FILETIME, parent via Toolhelp32, signature via WinVerifyTrustEx) + scan-time ◆ post-pass + d-key 12-row panel; 76 tests green; TDD RED a382549 / GREEN 70949b2 / TUI 86714a0 / details-wiring fe48047 |
| WinVerifyTrust 3-way verdict + per-PID cache cleared per scan | 2 | D-07: 0→Signed, TRUST_E_NOSIGNATURE→Unsigned, other→Unknown; cache invalidated on ScanComplete (T-02-07); no stale signature displayed |
| Detail start time shares the kill identity mechanism (D-08) | 2 | Panel never caches the creation FILETIME; the kill re-captures fresh via snapshot_for(pid) at kill time — no stale identity verified (PROC-07) |
| windows features += Win32_Security_Cryptography | 2 | WINTRUST_DATA/WinVerifyTrustEx gated behind it in windows-rs 0.62.2; zero new crates (T-02-SC) |

### Key TODOs across phases

- [x] **Phase 1:** Set up single Cargo workspace; implement dual-stack TCP/UDP scanner with buffer retry; build Ratatui Elm Architecture main loop with VirtualTable — COMPLETE
- [ ] **Phase 2:** ProcessSnapshot safety wrapper + built-in whitelist (25 entries) + smart kill escalation + x-key TUI kill surface — 02-01 DONE; detail fetchers + 12-row detail panel + protection markers — 02-02 DONE; remaining: 02-03 whitelist overlay + Help overlay
- [ ] **Phase 3:** Integrate ferrisetw with startup orphan cleanup; build lock-free callback-to-async bridge; implement SQLite history append-only log
- [ ] **Phase 4:** Build COM firewall rule CRUD abstraction; implement right-click quick actions; implement JSON/CSV export
- [ ] **Phase 5:** Register Tauri commands wrapping port-core APIs; build system tray with popup; implement Svelte reactive stores
- [ ] **Phase 6:** Build auto-label lookup table (50+ ports); implement Fluent i18n engine; create 6 theme preset TOML files

### Blockers

(None — Phase 1 complete, ready to plan Phase 2)

## Performance Metrics

| Metric | Current | Target | Phase |
|--------|---------|--------|-------|
| Port scan time (1000 ports) | TBD | < 500ms | 1 |
| Memory (idle, TUI) | TBD | < 50MB | 1 |
| TUI scroll latency (5000 rows) | TBD | < 50ms | 1 |
| ETW events dropped | N/A | 0 under 100 conn/sec | 3 |
| Binary size (TUI, release) | 1.1MB | < 10MB stripped | 1 | Actual: 1.1MB — well under 10MB target |
| SQLite concurrent access | N/A | No BUSY errors, dual-frontend | 3 |
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 02-process-management-smart-kill P02-01 | 2.5h | 2 tasks | 18 files |
| Phase 02-process-management-smart-kill P02-02 | 1.5h | 2 tasks | 12 files |
| Phase 02-process-management-smart-kill P02 | 1.5h | 2 tasks | 12 files |

## Session Continuity

**Last session:** 2026-08-01T00:00:00Z
**Stopped at:** Completed 02-01-PLAN.md (smart kill core + TUI surface)
**Resume file:** .planning/phases/02-process-management-smart-kill/02-01-SUMMARY.md

- **Last action:** Plan 02-02 executed — 4 commits (a382549 RED tests, 70949b2 info.rs + protection post-pass, 86714a0 TUI detail panel, fe48047 details() wiring). 76 tests green; TDD gates honored; 02-01 details() stub resolved and ledger entry 5 marked fixed.
- **Next action:** Execute plan 02-03 (whitelist overlay + Help overlay); end-of-phase manual UAT per human_verify_mode
- **Research flags:** Phase 3 (ETW event schemas), Phase 5 (Tauri system tray), Phase 6 (windows-wfp API, SQLite FTS5)

---

*State initialized: 2026-07-26*
*Last updated: 2026-07-31 after Phase 1 completion*

---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: in_progress
stopped_at: Agent takeover: GSD-lite mode; Phase 2 UAT deferred (known risk); review groundwork in flight
last_updated: "2026-08-20T09:38:12.000Z"
progress:
  total_phases: 2
  completed_phases: 1
  total_plans: 7
  completed_plans: 7
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
| **Current Phase** | 2 — Process Management & Smart Kill (3/3 plans) |
| **Current Plan** | 02-03 complete (whitelist overlay + Help overlay + SRCH-02 traceability). All Phase 2 plans done — end-of-phase UAT pending per human_verify_mode |
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
| Whitelist overlay (w key): validate+save off-runtime, working copy on the main loop | 2 | PROC-05/D-15: WhitelistAdd spawn_blocking validate (normalize, 8.3 via GetLongPathNameW, existence) + save_settings; delete mutates the working copy first, persists post-removal state; duplicate add = case-insensitive no-op (PROC-05); WhitelistSaved {added:false} disambiguated by presence — duplicate info string vs Removed string |
| Whitelist input validation backstop | 2 | T-02-02: absolute-path syntax (drive/UNC), control-char rejection, ONE quote pair strip, trailing-separator strip, 4096 cap, existence check → "Path does not exist" error, entry not added (UI-SPEC backstop) |
| windows features += Win32_Storage_FileSystem | 2 | GetLongPathNameW/GetShortPathNameW (8.3 resolution) gated behind it in windows-rs 0.62.2; zero new crates (T-02-SC), same precedent as Cryptography flag |
| Help overlay (? key) as canonical key reference | 2 | Full keyboard contract incl. footer-dropped s/w + y/n confirm; secrets note (P4) + name-based-matching note (T-02-06); renders above whitelist, below confirm; full-area swallow with confirm dispatch earlier |
| SRCH-02 verified + traced Complete | 2 | Additive OR-within-Vec ∧ AND-across-dimensions assertion test in filter.rs (engine untouched); REQUIREMENTS.md [x] + traceability Complete (02-03); PROC-05 closed too |

### Key TODOs across phases

- [x] **Phase 1:** Set up single Cargo workspace; implement dual-stack TCP/UDP scanner with buffer retry; build Ratatui Elm Architecture main loop with VirtualTable — COMPLETE
- [x] **Phase 2:** ProcessSnapshot safety wrapper + built-in whitelist (25 entries) + smart kill escalation + x-key TUI kill surface — 02-01 DONE; detail fetchers + 12-row detail panel + protection markers — 02-02 DONE; whitelist overlay + Help overlay + SRCH-02 traceability — 02-03 DONE (94 tests green; end-of-phase UAT pending)
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
| Phase 02-process-management-smart-kill P02-02 | 25m | 2 tasks | 12 files |
| Phase 02-process-management-smart-kill P02-03 | 41m | 2 tasks | 12 files |

## Session Continuity

**Session 2026-08-20 Batch A (code review groundwork — COMPLETE):** Audit-driven hardening landed as 7 atomic commits: `b2d1bef` (single sysinfo refresh + resolve in spawn_blocking + dedup by pid), `fc9b1eb` (filter: no full-list clone, precomputed terms), `7cd0148` (settings mtime cache + atomic save), `9ec0574` (fail closed on None creation-time kill; saturate timeout), `4b6b9a0` (confirm dispatch beats search/filter; ElevateFailed resets guard), `1521cbf` (Connection.local_address + serde derives), `113bd05` (route_strategy wired; KillExecute removed). New tests: settings atomic-save, open_verified None-creation-time refusal, kill timeout saturation, map_key_event dispatch (3). Full 4 reviews in `.planning/review/`.

**Deferred to Batch B / Phase 3 (documented decisions):** (a) `spawn_kill` shared-task extraction + `main.rs` module split — Batch B refactor (touches untested drain-arm paths); (b) scan-time `start_time` population — D-16 cost tradeoff, decide in Phase 3; (c) the 2 kill_integration tests `test_graceful_console_child`/`test_timeout_then_force` are KNOWN-FLAKY at their `wait_for_exit` PID-polling assertion (02-VERIFICATION carry-forward) — reproduced identically WITHOUT these changes; harden the harness in Phase 3.

**Prior takeover record (2026-08-20):** Decisions: GSD-lite (keep `.planning` + review/verify gates, streamline ceremony); Phase 2's 5 manual-UAT items deferred as known risk (auto-verification passed 22/29; interactive items in `02-VERIFICATION.md` Human Verification section), backfill after Phase 3; gsd-core synced diff-only (v1.8.0), no replace — installed skills are adapter-wrapped targeting missing `~/.codex/gsd-core`, Claude-side install is v1.10.0 at `~/.claude/gsd-core`.

**Next action:** Phase 3 planning — start with ferrisetw transitive-dependency verification (windows 0.62 vs STACK.md's 0.73/ferrisetw 1.2.0) before committing to `monitor/etw.rs`; then Wave 3.1 (EventBus + ETW + 2s poll) per `.planning/review/REVIEW-phase3-readiness.md`.

**Last session:** 2026-08-01T11:40:00Z
**Stopped at:** Completed 02-03-PLAN.md (whitelist overlay + Help overlay)
**Resume file:** .planning/phases/02-process-management-smart-kill/02-03-SUMMARY.md

- **Last action:** Plan 02-03 executed — 6 commits (cd345ef RED validation tests, 57e8da4 validate/normalize, 08eca39 w-key overlay, f405947 SRCH-02 AND/OR test, dfafe70 Help overlay + REQUIREMENTS traceability, 0d94bb1 status-string 80-col lock). 94 tests green; TDD gate honored; SRCH-02 + PROC-05 traced Complete; all Phase 2 plans done.
- **Next action:** End-of-phase manual UAT per human_verify_mode (whitelist hard-block UX, immediate effect, Help overlay, kill escalation visuals); then Phase 3 planning (ETW + history)
- **Research flags:** Phase 3 (ETW event schemas), Phase 5 (Tauri system tray), Phase 6 (windows-wfp API, SQLite FTS5)

---

*State initialized: 2026-07-26*
*Last updated: 2026-07-31 after Phase 1 completion*

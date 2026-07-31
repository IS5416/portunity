---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: in_progress
last_updated: "2026-07-31T00:00:00Z"
progress:
  total_phases: 6
  completed_phases: 1
  total_plans: 4
  completed_plans: 4
---

# Project State: Portunity

**Project:** Windows Port Management Tool
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.
**Current Focus:** Phase 01 — tui-port-viewer

## Current Position

| Field | Value |
|-------|-------|
| **Milestone** | 1 |
| **Current Phase** | 1 — TUI Port Viewer | Complete (4/4 plans) |
| **Current Plan** | All plans complete. Next: Phase 2 — Process Management & Smart Kill |
| **Phase Status** | Complete |
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
| 5-tab dashboard with tab bar highlighting | 1 | active_tab: usize (0-based, 0=Overview default per D-14), per-tab Component trait dispatch, tab bar: Bold+accent_primary bg for active, Dim+fg_muted for inactive |
| Resize gate: 80x24 minimum terminal | 1 | frame.area() checked at start of render_app(); below threshold renders centered "Terminal too small" message, hides normal layout (TUI-07) |
| Placeholder tab pattern | 1 | History/Traffic/Firewall tabs render centered "Coming later" message with "Press 1 or 2 to view active tabs" nav hint; content deferred to Phases 3/4 |
| Release build profile | 1 | LTO + single codegen unit + strip symbols + opt-level=3; binary size: 1.1MB (target <10MB) |
| SKELETON.md architectural record | 1 | 227-line comprehensive document covering all 20+ Phase 1 decisions, data flow, crate graph, module map, pitfall coverage, out-of-scope map, subsequent slice plan |

### Key TODOs across phases

- [x] **Phase 1:** Set up single Cargo workspace; implement dual-stack TCP/UDP scanner with buffer retry; build Ratatui Elm Architecture main loop with VirtualTable — COMPLETE
- [ ] **Phase 2:** Implement ProcessHandle safety wrapper; build shipped system-critical whitelist (~30 processes); implement smart kill escalation
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

## Session Continuity

- **Last action:** Phase 1 (TUI Port Viewer) fully executed and verified — 13 commits, UAT 10/10 passed. Human verification found and fixed 9 bugs (keyboard double-fire, sort cycle, filter input model, screen flash).
- **Next action:** `/gsd-plan-phase 2` to create detailed plan for Phase 2: Process Management & Smart Kill
- **Research flags:** Phase 2 (ProcessHandle safety wrapper, smart kill escalation, system whitelist), Phase 3 (ETW event schemas), Phase 5 (Tauri system tray), Phase 6 (windows-wfp API, SQLite FTS5)

---

*State initialized: 2026-07-26*
*Last updated: 2026-07-31 after Phase 1 completion*

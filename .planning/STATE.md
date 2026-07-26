# Project State: Portunity

**Project:** Windows Port Management Tool
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.
**Current Focus:** Roadmap defined — 6 phases within 1 milestone. Ready to plan Phase 1.

## Current Position

| Field | Value |
|-------|-------|
| **Milestone** | 1 |
| **Current Phase** | 1 — TUI Port Viewer |
| **Current Plan** | UI-SPEC approved, CONTEXT captured — ready for plan-phase |
| **Phase Status** | Context gathered (16 decisions) |
| **Progress** | `[⬜⬜⬜⬜⬜⬜]` 0/6 phases complete |

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

### Key TODOs across phases

- [ ] **Phase 1:** Set up single Cargo workspace; implement dual-stack TCP/UDP scanner with buffer retry; build Ratatui Elm Architecture main loop with VirtualTable
- [ ] **Phase 2:** Implement ProcessHandle safety wrapper; build shipped system-critical whitelist (~30 processes); implement smart kill escalation
- [ ] **Phase 3:** Integrate ferrisetw with startup orphan cleanup; build lock-free callback-to-async bridge; implement SQLite history append-only log
- [ ] **Phase 4:** Build COM firewall rule CRUD abstraction; implement right-click quick actions; implement JSON/CSV export
- [ ] **Phase 5:** Register Tauri commands wrapping port-core APIs; build system tray with popup; implement Svelte reactive stores
- [ ] **Phase 6:** Build auto-label lookup table (50+ ports); implement Fluent i18n engine; create 6 theme preset TOML files

### Blockers

(None — ready to plan Phase 1)

## Performance Metrics

| Metric | Current | Target | Phase |
|--------|---------|--------|-------|
| Port scan time (1000 ports) | N/A | < 500ms | 1 |
| Memory (idle, TUI) | N/A | < 50MB | 1 |
| TUI scroll latency (5000 rows) | N/A | < 50ms | 1 |
| ETW events dropped | N/A | 0 under 100 conn/sec | 3 |
| Binary size (TUI, release) | N/A | < 10MB stripped | 1 |
| SQLite concurrent access | N/A | No BUSY errors, dual-frontend | 3 |

## Session Continuity

- **Last action:** Roadmap created (6 phases, 59 requirements mapped)
- **Next action:** `/gsd-plan-phase 1` to create detailed plan for Phase 1: TUI Port Viewer
- **Research flags:** Phase 3 (ETW event schemas), Phase 5 (Tauri system tray + Svelte reactive stores), Phase 6 (windows-wfp API completeness, SQLite FTS5 faceted query syntax)

---

*State initialized: 2026-07-26*
*Last updated: 2026-07-26 after roadmap creation*

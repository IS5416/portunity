---
phase: 2
slug: process-management-smart-kill
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-31
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`#[cfg(test)]` + `tests/` integration dir) — no new test deps |
| **Config file** | none — standard cargo layout (Phase 1 precedent: inline `#[cfg(test)]` in `port-core/src/filter.rs`) |
| **Quick run command** | `cargo test -p port-core` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p port-core`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD-01 | 01 | 1 | PROC-04 | T-02-01 / — | Built-in whitelist ≥25 entries, unique lowercase names, PID 0/4 special case, reason per entry | unit | `cargo test -p port-core process::whitelist` | ❌ W0 | ⬜ pending |
| TBD-02 | 01 | 1 | PROC-05 | — | Settings serde round-trip; defaults on Phase-1-era TOML; path normalization (quotes, case); full-path vs basename distinction | unit | `cargo test -p port-core config::settings` | ❌ W0 | ⬜ pending |
| TBD-03 | 01 | 1 | PROC-03 | — | `protection_status` matrix: built-in / user-listed / none → HardBlocked / UserConfirm / None | unit | `cargo test -p port-core process::whitelist` | ❌ W0 | ⬜ pending |
| TBD-04 | 02 | 1 | PROC-02 | — | `route_strategy` pure fn: windows / console / neither → WmClose / CtrlC / ForceDirect | unit | `cargo test -p port-core kill::strategy` | ❌ W0 | ⬜ pending |
| TBD-05 | 02 | 1 | PROC-02 | — | WM_CLOSE on real windowed child; timeout→force on signal-ignoring child | integration | `cargo test -p port-core --test kill_integration` | ❌ W0 | ⬜ pending |
| TBD-06 | 02 | 1 | PROC-07 | — | FILETIME verification logic; rapid spawn/kill churn, assert no wrong-process kill | unit+integration | `cargo test -p port-core process::handle` / `--test process_handle_integration` | ❌ W0 | ⬜ pending |
| TBD-07 | 02 | 1 | PROC-01 | — | Kill owning process via core API against real spawned child; assert exit | integration | `cargo test -p port-core --test kill_integration` | ❌ W0 | ⬜ pending |
| TBD-08 | 03 | 1 | PROC-06 | — | FILETIME→SystemTime; UNICODE_STRING bounds; cmdline parse; details() on current_exe populates all fields | unit+integration | `cargo test -p port-core process::info` | ❌ W0 | ⬜ pending |
| TBD-09 | 03 | 1 | SRCH-02 | — | AND/OR filter combination — existing Phase 1 tests; traceability update only | unit | `cargo test -p port-core filter` | ✅ exists | ⬜ pending |
| TBD-10 | 04 | 2 | TUI (UI-SPEC) | — | Overlay render states, key mapping, kill flow UX | manual UAT | manual (Ratatui has no headless infra in Phase 1) | — | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `port-core/src/process/whitelist.rs` tests — built-in list contract (PROC-04), `user_match` normalization (PROC-05)
- [ ] `port-core/src/process/kill.rs` tests — `route_strategy` pure routing, outcome mapping (PROC-02)
- [ ] `port-core/src/process/handle.rs` tests — FILETIME verification logic (PROC-07)
- [ ] `port-core/src/process/info.rs` tests — FILETIME→SystemTime, UNICODE_STRING extraction (PROC-06)
- [ ] `port-core/tests/kill_integration.rs` — spawn real child processes, exercise WM_CLOSE/Ctrl+C/force against them (Windows-gated, interactive-session)
- [ ] `port-core/src/config/settings.rs` tests — serde defaults for `whitelist` + `kill_timeout_secs` on a Phase-1-era TOML fixture (backward compat proof)
- [ ] No framework install needed — cargo test built-in

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TUI overlay render states + key mapping (d/x/w, confirm dialog, whitelist overlay) | PROC-01..03, UI-SPEC | Ratatui rendering has no headless test infra in Phase 1 | Run `port-tui`, select port, press d/x/w; verify overlays per UI-SPEC layout, footer ≤80 cols |
| Whitelist hard-block UX + immediate effect | PROC-04/05 | Requires real system-critical process context | Press x on protected row; verify explanation + no kill path; add/remove user entry in w overlay; verify next kill reflects it without restart |
| Graceful escalation outcome messages | PROC-02 | Requires interactive desktop session | Kill a windowed app (WM_CLOSE observed), a console app (Ctrl+C helper), a signal-ignoring app (force after 5s); verify status bar strings per UI-SPEC |
| Built-in whitelist final membership (~30 entries) | PROC-04 | Researcher flagged Assumption A1 — human verify exact list | Review constant against Microsoft Restart Manager critical services list + session/security tier |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

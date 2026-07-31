# Phase 2: Process Management & Smart Kill - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-31
**Phase:** 2-process-management-smart-kill
**Areas discussed:** Kill trigger & escalation, Detail panel, Protection semantics, Whitelist storage & UI
**Language:** Discussion held in Chinese per user request; artifacts written in English (English-only docs rule, CLAUDE.md 2026-07-31)

---

## Kill Trigger & Escalation

| Option | Description | Selected |
|--------|-------------|----------|
| x key | lazygit-style TUI convention, single key, no modifier, adjacent to hjkl cluster | ✓ |
| Uppercase K | Shift+k, semantic association with move-up | |
| d key | vim delete semantics; risky to reserve | |

**User's choice:** x key
**Notes:** k already bound to MoveUp (`port-tui/src/main.rs:281`).

| Option | Description | Selected |
|--------|-------------|----------|
| 5 seconds | Configurable (PROC-02), covers most graceful-close scenarios | ✓ |
| 3 seconds | Fast but slow-closing services may be force-killed prematurely | |
| 10 seconds | Lenient but long wait | |

**User's choice:** 5 seconds (default, `kill_timeout_secs` in settings.toml)

| Option | Description | Selected |
|--------|-------------|----------|
| Prompt + a-key elevation | Status bar message; Phase 1 D-08 rejected state transfer | ✓ |
| Auto-elevate relaunch | runas + `--kill-pid` arg; UAC denial loses old session | |
| Error only | Just show PermissionDenied | |

**User's choice:** Prompt + a-key elevation

| Option | Description | Selected |
|--------|-------------|----------|
| Status bar + auto refresh | Phase 1 status bar pattern; scan triggered after successful kill | ✓ |
| Toast overlay | New component, no precedent | |
| Detail panel feedback | Hidden if panel closed | |

**User's choice:** Status bar + auto refresh

---

## Detail Panel

| Option | Description | Selected |
|--------|-------------|----------|
| Soft overlay | Matches Phase 1 search/filter; no table squeeze at 80x24 | ✓ |
| Right fixed column | Squeezes table width | |
| Bottom panel | Wastes vertical space at low info density | |

**User's choice:** Soft overlay

| Option | Description | Selected |
|--------|-------------|----------|
| d-key toggle | Open/close; content refreshes on selection change | ✓ |
| Auto-open on selection | Constant interference | |

**User's choice:** d-key toggle

| Option | Description | Selected |
|--------|-------------|----------|
| On-demand async + cache | spawn_blocking WinVerifyTrust, "verifying..." → result | ✓ |
| Don't implement | is_signed always None, violates PROC-06 | |
| Pre-fetch at scan | Blows <500ms scan budget | |

**User's choice:** On-demand async + cache

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch on open + cache | Per-process <5ms; cache until next scan; start time reuses ProcessHandle | ✓ |
| Batch pre-fetch at scan | Every 5s scan walks all process details | |

**User's choice:** Fetch on open + cache

---

## Protection Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Built-in hard block + user whitelist confirm | System-critical unkillable; success criteria #4 | ✓ |
| Unified confirm-and-kill | Weaker than "cannot accidentally kill" | |
| Built-in block + user instant kill | Contradicts PROC-03 whitelist=confirm | |

**User's choice:** Built-in hard block; user whitelist confirmation-level

| Option | Description | Selected |
|--------|-------------|----------|
| Built-in by basename + user by path | PROC-04/05 literal; System(4)/Idle(0) PID special case | ✓ |
| Unified basename | Cannot distinguish same-name instances | |
| Unified path | SysWOW64 redirection inconsistency risk | |

**User's choice:** Built-in by basename, user by full path

| Option | Description | Selected |
|--------|-------------|----------|
| Non-modal overlay | Same family as search/filter/detail | ✓ |
| Modal dialog | Blocking, no precedent | |

**User's choice:** Non-modal overlay

| Option | Description | Selected |
|--------|-------------|----------|
| Confirmation-only | User entry = "kills need my confirmation"; PROC-05 scope | ✓ |
| Upgradeable to hard block | Second level, out of PROC-05 scope | |

**User's choice:** Confirmation-only

---

## Whitelist Storage & UI

| Option | Description | Selected |
|--------|-------------|----------|
| Extend settings.toml | AppSettings.whitelist Vec<String>; built-in hardcoded in port-core; CORE-05 | ✓ |
| Separate file | More files, more read/write logic | |
| SQLite table | Config is TOML per CORE-05 | |

**User's choice:** Extend settings.toml

| Option | Description | Selected |
|--------|-------------|----------|
| w-key overlay | Non-modal; built-in read-only section + editable user section; no new tab | ✓ |
| Manual edit only | Non-technical users excluded | |
| New tab | Breaks TUI-01 fixed 5-tab layout | |

**User's choice:** w-key overlay

| Option | Description | Selected |
|--------|-------------|----------|
| Re-read before each kill | <1ms, no watcher dependency | ✓ |
| File watcher (notify) | New dependency, low-value events | |

**User's choice:** Re-read before each kill

| Option | Description | Selected |
|--------|-------------|----------|
| Built-in read-only + user editable | User understands why a process is unkillable | ✓ |
| User list only | Confusion at hard-block | |

**User's choice:** Built-in read-only + user editable

---

## Claude's Discretion

None — every question answered explicitly by the user (all recommended options).

## Deferred Ideas

None — discussion stayed within Phase 2 scope.

---
phase: 02-process-management-smart-kill
plan: 03
subsystem: process-management
tags: [whitelist-overlay, help-overlay, path-validation, traceability, tui]
type: execute
status: complete
requires:
  - "02-02 (detail panel, protection markers, ProcessInfo fetchers)"
provides:
  - "port-core::process::whitelist (validate_user_entry, normalize_user_entry — UI-SPEC backstop validation)"
  - "port-tui w-key whitelist overlay (20-row Clear-over: read-only built-in list, editable user list, path input)"
  - "port-tui ?-key Help overlay (full key reference + secrets + name-based-matching notes)"
  - "SRCH-02 traceability closure (REQUIREMENTS.md [x] + Complete)"
affects:
  - port-core/src/process/whitelist.rs
  - port-core/src/process.rs
  - port-core/src/filter.rs
  - port-tui/src/{message,app,main,update,components}.rs
  - port-tui/src/components/{whitelist_overlay,help}.rs
  - Cargo.toml
  - .planning/REQUIREMENTS.md
  - .planning/ROADMAP.md
key-files:
  created:
    - port-tui/src/components/whitelist_overlay.rs
    - port-tui/src/components/help.rs
  modified:
    - port-core/src/process/whitelist.rs
    - port-core/src/process.rs
    - port-core/src/filter.rs
    - port-tui/src/message.rs
    - port-tui/src/app.rs
    - port-tui/src/main.rs
    - port-tui/src/update.rs
    - port-tui/src/components.rs
    - Cargo.toml
    - .planning/REQUIREMENTS.md
decisions:
  - "WhitelistDeleteSelected mutates the working copy on the main event loop FIRST (PROC-05), then persists the post-removal state off-runtime — the working copy is never stale while the save runs"
  - "WhitelistSaved { added: false } is ambiguous by design (duplicate no-op OR completed removal); update() disambiguates by case-insensitive presence in the working copy — duplicate string vs Removed string"
  - "Help overlay swallows all keys except Esc/'?' (full-area overlay) — the confirm dialog's dispatch runs earlier in map_key_event, so y/n stay reachable if a dialog sits under Help (UI-SPEC stack: help below confirm)"
  - "windows feature Win32_Storage_FileSystem added for GetLongPathNameW (8.3 resolver) + test-only GetShortPathNameW — feature flag on the already-audited windows crate, zero new crates (T-02-SC posture unchanged)"
  - "validate/normalize re-exported from port-core::process (process.rs) — the TUI imports the validation contract from the crate root, not the leaf module"
  - "Built-in section renders PID 4/0 special-case rows FIRST then the first 7 BUILTIN entries — 9 read-only rows per the UI-SPEC 20-row layout (Tier-1 canonical entries visible at 80x24)"
  - "Overlay open-load is a synchronous settings.toml read (UI-SPEC backstop: 'no loading state exists by design', <1ms) — the one allowed sync file read on the runtime"
tech-stack:
  added:
    - "windows feature Win32_Storage_FileSystem (GetLongPathNameW/GetShortPathNameW — 8.3 resolution)"
  patterns:
    - "spawn_blocking validate+save with the working copy cloned into the closure (file I/O + Win32 off the runtime; duplicate add short-circuits before save)"
    - "Tail-preserving path truncation for whitelist status strings (…\\dir\\name.exe, U+2026) — the file name is the actionable part (A9)"
    - "Ports.rs render_scrollbar pattern reused for the user-list overflow (track │ / thumb █, thumb proportional to viewport/total)"
    - "Search.rs block-cursor input pattern reused verbatim for the Path: >_ input row"
metrics:
  duration: "~41m execution (commits 2026-08-01T10:53:54Z..11:34:50Z +0800)"
  tasks: 2
  commits: 6 (cd345ef, 57e8da4, 08eca39, f405947, dfafe70, 0d94bb1)
  tests: 94 passing (74 unit + 5 kill integration + 1 churn + 14 TUI)
actuals:
  tokens: 17240   # chars/4 over the ~69k-char realized diff (1338 insertions)
  tasks: 2
  commits: 6
---

# Phase 2 Plan 3: Whitelist Management Overlay + Help Overlay + SRCH-02 Traceability Summary

One-liner: users manage their protection list live from the w-key overlay — read-only built-in section, validated user add-by-path with immediate settings.toml persistence, instant removal — plus the ?-key Help overlay as the canonical reference for every keyboard capability, and the SRCH-02 verification/traceability closure.

## What Was Built

**Task 1 — w overlay (TDD: RED cd345ef / GREEN 57e8da4 / overlay 08eca39):**

- `port-core/src/process/whitelist.rs` — `normalize_user_entry` (trim, ONE quote pair strip, control-char rejection, absolute-syntax check incl. UNC, 8.3 via `GetLongPathNameW` on existing files only, one trailing separator strip) + `validate_user_entry` (adds existence check via `std::fs::metadata` + human-readable reasons: "Path must be absolute (C:\... or \\server\share\...)", "Path contains control characters", "Path does not exist", "Path is too long (max 4096)"). Pure normalization is cfg-gated so the matrix unit-tests on any host; the Win32 8.3 resolver is the one non-pure step. `process.rs` re-exports both.
- 12 new unit tests (37 total in the module): existing-absolute accept (test binary path), nonexistent/relative/control-char/overlong rejection, quote+whitespace strip, trailing separator, UNC accept, case-insensitive duplicate no-op semantics, Windows-only 8.3 round-trip test (temp dir → `GetShortPathNameW` short form → normalize → long form restored).
- `port-tui` message/app/update wiring: 12 new `Message` variants; `WhitelistFocus { List, Input }` with next()/prev(); 7 app fields; update handlers (toggle-with-open-load, focus cycle, selection clamp, input insert/backspace/cursor, WhitelistSaved disambiguation, WhitelistError).
- `main.rs` — `w` bound in default dispatch (any tab); overlay dispatch per the UI-SPEC pass-through table (Esc/Tab/BackTab/j/k/d/Enter/Backspace/arrows/printable in the right focus states; r/s and tab/quit keys fall through to the default match). `WhitelistAdd` intercept: `!kill_in_flight` guard → spawn_blocking validate → case-insensitive duplicate short-circuit (no save) → else push + `save_settings` → `WhitelistSaved`/`WhitelistError`. `WhitelistDeleteSelected` intercept: bounds-checked, working copy mutated on the main loop first, post-removal state saved off-runtime, selection clamped.
- `WhitelistOverlayComponent` — 20-row Clear-over per the UI-SPEC layout: title + [Esc]; built-in label; 9 read-only built-in rows (PID 4/0 special rows first, then 7 BUILTIN entries; `◆` in status.error, name fg_muted Dim); user label; Min(5) user list (`→ path`, Reverse/bg_selection selection, empty-state copy, scrollbar on overflow); `Path: >_` block-cursor input with placeholder; hint row; bottom border. Footer branch `[j/k]Move [d]Delete [Tab]Focus [Enter]Add [Esc]Close`.
- Status strings (all status.info / status.error tone, locked to the 80-col gate by tests): `Added {path} — kills now require confirmation`, `Removed {path} — kills are instant again`, `{path} is already on your protection list` (duplicate no-op), `Cannot add {path}: {reason}` — `{path}` truncates tail-preserving with U+2026.

**Task 2 — Help overlay + SRCH-02 (f405947 / dfafe70 / 0d94bb1):**

- `HelpComponent` — bordered Clear-over covering the content area, rendered above whitelist and below confirm (stack order verified). Sections: Universal (1-5 tabs, Tab/Shift+Tab, Esc), Navigation (j/k, g/G), Actions (d/x/w/s/r, /, f, a-Elevate-when-not-admin), Kill confirmation (y/n), Power (q). Two note lines: command-line secrets are display-only (prohibition P4) and protection matching is name-based — all instances of a built-in name are protected (T-02-06). `?` binds in default dispatch; Esc/`?` close; all other keys swallowed (full-area overlay); confirm-dialog keys stay reachable (dispatch runs earlier).
- SRCH-02 verification: `filter.rs` gained ONE additive assertion test `or_within_vec_and_across_dimensions` (OR within protocols Vec ∧ AND across process_names — 2 of 3 rows match; engine untouched, no behavior change). Existing 8 filter tests remain green.
- Traceability: REQUIREMENTS.md SRCH-02 `- [x]` + traceability row `Complete (02-03)`; PROC-05 also closed to `[x]` + Complete (the w overlay ships in this plan). ROADMAP.md: 02-03 plan list item marked COMPLETE (2026-08-01, 94 tests green), progress table lists 02-01, 02-02, 02-03.

## Deviations from Plan

### Auto-fixed Issues

None — the plan executed cleanly; every auto-fix pattern below was already in the implementation commit set when this wave was verified.

### Deviations from Plan Text

**1. [windows feature] `Win32_Storage_FileSystem` feature flag added**
- **Found during:** Task 1 GREEN (validation compile)
- **Issue:** `GetLongPathNameW`/`GetShortPathNameW` are gated behind `Win32_Storage_FileSystem` in windows-rs 0.62.2 — not enabled by plan 02-01's feature set (which covered WinTrust/Cryptography/Console). The plan's T-02-SC wording listed "windows features from plan 02-01".
- **Fix:** added the feature flag to the workspace `windows` entry — a feature on the already-audited crate, zero new crates (T-02-SC posture unchanged, consistent with the 02-02 Cryptography precedent).
- **Files modified:** Cargo.toml
- **Commit:** 57e8da4

**2. [re-export] validate/normalize exported from `port-core::process` root**
- The plan listed `process.rs` re-exports only implicitly ("process.rs — extend"); the TUI imports the validation contract via `port_core::process::validate_user_entry`, so the two new fns were added to the `pub use whitelist::{...}` re-export list.
- **Files modified:** port-core/src/process.rs
- **Commit:** 57e8da4

**3. [delete flow] working copy mutated on the main loop before the off-runtime save**
- Plan step 5c wording: "clone working copy, remove the entry at whitelist_selected, save_settings". Implementation: the intercept removes the entry from `app.whitelist_settings.whitelist` directly (main-loop-owned state, PROC-05), then clones the post-removal state for the spawn_blocking save — the working copy is never stale while the save runs, and a save failure can be retried by re-selecting. Same outcome contract (`WhitelistSaved { added: false }`), instant effect per D-15.
- **Files modified:** port-tui/src/main.rs
- **Commit:** 08eca39

**4. [status disambiguation] `WhitelistSaved { added: false }` carries two meanings**
- The message means "duplicate add no-op" OR "removal completed" (plan defined both on one variant). update() disambiguates by case-insensitive presence in the working copy: still present → `{path} is already on your protection list`; absent → `Removed {path} — kills are instant again`. Both status.info per the UI-SPEC color map.
- **Files modified:** port-tui/src/update.rs
- **Commit:** 08eca39

**5. [Help dispatch] full-area swallow vs pass-through**
- The plan said "verify it dispatches before the default match"; implementation swallows all keys except Esc/`?` while help is active (it covers the whole content area — no hidden table interaction), with the confirm-dialog dispatch running earlier in map_key_event so y/n remain reachable. Matches the UI-SPEC stack (help below confirm) and the declared pass-through posture for full-area overlays.
- **Files modified:** port-tui/src/main.rs
- **Commit:** dfafe70

## Auth Gates

None — no authentication was required at any point.

## Known Stubs

None — the `Message::KillExecute` variant remains declared-but-unconstructed by design (documented since 02-01; future kill paths may emit it, `#[allow(dead_code)]` with rationale). No placeholder data sources, no empty-value rendering stubs: the overlay's built-in section renders the real `BUILTIN` constant, the user list renders `whitelist_settings.whitelist` live, and the empty state is the UI-SPEC copy.

## Threat Flags

None — no security surface beyond the plan's threat_model. The settings.toml write path (w overlay add/remove → `save_settings`) is the plan's T-02-02 mitigation (input validator gate: absolute-path syntax, control-char rejection, quote strip, trailing-separator strip, 8.3 resolution, 4096 cap, existence check); T-02-08 TOCTOU accepted per plan (kill-time re-read, <1ms window, same trust domain); T-02-06 documented to users via the Help note; T-02-03 via the secrets note. The `Win32_Storage_FileSystem` flag is configuration-only on the audited windows crate.

## Verification

- `cargo test -p port-core process::whitelist` — 37 tests green (validation/normalization matrix, 8.3 round-trip on Windows)
- `cargo test -p port-core filter` — 9 tests green (SRCH-02 AND/OR combination asserted; engine untouched)
- `cargo test --workspace` — 94 tests green (74 unit + 5 kill integration + 1 churn + 14 TUI); the "Unrecognized option: 'ctrl-c'" stderr lines are the expected 02-01 helper-binary noise
- `cargo build -p port-tui` — clean
- `grep -n 'SRCH-02.*Complete' .planning/REQUIREMENTS.md` — matches (traceability row "Complete (02-03)")
- `grep BUILTIN port-tui/src/components/whitelist_overlay.rs` — read-only render only (`BUILTIN.iter().take(7)` in `builtin_display_rows`; no add/delete path into BUILTIN)
- TDD gate compliance: `test(02-03)` RED cd345ef precedes `feat(02-03)` GREEN 57e8da4 in git log
- Manual UAT deferred to end-of-phase per `human_verify_mode=end-of-phase` (w overlay visuals, add/remove UX, help stack order, hard-block message — interactive terminal session required)

## Self-Check: PASSED

- Files exist: port-tui/src/components/{whitelist_overlay,help}.rs ✓
- Commits exist: cd345ef (RED), 57e8da4 (GREEN), 08eca39 (overlay), f405947 (SRCH-02 test), dfafe70 (Help + traceability), 0d94bb1 (status strings) ✓
- 94 workspace tests green; port-tui builds clean; SRCH-02 grep matches; BUILTIN read-only verified ✓

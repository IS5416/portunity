---
review: TUI terminal frontend (phase-3-readiness)
reviewed: <review-date>
depth: standard (read-only)
scope:
  - port-tui/src/main.rs (1503)
  - port-tui/src/update.rs (809)
  - port-tui/src/app.rs (565)
  - port-tui/src/message.rs (279)
  - port-tui/src/components.rs + components/*
  - port-tui/src/theme.rs, elevate.rs
status: findings (0 CRITICAL, 5 WARNING, 7 INFO)
tests: cargo test -p port-tui -> 14 passed
---

# TUI Terminal Frontend — Architecture / Event-Loop Review

## Summary

- TEA discipline is intact and healthy: `Message` → pure `update()` → stateless `Component::render` is consistent; the runtime-only concerns (spawning tasks, snapshot capture, signature verify, settings re-read) are deliberately parked in `main.rs`'s event loop and the drain arms, so `update()` stays synchronous and unit-testable. This is the right separation and the foundation the Phase-3/5 additions should preserve.
- The single biggest maintainability risk is now **`main.rs`'s event loop growing into three distinct concerns in one 1500-line file**: (1) channel + task-spawn infrastructure (`spawn_scan`/`spawn_detail_fetch` and the six inline `spawn`/`spawn_blocking` intercepts), (2) the input→Message dispatch table (`map_key_event`, 214 lines), and (3) the render frame + four bar/overlay renderers and the drain-loop "intercept" arms. They are interleaved but separable along clean seams.
- The **drain-loop intercepts** (main.rs:609-724) are the trickiest, highest-value code in the crate: `ScanComplete`/`DetailDataLoaded`/`KillPrepared` each have bespoke special-casing (signature cache invalidation, `ProcessExited` synthesis, instant-kill re-spawn) that is invisible to `update()` and therefore **untested**. Phase 3 will add more of these; they are the prime candidate for a dedicated module.
- Event-loop robustness is good: unbounded channel prevents lost messages, the `scan_spawned` guard plus `scanning` flag prevent double-spawn, and the prior Phase-2 **CR-01 (stuck "Scanning…")**, **WR-04 (stale settings clobber)** and **WR-05 (fake "delivered")** are all correctly fixed. Remaining robustness gaps are overlay-dispatch ordering and the never-reset `elevating` flag.
- Test coverage is real but shallow in the risky spots: all 14 tests exercise pure string/number formatting (`format_kill_status`, whitelist strings). **Zero tests cover `map_key_event` dispatch, the drain-loop intercepts (instant-kill route, stale result dropping, signature re-spawn, `ProcessExited` synthesis), or the overlay order.** The most defect-prone routing logic is exactly what the tests skip.

## Findings

| Sev | File:Line | Issue | Suggested fix | Why-matter-now |
|-----|-----------|-------|---------------|----------------|
| WARNING | main.rs:754-810 vs 1110 | **Dispatch order contradicts render stack.** `map_key_event` checks search (754) and filter (774) BEFORE the confirm dialog (798), but the render stack (main.rs:1110) draws confirm *topmost*. Reachable: open `/` search, move to a protected row, press `x` → `KillPrepared(UserConfirm)` sets `confirm_pid` via the drain while search is still active; now `y`/`Enter`/`n`/`Esc` are swallowed by the search bar (754/757) and the user cannot confirm/cancel the modal without first Esc-ing out of search. Same for a `FilterTabField`-driven conflict. | Move the confirm-dialog dispatch block above the search/filter blocks, or add an early `if app.confirm_pid.is_some() { … }` gate at the very top of `map_key_event` (the modal is "topmost" and should win regardless of search/filter state). | A user-facing dead-end (stuck modal with inoperative y/n) reachable via normal keys; trivial to fix, and the "topmost" contract is already documented in the code comments (main.rs:797). |
| WARNING | main.rs:257-275 + update.rs:53-57 | **`app.elevating` is never reset on a hard elevation failure.** `main()` sets `elevating = true` on `ElevateRequest` (main.rs:259); on decline `ElevateDeclined` resets it (update.rs:208); on a *successful* elevate the process exits (elevate.rs:67), so the flag doesn't matter there — but on a **non-1223 `ShellExecuteExW` error** (`current_exe` failure, etc.) the loop sends `ScanError` (main.rs:268-270) and `ScanError` (update.rs:53-57) does **not** reset `elevating`. Result: `a` becomes dead for the rest of the session. | In the `Err` branch of the ElevateRequest intercept (main.rs:268-270), also send/clear `elevating` (e.g. a dedicated `ElevateFailed` message, or reset the flag inline before sending `ScanError`). | Edge case, but the fix is one line and this exact class (latched-once-never-cleared guard) already burned the project once (prior CR-01). |
| WARNING | main.rs:581-597 | **`MoveUp`/`MoveDown`/`ScrollTop`/`ScrollBottom` intercept duplicates the "refresh detail on selection change" logic that lives partly in render and partly here, and it re-fetches even when the selection did not change.** The guard `if app.detail_pid != Some(row.pid)` (main.rs:587) only suppresses identical-PID; it always calls `update(app, m)` first and sets `needs_render`. Minor, but combined with the `MoveUp`/`MoveDown` handlers in update.rs (69-91) it means a scroll+detail-open path re-runs `selected_connection()` + potential spawn on every movement keystroke. | Fold the D-06 refresh into a single helper (e.g. `refresh_detail_for_selection(app, tx)`) called from both the toggle intercept (559-578) and the move intercept (581-597), with the `detail_pid != pid` guard as the only spawn gate. | Phase 3 adds movement-like messages for history/traffic; a shared helper prevents each new tab re-implementing (and mis-implementing) the selection→detail choreography. |
| WARNING | main.rs:126-157, 116-128 (spawn fns) | **The task-spawn helpers and the six inline `spawn`/`spawn_blocking` blocks (main.rs:257-552, 636-645, 684-706) are duplicated in shape and untested.** The instant-kill body at 684-706 is an exact copy of the `KillConfirmed` body at 358-382 (kill + `KillTimeout` callback + `KillOutcome`), differing only in the source of the snapshot. Any change to the kill pipeline must be made **twice** or it drifts. Signature verify (636-645) and the two scan helpers are further near-duplicates. | Extract a `spawn.rs`/`tasks.rs` module owning `spawn_scan`, `spawn_detail_fetch`, `spawn_signature_verify`, and a single `spawn_kill(tx, snapshot, name, pid, timeout)` used by **both** the instant-kill drain arm and the `KillConfirmed` intercept. Give it unit tests (they need only an `App`/channel — the spawn fns take a `tx` and can be driven with `try_recv`). | This is the highest-leverage refactor: kill is the app's most safety-critical path and is currently defined twice. |
| WARNING | main.rs:979-1000 (resize gate width) | **Truncation budgets assume `term_width` fits the status/footer, but `app.term_width` is set from `terminal.size()` (main.rs:218-220) *after* the resize gate passes at ≥80; the thousands-separator/japanese-width and multi-codepoint concerns are fine, yet `truncate_footer_name`/`format_kill_status` budgets (`area.width - 63`, `-66`, `term_width-fixed`) can still produce a string wider than its area when the name is very long and the budget math uses `saturating_sub` on `u16`.** Low severity; the guard definitions are consistent. | (Verify only): add a unit test that `render_footer`-style budgets stay ≥1 across the 80-term_width range; no production change needed if confirmed. | Guards against a regression that would wrap the 1-row status/footer, which is a visible UI defect. |
| INFO | message.rs:206-207 | `Message::KillExecute { snapshot }` is still a declared-but-dead variant (`#[allow(dead_code)]`, update.rs:295-297 is a no-op arm, no producer anywhere in the workspace). Context note said this is known; it is accurate. | Either remove the variant + its no-op update arm + the `#[allow(dead_code)]`, or actually wire it as the single kill-execute message the instant-kill and confirm paths both emit (leveraging the `spawn_kill` extraction above). | Shipping dead variants with allowed warnings hides drift and gives the wrong impression the flow uses them. |
| INFO | update.rs:506-535 | `merge_scan_results` keys on `(port.number, protocol)` — many established connections on one local port collapse to a single arbitrary row after the first refresh. Pre-existing (also flagged in 02-REVIEW.md IN-03). | Re-key on a fuller identity (pid/remote) when a connection has one; at minimum document the intentional collapse. | Directly impacts the data the User sees between the first scan and refresh; wrong if the port table is meant to show per-connection rows. |
| INFO | ports.rs:403-411 / overview.rs:480-488 / detail_panel.rs:369-377 / kill_confirm.rs:105-113 / whitelist_overlay.rs:279-287 | **`fn truncate` is copy-pasted 6 times** (ports, overview, detail_panel, kill_confirm, whitelist_overlay); `protocol_label` 3× (ports:394, overview:471, detail_panel:360); `truncate_path_tail` 2× (update.rs:715, detail_panel.rs:382); `truncate_ellipsis` 2× (app.rs:422, update.rs:725); `render_scrollbar` 2× (ports.rs:414, whitelist_overlay.rs:239). These are byte-identical helpers. | Consolidate into a `components/util.rs` (or `text.rs`) module: `truncate`, `truncate_ellipsis`, `truncate_path_tail`, `protocol_label`, `state_display` variants, `render_scrollbar`. | Phase 3 adds history/traffic tables that will need the same 5 helpers again; a shared module should exist *before* the next copy is introduced. Pure de-dup, zero behavior change. |
| INFO | main.rs:148-149 | `available_rows = table_area.height − 1 − 2` subtracts 2 for "block borders", but the block uses `Borders::NONE` (ports.rs:38-40). Two phantom rows are reserved → the viewport holds 2 fewer rows than the terminal actually shows, biasing the scrollbar/thumb math. | Drop the `saturating_sub(2)` (keep `− 1` for the header) since there are no borders. | Cosmetic scroll precision; irrelevant at ≥80 rows but trivially wrong. |
| INFO | main.rs:205-746 | The event loop keeps the transient `scan_spawned: bool` as a **local in `main()` (main.rs:96) passed by `&mut` into the loop (main.rs:105, 211)**, rather than a field on `App`. This works but splits scan-lifecycle state across two places (the `App.scanning` flag + the local guard) and resets in three drain arms (main.rs:611, 661) + two spawn sites (main.rs:728-729, 738-739). | Fold the guard into the loop module (or `App`) once the spawn helpers are extracted, so lifecycle resets are single-sourced. | Reduces the surface where a Phase-3 periodic-scan path could forget a reset (the exact bug class of old CR-01). |
| INFO | appeal to `port_core::windows::WindowsPortScanner` (main.rs:118), `.scan()` — untyped | The initial scan + every refresh concretely binds the TUI to `WindowsPortScanner`, hardcoding the trait object (`use port_core::scanner::PortScanner`, main.rs:44) rather than taking the scanner as a dependency. Fine for now (single platform). | Leave as-is this phase; note for Phase 5 GUI-share that a shared scanner provider could be injected. | Phase-5 concern only. |
| INFO | history.rs / traffic.rs / firewall.rs (all ~61-62 lines) | Three byte-identical "Coming later" placeholders (only the doc-comment phase numbers differ). They are OK as stubs but currently hide real work behind a per-tab `Component` that mimics the final shape. | When Phase 3 lands, replace each with its data source; until then consider a single shared `PlaceholderComponent` keyed by label/phase to avoid triplicating a message that will be deleted anyway. | Phase-3 integration (below) depends on finally replacing these; nothing to fix before then. |

### Notes on context accuracy

The supplied context notes are accurate: `main.rs` is 1503 lines, `update.rs` 809, `app.rs` 565, `message.rs` 279; the TEA style holds; Phase 1/2 complete; 14 tests pass (verified by executing `cargo test -p port-tui` → `test result: ok. 14 passed`); `KillExecute` is the dead variant with `#[allow(dead_code)]`. The claim that "review findings from Phase 2 were all fixed" is **mostly true but overstated**: CR-01, WR-04, WR-05 are indeed fixed in the current `main.rs` (verified by reading the reset/merge logic), but the report's **IN-03** (`merge_scan_results` collapse) and **IN-06** (`KillExecute`) remain open exactly as flagged. I did not re-audit the `port-core` findings (WR-01/02/03, IN-01/02/05) since `port-core` is out of this review's scope.

## Refactor plan for main.rs

Goal: split the 1503-line monolith without breaking the TEA discipline — keep `Message`/`update()`/`App` exactly as they are, and move only the *infrastructure* that the loop, dispatcher, and renderer each own. Preserve behavior exactly; this is churn, not redesign. Move entire cohesive blocks in the listed order so each move compiles independently (the seam is module boundaries, not `pub` signatures — the new modules are internal `mod` and only expose what the loop needs).

### Proposed module layout

```
port-tui/src/
  main.rs            (shrink to ~250): Args + helper-mode guard + setup/teardown + thin loop
  app.rs             (unchanged)
  update.rs          (unchanged)
  message.rs         (unchanged)
  input.rs           NEW — map_key_event + the key-dispatch precedence fix (finding #1)
  tasks.rs           NEW — spawn_scan / spawn_detail_fetch / spawn_signature_verify / spawn_kill
  loop_ctl.rs        NEW — run_event_loop + the drain-arm intercepts (ScanComplete/DetailDataLoaded/KillPrepared special cases)
  overlay.rs         NEW — render_app's overlay stack + render_tab_bar/render_status_bar/render_footer/truncate_footer_name
  components/util.rs NEW — the shared text/table helpers (finding #7)
```

Move order (each step is a compile-greenable commit):

1. **`components/util.rs`** — move `truncate`, `truncate_ellipsis`, `truncate_path_tail`, `protocol_label`, `render_scrollbar` (and, if desired, `state_display`) from their 6+ current homes into one module; add `use` sites in the component files. Zero signature change; the biggest mechanical but safest win. (Finding #7.)

2. **`tasks.rs`** — move `spawn_scan` (main.rs:116-128) and `spawn_detail_fetch` (main.rs:136-157) as-is. Add `spawn_kill(tx, snapshot, name, pid, timeout)` by extracting the **shared** body of the `KillConfirmed` intercept (main.rs:358-382) and the instant-kill drain arm (main.rs:684-706), and `spawn_signature_verify(tx, pid, path)` from main.rs:636-645. Now both kill paths call one function. (Finding #3/#6.) The module returns the `UnboundedSender<Message>` it needs; unit-testable via a real channel + `try_recv`.

3. **`input.rs`** — move `map_key_event` (main.rs:752-966) verbatim. While it is the sole owner of its helper, apply the **dispatch-order fix** (confirm-dialog check hoisted above search/filter) and add the first `map_key_event` unit tests (pure function — trivially testable with a constructed `App`). This is the highest-value test gap. (Findings #1, #8.)

4. **`loop_ctl.rs`** — move `run_event_loop` (main.rs:205-746) including the drain-arm intercepts. It calls `tasks::` and `input::map_key_event` (already extracted) and `crate::update::update`. The only new seam: it must receive the loop-local `scan_spawned` (`&mut bool`) — either keep the `&mut` param (least churn) or promote it to `App`. Recommend keeping the param this phase. (Finding #9.) Add tests for the drain arms by feeding an `mpsc::channel` and calling the loop body once (needs the loop factored to run a bounded number of iterations).

5. **`overlay.rs`** — move `render_app` (main.rs:973-1127), `render_tab_bar` (1133-1172), `render_status_bar` (1178-1289), `render_footer` (1292-1492), and `truncate_footer_name` (1496-1503). `render_app` becomes a thin coordinator that calls the four bar renderers + the per-tab `Component`s (all already module-local). (Finding #5 make the budgets testable here.)

6. **`main.rs`** keeps only `Args`, the helper-mode guard (main.rs:49-68), tracing/terminal setup (70-80), the channel + `App` creation (82-96), and a ~15-line loop that calls `loop_ctl::run_event_loop` then tears down raw mode (98-113). Final `main.rs` ≈ 250 lines.

**Least-churn seam (the single most important decision):** put the *whole* `run_event_loop` in one module rather than trying to split the loop from the drain arms. The loop and its intercepts share the `scan_spawned` guard, `app`, `tx`, and `rx` in ways that fight a finer split; a single `loop_ctl` module keeps that coupling local and gives Phase 3 one obvious place to add drain arms, while `input.rs` and `tasks.rs` are the genuinely reusable pieces.

## Phase-3 tab integration notes

- **Adding real History/Traffic tabs touches three places, not one**: (a) the `Component` in `components/history.rs|traffic.rs` (replace the placeholder with a real renderer — see findings #11), (b) the **key dispatch** in `input::map_key_event` (new per-tab keys) and any per-tab pass-through rules, and (c) the **event-loop drain** in `loop_ctl` for any async data (e.g. periodic traffic sampling, history reads from SQLite). The current placeholder structure already routes tab 2/3 through `render_app`'s match (main.rs:1080-1083), so the render seam is clean; the dispatch + drain seams are where the new interaction complexity will land — exactly the modules this plan creates.
- **History/traffic will need new `Message` variants + `update()` arms + `App` fields.** Keep the TEA discipline: add pure state mutations to `update()`, spawn any I/O in `tasks.rs`/`loop_ctl`, keep components stateless. Do **not** reintroduce `spawn` calls inside `update()` (there are currently none; preserve that).
- **Auto-refresh policy is tab-agnostic today** (main.rs:732-742 fires whenever `!scanning && error.is_none()`). A traffic tab sampling continuously would fight the 500ms idle poll and the render-on-need gate (main.rs:228-232). Plan a per-tab "needs periodic update" signal (e.g. a `Message::TrafficTick` or a `needs_render`/clock tick already present) so Phase 3 doesn't have to loosen the render gate globally.
- **Overlay stack must stay centralized.** Phase 3 tabs will add their own overlays/panels; keep them in the render stack list in `overlay.rs` and the dispatch precedence in `input.rs` consistent, or the Phase-2 confirm-vs-search ordering bug (finding #1) will recur. Consider a single explicit `OverlayLayer` enum to make the stack order declarative once there are ≥3 overlays.
- **`WhitelistFocus`/kills are Ports-tab concerns but `w` works on any tab** (main.rs:841, 1088-1098). If History/Traffic add their own focused lists, the `WhitelistFocus` and per-overlay dispatch precedence (input.rs) should be per-overlay-keyed, not ordered by a fixed chain.

## Suggested follow-up commits

1. `refactor(tui): hoist confirm-dialog dispatch above search/filter in map_key_event` — fixes finding #1; standalone and safe.
2. `fix(tui): reset elevating flag on hard elevation failure` — finding #2; one line.
3. `refactor(tui): dedupe instant-kill and confirmed-kill into shared spawn_kill task` — findings #3/#6.
4. `refactor(tui): extract shared text/table helpers into components::util` — finding #7 (de-dup 6x truncate, 3x protocol_label, …).
5. `refactor(tui): split main.rs into input/tasks/loop_ctl/overlay modules` — the plan above; land after 3 & 4 so the moves are mechanical.
6. `test(tui): add map_key_event + drain-arm unit tests` — closes the coverage gap (finding #8); highest-value test ROI.
7. `chore(tui): drop KillExecute dead variant (or wire it as the single kill-execute message)` — finding #6.
8. `refactor(tui): fold scan_spawned guard into App or loop module; fix ports row-height -2 offset` — findings #9/#10 (optional polish, safe to defer).

---

_Reviewer: delegated TUI architecture/subagent (read-only)._

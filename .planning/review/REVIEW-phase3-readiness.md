# Phase 3 Readiness Review — Real-Time Monitoring & History

**Reviewed:** 2026-08-01 (read-only analysis; no files modified)
**Scope:** Phase 3 of ROADMAP.md — ETW-driven refresh (SCAN-05), SQLite port history (HIST-01..04), per-port/per-process traffic with sparklines (TRAF-01..03), EventBus decoupling (CORE-03), 2s polling fallback
**Sources grounded in:** `.planning/STATE.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/research/{ARCHITECTURE,STACK,PITFALLS,FEATURES}.md`, `.planning/phases/01-tui-port-viewer/SKELETON.md`, `.planning/phases/02-process-management-smart-kill/02-RESEARCH.md`, and the current code in `port-core/src` and `port-tui/src`.

> **Headline:** The Phase 3 data models and trait stubs already exist and are 80% compatible with the plan; what is entirely missing is the *plumbing* — EventBus, ETW monitor (ferrisetw), SQLite schema/migrations, the history diff/recorder, traffic counters, and the polling fallback. The TEA message loop in the TUI is the correct integration seam and needs only additive variants, not restructuring. One planning-doc contradiction must be resolved before planning: **STACK.md recommends `windows` 0.73 + `ferrisetw` 1.2.0, but STATE.md locks `windows` 0.62** (STATE.md:48, 02-RESEARCH.md:95) — ferrisetw's transitive dependency tree must be verified against 0.62 before Wave 1.

---

## 1. Gap Analysis

Legend: **Exists** = usable code today; **Stub** = type/trait declared, no implementation; **Missing** = absent; **Plan-only** = described in docs, not in code.

| Phase 3 feature (req) | Exists today (file:line) | Missing today | Minimal API/core changes needed |
|---|---|---|---|
| **EventBus / CORE-03** | Nothing in code. TUI's de-facto event path is `tokio::sync::mpsc::unbounded_channel` (port-tui/src/main.rs:83) — single producer channel, not a bus. ARCHITECTURE.md:374-402 specifies `broadcast::channel(256)` + `CoreEvent` enum; plan-only. | `events` module, `CoreEvent` enum, `EventBus` (`subscribe`/`publish`), subscriber tasks. | Add `port-core/src/events.rs` (+ `events/bus.rs` per new-style layout). `CoreEvent` must be `Clone` (`Connection` is already `Clone`, models/connection.rs:7). Add `pub mod events` to lib.rs (lib.rs:26-38). |
| **ETW change trigger / SCAN-05** | Scan pipeline ready to be *driven*: `scan_all()` is async, everything blocking inside `spawn_blocking` (scanner.rs:42-86, tcp.rs:356, udp.rs), TUI `spawn_scan()` (main.rs:116-128) and the 5s auto-refresh (main.rs:47, 733-742) already refresh the table. | ferrisetw dependency (absent from root Cargo.toml:15-45), `monitor/etw.rs` kernel trace session, startup orphan cleanup (PITFALLS #5), lock-free callback→async bridge (PITFALLS #6), ETW→"refresh please" wiring, debounce against concurrent scans. | Add `monitor.rs` + `monitor/etw.rs` + `monitor/poller.rs` (ARCHITECTURE.md:114-117). New pub API: `EtwMonitor::start/stop` + event → bus. Reuse `scanner::scan_all()` as the ground-truth trigger (STATE.md:43, PITFALLS #15). |
| **2s polling fallback / SCAN-05, TRAF-03** | Auto-refresh exists but is a fixed 5s full TCP+UDP poll (main.rs:47, AUTO_REFRESH_INTERVAL), i.e. the *old* model the phase is replacing. | Mode-aware scheduling: when ETW is live, TCP refreshes on events and only UDP polls at 2s (ETW TCPIP provider does not cover UDP; FEATURES.md:110, ROADMAP.md:79, 85); when ETW is unavailable (non-admin or start failure), full 2s poll. | `monitor/poller.rs`: a 2s timer task publishing `PollTick` events to the bus; replace/augment the 5s constant in main.rs:47. Decision to record: drop or keep the 5s auto-refresh as a safety net. |
| **History recording / HIST-01** | Model + trait: `HistoryEntry`/`HistoryEvent`/`HistoryFilter` (models/connection.rs:17-40), `HistoryStore` trait with `record`/`query` (history.rs:3-6). `Connection` carries `bytes_sent`/`bytes_received` fields (models/connection.rs:12-13) — currently hardcoded `0` in both scanners (tcp.rs:294-295, 344-345). | SQLite impl, append-only `port_events` table, schema migration, **snapshot-diff logic** (occupied/released/changed): the only diff today is TUI-side view-merging `merge_scan_results` keyed on `(port.number, protocol)` (update.rs:506-535) — not a persisted event stream, and it lives in the frontend (Anti-Pattern #1 form). | `store/history.rs`: `SqliteHistoryStore` implementing `HistoryStore` + diff function in port-core (e.g. `diff_snapshots(prev, next) -> Vec<HistoryEntry>`). Recorder task subscribes to the bus and writes (ARCHITECTURE.md:582). |
| **History query / HIST-02, HIST-03** | `HistoryFilter` already models port/pid/name/since/limit (models/connection.rs:33-40). | Any query implementation (SQL), TUI History tab rendering + search input. | Extend `HistoryStore` trait with `query` impl in `store/history.rs`; TUI reuses the Search-overlay input pattern for the query string; HISTORY tab replaces placeholder (components/history.rs:17-61). |
| **Auto-prune / HIST-04** | Nothing. `AppSettings` has no retention field (config/settings.rs:10-27). `store/connection.rs` only creates the `settings` table + `schema_version=1` (connection.rs:43-60); no migrations framework. | retention setting (`default 30 days` per ROADMAP.md:89, REQUIREMENTS.md:44), prune job (daily or on write), schema-version migration to 2. | Add `history_retention_days: u64` with `#[serde(default = "default_history_retention_days")]` (precedent: `kill_timeout_secs` + its backward-compat test, settings.rs:34-36, 123-137). Add `store/migrations.rs` (ARCHITECTURE.md:124). |
| **Traffic counters / TRAF-01, TRAF-03** | `TrafficStats` model with rates (models/connection.rs:43-53), `TrafficMonitor` trait `stats/start_monitoring/stop_monitoring` (traffic.rs:3-7). `Connection.bytes_sent/received` fields ready to be filled. | Counter accumulation from ETW `Microsoft-Windows-Kernel-Network` events aggregated per (pid, protocol, local port) into ~1s snapshots (ARCHITECTURE.md:459-479); no per-connection EStats polling decision made (FEATURES.md:37 suggests `GetPerTcpConnectionEStats` as the alternative — pick ONE source). | `monitor/traffic.rs` (or fold into `monitor/etw.rs`): `WindowsTrafficMonitor` implementing the trait; publish `TrafficUpdate` to the bus every ~1s (ARCHITECTURE.md:473). |
| **Sparklines / TRAF-02** | Nothing. TUI Traffic tab is a placeholder (components/traffic.rs:17-61). ratatui has no built-in sparkline widget; ratatui-widgets `tui-widgets` meta-package (STACK.md:60) or a tiny hand-rolled `▁▂▃▄▅▆▇█` renderer is needed. | Ring buffer of last N rate samples per row (e.g. 60 × 1s); table column rendering. | TUI-only: `TrafficTabComponent` table with a sparkline column + totals; per-row sample history in `App` state. |
| **Status-bar live indicator / ROADMAP success crit. 1** | Status bar already shows `"Live · {n} ports · {time}"` (main.rs:1275) and scanning/error states. | `"Live (ETW)"` vs `"Live (polling)"` mode string + a last-updated timestamp. | TUI-only string change driven by a new bus/live-mode message. |

**Doc-vs-code contradictions to flag:**
1. **windows version:** STACK.md:23,25 says `windows 0.73` and `ferrisetw 1.2.0`; the project pins `windows 0.62` (root Cargo.toml:21-36) per STATE.md:48 and 02-RESEARCH.md:95 ("Do NOT upgrade to 0.73"). STACK.md is stale on this axis; phase planning must verify **ferrisetw 1.2.0's transitive deps** (it may pull a second `windows-sys`/`windows` version → double compilation + possible API friction, Pitfall #10 family).
2. **Traffic data source:** ARCHITECTURE.md:78,459-479 uses the ETW **Kernel-Network** provider (`ferrisetw::KernelTrace`); FEATURES.md:37 suggests **GetPerTcpConnectionEStats**. These are different mechanisms with different privilege/accuracy profiles — the plan must commit to one (recommendation below: ETW per-flow counters for the live path, EStats rejected as it requires per-connection handles held across the refresh cycle).
3. **Event volume / recorder guarantees:** ARCHITECTURE.md:372 and :582 say the history recorder should use a `ringbuf` crate to "guarantee delivery" while bus capacity is 256 — the report recommends instead a bounded `tokio::sync::mpsc` writer with its own backpressure (see §2) because the recorder is the one consumer that must not drop.
4. **Stale traceability/state:** REQUIREMENTS.md:156-161 lists CORE-01/04/05/06 as "Phase 1 Pending" and STATE.md:102 still says "ready to plan Phase 2" — phase-traceability and blockers sections lag reality (WAL init, config, workspace are done). Cosmetic, but it will confuse the Phase 3 planner's requirement sweep.

---

## 2. Recommended Core-First Module Plan

All business logic goes in `port-core` (Anti-Pattern #1/2). TUI changes are limited to additive Message variants, App state, and component rendering.

### 2.1 New modules (new-style layout: `name.rs` + `name/` dir, never `mod.rs` — CLAUDE.md rule)

```
port-core/src/
  events.rs            # pub mod bus; re-export EventBus, CoreEvent
  events/bus.rs        # broadcast::channel(256); EventBus{tx}; subscribe/publish (ARCHITECTURE.md:386-402)
  monitor.rs           # pub mod etw; pub mod poller; pub mod traffic; re-exports
  monitor/etw.rs       # KernelTrace session (ferrisetw), fixed session name, orphan cleanup,
                       #   lock-free callback bridge (crossbeam or tokio mpsc), event→bus
  monitor/poller.rs    # 2s timer task (tokio::time::interval); publishes PollTick / triggers scan_all
  monitor/traffic.rs   # per-(pid,protocol,local-port) byte counters; 1s snapshots → TrafficUpdate
  store/migrations.rs  # schema versioning (settings.schema_version 1 → 2), idempotent CREATE TABLEs
  store/history.rs     # SqliteHistoryStore: record/query/prune + diff_snapshots(prev,next)
```

`lib.rs` additions (lib.rs:26-38 region): `pub mod events; pub mod monitor;` plus re-exports (`pub use events::*; pub use monitor::*;`).

### 2.2 How the modules plug into the existing scanner + TEA pattern

The codebase's concurrency spine is already correct and must be reused, not replaced:

- **Ground truth stays `scan_all()`** (scanner.rs:42) — ETW events only set `app.scanning = true`-equivalent and request a scan. Never render ETW-attributed data (PITFALLS #15; STATE.md:43).
- **ETW callback path (PITFALLS #6):** the ferrisetw `EventRecordCallback` does exactly one thing — push a minimal raw event (`enum RawNetEvent { TcpConnectAttempt{addr,port}, TcpConnect, TcpDisconnect, ... }` or even just `bool`) into a lock-free channel, then returns. No Win32 calls, no mutex, no I/O, no allocation beyond the push. `ProcessTrace` runs on a dedicated thread (PITFALLS integration gotcha, PITFALLS.md:465).
- **Consumer side:** a tokio task drains that channel, publishes `CoreEvent::NetworkChanged` to the bus; the TUI's existing subscribe task (see 2.4) turns it into `Message::EtwTickle` → the existing `spawn_scan`/`scan_spawned` guard machinery (main.rs:116-128, 726-730) does the rescan with spawn_blocking inside port-core. Debounce bursts (≥1 scan per ~500ms) so 100 conn/s ETW storms cannot pile up scans.
- **History recorder (HIST-01):** a port-core-owned task subscribes to the bus. On each `CoreEvent::PortsScanned(Vec<Connection>)` it diffs against the previous snapshot: key = `(port.number, protocol)`; new key not in old → `Occupied`; old key missing in new → `Released`; same key, different owning PID → `Changed`. Writes batch (accumulate ~100ms or ≥N events, one transaction — ARCHITECTURE.md:510) via a **single writer** on `spawn_blocking` (rusqlite is sync; Pitfall #9/#12). This is the one consumer that must not drop events, so its channel is a bounded mpsc with the writer applying backpressure by blocking the bus→recorder hand-off (or a `crossbeam` channel); do **not** rely on broadcast's drop policy (ARCHITECTURE.md:372 overstates the ringbuf need).
- **Traffic monitor (TRAF-01/03):** ETW Kernel-Network flow events accumulate into `HashMap<(u32 pid, Protocol, u16 port), (u64 sent, u64 recv)>` (writes from the etw consumer task — never the callback). Every ~1s, publish `TrafficUpdate` with per-snapshot deltas → rates (ARCHITECTURE.md:473). Non-admin or no-ETW mode: Traffic tab degrades to "counters unavailable (elevate for ETW)" — consistent with SCAN-07's non-admin posture.
- **Status/live mode (ROADMAP crit. 1):** bus publishes `CoreEvent::LiveMode { etw: bool, last_update: Instant }`; TUI renders `Live (ETW)` vs `Live (polling)` + last-updated in the status bar (main.rs:1275).

### 2.3 SQLite schema draft + WAL note

Reference schema (create idempotently in `store/migrations.rs`; bump `settings.schema_version` to `2` — currently inserted as `'1'` in connection.rs:54-60):

```sql
-- WAL + busy_timeout already enforced per-connection in init_db (connection.rs:16-40) —
-- those pragmas are connection-level and must keep running on EVERY new connection (Pitfall #12).
CREATE TABLE IF NOT EXISTS port_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- append-only: inserts only; no UPDATE/DELETE except prunes
    ts            TEXT    NOT NULL,                    -- ISO-8601 UTC (chrono already in deps; Cargo.toml:17)
    event         TEXT    NOT NULL CHECK (event IN ('occupied','released','changed')),
    port          INTEGER NOT NULL,
    protocol      TEXT    NOT NULL CHECK (protocol IN ('tcp','udp','tcp6','udp6')),
    state         TEXT,                                -- PortState at event time (May be NULL for UDP)
    pid           INTEGER,
    process_name  TEXT,
    process_path  TEXT,
    remote_address TEXT,
    remote_port   INTEGER
);
CREATE INDEX IF NOT EXISTS idx_events_ts   ON port_events(ts);
CREATE INDEX IF NOT EXISTS idx_events_port ON port_events(port);
CREATE INDEX IF NOT EXISTS idx_events_pid  ON port_events(pid);
CREATE INDEX IF NOT EXISTS idx_events_name ON port_events(process_name);

-- HIST-04 prune (run by the recorder task once per day / on startup):
-- DELETE FROM port_events WHERE ts < datetime('now', '-' || ?retention_days || ' days');
```

**WAL note:** WAL is already the project norm (STATE.md:39; connection.rs:17-34 verifies `journal_mode=wal`). Keep the existing `init_db` contract for shared access; add a **single-writer rule** (all inserts/updates go through the one recorder task's `spawn_blocking` scope; queries from the TUI history tab can run read-only on a second connection) — this satisfies "No BUSY errors, dual-frontend" (STATE.md:113). Indexes on `ts` + the query axes cover HIST-02 ("query by port/PID/name/date-range") and the prune DELETE. Append-only is a policy enforced by code (only recorder writes; prune is the sole DELETE) — `AUTOINCREMENT` guards ID reuse, not deletion.

### 2.4 EventBus wiring — who publishes, who subscribes

Producers (publish to `EventBus`): scanner result flow (`PortsScanned`), ETW monitor (`NetworkChanged`), traffic monitor (`TrafficUpdate`), poller (`PollTick`). Consumer paths: **history recorder** (port-core task) and **TUI adapter** (a small task spawned in `main()`/`run_event_loop` that converts `CoreEvent → Message` and forwards into the existing mpsc — mirrors the GUI forwarding adapter ARCHITECTURE.md:338-346). The TUI keeps its single-threaded TEA loop; the bus never touches `App` directly. Key constraint: `update()` cannot spawn (documented in main.rs:627-628) — every bus task is spawned from `main()` or the `run_event_loop` intercept arms, exactly like the existing `spawn_scan`/kill/detail/whitelist tasks (main.rs:116-128, 278-335, 559-578).

### 2.5 Settings additions

`AppSettings` gains one field (config/settings.rs:10-27): `history_retention_days: u64` with `#[serde(default = "default_history_retention_days")]` returning `30`. Update `default_settings()` (settings.rs:39-46) and add a backward-compat test in the existing style (Phase-2-era TOML parses with `retention == 30`) — the exact precedent is `serde_defaults_on_phase1_era_toml` (settings.rs:123-137). No other settings surface is needed in Phase 3 (poll interval stays constant at 2s per ROADMAP).

---

## 3. Ordering / Waves (mvp mode — every wave ends user-visible)

Dependency logic: EventsBus ≥ ETW monitor (bus is the rabbit hole ETW results flow through); history needs the bus + diffs of scan results (can ride the first waves); traffic needs the ETW monitor's packet path + bus; pruning is a history-store concern. Proposed 4 waves:

**Wave 3.1 — Live refresh (SCAN-05, CORE-03, ROADMAP crit. 1).** EventBus module + tests → ETW monitor with startup orphan cleanup (fixed session name e.g. `Portunity-TCP-Monitor`, PITFALLS #5), lock-free callback bridge (#6), 2s polling fallback incl. UDP-only mode and non-admin degradation. Wire bus → TUI adapter → existing scan machinery; status bar shows `Live (ETW)` / `Live (polling)` + last-updated. **User-visible:** the Ports table updates on its own; the 5s brute-force auto-refresh (main.rs:47) is retired/relegated to safety net. Verifiable: kill/start a server → table changes without pressing `r`; `logman query -ets` shows no orphaned session after a hard kill.

**Wave 3.2 — History timeline (HIST-01..03).** `store/migrations.rs` (schema v2) + `store/history.rs` (SqliteHistoryStore + `diff_snapshots`) → recorder task on the bus → History tab replaces placeholder (components/history.rs): timeline table + reuse of the Search-overlay input pattern for query (port/PID/name/date-range → HistoryFilter). Prune job (HIST-04) lands here too. **User-visible:** History tab shows occupied/released/changed timeline, queryable. Verifiable: open/close a listener 5 times → 10+ rows; query by PID filters correctly.

**Wave 3.3 — Traffic tab with sparklines (TRAF-01..03, CORE-03).** ETW Kernel-Network counter accumulation + 1s `TrafficUpdate` snapshots → `TrafficTabComponent` table (totals + rate) with a sparkline column (60-sample ring buffer; hand-rolled `▁▂▃▄▅▆▇█` or tui-widgets sparkline). Non-ETW mode shows graceful "elevate for live traffic" empty state. **User-visible:** Traffic tab shows per-port/per-process bytes sent/recv with rate trends; data refreshes on the same cycle as the port list (TRAF-03).

**Wave 3.4 — Hardening & verification.** Pitfall torture tests (#5 kill-and-relaunch orphan check, #6 100 conn/s `EventsLost=0` — STATE.md:111, #15 PID cross-reference vs `GetExtendedTcpTable`), dual-frontend SQLite concurrency check (STATE.md:113) with TUI+GUI both opening the DB, retention edge cases (0 days, huge history), Help overlay + footer updates for the new keys, end-of-phase UAT per `human_verify_mode` (config.json:32). This wave is mostly tests/verification docs, matching the 02-03 precedent (REVIEW coverage).

---

## 4. Risks & Pitfalls Mapping (PITFALLS.md numbers)

| Pitfall | Where it bites Phase 3 | Mitigation (grounded in existing code) |
|---|---|---|
| **#5 ETW session orphaning** (PITFALLS.md:106-129) | Session survives crash → kernel session quota exhaustion; next start fails cryptically. The TUI already has a clean exit path (main.rs:108-112) but no ETW teardown. | Fixed well-known session name; on start, stop any pre-existing session (handle `ERROR_ALREADY_EXISTS`); register `SetConsoleCtrlHandler` (windows 0.62 `Win32_System_Console` already enabled, Cargo.toml:34); call stop in the TUI exit path; keep `ferrisetw` `StopOnDispose` as second line. Verification per PITFALLS.md:527. |
| **#6 ETW callback blocking** (PITFALLS.md:131-156) | Callback does process lookup/DB write → dropped events under load; silent staleness. | Callback = single lock-free channel push (raw event only). All parsing, PID resolution, and SQLite writes happen in consumer tasks (which have access to spawn_blocking). Audit rule: callback body must stay <20 lines, no mutex, no Win32, no I/O. Gate test: `EventsLost == 0` at 100 conn/s (STATE.md:111). |
| **#9 async blocking** (PITFALLS.md:215-248) | rusqlite writes, scans, or ETW session calls on the tokio runtime stall the UI. | The scan path is already `spawn_blocking`-wrapped (scanner.rs:42-86, tcp.rs:356, udp.rs). Extend the same rule: recorder writes and `ProcessTrace` (blocking by design — PITFALLS.md:465) run on dedicated threads; the TUI never calls any of it directly. Reuse the documented pattern from main.rs intercepts. |
| **#15 ETW PID inaccuracy (~30%)** (PITFALLS.md:412-435) | Trusting event payload PIDs misattributes ports (PID 0/System). | ETW is a *trigger only*; every refresh re-runs `scan_all()` whose data is displayed (STATE.md:43 decision; already how the scanner works). Never display event-attributed PID. Cross-reference test comparing event PID vs table PID. |
| **#12 SQLite WAL omission / BUSY** (PITFALLS.md:327-353) | Dual-frontend write contention; TUI history queries blocked by recorder writes. | WAL + `busy_timeout=5000` already enforced per connection (connection.rs:16-40) — keep the per-connection pragma practice on every new connection (the planned second reader connection must re-run them). Single-writer discipline via the recorder task. Dual-frontend test is Wave 3.4. |
| **#8 std Mutex across .await** (PITFALLS.md:187-212) | Sharing counters/state between async tasks. | The TUI already centralizes state in `App` on the main loop (app.rs:64-226) and moves owned data into spawn_blocking closures — no shared mutex pattern to reuse. Keep counters owned by the single traffic task; hand snapshots over the channel. |
| **#13 Ratatui table lag / #10 workspace / #14 allocator** | History/traffic tables at scale; ferrisetw's dep tree; ETW FFI allocation. | Reuse the VirtualTable pattern from the Ports tab for the two new tables; render viewport rows only. Verify ferrisetw's transitive deps do not introduce a second windows/allocator mismatch (Pitfall #14 applies only to raw FFI — ferrisetw wraps ETW, so rely on the crate; audit only our boundary). |
| **Performance traps** (PITFALLS.md:473-487) | Full-table rebuild every 2s poll (line 487); sync disk I/O in callback path (line 483); per-second bus storms. | ETW-driven refresh removes idle polling; poll only UDP when ETW is live; recorder batches writes (~100ms); bus capacity 256 with consumers that drain fast; debounce scans. |

**Cross-cutting risk:** ferrisetw + windows crate version skew (see §1 contradiction #1). Mitigation: perform a `cargo tree -i windows -i windows-sys` check in Wave 1 planning; if ferrisetw forces a second major version, isolate it behind the `monitor/etw.rs` module boundary (the only file that imports ferrisetw) so the rest of port-core/port-tui keeps 0.62 types.

---

## 5. API Compatibility Checklist

### port-core public surface (lib.rs:26-38, models, traits)

| Item | Status | Impact | Action |
|---|---|---|---|
| `lib.rs` module list | No `events`/`monitor` | Additive | `pub mod events; pub mod monitor;` + re-exports — zero breakage. |
| `Connection` (models/connection.rs:7-14) | Unchanged | Additive | `bytes_sent`/`bytes_received` already exist (tcp.rs:294-295 sets them to 0) — traffic layer will populate them; no field additions needed. |
| `HistoryEntry`/`HistoryEvent`/`HistoryFilter` (models/connection.rs:17-40) | Model matches HIST-01/02 | Compatible | `remote_address`/`remote_port` are **optional** additions (FEATURES.md:36 records them; the model omits them) — add as `Option` fields only if the planner wants remote endpoints in the timeline. Add serde derives now (cheap; needed by GUI in Phase 5, ARCHITECTURE.md:579) — additive. |
| `HistoryStore` trait (history.rs:3-6) | Too thin | Extend | Keep `record`/`query`; add `prune(retention_days)` + `diff_snapshots` (free fn or associated). No existing implementor breaks — trait has no impls today. |
| `TrafficMonitor` trait (traffic.rs:3-7) | Shape OK | Extend | Keep `start/stop/stats`; add a rate/sample accessor if the TUI needs incremental deltas instead of full snapshots. No implementors exist — safe. |
| `AppSettings` (config/settings.rs:10-27) | Compatible | Additive | `history_retention_days` + serde default + default_settings() + backward-compat test. Proven pattern (settings.rs:123-137). |
| `scanner::{PortScanner, scan_all, scan_tcp, scan_udp}` + `WindowsPortScanner` (scanner.rs:28-35, 42-86; windows.rs:16-47) | Unchanged | None | ETW/poller only *invoke* scan_all; signatures untouched. |
| `ProcessSnapshot` (process/handle.rs:30-43) | Unchanged | None | Already the Send-safe channel identity; ETW consumer tasks never touch handles. |
| `store::connection::init_db` (connection.rs:11-63) | Compatible | Additive | Keep contract; migrations.rs calls it then runs v2 DDL. No signature change. |
| New: `CoreEvent`, `EventBus`, `EtwMonitor`, `SqliteHistoryStore` | New public API | Additive | Must be `Send + Clone` where they cross async boundaries. |

### Ripple into the TUI (must be planned, not silent)

- **message.rs (port-tui/src/message.rs:80+):** add variants (`EtwTickle`/`LiveMode{etw}`, `TrafficUpdate(Vec<TrafficStats>)`, `HistoryLoaded(Vec<HistoryEntry>)`, `HistoryQuery(String)` or reuse of the Search overlay for the History tab, `PollTick` if the poller talks to the TUI directly). Namespaced per the existing per-domain style.
- **app.rs (app.rs:64-226):** new state blocks — traffic snapshots + per-row sparkline ring buffers, history results + query input/buffer/cursor (reuse the search/filter field-buffer pattern, app.rs:121-123), live-mode flag for the status bar.
- **main.rs:** one bus≈adapter task spawned in `main()`/`run_event_loop` (mirrors main.rs:116-128 spawn pattern); 5s auto-refresh constant (main.rs:47) replaced/augmented by the poller; new drain arms for the ETW/traffic/history messages (pattern exists at main.rs:607-724). Tab dispatch at main.rs:1081-1083 swaps the two placeholders.
- **components/history.rs & traffic.rs:** replace the "Coming later" placeholders (components/history.rs:34-46, traffic.rs:34-46) with real tables; reuse the VirtualTable approach.
- **update.rs:** handlers mirroring the new messages (pattern at update.rs:26-52); keep `merge_scan_results` (update.rs:506-535) — it is the model for the port-core diff, and the diff must not be duplicated in the frontend (move the *event-classification* logic to port-core; the TUI keeps only view-merging).
- **Help overlay + status/footer strings:** add new key documentation and the `Live (ETW)` indicator (main.rs:1275); string lock tests precedent at update.rs:757-791.

**Bottom line:** no existing public port-core API breaks; all Phase 3 additions are additive new modules/types/fields. The only behavioral change that touches existing UI copy is the status-bar live-mode string, which the ROADMAP explicitly requires (crit. 1). Plan the TUI ripple as Wave-scoped additive work, not a refactor.
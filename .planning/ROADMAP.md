# Roadmap: Portunity

**Project:** Windows Port Management Tool (Dual Frontend: Tauri GUI + Ratatui TUI)
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.
**Created:** 2026-07-26
**Granularity:** Standard (6 phases)
**Mode:** mvp (every phase delivers end-to-end user-visible capability)
**Milestone:** 1 (all v1 features in one milestone)

## Phases

- [x] **Phase 1: TUI Port Viewer** — Users launch the terminal app and view a live, sortable, filterable table of all TCP/UDP ports with owning process details, color-coded by connection state. Admins can elevate.
- [ ] **Phase 2: Process Management & Smart Kill** — Users inspect full process details for any port and terminate owning processes with smart kill escalation and whitelist-gated protection.
- [ ] **Phase 3: Real-Time Monitoring & History** — Port listing updates automatically via ETW; users monitor per-port traffic rates with sparklines and browse the port occupation change history timeline.
- [ ] **Phase 4: Firewall Management & Export** — Users manage Windows Firewall rules (view, create, delete, enable/disable) with right-click quick actions, and export port data as JSON/CSV/clipboard.
- [ ] **Phase 5: Desktop GUI** — Users have a native Windows desktop app (Tauri + Svelte) with reactive port table, system tray integration, and all Phase 1-4 capabilities in a polished UI.
- [ ] **Phase 6: Polish — Labels, Favorites, Themes & i18n** — Users get auto-labeled common ports, custom labels, bookmarked favorites, 6 switchable visual themes, and Chinese/English language toggle.

## Phase Details

### Phase 1: TUI Port Viewer

**Goal:** Users can launch the terminal application and view a live, sortable, filterable table of all active TCP and UDP ports with owning process details, color-coded by connection state. Admin elevation is auto-detected and offered when needed.
**Mode:** mvp
**Depends on:** Nothing (first phase)
**Requirements:** CORE-01, CORE-02, CORE-04, CORE-05, CORE-06, SCAN-01, SCAN-02, SCAN-03, SCAN-04, SCAN-06, SCAN-07, TUI-01, TUI-02, TUI-03, TUI-04, TUI-07, TUI-08, SRCH-01, SRCH-03
**Success Criteria** (what must be TRUE):

  1. User launches `port-tui` at 80x24 terminal or larger and sees a table of all active TCP ports (with connection state) and UDP ports, each showing owning process name and PID
  2. Connection states are visually distinguishable by color (LISTENING=green, ESTABLISHED=blue, TIME_WAIT=gray, CLOSE_WAIT=yellow) and the user can sort by any column header with preserved sort order across manual refreshes
  3. User presses `/` to open a fuzzy-search bar; the port table filters in real-time as they type across all fields
  4. User can apply combined filters (port number range, process name substring, PID, protocol, connection state) and see results update immediately
  5. Running without admin rights, the app still displays all ports read-only with a clear indication of which system-owned processes have limited detail; the app auto-detects when admin rights are needed and offers a UAC elevation prompt

**Plans:** 4/4 plans executed

Plans:

- [x] 01-01-PLAN.md — Walking Skeleton: workspace scaffold, Windows TCP scanner, TUI event loop rendering live port table
- [x] 01-02-PLAN.md — Scanner completeness (dual-stack, UDP, retry) + TUI polish (DataTable, sort, colors, keyboard nav, auto-refresh)
- [x] 01-03-PLAN.md — Filter engine + fuzzy search (/ key) + filter panel (f key) + admin elevation (ShellExecuteExW UAC relaunch)
- [x] 01-04-PLAN.md — Overview tab + placeholder tabs + tab bar interaction + resize gate (80x24 min) + release build optimization + SKELETON.md

**UI hint:** yes

### Phase 2: Process Management & Smart Kill

**Goal:** Users can inspect detailed process information for any port's owning process and terminate it with smart kill escalation (graceful shutdown, timeout, force kill) and whitelist-gated protection against accidental termination of system-critical processes.
**Mode:** mvp
**Depends on:** Phase 1 (port list provides selection targets; scanner supplies PID-to-process mapping)
**Requirements:** PROC-01, PROC-02, PROC-03, PROC-04, PROC-05, PROC-06, PROC-07, SRCH-02
**Success Criteria** (what must be TRUE):

  1. User selects a port and views its owning process details: full executable path, start time, command-line arguments, digital signature status, and parent PID — all in a dedicated detail panel
  2. User terminates the owning process with a single keypress (`k`); for non-whitelisted processes the kill is instant, for whitelisted processes a confirmation dialog appears explaining why protection is in place
  3. When the user kills a process, the app first attempts graceful shutdown (WM_CLOSE for GUI processes), waits a configurable timeout, and forces termination if unresponsive — the outcome (success or failure with reason) is clearly displayed
  4. User cannot accidentally kill system-critical processes (smss.exe, csrss.exe, lsass.exe, svchost.exe, etc.) — attempting to kill a protected process shows a clear, non-technical explanation that this would crash the system
  5. User can add or remove processes from the protection whitelist via settings; whitelist changes take effect immediately without restart

**Plans:** 3 plans

Plans:

- [ ] 02-01-PLAN.md — Smart kill: core escalation pipeline (ProcessSnapshot, whitelist gate, graceful→timeout→force) + TUI kill surface (x key, confirm dialog, status bar outcomes, auto-refresh)
- [ ] 02-02-PLAN.md — Process detail inspection: core detail fetchers (path/cmdline/start/parent/signature), scan-time protection markers, 12-row detail panel, ◆ table markers
- [ ] 02-03-PLAN.md — Whitelist management overlay (w key, validated add/remove, instant effect) + Help overlay (? key) + SRCH-02 verification & traceability

**UI hint:** yes

### Phase 3: Real-Time Monitoring & History

**Goal:** The port listing updates automatically as connections are created, closed, or change state. Users can monitor per-port traffic rates with sparkline indicators and browse a searchable timeline of port occupation events. ETW event-driven refresh eliminates CPU burn during idle; 2s polling catches UDP and edge cases.
**Mode:** mvp
**Depends on:** Phase 1 (port scanner foundation), Phase 2 (PID handling patterns for safe event processing)
**Requirements:** SCAN-05, CORE-03, TRAF-01, TRAF-02, TRAF-03, HIST-01, HIST-02, HIST-03, HIST-04
**Success Criteria** (what must be TRUE):

  1. Port list updates automatically when connections are created, closed, or change state — no manual refresh needed; the status bar shows "Live (ETW)" or an auto-refresh indicator with a last-updated timestamp
  2. User opens the Traffic tab (Tab 4) and sees bytes sent/received per port and per process, with sparkline indicators showing recent rate trends over time
  3. User opens the History tab (Tab 3) and browses a searchable timeline of port occupation events — which port was occupied, released, or changed, by which process, at what time
  4. User queries history by port number, PID, process name, or date range and sees filtered results in the timeline
  5. History entries older than the configured retention period (default 30 days, configurable in settings) are automatically pruned to bound storage growth

**Plans:** TBD
**UI hint:** yes

### Phase 4: Firewall Management & Export

**Goal:** Users can manage Windows Firewall rules (list, create, delete, enable/disable) with right-click quick actions that create sensible defaults, and export the current port list as JSON, CSV, or clipboard text — all from the TUI with admin elevation auto-prompting.
**Mode:** mvp
**Depends on:** Phase 1 (foundation + tab infrastructure), Phase 2 (admin elevation patterns)
**Requirements:** FW-01, FW-02, FW-03, FW-04, FW-05, FW-06, EXP-01, EXP-02, EXP-03
**Success Criteria** (what must be TRUE):

  1. User opens the Firewall tab (Tab 5) and browses, filters, and sorts all Windows Firewall rules (inbound/outbound, allow/block, by port, by program path)
  2. User creates a new firewall rule specifying name, direction, action, protocol, local port, and program path; deletes a user-created rule; and toggles any rule's enabled/disabled state with a single action — admin-required operations auto-prompt for UAC elevation with clear failure messaging on denial
  3. User right-clicks any port entry in the port list and selects "Block this port in Firewall" or "Allow this port in Firewall" — a firewall rule is created with sensible defaults (name, direction, protocol) in a single action
  4. User exports the current port list as JSON (structured, with schema version) or CSV (spreadsheet-compatible, with column headers), and copies selected rows to clipboard as tab-delimited text
  5. Exported data matches the visible, filtered port list exactly — what the user sees is what they get

**Plans:** TBD
**UI hint:** yes

### Phase 5: Desktop GUI

**Goal:** Users launch a native Windows desktop application (Tauri v2 + Svelte) with a reactive port table, system tray integration (popup panel with mini port list, quick actions menu), and all Phase 1-4 capabilities available through a polished Svelte UI without touching the terminal.
**Mode:** mvp
**Depends on:** Phase 1-4 (all port-core APIs must be battle-tested before the GUI wraps them)
**Requirements:** GUI-01, GUI-02, GUI-03, GUI-04
**Success Criteria** (what must be TRUE):

  1. User launches the desktop app and sees a reactive port table with sort, filter, and search matching the TUI's capabilities — all accessible through mouse and keyboard
  2. User can manage firewall rules, view traffic graphs (rate charts replacing sparklines), browse history timelines, and export data — all through the Svelte UI without using the TUI
  3. The system tray icon is visible when the app is open; right-click shows Settings, Open Window, and Quit; left-click opens a popup panel with a mini port list and quick search; double-click opens the main window
  4. Closing the main window can be configured to minimize to tray or fully quit (configurable in settings); admin-required operations (kill, firewall) auto-detect and prompt for UAC elevation with clear, non-technical messaging
  5. All data streams from port-core to the Svelte UI via reactive stores that update automatically when ETW events or scan results arrive — no manual polling in the frontend

**Plans:** TBD
**UI hint:** yes

### Phase 6: Polish — Labels, Favorites, Themes & i18n

**Goal:** Users benefit from auto-labeled common dev ports (PostgreSQL, Redis, Next.js, etc.), can assign custom searchable labels, bookmark favorite ports for quick access, switch between 6 visual themes instantly, and toggle between English and Chinese UI with all strings externalized to shared Fluent FTL files.
**Mode:** mvp
**Depends on:** Phase 1-5 (all features must exist before polish — labels and favorites are overlays on the port list; themes and i18n touch every UI surface)
**Requirements:** SRCH-04, SRCH-05, SRCH-06, TUI-05, TUI-06, GUI-05, GUI-06, I18N-01, I18N-02, I18N-03
**Success Criteria** (what must be TRUE):

  1. User sees common dev ports auto-labeled (5432→PostgreSQL, 3306→MySQL, 6379→Redis, 3000→Next.js, 5173→Vite, 8080→HTTP Dev, etc.) without any configuration — labels appear inline in the port table
  2. User assigns a custom label to any port ("my dev server", "prod DB tunnel") via right-click or keyboard shortcut; custom labels are searchable alongside auto-labels
  3. User bookmarks a port (star/favorite); bookmarked ports appear as a persistent sidebar section or filter preset for one-click access
  4. User presses `t` in TUI or selects from GUI settings to cycle through 6 preset themes (One Dark, Dracula, Solarized, Nord, Monokai, High Contrast); the theme applies instantly to all UI surfaces in both frontends
  5. User presses `l` in TUI or toggles a setting in GUI to switch between English and Chinese; all labels, headers, messages, and UI text switch language immediately, with auto-detection from system locale on first launch

**Plans:** TBD
**UI hint:** yes

## Progress

| Phase | Requirements | Success Criteria | Status | Completed |
|-------|-------------|-----------------|--------|-----------|
| 1. TUI Port Viewer | 19 | 5 | In Progress|  |
| 2. Process Management & Smart Kill | 8 | 5 | Not started | - |
| 3. Real-Time Monitoring & History | 9 | 5 | Not started | - |
| 4. Firewall Management & Export | 9 | 5 | Not started | - |
| 5. Desktop GUI | 4 | 5 | Not started | - |
| 6. Polish — Labels, Favorites, Themes & i18n | 10 | 5 | Not started | - |

## Pitfall Coverage

Documented in [PITFALLS.md](research/PITFALLS.md). Each phase addresses the pitfalls relevant to its domain:

| Phase | Pitfalls Addressed |
|-------|--------------------|
| Phase 1 | #10 (single workspace), #12 (WAL mode), #14 (allocator mismatch) |
| Phase 2 | #1 (PID reuse), #2 (buffer retry), #3 (dual-stack), #4 (byte order), #11 (protected process) |
| Phase 3 | #5 (ETW orphaning), #6 (callback blocking), #9 (async blocking), #15 (ETW PID inaccuracy) |
| Phase 4 | #7 (COM leaks), #9 (async blocking) |
| Phase 5 | #8 (Mutex across .await), #9 (async blocking), anti-patterns #1, #2, #4 |
| Phase 6 | #13 (Ratatui table lag, addressed in Phase 1 — verification pass) |

---

*Roadmap created: 2026-07-26*
*Research-backed: ARCHITECTURE.md, PITFALLS.md, SUMMARY.md*

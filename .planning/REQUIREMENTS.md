# Requirements: Portunity

**Defined:** 2026-07-26
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.

## v1 Requirements

Requirements for initial release. All features ship in one milestone, organized by wave.

### Port Scanning (SCAN)

- [x] **SCAN-01**: User can view all active TCP ports with owning process name, PID, local address:port, remote address:port, and connection state
- [x] **SCAN-02**: User can view all active UDP ports with owning process name, PID, local address:port
- [x] **SCAN-03**: Connection states are color-coded (LISTENING=green, ESTABLISHED=blue, TIME_WAIT=gray, CLOSE_WAIT=yellow)
- [x] **SCAN-04**: User can sort the port table by any column (port number, PID, process name, protocol, state)
- [ ] **SCAN-05**: Port list refreshes via ETW events (TCP connection changes); 2s polling fallback for UDP and edge cases
- [ ] **SCAN-06**: App auto-detects when admin rights are needed and triggers UAC elevation prompt
- [ ] **SCAN-07**: Non-admin users can view all ports read-only; system-owned processes show limited detail until elevated

### Search & Filter (SRCH)

- [ ] **SRCH-01**: User can filter ports by port number (exact or range), process name (substring), PID, protocol (TCP/UDP), and connection state
- [ ] **SRCH-02**: User can combine multiple filter dimensions with AND/OR logic (faceted search)
- [ ] **SRCH-03**: User can fuzzy-search across all fields with a single text input (`/` key in TUI, search bar in GUI)
- [ ] **SRCH-04**: App auto-labels known ports (5432->PostgreSQL, 3306->MySQL, 6379->Redis, 3000->Next.js, 5173->Vite, etc.) — static mapping for ~50 common ports
- [ ] **SRCH-05**: User can assign custom labels to ports ("my dev server", "production DB tunnel"); labels are searchable
- [ ] **SRCH-06**: User can bookmark/favorite ports for quick access; favorites are displayed as a sidebar section or filter preset

### Process Management (PROC)

- [ ] **PROC-01**: User can terminate the process owning a selected port directly from the port list
- [ ] **PROC-02**: Smart kill: app sends graceful shutdown first (WM_CLOSE for GUI, Ctrl+C for console), waits configurable timeout, force-kills (TerminateProcess) if unresponsive
- [ ] **PROC-03**: Instant kill is the default action for non-whitelisted processes; whitelisted processes show confirmation dialog before kill
- [ ] **PROC-04**: Built-in whitelist protects ~30 system-critical processes (smss.exe, csrss.exe, wininit.exe, services.exe, lsass.exe, svchost.exe, winlogon.exe, System, Idle, etc.)
- [ ] **PROC-05**: User can customize the whitelist in settings (add/remove processes by executable path)
- [ ] **PROC-06**: User can view process details: full executable path, start time, command line arguments, digital signature status, parent PID
- [ ] **PROC-07**: Process HANDLE is retained from OpenProcess; PID is never re-derived after storage (PID reuse safety)

### History (HIST)

- [ ] **HIST-01**: App records port occupation changes (occupied, released, changed) with timestamp, port, protocol, and process info to SQLite
- [ ] **HIST-02**: User can query history by port number, PID, process name, or time range
- [ ] **HIST-03**: History is displayed as a searchable timeline in both TUI (Tab 3) and GUI
- [ ] **HIST-04**: Old history entries are auto-pruned to bound storage (configurable retention, default 30 days)

### Traffic Monitoring (TRAF)

- [ ] **TRAF-01**: App displays bytes sent/received per port and per process
- [ ] **TRAF-02**: Traffic stats are displayed in the TUI (Tab 4) as a table with sparkline indicators; in GUI as a panel with rate graphs
- [ ] **TRAF-03**: Traffic counters refresh on the same ETW/polling cycle as the port list (near-zero overhead)

### Firewall (FW)

- [ ] **FW-01**: User can view Windows Firewall rules (filterable: inbound/outbound, allow/block, by port)
- [ ] **FW-02**: User can create a new firewall rule (name, direction, action, protocol, local port, program path)
- [ ] **FW-03**: User can delete a user-created firewall rule
- [ ] **FW-04**: User can enable/disable a firewall rule
- [ ] **FW-05**: User can right-click any port -> "Block this port in Firewall" or "Allow this port in Firewall" — quick actions create rules with sensible defaults
- [ ] **FW-06**: Firewall rule management requires admin elevation; app prompts for elevation on first firewall operation

### Export & Data (EXP)

- [ ] **EXP-01**: User can export current port list as JSON (structured, programmatic consumption)
- [ ] **EXP-02**: User can export current port list as CSV (spreadsheet-compatible)
- [ ] **EXP-03**: User can copy selected rows to clipboard as tab-delimited text

### Core Architecture (CORE)

- [ ] **CORE-01**: Shared `port-core` Rust library defines all models, traits, and platform abstractions — consumed by both frontends
- [ ] **CORE-02**: Platform abstraction via traits (PortScanner, ProcessManager, FirewallManager) with `#[cfg(target_os = "windows")]` Windows implementation
- [ ] **CORE-03**: EventBus (tokio::sync::broadcast) decouples producers (scanner, ETW, process manager) from consumers (TUI tabs, GUI panels, history recorder)
- [ ] **CORE-04**: SQLite database with WAL mode enabled from first connection; schema defined in port-core for shared access
- [ ] **CORE-05**: Config (settings, whitelist, labels, favorites, theme) stored as TOML files in app data directory; hot-reloadable
- [ ] **CORE-06**: Workspace monorepo: `port-core` (lib), `port-tui` (bin), `port-gui` (bin). Unidirectional dependency: frontends depend on core, never reverse.

### TUI (TUI)

- [ ] **TUI-01**: Tab-based Widget Dashboard: [1] Overview, [2] Ports, [3] History, [4] Traffic, [5] Firewall
- [x] **TUI-02**: Keyboard-first navigation: `1`-`5` switch tabs, `Tab`/`Shift+Tab` cycle panels, `/` search, `?` help overlay, `q` quit
- [ ] **TUI-03**: Ratatui Elm Architecture (TEA): centralized Message enum, single update() function, per-tab Component trait
- [x] **TUI-04**: VirtualTable for port list (prevents scroll lag at 1000+ connections)
- [ ] **TUI-05**: Theme system: modular, serde-deserializable theme files (TOML). Switch via `t` key. Presets: One Dark, Dracula, Solarized, Nord, Monokai, High Contrast
- [ ] **TUI-06**: Language toggle: `l` key switches Chinese/English. fluent-i18n based, modular extension points for additional languages
- [ ] **TUI-07**: Works at 80x24 minimum terminal size; graceful degradation on resize; works in tmux/zellij/Windows Terminal
- [ ] **TUI-08**: Flicker-free rendering: double buffering + batched writes + synchronized output

### GUI (GUI)

- [ ] **GUI-01**: Tauri v2 desktop app with Svelte frontend; reactive port table with sort/filter/search
- [ ] **GUI-02**: System tray: icon visible when app is open; close window minimizes to tray or quits (configurable in settings)
- [ ] **GUI-03**: Left-click tray icon: popup panel with active port list + search; double-click: open main window; right-click: settings/open/quit menu
- [ ] **GUI-04**: IPC via Tauri commands wrapping port-core APIs; EventBus events forwarded to Svelte stores via `AppHandle::emit`
- [ ] **GUI-05**: Theme system: matching presets to TUI themes, applied to Svelte UI and Tauri window chrome
- [ ] **GUI-06**: Language toggle in settings; Chinese/English UI

### Internationalization (I18N)

- [ ] **I18N-01**: Default language auto-detected from system locale; user can override in settings
- [ ] **I18N-02**: All user-facing strings externalized to Fluent (FTL) files; adding a language = adding one directory of FTL files
- [ ] **I18N-03**: TUI and GUI share the same FTL source files (in port-core)

## Out of Scope

| Feature | Reason |
|---------|--------|
| TCP active port scanning (connect-scan) | Different product category (nmap). PortScanner trait reserved as extension point. |
| Automatic process killing (watch mode) | Dangerous on shared machines. Watch+notify is safer; user must confirm kill. |
| Raw packet capture / DPI | Different product category (Wireshark). |
| "Kill all" button | Risk of system instability. Batch actions require explicit multi-select. |
| Built-in HTTP server for remote access | Security risk exposing admin-privileged local tool over network. TUI works over SSH natively. |
| Service management (start/stop Windows services) | Different product category (services.msc). Show service name as informational context only. |
| DNS/WHOIS lookup on every connection (auto) | Blocking and slow. Available as opt-in right-click action. |
| Linux/macOS builds | Platform abstraction layer in place; implementations deferred. |
| Mobile support | Not applicable. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| SCAN-01 | Phase 1 | Complete |
| SCAN-02 | Phase 1 | Complete |
| SCAN-03 | Phase 1 | Complete |
| SCAN-04 | Phase 1 | Complete |
| SCAN-05 | Phase 3 | Pending |
| SCAN-06 | Phase 1 | Pending |
| SCAN-07 | Phase 1 | Pending |
| SRCH-01 | Phase 1 | Pending |
| SRCH-02 | Phase 2 | Pending |
| SRCH-03 | Phase 1 | Pending |
| SRCH-04 | Phase 6 | Pending |
| SRCH-05 | Phase 6 | Pending |
| SRCH-06 | Phase 6 | Pending |
| PROC-01 | Phase 2 | Pending |
| PROC-02 | Phase 2 | Pending |
| PROC-03 | Phase 2 | Pending |
| PROC-04 | Phase 2 | Pending |
| PROC-05 | Phase 2 | Pending |
| PROC-06 | Phase 2 | Pending |
| PROC-07 | Phase 2 | Pending |
| HIST-01 | Phase 3 | Pending |
| HIST-02 | Phase 3 | Pending |
| HIST-03 | Phase 3 | Pending |
| HIST-04 | Phase 3 | Pending |
| TRAF-01 | Phase 3 | Pending |
| TRAF-02 | Phase 3 | Pending |
| TRAF-03 | Phase 3 | Pending |
| FW-01 | Phase 4 | Pending |
| FW-02 | Phase 4 | Pending |
| FW-03 | Phase 4 | Pending |
| FW-04 | Phase 4 | Pending |
| FW-05 | Phase 4 | Pending |
| FW-06 | Phase 4 | Pending |
| EXP-01 | Phase 4 | Pending |
| EXP-02 | Phase 4 | Pending |
| EXP-03 | Phase 4 | Pending |
| CORE-01 | Phase 1 | Pending |
| CORE-02 | Phase 1 | Pending |
| CORE-03 | Phase 3 | Pending |
| CORE-04 | Phase 1 | Pending |
| CORE-05 | Phase 1 | Pending |
| CORE-06 | Phase 1 | Pending |
| TUI-01 | Phase 1 | Pending |
| TUI-02 | Phase 1 | Complete |
| TUI-03 | Phase 1 | Pending |
| TUI-04 | Phase 1 | Complete |
| TUI-05 | Phase 6 | Pending |
| TUI-06 | Phase 6 | Pending |
| TUI-07 | Phase 1 | Pending |
| TUI-08 | Phase 1 | Pending |
| GUI-01 | Phase 5 | Pending |
| GUI-02 | Phase 5 | Pending |
| GUI-03 | Phase 5 | Pending |
| GUI-04 | Phase 5 | Pending |
| GUI-05 | Phase 6 | Pending |
| GUI-06 | Phase 6 | Pending |
| I18N-01 | Phase 6 | Pending |
| I18N-02 | Phase 6 | Pending |
| I18N-03 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 59 total
- Mapped to phases: 59 (100%)
- Unmapped: 0

**Phase mapping summary:**

| Phase | Count | Requirement IDs |
|-------|-------|-----------------|
| Phase 1 — TUI Port Viewer | 19 | CORE-01, CORE-02, CORE-04, CORE-05, CORE-06, SCAN-01, SCAN-02, SCAN-03, SCAN-04, SCAN-06, SCAN-07, SRCH-01, SRCH-03, TUI-01, TUI-02, TUI-03, TUI-04, TUI-07, TUI-08 |
| Phase 2 — Process Management & Smart Kill | 8 | PROC-01, PROC-02, PROC-03, PROC-04, PROC-05, PROC-06, PROC-07, SRCH-02 |
| Phase 3 — Real-Time Monitoring & History | 9 | CORE-03, SCAN-05, TRAF-01, TRAF-02, TRAF-03, HIST-01, HIST-02, HIST-03, HIST-04 |
| Phase 4 — Firewall Management & Export | 9 | FW-01, FW-02, FW-03, FW-04, FW-05, FW-06, EXP-01, EXP-02, EXP-03 |
| Phase 5 — Desktop GUI | 4 | GUI-01, GUI-02, GUI-03, GUI-04 |
| Phase 6 — Polish (Labels, Favorites, Themes & i18n) | 10 | SRCH-04, SRCH-05, SRCH-06, TUI-05, TUI-06, GUI-05, GUI-06, I18N-01, I18N-02, I18N-03 |

---
*Requirements defined: 2026-07-26*
*Last updated: 2026-07-26 after roadmap creation (traceability populated)*

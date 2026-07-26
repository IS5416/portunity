# Requirements: Portunity

**Defined:** 2026-07-26
**Core Value:** Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.

## v1 Requirements

Requirements for initial release. All features ship in one milestone, organized by wave.

### Port Scanning (SCAN)

- [ ] **SCAN-01**: User can view all active TCP ports with owning process name, PID, local address:port, remote address:port, and connection state
- [ ] **SCAN-02**: User can view all active UDP ports with owning process name, PID, local address:port
- [ ] **SCAN-03**: Connection states are color-coded (LISTENING=green, ESTABLISHED=blue, TIME_WAIT=gray, CLOSE_WAIT=yellow)
- [ ] **SCAN-04**: User can sort the port table by any column (port number, PID, process name, protocol, state)
- [ ] **SCAN-05**: Port list refreshes via ETW events (TCP connection changes); 2s polling fallback for UDP and edge cases
- [ ] **SCAN-06**: App auto-detects when admin rights are needed and triggers UAC elevation prompt
- [ ] **SCAN-07**: Non-admin users can view all ports read-only; system-owned processes show limited detail until elevated

### Search & Filter (SRCH)

- [ ] **SRCH-01**: User can filter ports by port number (exact or range), process name (substring), PID, protocol (TCP/UDP), and connection state
- [ ] **SRCH-02**: User can combine multiple filter dimensions with AND/OR logic (faceted search)
- [ ] **SRCH-03**: User can fuzzy-search across all fields with a single text input (`/` key in TUI, search bar in GUI)
- [ ] **SRCH-04**: App auto-labels known ports (5432→PostgreSQL, 3306→MySQL, 6379→Redis, 3000→Next.js, 5173→Vite, etc.) — static mapping for ~50 common ports
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
- [ ] **FW-05**: User can right-click any port → "Block this port in Firewall" or "Allow this port in Firewall" — quick actions create rules with sensible defaults
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
- [ ] **TUI-02**: Keyboard-first navigation: `1`-`5` switch tabs, `Tab`/`Shift+Tab` cycle panels, `/` search, `?` help overlay, `q` quit
- [ ] **TUI-03**: Ratatui Elm Architecture (TEA): centralized Message enum, single update() function, per-tab Component trait
- [ ] **TUI-04**: VirtualTable for port list (prevents scroll lag at 1000+ connections)
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
| SCAN-01 | Pending | Pending |
| SCAN-02 | Pending | Pending |
| SCAN-03 | Pending | Pending |
| SCAN-04 | Pending | Pending |
| SCAN-05 | Pending | Pending |
| SCAN-06 | Pending | Pending |
| SCAN-07 | Pending | Pending |
| SRCH-01 | Pending | Pending |
| SRCH-02 | Pending | Pending |
| SRCH-03 | Pending | Pending |
| SRCH-04 | Pending | Pending |
| SRCH-05 | Pending | Pending |
| SRCH-06 | Pending | Pending |
| PROC-01 | Pending | Pending |
| PROC-02 | Pending | Pending |
| PROC-03 | Pending | Pending |
| PROC-04 | Pending | Pending |
| PROC-05 | Pending | Pending |
| PROC-06 | Pending | Pending |
| PROC-07 | Pending | Pending |
| HIST-01 | Pending | Pending |
| HIST-02 | Pending | Pending |
| HIST-03 | Pending | Pending |
| HIST-04 | Pending | Pending |
| TRAF-01 | Pending | Pending |
| TRAF-02 | Pending | Pending |
| TRAF-03 | Pending | Pending |
| FW-01 | Pending | Pending |
| FW-02 | Pending | Pending |
| FW-03 | Pending | Pending |
| FW-04 | Pending | Pending |
| FW-05 | Pending | Pending |
| FW-06 | Pending | Pending |
| EXP-01 | Pending | Pending |
| EXP-02 | Pending | Pending |
| EXP-03 | Pending | Pending |
| CORE-01 | Pending | Pending |
| CORE-02 | Pending | Pending |
| CORE-03 | Pending | Pending |
| CORE-04 | Pending | Pending |
| CORE-05 | Pending | Pending |
| CORE-06 | Pending | Pending |
| TUI-01 | Pending | Pending |
| TUI-02 | Pending | Pending |
| TUI-03 | Pending | Pending |
| TUI-04 | Pending | Pending |
| TUI-05 | Pending | Pending |
| TUI-06 | Pending | Pending |
| TUI-07 | Pending | Pending |
| TUI-08 | Pending | Pending |
| GUI-01 | Pending | Pending |
| GUI-02 | Pending | Pending |
| GUI-03 | Pending | Pending |
| GUI-04 | Pending | Pending |
| GUI-05 | Pending | Pending |
| GUI-06 | Pending | Pending |
| I18N-01 | Pending | Pending |
| I18N-02 | Pending | Pending |
| I18N-03 | Pending | Pending |

**Coverage:**
- v1 requirements: 57 total
- Mapped to phases: 0 (populated during roadmap creation)
- Unmapped: 57

---
*Requirements defined: 2026-07-26*
*Last updated: 2026-07-26 after initial definition*

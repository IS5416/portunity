# Portunity

## What This Is

A high-performance Windows port management tool with classified search, process termination, traffic monitoring, and firewall rule management. Dual frontend — Tauri desktop GUI and Ratatui terminal TUI — sharing a common Rust core library. Built for developers debugging port conflicts, operators managing services, and security auditors investigating connections.

## Core Value

Instantly find, classify, and act on any active port and its owning process — zero friction from discovery to action.

## Requirements

### Validated

(None yet — ship to validate)

### Active

**Port Management:**
- [ ] Multi-dimensional port search and classification (by app name, port range, protocol type, process attributes)
- [ ] Combined/faceted search across all dimensions
- [ ] Process termination with smart kill strategy (SIGTERM/TerminateProcess graceful → timeout → SIGKILL/force)
- [ ] Instant kill as default action; whitelist-gated confirmation dialog for protected processes
- [ ] Built-in system-critical process whitelist; user-customizable whitelist in settings

**Monitoring & History:**
- [ ] Port occupation change history with timestamps (SQLite-backed)
- [ ] Network traffic statistics per port and per process (bytes sent/received)
- [ ] Event-driven refresh via Windows Event Tracing (ETW), 2s polling fallback

**Firewall:**
- [ ] Windows Firewall rule listing, creation, deletion, enable/disable
- [ ] Quick actions: right-click block/allow by port or process

**Process Details:**
- [ ] Process detail panel: executable path, start time, command line args, digital signature status

**Productivity:**
- [ ] Favorites/bookmarks for commonly monitored ports
- [ ] Custom labels ("my dev server", "database")
- [ ] Export port/process info as JSON/CSV

**Platform:**
- [ ] Windows 10/11 primary target
- [ ] Platform abstraction layer in core for future Linux/macOS support

**System Tray (GUI):**
- [ ] Window mode default; tray icon present when window is open
- [ ] Close window: option to fully quit or minimize to tray (configurable in settings)
- [ ] Left-click tray icon: popup panel with active port list + search
- [ ] Double-click tray icon: open main window
- [ ] Right-click tray icon: settings, open window, quit, etc.

**UI/UX Polish:**
- [ ] Theme system: modular, multiple presets (One Dark, Dracula, Solarized, Nord, Monokai, High Contrast, etc.), switchable via `t` key in TUI
- [ ] i18n: Chinese/English toggle via `l` key in TUI, modular extension points for additional languages
- [ ] Admin elevation: auto-detect when admin rights needed, trigger UAC prompt

### Out of Scope

- TCP active port scanning (connect-scan range probing) — extension point reserved in `PortScanner` trait
- Linux/macOS builds — platform abstraction layer in place, implementations deferred
- Mobile support — not applicable

## Context

- **Developer pain point:** Port 3000/8080/5432 already in use, hunt through netstat output, copy PID, kill manually. Repeat daily.
- **No existing solution:** Windows lacks a dedicated port manager. netstat + taskkill is the workflow; Resource Monitor is buried and read-only.
- **Rust ecosystem fit:** Tauri v2 (desktop GUI), Ratatui (terminal TUI), both Rust-native. Shared core crate avoids logic duplication.
- **Windows APIs available:** IP Helper API (`GetExtendedTcpTable`, `GetExtendedUdpTable`), Event Tracing for Windows (ETW) for connection change events, Windows Filtering Platform (WFP) for firewall, `TerminateProcess` for kill.

## Constraints

- **Tech stack:** Rust (edition 2024), Tauri v2 + Svelte (GUI), Ratatui (TUI)
- **Platform:** Windows 10/11 primary; Linux and macOS extension points only
- **Performance:** Port scan and refresh must not block UI; async runtime (tokio)
- **Storage:** SQLite for history, favorites, labels, settings
- **Safety:** System-critical processes must not be killable without confirmation; whitelist extensible

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Shared core crate (`port-core`) | One source of truth for port scanning, process management, filtering. Both frontends consume it. | — Pending |
| Tauri v2 over v1 | Latest stable, better IPC perf, plugin system, mobile-capable. v1 is legacy. | — Pending |
| Svelte over React/Vue for GUI | Compiled output, no virtual DOM runtime, smallest bundle. Tauri's recommended pairing. | — Pending |
| Tab-based Widget Dashboard for TUI | 5 distinct function domains (overview, ports, history, traffic, firewall). Tabs give each full canvas. | — Pending |
| SQLite over JSON/sled | Universal, Tauri official plugin, rusqlite mature. JSON doesn't scale for history; sled is niche. | — Pending |
| ETW + polling fallback for refresh | Event-driven is ideal (no CPU burn); 2s poll catches what ETW might miss (non-TCP, rare edge cases). | — Pending |
| Smart kill (graceful → force) | Graceful termination lets servers close cleanly; force kill as last resort. Best UX. | — Pending |
| Instant kill + whitelist confirmation | Most kills are intentional (dev stopping own server). Whitelist protects against fat-fingering system processes. | — Pending |
| System API only for port scanning | `GetExtendedTcpTable`/`GetExtendedUdpTable` covers 100% of local port management use cases. TCP connect scanning is a different tool (nmap). Extension point reserved. | — Pending |
| Auto-detect admin + UAC elevation | Some processes need admin to kill; auto-detect and prompt is better than "run as admin always" or silent failure. | — Pending |
| All v1+v2+v3 features in one milestone | User wants full-featured tool from day one, not incremental MVP. | — Pending |
| English artifacts for agent efficiency | Agents process English more reliably. Chinese PROJECT.zh.md provided for human reading. | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-07-26 after initialization*

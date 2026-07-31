# Portunity

**Port + Opportunity** — A high-performance Windows port management tool.

Find, classify, and manage active ports and their owning processes. Kill with confidence. Monitor traffic. Manage firewall rules. All with zero friction.

## Why

Port 3000 is already in use. You open `netstat`, scan for PID, copy it, run `taskkill`. Repeat daily.

Portunity replaces that workflow. One keystroke to see everything, one keystroke to act.

## Status

**Phase 1 complete** — TUI Port Viewer shipped. Terminal dashboard with live dual-stack TCP/UDP port table, fuzzy search, multi-dimension filters, color-coded connection states, admin elevation. Production-ready with render-on-demand, 1.1MB release binary.

| Phase | Status |
|-------|--------|
| 1 — TUI Port Viewer | ✅ Complete |
| 2 — Process Management & Smart Kill | 🔲 Planned |
| 3 — Real-Time Monitoring & History | 🔲 Planned |
| 4 — Firewall Management & Export | 🔲 Planned |
| 5 — Desktop GUI (Tauri + Svelte) | 🔲 Planned |
| 6 — Labels, Themes & i18n | 🔲 Planned |

## Features (Planned / Implemented)

- ✅ **Live port table** — dual-stack TCP/UDP, color-coded states, zebra stripes, virtual scrolling
- ✅ **Multi-dimensional search** — fuzzy search across all fields with `/` key
- ✅ **Combined filters** — port range, process name, PID, protocol, connection state
- ✅ **Admin elevation** — UAC prompt via ShellExecuteExW, non-admin graceful degradation
- ✅ **Tab dashboard** — Overview (stats + top ports), Ports, placeholder tabs for History/Traffic/Firewall
- ✅ **Keyboard-first** — full navigation, sort, search, filter without touching mouse
- 🔲 **Smart kill** — graceful termination first, force kill if process doesn't respond
- 🔲 **Process details** — executable path, start time, command line, digital signature
- 🔲 **Port history** — SQLite-backed timeline of which process occupied which port, and when
- 🔲 **Traffic monitoring** — per-port, per-process bytes sent/received via ETW
- 🔲 **Firewall rules** — list, create, delete, toggle Windows Firewall rules from the app
- 🔲 **Favorites + labels** — bookmark common ports, tag them ("my dev server", "database")
- 🔲 **Export** — JSON/CSV for sharing with teammates or attaching to bug reports
- 🔲 **Themes** — One Dark, Dracula, Solarized, Nord, Monokai, High Contrast, and more
- 🔲 **i18n** — Chinese/English, modular extension points for more languages

## Two Frontends

| | GUI | TUI |
|---|---|---|
| **Tech** | Tauri v2 + Svelte | Ratatui |
| **Best for** | Daily driver, system tray, mouse | Speed, keyboard, SSH, tmux |
| **Tray** | Popup panel + quick actions | n/a |
| **Themes** | Built-in | `t` key toggle |
| **Status** | 🔲 Phase 5 | ✅ Phase 1 shipped |

Both share the same `port-core` Rust library — identical data, identical logic.

## Quick Start

```bash
# Build everything
cargo build --release

# Terminal TUI (Phase 1 — shipped)
cargo run --bin port-tui
```

## Platform

Windows 10/11 primary. Linux and macOS extension points reserved in the platform abstraction layer.

## License

MIT

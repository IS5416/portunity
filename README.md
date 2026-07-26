# Portunity

**Port + Opportunity** — A high-performance Windows port management tool.

Find, classify, and manage active ports and their owning processes. Kill with confidence. Monitor traffic. Manage firewall rules. All with zero friction.

## Why

Port 3000 is already in use. You open `netstat`, scan for PID, copy it, run `taskkill`. Repeat daily.

Portunity replaces that workflow. One keystroke to see everything, one keystroke to act.

## Features

- **Multi-dimensional search** — filter by app name, port range, protocol, process attributes, or any combination
- **Smart kill** — graceful termination first (SIGTERM), force kill (SIGKILL) if process doesn't respond
- **Port history** — SQLite-backed timeline of which process occupied which port, and when
- **Traffic monitoring** — per-port, per-process bytes sent/received via ETW
- **Firewall rules** — list, create, delete, toggle Windows Firewall rules from the app
- **Process details** — executable path, start time, command line, digital signature
- **Favorites + labels** — bookmark common ports, tag them ("my dev server", "database")
- **Export** — JSON/CSV for sharing with teammates or attaching to bug reports
- **Themes** — One Dark, Dracula, Solarized, Nord, Monokai, High Contrast, and more
- **i18n** — Chinese/English, modular extension points for more languages

## Two Frontends

| | GUI | TUI |
|---|---|---|
| **Tech** | Tauri v2 + Svelte | Ratatui |
| **Best for** | Daily driver, system tray, mouse | Speed, keyboard, SSH, tmux |
| **Tray** | Popup panel + quick actions | n/a |
| **Themes** | Built-in | `t` key toggle |

Both share the same `port-core` Rust library — identical data, identical logic.

## Quick Start

```bash
# Build everything
cargo build --release

# Terminal TUI
cargo run --bin port-tui

# Desktop GUI (requires Node.js for Svelte frontend)
cd port-gui && npm install && cargo tauri dev
```

## Platform

Windows 10/11 primary. Linux and macOS extension points reserved in the platform abstraction layer.

## License

MIT

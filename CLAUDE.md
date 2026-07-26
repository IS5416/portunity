# Portunity

Port + Opportunity — Windows port management tool. Dual frontend (Tauri GUI + Ratatui TUI) sharing Rust core library.

## Architecture

```
port-core/     Shared library: models, scanner, process, firewall, history, traffic, filter
port-tui/      Ratatui terminal dashboard (Tab-based: overview, ports, history, traffic, firewall)
port-gui/      Tauri v2 + Svelte desktop app
```

## Stack

- **Language:** Rust (edition 2024)
- **GUI:** Tauri v2 + Svelte
- **TUI:** Ratatui
- **Storage:** SQLite (rusqlite)
- **Async:** tokio
- **Platform:** Windows 10/11 primary

## Interaction Rules

- When presenting options, always provide analysis, reasoning, and a recommendation. Don't just list choices.
- When user raises a feature/question, first give your own analysis and extension suggestions, then confirm direction.
- Use English for code, commits, docs consumed by agents. Provide Chinese translations (`.zh.md`) for human reading.
- Every planning artifact (REQUIREMENTS.md, ROADMAP.md, research/*.md, etc.) MUST have a corresponding `.zh.md` Chinese version. Generate the `.zh.md` immediately after writing the English original.
- Caveman mode active (full): drop articles/filler/pleasantries. Fragments OK. Code/commits: write normal.

## GSD Workflow

This project uses GSD (Goal-Structured Development). See `.planning/` for:
- `PROJECT.md` — project context
- `config.json` — workflow preferences
- `REQUIREMENTS.md` — scoped requirements
- `ROADMAP.md` — phase structure
- `STATE.md` — project memory

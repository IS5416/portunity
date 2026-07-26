# Stack Research

**Domain:** Windows port management desktop tool (dual frontend: Tauri GUI + Ratatui TUI)
**Researched:** 2026-07-26
**Confidence:** HIGH

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | edition 2024 (1.85+) | Systems language | Required by project constraints; zero-cost abstractions for Win32 interop |
| Tauri | 2.11.3 | Desktop GUI framework | Mature v2 stable with IPC, plugin system, tray support. Latest as of June 2026 |
| Svelte | 5.25.8 (SvelteKit 2.20.4) | GUI frontend framework | Compiled output, no virtual DOM runtime, smallest bundle. Tauri's officially recommended pairing |
| Ratatui | 0.30.2 | Terminal TUI framework | Industry standard Rust TUI. v0.30 restructured into sub-crates (ratatui-core, ratatui-widgets, ratatui-crossterm) |
| Tokio | 1.50.0 | Async runtime | Event-driven, non-blocking I/O. De facto standard for Rust async. Multi-threaded work-stealing scheduler + IOCP on Windows |

### Windows System APIs

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| windows (windows-rs) | 0.73 | Official Win32 API bindings | Microsoft-maintained. Generates Rust bindings from Windows metadata. Covers IP Helper (`GetExtendedTcpTable`, `GetExtendedUdpTable`), Process API (`TerminateProcess`, `OpenProcess`), ETW, WFP. Replaces deprecated `winapi` crate |
| sysinfo | 0.39.3 | Cross-platform system information | Process enumeration, CPU/memory stats, disk info. v0.39+ adds `Process::kill_and_wait()`, `Process::exists()`. MSRV 1.95 |
| ferrisetw | 1.2.0 | ETW event consumer | Safe Rust abstractions for ETW trace sessions. Real-time and ETL file processing. Used for connection change events (TcpIp provider). Stable since June 2024; no breaking changes needed |
| windows-wfp | 0.2.1 | Windows Filtering Platform (firewall) | Safe Rust wrapper with automatic DOS-to-NT path conversion (critical for WFP), RAII engine management, builder-pattern FilterRule API, event monitoring. More mature than `wfp` crate |
| tray-icon | 0.24.1 | System tray integration | Used by Tauri internally; activate via `tauri = { features = ["tray-icon", "image-png"] }`. Supports left-click/double-click/right-click events, dynamic menus |

### Database

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| rusqlite | 0.40.1 | Core library SQLite access | Direct, synchronous, minimal dependencies. ~5,500 dependent crates. Used inside `port-core` for history, favorites, labels, settings. `bundled` feature compiles SQLite from source |
| tauri-plugin-sql | 2.4.0 | GUI frontend SQLite access | Official Tauri plugin. Uses sqlx internally for async/connection pooling. Only for the GUI frontend's own queries; core logic uses rusqlite directly |

### Serialization & Data Formats

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| serde | 1.0.229 | Serialization framework | Universal Rust serialization. 815M+ downloads. `#[derive(Serialize, Deserialize)]` on all data types |
| serde_json | 1.0.149 | JSON serialization | Frontend IPC format (Tauri commands). Human-readable, JS-interoperable |
| rmp-serde | 0.3+ | MessagePack serialization | Optional: binary IPC for large payloads (e.g., bulk port data export). Use only if JSON becomes a bottleneck |

### Error Handling & Observability

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| thiserror | 2.0.17 | Library error types | `#[derive(Error)]` for precise, matchable error enums in `port-core`. Used in library crates |
| anyhow | 1.0.102 | Application error propagation | `anyhow::Result<T>`, `.context()`, `bail!` macros. Used in binary crates (GUI, TUI). Reduces boilerplate |
| tracing | 0.1+ | Structured logging | Span-based observability. De facto Rust standard |
| tracing-subscriber | 0.3+ | Log output formatting | `fmt::layer()` for console, composable `Layer` architecture for future monitoring integrations |

### TUI Ecosystem

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| crossterm | 0.29.0 | Terminal backend | Required by Ratatui 0.30.x. Cross-platform terminal manipulation |
| ratatui-textarea | 0.9.1 | Multi-line text editor widget | Official Ratatui fork. Emacs-like shortcuts, undo/redo, search, selection. 49K downloads/month |
| tui-logger | 0.12+ | TUI log display widget | Smart widget with scrollback, per-target filtering, circular buffer. Actively maintained through July 2026 |
| tui-widgets | 0.7.10 | Widget collection | Meta-package: tui-big-text, tui-popup, tui-scrollbar, tui-scrollview. Official Ratatui organization project |
| eddacraft-tui | 0.4.0 | Themed component library | Pre-built styled widgets (DataTable, Tree, ProgressBar, Spinner). Ratatui 0.30 compatible. Use for polished TUI components |
| ratatui-macros | 0.7.2 | Constraint/span macros | Shipped with Ratatui 0.30.2. `constraints![]`, `span![]` macros for ergonomic layout |

### Internationalization

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| fluent-i18n | 0.1.0-rc.0 | Declarative i18n macros | `i18n!("locales")` + `t!("key")` macros. Thread-safe locale switching. Built on fluent-templates. Simplest ergonomic API for Rust desktop apps |
| unic-langid | 0.9+ | Language identifier types | Type-safe language tags. Foundation for language negotiation |

### CLI Argument Parsing

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| clap | 4.6.1 | Argument parsing | Derive-based API. Used for TUI binary CLI flags (e.g., `--theme`, `--locale`). Industry standard |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| rustup | Rust toolchain management | Install `stable-msvc` for Windows native compilation |
| cargo-tauri | Tauri CLI | `cargo install tauri-cli --version "^2"` |
| Node.js 22 LTS | Frontend build tooling | Required by SvelteKit/Vite. Use `fnm` or `nvm-windows` for version management |
| pnpm | Package manager | Faster, disk-efficient alternative to npm. Tauri templates support it |
| cargo-deny | License/security audit | Check dependency licenses and advisories in CI |
| cargo-udeps | Unused dependency detection | Keep crate graph lean |

## Installation

```bash
# Rust toolchain
rustup default stable-msvc
rustup component add rustfmt clippy

# Tauri CLI
cargo install tauri-cli --version "^2"

# Frontend (in gui/ directory)
corepack enable
pnpm install

# Core library dependencies (in Cargo.toml)
# [dependencies]
# tokio = { version = "1.50", features = ["full"] }
# serde = { version = "1.0", features = ["derive"] }
# serde_json = "1.0"
# rusqlite = { version = "0.40", features = ["bundled"] }
# windows = { version = "0.73", features = [...] }
# thiserror = "2.0"
# anyhow = "1.0"
# tracing = "0.1"
# tracing-subscriber = "0.3"
# ratatui = "0.30"
# crossterm = "0.29"
# clap = { version = "4.6", features = ["derive"] }
# tray-icon = "0.24"
# fluent-i18n = "0.1.0-rc.0"
```

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| GUI framework | Tauri v2 | Electron | 10x larger bundle, Chromium overhead, no Rust-native IPC |
| GUI framework | Tauri v2 | egui | Immediate-mode GUI not suitable for polished desktop app |
| TUI framework | Ratatui | tui (original) | Archived/deprecated. Ratatui is the maintained successor |
| TUI framework | Ratatui | cursive | Less flexible, callback-based, smaller ecosystem |
| Win32 bindings | windows (windows-rs) | winapi | Deprecated. windows-rs is the official Microsoft replacement |
| Process info | sysinfo + windows | procfs | Linux-only crate; no Windows support |
| Database | rusqlite | sqlx | Async overhead unnecessary for local SQLite. rusqlite is faster (20K sequential queries: 0.069s vs 0.402s) |
| Database | rusqlite | sled | Niche embedded DB. SQLite has universal tooling, Tauri plugin support |
| Firewall | windows-wfp | wfp (dlon/wfp-rs) | Less mature, missing automatic DOS-to-NT path conversion, no event monitoring |
| Network enumeration | windows crate directly | netstat-esr | Thin wrapper adds indirection. Direct `windows` calls are simpler and have no extra dep |
| i18n | fluent-i18n | rust-i18n | YAML-based, less expressive than Fluent's ICU message format. No plural rules, gender agreement |
| i18n | fluent-i18n | i18n-embed | i18n-embed is more mature but has heavier setup. fluent-i18n's macro API is simpler for this project's Chinese/English toggle |
| Logging | tracing | env_logger / log | Superseded. tracing provides spans, structured fields, composable layers |
| Serialization | serde_json | simd-json | Performance overkill for config/export. serde_json is universal, better ecosystem |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `winapi` crate | Deprecated. No new features. Microsoft officially endorses `windows-rs` | `windows` 0.73 |
| `procfs` crate | Linux-only. Will not compile on Windows | `sysinfo` + `windows` for process info |
| `sqlx` for core library | Async connection pool overhead unnecessary for local SQLite. Slower sequential queries | `rusqlite` 0.40 with `bundled` feature |
| `sled` embedded DB | Niche, smaller community, no SQL querying, no external tooling | `rusqlite` (SQLite) |
| Original `tui` crate | Archived and unmaintained. Community moved to Ratatui | `ratatui` 0.30 |
| `env_logger` / bare `log` | No span support, no structured fields, no composable layers | `tracing` + `tracing-subscriber` |
| `rust-i18n` | YAML-based, lacks ICU message format (no plural rules, gender, selectors) | `fluent-i18n` (Fluent/ICU-based) |
| `wfp` 0.0.7 (dlon/wfp-rs) | Missing critical path conversion feature. WFP requires NT kernel paths, not DOS paths | `windows-wfp` 0.2.1 |
| `netstat-esr` | Extra dependency with no unique value. Direct `windows` calls are more maintainable | `windows` crate `GetExtendedTcpTable` |

## Stack Patterns by Variant

**If building the shared core library (`port-core`):**
- Use `thiserror` for error types (matchable by consumers)
- Use `rusqlite` directly (not tauri-plugin-sql; core is frontend-agnostic)
- Depend on `windows` crate for Win32 APIs, `sysinfo` for process info
- Expose async API via `tokio` but keep internals sync where Windows APIs are sync
- Do NOT depend on Tauri, Ratatui, or any frontend crate

**If building the Tauri GUI frontend:**
- Use `tauri-plugin-sql` for GUI-side queries (connects to same SQLite DB)
- Communicate with backend via Tauri `invoke` IPC commands
- SvelteKit with `@sveltejs/adapter-static` in SPA mode (not SSG/prerender)
- Disable SSR via root `+layout.ts` to access Tauri APIs without `window` checks
- Set `dragDropEnabled: false` in `tauri.conf.json` if using drag-and-drop
- Enable `tray-icon` and `image-png` features in tauri dependency

**If building the Ratatui TUI frontend:**
- Use `crossterm` 0.29 backend (NOT termion -- Windows target)
- Use `clap` for CLI flags (`--theme`, `--locale`, `--refresh-interval`)
- Use `eddacraft-tui` for polished pre-built widgets (DataTable for port list, Tree for process tree)
- Use `tui-logger` for in-TUI log panel
- Load themes from JSON/TOML config files, apply via Ratatui `Style`
- Use `fluent-i18n` with `.ftl` files embedded or loaded from config dir

## Version Compatibility Matrix

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| ratatui 0.30.2 | crossterm 0.29.0 | Required pairing. ratatui-crossterm sub-crate enforces this |
| ratatui-macros 0.7.2 | ratatui 0.30.2 | Shipped together; same version family |
| tauri 2.11.3 | tray-icon 0.24.1 | Bundled; enable via features |
| tauri 2.11.3 | tauri-plugin-sql 2.4.0 | Compatible within v2.x plugin family |
| tokio 1.50.0 | Rust 1.71+ (MSRV) | tokio MSRV policy: rolling 6 months |
| windows 0.73 | Rust 1.70+ | windows-rs tracks stable Rust closely |
| sysinfo 0.39.3 | Rust 1.95+ (MSRV) | Raised MSRV in 0.39.x series |

## Sources

- [docs.rs/tauri/2.11.3](https://docs.rs/tauri/2.11.3) — Tauri v2 crate version history (verified 2.11.3, June 2026)
- [docs.rs/ratatui/0.30.2](https://docs.rs/ratatui/latest) — Ratatui sub-crate restructuring in 0.30.x (verified 0.30.2, June 2026)
- [crates.io/crates/windows](https://crates.io/crates/windows) — windows-rs v73 (February 2026)
- [crates.io/crates/tokio](https://crates.io/crates/tokio) — tokio v1.50.0 (March 2026)
- [crates.io/crates/rusqlite](https://crates.io/crates/rusqlite) — rusqlite v0.40.1 (June 2026)
- [crates.io/crates/serde](https://crates.io/crates/serde) — serde 1.0.229 (July 2026)
- [crates.io/crates/anyhow](https://crates.io/crates/anyhow) — anyhow 1.0.102 (February 2026)
- [crates.io/crates/sysinfo](https://crates.io/crates/sysinfo) — sysinfo 0.39.3 (May 2026)
- [docs.rs/windows-wfp](https://docs.rs/windows-wfp/latest) — windows-wfp 0.2.1 (March 2026)
- [github.com/n4r1b/ferrisetw](https://github.com/n4r1b/ferrisetw) — ferrisetw 1.2.0 ETW consumer
- [crates.io/crates/tauri-plugin-sql](https://crates.io/crates/tauri-plugin-sql) — tauri-plugin-sql 2.4.0 (April 2026)
- [crates.io/crates/tray-icon](https://crates.io/crates/tray-icon) — tray-icon 0.24.1 (June 2026)
- [crates.io/crates/clap](https://crates.io/crates/clap) — clap 4.6.1 (April 2026)
- [lib.rs/crates/fluent-i18n](https://lib.rs/crates/fluent-i18n) — fluent-i18n 0.1.0-rc.0
- [tauri.app/start/frontend/sveltekit](https://tauri.app/start/frontend/sveltekit/) — Official Tauri+SvelteKit integration guide
- [ratatui.rs/showcase/third-party-widgets](https://ratatui.rs/showcase/third-party-widgets/) — Ratatui widget ecosystem catalog
- [lib.rs/crates/ratatui-textarea](https://lib.rs/crates/ratatui-textarea) — ratatui-textarea 0.9.1
- [lib.rs/crates/tui-widgets](https://lib.rs/crates/tui-widgets) — tui-widgets 0.7.10 meta-package
- [crates.io/crates/eddacraft-tui](https://crates.io/crates/eddacraft-tui) — eddacraft-tui 0.4.0
- [byteiota.com/rust-orms-2026](https://byteiota.com/rust-orms-2026-sqlx-vs-diesel-vs-seaorm-comparison/) — rusqlite vs sqlx benchmark data

---
*Stack research for: Windows port management tool (Tauri v2 + Ratatui dual frontend)*
*Researched: 2026-07-26*

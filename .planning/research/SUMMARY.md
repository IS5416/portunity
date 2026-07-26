# Project Research Summary

**Project:** Portunity
**Domain:** Windows Port Management Desktop Tool (Dual Frontend: Tauri GUI + Ratatui TUI)
**Researched:** 2026-07-26
**Confidence:** HIGH (stack + pitfalls verified against official sources; features and architecture are medium from community/competitor analysis)

## Executive Summary

Portunity is a Windows-native port management tool with a dual frontend -- a Tauri desktop GUI and a Ratatui terminal TUI -- both backed by a shared Rust core library. This is not a traditional MVP; the project scope calls for a full-featured v1 launch with 20+ features spanning port enumeration, process management, traffic monitoring, firewall integration, and intelligent labeling. Experts build this kind of tool as a Rust workspace with a layered architecture: a platform-abstracted core crate (port-core) handling all Windows API interaction (IP Helper, ETW, WFP/COM), with thin presentation layers in Svelte (GUI) and Ratatui (TUI) that delegate all business logic to the core.

The recommended approach is bottom-up: foundation data types and storage first, then core Windows API capabilities (scanning, process management), then the TUI as a fast-iteration validation frontend, followed by event-driven monitoring (ETW), the Tauri GUI, and finally advanced features (firewall CRUD, faceted search, export). The single Cargo workspace, async-first API design, and strict HANDLE-based process tracking are architectural table stakes -- get these wrong and the tool will be unreliable or unbuildable. The key risk is systemic: 15 domain-specific pitfalls have been documented, five of which (PID reuse, buffer retry, dual-stack enumeration, async blocking, workspace misconfiguration) are correctness-critical and must be addressed in their respective phases before any downstream work proceeds.

Mitigation strategy: every phase that touches Windows APIs must wrap blocking calls in tokio::task::spawn_blocking, never store bare PIDs (only HANDLE-wrapped ProcessHandle structs), always enumerate both IPv4 and IPv6 tables, and always enable SQLite WAL mode on first connection. The ETW integration layer treats ETW as a change-notification trigger only -- ground truth always comes from GetExtendedTcpTable polling, which eliminates the 30% PID inaccuracy rate inherent to ETW kernel-network events.

## Key Findings

### Recommended Stack

The stack is a Rust monorepo workspace with three crates: port-core (shared library), port-tui (Ratatui binary), and port-gui/src-tauri (Tauri v2 binary + Svelte frontend). Rust edition 2024 with the stable-msvc toolchain is required -- all Win32 interop goes through the official windows crate (0.73), not the deprecated winapi. The Tauri GUI uses Svelte 5 + SvelteKit in SPA mode, communicating with the Rust backend via Tauri IPC. The TUI uses Ratatui 0.30 with crossterm 0.29 backend (termion is incompatible with Windows), following the Elm Architecture pattern. Both frontends share the same SQLite database (via rusqlite in WAL mode), TOML configuration, and Fluent i18n .ftl resource files.

**Core technologies:**
- **Rust edition 2024 (1.85+):** Systems language with zero-cost Win32 interop -- required by project constraints
- **Tauri 2.11.3 + Svelte 5.25.8:** Desktop GUI with WebView2 rendering, compiled Svelte output (no virtual DOM), system tray support
- **Ratatui 0.30.2 + crossterm 0.29:** Terminal TUI with sub-crate architecture, Elm Architecture pattern, virtualized table rendering
- **windows (windows-rs) 0.73:** Official Microsoft Win32 API bindings -- covers IP Helper, Process API, ETW, WFP/COM
- **rusqlite 0.40.1 (bundled):** Direct SQLite access with WAL mode for concurrent dual-frontend access; faster than sqlx for sequential queries (0.069s vs 0.402s for 20K queries)
- **ferrisetw 1.2.0:** Safe Rust ETW consumer for real-time network event subscription
- **windows-wfp 0.2.1:** Windows Filtering Platform wrapper with automatic DOS-to-NT path conversion, RAII engine management, event monitoring
- **tokio 1.50:** Async runtime with IOCP on Windows; multi-threaded work-stealing scheduler

### Expected Features

**Must have (table stakes):**
- Real-time TCP/UDP port listing with process mapping (name + PID + full path + signature) -- this IS the product
- Protocol and state display with color coding (LISTENING/ESTABLISHED/TIME_WAIT) -- TCPView-level visual quality expected
- Sortable columns, admin elevation auto-detection, clipboard copy/export, configurable auto-refresh
- Process termination with smart kill (graceful WM_CLOSE fallback to TerminateProcess after timeout)
- System-critical process whitelist (30+ protected processes) with confirmation gating
- Search/filter with substring and exact match across port, process name, PID, protocol, state
- Process detail panel (executable path, start time, command line args, digital signature status)

**Should have (competitive differentiators):**
- Auto-labeled ports (50+ common dev ports) -- no competitor offers this
- Custom user-assignable port labels -- makes the port list personal and searchable
- Combined/faceted search with AND/OR logic across all dimensions (SQLite FTS5-backed)
- Port occupation change history with queryable SQLite timeline
- Network traffic statistics (bytes sent/received per port and per process via ETW/IP Helper counters)
- Windows Firewall rule CRUD (list/create/delete/enable/disable) with right-click quick block/allow actions
- ETW event-driven refresh (near-zero CPU when idle) with 2s polling fallback
- Export as JSON and CSV with all fields
- Dual interface (TUI + GUI) from shared port-core crate -- structural differentiator
- Favorites/bookmarks for commonly monitored ports

**Defer (v2+, explicitly out of scope):**
- Linux/macOS support -- platform abstraction traits exist as extension points, implementations deferred
- Active TCP port scanning (nmap-style) -- PortScanner trait reserved, but scanning is a different product category
- Built-in HTTP server for remote access -- security nightmare for a local system tool; TUI works over SSH natively
- Service management (start/stop/restart Windows services) -- complex dependency trees belong in services.msc

### Architecture Approach

The architecture is a four-layer system: Frontend (GUI + TUI), IPC/Adapter (Tauri Commands + TUI Event Loop), Core (port-core shared library with trait-based platform abstraction), and OS (Windows APIs via windows-rs, ferrisetw, windows-wfp). A tokio::sync::broadcast EventBus decouples core producers (scanner, ETW monitor, process manager) from frontend consumers (TUI tabs, GUI panels, history recorder). The platform abstraction layer defines traits with Windows implementations behind conditional compilation -- Linux/macOS are stubbed for future extension. All business logic, data models, and OS interaction live exclusively in port-core; both frontends are thin adapters that call core APIs and render results.

**Major components:**
1. **PortScanner** -- Enumerates TCP/UDP ports via GetExtendedTcpTable/GetExtendedUdpTable (both AF_INET and AF_INET6), resolves owning process, classifies connection state. Most critical component.
2. **ProcessManager** -- Queries process details (path, args, signature, start time), implements smart kill with graceful escalation, enforces whitelist gating. Uses HANDLE-wrapped ProcessHandle structs to prevent PID reuse race conditions.
3. **FirewallManager** -- CRUD operations on Windows Firewall rules via INetFwPolicy2 COM API. Extracts data to plain Rust structs immediately to avoid COM reference leaks.
4. **TrafficMonitor** -- Subscribes to ETW Kernel-Network provider for real-time traffic events, runs 2s polling fallback. Treats ETW as change-notification trigger only; ground truth always from GetExtendedTcpTable.
5. **EventBus** -- tokio::sync::broadcast channel decoupling producers from consumers. Typed CoreEvent enum variants published by scanner, monitor, and process manager.
6. **DataStore** -- SQLite via rusqlite in WAL mode. Stores port history (append-only log), favorites, custom labels, and app settings. Single writer connection + reader pool for dual-frontend concurrency.

### Critical Pitfalls

1. **PID Reuse Race Condition** -- A PID can be recycled within milliseconds on Windows. Storing bare PIDs and later calling OpenProcess(PID) kills the wrong process. **Prevent by:** Always storing ProcessHandle structs (PID + HANDLE + creation time), never re-deriving handles from PIDs. Verify creation time before terminating.

2. **GetExtendedTcpTable Buffer Retry** -- The two-call buffer allocation pattern is mandatory. A single call with a fixed buffer silently truncates data. The table can grow between calls, so a retry loop with doubling buffer size is required for correctness.

3. **IPv4/IPv6 Dual-Stack Enumeration Gap** -- Calling GetExtendedTcpTable only with AF_INET misses all dual-stack connections. Always call with both AF_INET and AF_INET6, using TCP_TABLE_OWNER_PID_ALL.

4. **Blocking Win32 Calls on Async Runtime Threads** -- GetExtendedTcpTable, OpenProcess, COM calls are synchronous. Calling them directly blocks worker threads, freezing the UI. Wrap every Win32 call in tokio::task::spawn_blocking. The port-core public API must be async-first.

5. **Single Cargo Workspace (Not Two)** -- If src-tauri/ has its own [workspace] declaration, Cargo creates two target/ directories, two Cargo.lock files, double build times. Set up a single root workspace from day one.

Additional documented pitfalls (15 total): ETW session orphaning, ETW callback thread blocking, COM resource leaks, std::sync::Mutex across .await, protected process silent failure, SQLite WAL mode omission, Ratatui large-table rendering degradation, Windows-rs allocator mismatch, ETW PID inaccuracy, and port number byte order reversal. Full details in PITFALLS.md.

## Implications for Roadmap

Based on combined research, the project should be organized into 6 phases within a single milestone. This ordering respects architectural dependency chains, minimizes rework from discovering core API problems late, and ensures each phase avoids its mapped pitfalls before proceeding.

### Phase 1: Foundation (Scaffolding + Data Layer)
**Rationale:** Everything depends on data types, configuration, and storage. Getting the workspace structure right here avoids the double-compilation pitfall (Pitfall 10 from PITFALLS.md). WAL mode must be enabled on first connection (Pitfall 12).
**Delivers:** Single Cargo workspace with three crate members, port-core::models (all data types with serde), port-core::config (TOML loading), port-core::store (SQLite schema, migrations, WAL mode connection), port-core::i18n (Fluent bundle loader), port-core::platform (trait definitions with Windows stubs).
**Addresses:** Workspace structure, data model foundation, persistence layer.
**Avoids:** Separate Tauri workspace (Pitfall 10), SQLite WAL omission (Pitfall 12).
**Research needed:** None -- standard Rust workspace + SQLite patterns are well-documented.

### Phase 2: Core Engine (Scanner + Process Management)
**Rationale:** The port scanner is the most critical component. If it does not work, nothing downstream matters. Process management must be safe from day one (HANDLE-wrapped, not bare PID). This phase validates the platform abstraction layer against real Windows APIs.
**Delivers:** port-core::scanner (dual-stack TCP/UDP enumeration with buffer retry, port byte order conversion, IPv4-mapped address deduplication), port-core::process (ProcessHandle struct, process info querying, basic kill, smart kill with graceful escalation, shipped system-critical process whitelist).
**Addresses:** Real-time port listing, port-to-process mapping, protocol/state display, sortable data structures, process termination, smart kill, whitelist confirmation, process detail panel.
**Avoids:** PID reuse (Pitfall 1), buffer allocation pattern (Pitfall 2), IPv6 enumeration gap (Pitfall 3), port byte order (Pitfall 4), protected process silent failure (Pitfall 11), allocator mismatch (Pitfall 14).

### Phase 3: First Frontend -- TUI (Ratatui Terminal Interface)
**Rationale:** The TUI is the fastest frontend to build and iterate -- it validates the core API surface without the Tauri/Svelte toolchain overhead. If port-core APIs are awkward, you discover it here rather than in the more complex GUI phase. Building the TUI first also provides a visualization target for Phase 4 event streams.
**Delivers:** port-tui binary with Elm Architecture main loop, tabbed interface (Overview, Ports tabs initially), sortable/filterable port table with virtualized rendering, search input bar, status bar (admin state, refresh indicator), keyboard-driven navigation, theme system loading from TOML.
**Addresses:** Sortable columns (keyboard-driven), search/filter (basic), protocol/state color coding, copy/export (clipboard), auto-refresh with configurable interval, admin elevation detection display.
**Avoids:** Ratatui large-table rendering degradation (Pitfall 13 -- virtualize from the start), blocking Win32 calls on main thread (Pitfall 9).

### Phase 4: Event System + Real-Time Monitoring
**Rationale:** ETW integration is complex and has multiple failure modes (orphaned sessions, callback blocking, PID inaccuracy). Building it after a working TUI means you can visualize ETW events arriving in real time, making debugging tractable. The EventBus is a prerequisite for the GUI streaming updates in Phase 5.
**Delivers:** port-core::events (EventBus with broadcast channel), port-core::monitor (ETW subscription with ferrisetw, startup orphan cleanup, lock-free callback-to-async bridge, 2s polling fallback, per-port/per-process traffic counters), port occupation change history (SQLite append-only log with timestamped events).
**Addresses:** ETW event-driven refresh, network traffic statistics, port occupation change history, stale-data indicator (last-refresh timestamp).
**Avoids:** ETW session orphaning (Pitfall 5), ETW callback thread blocking (Pitfall 6), ETW PID inaccuracy (Pitfall 15), blocking Win32 on async threads (Pitfall 9).
**Research needed: YES -- ETW event schemas, ferrisetw buffer sizing, PID cross-referencing accuracy.**

### Phase 5: Second Frontend -- GUI (Tauri + Svelte Desktop App)
**Rationale:** The GUI provides the richer UX (system tray, context menus, faceted search UI, traffic graphs) and is what most users will interact with. By this phase, the core API is battle-tested from TUI usage and the event system is delivering streaming data. The GUI is a thin IPC adapter over proven core functionality.
**Delivers:** port-gui/src-tauri (Tauri command registration, event forwarding from core EventBus, system tray with mini port list popup, UAC elevation detection), port-gui/src/ (Svelte components: PortTable with faceted search, ProcessDetail slide-out, FirewallPanel, TrafficGraph, HistoryTimeline, TrayPopup, Settings, ConfirmDialog), Svelte stores for reactive state.
**Addresses:** System tray integration, faceted search UI, admin elevation with graceful degradation, settings/preferences panel.
**Avoids:** std::sync::Mutex across .await (Pitfall 8), blocking Win32 on async threads (Pitfall 9), COM resource leaks (Pitfall 7), business logic in frontend (ARCHITECTURE.md Anti-Pattern 1).
**Research needed: YES -- Tauri v2 system tray capabilities, Svelte 5 runes API for IPC-reactive stores.**

### Phase 6: Advanced Features (Firewall, Export, Labels, Favorites, i18n Polish)
**Rationale:** These features have no hard dependency on the GUI being complete but have their own internal dependencies (quick block/allow requires firewall CRUD, which requires admin elevation, existing from Phase 2). Bundled as advanced features building on the solid Phase 1-5 foundation.
**Delivers:** port-core::firewall (WFP/COM rule CRUD via windows-wfp, rule validation, mutation logging), port-core::export (JSON and CSV export with schema versions), port-core::labels (auto-label lookup table for 50+ common ports, custom label CRUD), port-core::favorites, Chinese localization (zh-CN Fluent bundles), theme presets (One Dark, Dracula, Solarized, Nord, Monokai, High Contrast).
**Addresses:** Firewall rule management, quick block/allow actions, auto-labeled ports, custom labels, favorites/bookmarks, export as JSON/CSV, combined/faceted search backend (SQLite FTS5), i18n (Chinese toggle), theme switching.
**Avoids:** COM resource leaks (Pitfall 7), firewall operation without admin check, no integrity validation on rule operations.
**Research needed: YES -- windows-wfp 0.2.1 API completeness, INetFwPolicy2 COM interface details, SQLite FTS5 faceted query syntax.**

### Phase Ordering Rationale

- **Foundation must be first:** Data types and storage are prerequisites for everything. Getting the single workspace right avoids structural debt.
- **Core before any frontend:** The scanner and process manager are the hardest, most failure-prone components. Validate against real Windows APIs before wrapping in UI.
- **TUI before GUI:** The TUI is simpler, faster to iterate, and validates the core API without Tauri/Svelte complexity. If the API is awkward, fix it in Phase 3, not Phase 5.
- **Event system after TUI:** The TUI provides a visualization target for ETW event streams. Debugging ETW without being able to see events arrive is painful.
- **GUI after event system:** The GUI needs streaming data (traffic stats, port changes) from the EventBus, which is built in Phase 4.
- **Advanced features last:** Firewall, labels, faceted search, and export are value-add features that need a stable core. They have their own internal dependency chains but no hard dependency on the GUI.
- **Pitfall-driven ordering:** Each phase is assigned the pitfalls it must prevent. Phase 2 is the riskiest -- it carries 6 correctness-critical pitfalls. Building it early gives maximum time to get it right.

### Research Flags

Phases likely needing deeper research during /gsd-plan-phase:
- **Phase 4 (Event System + Monitoring):** ETW Kernel-Network provider event schemas are sparsely documented. ferrisetw buffer sizing and event loss characteristics need empirical testing. ETW PID accuracy cross-referencing needs validation against real workloads.
- **Phase 5 (GUI):** Tauri v2 system tray popup panel capabilities (can it host a live Svelte component?). Svelte 5 runes API for derived stores that react to Tauri IPC events. WebView2 memory usage with live-updating tables.
- **Phase 6 (Advanced Features):** windows-wfp 0.2.1 API completeness for rule CRUD and event monitoring. COM interop for INetFwPolicy2 profile handling (Domain/Private/Public). SQLite FTS5 query builder for faceted AND/OR search across text + numeric dimensions.

Phases with standard, well-documented patterns (skip research-phase):
- **Phase 1 (Foundation):** Rust workspace structure, SQLite with rusqlite, TOML config, Fluent i18n -- all extensively documented.
- **Phase 2 (Core Engine):** IP Helper API, TerminateProcess, CreateToolhelp32Snapshot -- all heavily documented on Microsoft Learn. Pitfalls research already covers the tricky parts.
- **Phase 3 (TUI):** Ratatui + Elm Architecture + crossterm are the de facto Rust TUI stack. Virtualization via ratatui-layout has documented patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified against crates.io/docs.rs as of July 2026. Compatibility matrix cross-checked. Alternatives analyzed with clear rationale. |
| Features | MEDIUM | Cross-referenced across 6+ competitor tools and community forums. No primary user interviews. Feature complexity estimates informed but not implementation-validated. |
| Architecture | MEDIUM | Patterns from Ratatui and Tauri official docs, real-world Rust workspace projects. Domain-specific architecture synthesized from multiple sources, not validated in a working system. |
| Pitfalls | HIGH | 15 pitfalls sourced from official docs, GitHub issues, real-world debugging threads. Each has concrete prevention strategy and verification checklist. Mitigation patterns battle-tested in other Windows tools. |

**Overall confidence:** MEDIUM-HIGH -- stack and pitfalls are solidly researched; features and architecture are well-informed but would benefit from prototype validation during Phase 2.

### Gaps to Address

- **ETW event schema details:** The exact event payload fields for Microsoft-Windows-TCPIP provider (Event ID 1033 for connection attempts) need empirical verification. Handle during Phase 4 planning with a spike that captures and inspects raw ETW events.
- **windows-wfp API surface completeness:** Version 0.2.1 supports rule CRUD and event monitoring, but the exact API surface for rule filtering (by port, program path, direction) needs validation. Handle during Phase 6 planning.
- **Svelte 5 + Tauri IPC performance under rapid updates:** With ETW firing dozens of events per second under load, the Svelte store update path needs profiling. Handle during Phase 5 execution with performance benchmarks.
- **Dual-frontend concurrent SQLite access edge cases:** WAL mode handles concurrent reads but edge cases around simultaneous writes from TUI and GUI need testing. Handle during Phase 1 with concurrent access integration tests.
- **No user research:** All feature priorities are based on competitor analysis and community sentiment, not direct user interviews. Mitigate by treating post-launch feedback as v1.1 input; the feature set is broad enough to cover known needs.

## Sources

### Primary (HIGH confidence)
- Microsoft Learn: GetExtendedTcpTable, GetExtendedUdpTable, TerminateProcess, OpenProcess, INetFwPolicy2 -- Windows API documentation
- Microsoft Learn: TCPIP ETW Provider events -- ETW event schema documentation
- Microsoft Learn: Windows Filtering Platform API -- Firewall management documentation
- docs.rs: tauri 2.11.3, ratatui 0.30.2, windows 0.73, tokio 1.50, rusqlite 0.40 -- Official crate documentation
- crates.io: all dependency versions verified as of July 2026
- Tauri v2 Official Documentation: Process Model, IPC, System Tray, SvelteKit Integration
- Ratatui Official Documentation: Application Patterns, Event Handling, Component Architecture

### Secondary (MEDIUM confidence)
- GitHub: ratatui/ratatui (Issues #1004, #1116, Discussion #409) -- Table performance at scale
- GitHub: tauri-apps/tauri (Discussion #10329) -- spawn_blocking in Tauri commands
- GitHub: winsiderss/systeminformer (Issue #2629) -- Process termination pitfalls
- GitHub: mmogr/gglib (Issue #106) -- Tauri workspace double compilation
- GitHub: n4r1b/ferrisetw -- ETW consumer crate with real-world usage patterns
- NirSoft CurrPorts Documentation -- Competitor feature analysis
- SaaSHub CurrPorts vs TCPView comparison -- Community feature comparison
- Multiple GitHub repositories: portio, Porter, pttr, portzap, PortPal -- Modern port tool feature analysis
- Arkenar Project -- Real-world Rust workspace with core + CLI + Tauri architecture

### Tertiary (LOW confidence)
- Stack Overflow: Port byte order issue with GetExtendedTcpTable
- dev.to: Rust Async in Tauri v2 -- Community experience with async patterns
- Windows Forum threads on Resource Monitor capabilities
- Automata Labs netstat Guide for Windows Devs -- Developer workflow analysis

---
*Research completed: 2026-07-26*
*Ready for roadmap: yes*

# Architecture Research

**Domain:** Windows Port Management Desktop Tool
**Researched:** 2026-07-26
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          FRONTEND LAYER                                   │
├─────────────────────────────┬────────────────────────────────────────────┤
│   port-gui (Tauri v2)       │        port-tui (Ratatui)                  │
│   ┌───────────────────────┐ │  ┌──────────────────────────────────────┐  │
│   │ Svelte WebView        │ │  │ Terminal UI (Crossterm backend)      │  │
│   │ ┌───────┐ ┌─────────┐ │ │  │ ┌──────────┐ ┌──────────┐           │  │
│   │ │Port   │ │Firewall │ │ │  │ │Overview  │ │Firewall  │           │  │
│   │ │Table  │ │Panel    │ │ │  │ │Tab       │ │Tab       │ ...       │  │
│   │ └───┬───┘ └────┬────┘ │ │  │ └────┬─────┘ └────┬─────┘           │  │
│   │     │          │      │ │  │      │             │                 │  │
│   └─────┼──────────┼──────┘ │  └──────┼─────────────┼─────────────────┘  │
│         │  invoke()│         │         │ Action enum │                    │
│         │  + events│         │         │ + channels  │                    │
├─────────┴──────────┴─────────┴─────────┴─────────────┴────────────────────┤
│                          IPC / ADAPTER LAYER                               │
│  ┌───────────────────────────┐  ┌──────────────────────────────────────┐  │
│  │ Tauri Commands            │  │ TUI App (main loop)                   │  │
│  │ #[tauri::command] fns     │  │ Message → Update → View               │  │
│  │ + AppHandle::emit events  │  │ (Elm Architecture)                    │  │
│  └─────────────┬─────────────┘  └──────────────────┬───────────────────┘  │
│                │                                    │                      │
├────────────────┴────────────────────────────────────┴──────────────────────┤
│                          CORE LAYER (port-core)                            │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │                        Event Bus (tokio::sync::broadcast)             │ │
│  │  ┌───────────────┐ ┌───────────────┐ ┌──────────────┐ ┌───────────┐ │ │
│  │  │ PortScanner   │ │ ProcessMgr   │ │ FirewallMgr  │ │ Traffic   │ │ │
│  │  │ (iphlpapi)    │ │ (Terminate-  │ │ (WFP/COM)    │ │ Monitor   │ │ │
│  │  │               │ │  Process)    │ │              │ │ (ETW)     │ │ │
│  │  └───────┬───────┘ └───────┬───────┘ └──────┬───────┘ └─────┬─────┘ │ │
│  │          │                 │                │               │        │ │
│  │  ┌───────┴─────────────────┴────────────────┴───────────────┴─────┐ │ │
│  │  │                    Platform Abstraction Layer                  │ │ │
│  │  │  (PortScanner trait, ProcessMgr trait, FirewallMgr trait)      │ │ │
│  │  │  windows-rs impl │ future linux impl │ future macos impl      │ │ │
│  │  └───────────────────────────────┬───────────────────────────────┘ │ │
│  └──────────────────────────────────┼─────────────────────────────────┘ │
│                                     │                                     │
├─────────────────────────────────────┼─────────────────────────────────────┤
│                          STORAGE LAYER                                    │
│  ┌──────────────┐  ┌───────────────┼──────────────┐  ┌────────────────┐  │
│  │ SQLite       │  │ Config Files  │ Theme Files  │  │ i18n Files     │  │
│  │ (rusqlite)   │  │ (TOML)        │ (TOML)       │  │ (Fluent .ftl)  │  │
│  │ history      │  │ settings      │ One Dark     │  │ en-US          │  │
│  │ favorites    │  │ whitelist     │ Dracula      │  │ zh-CN          │  │
│  │ labels       │  │ keybinds      │ Solarized    │  │ extension pt   │  │
│  └──────────────┘  └───────────────┴──────────────┘  └────────────────┘  │
├──────────────────────────────────────────────────────────────────────────┤
│                          OS LAYER (Windows 10/11)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ IP Helper API│  │ Kernel32     │  │ WFP / COM    │  │ ETW Kernel   │  │
│  │ GetExtended- │  │ OpenProcess  │  │ Firewall API │  │ Network      │  │
│  │ TcpTable     │  │ Terminate-   │  │ INetFwPolicy2│  │ Provider     │  │
│  │              │  │ Process      │  │              │  │              │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **PortScanner** | Enumerate TCP/UDP ports, resolve owning process, classify connections | `windows-rs` crate calling `GetExtendedTcpTable`/`GetExtendedUdpTable` from `iphlpapi.dll` |
| **ProcessManager** | Query process details (path, args, signature), terminate processes with smart kill strategy | `windows-rs` calling `OpenProcess`, `QueryFullProcessImageNameW`, `TerminateProcess` |
| **FirewallManager** | List/create/delete/enable/disable Windows Firewall rules | Windows COM API (`INetFwPolicy2`) via `windows-rs` |
| **TrafficMonitor** | Track per-port/per-process bytes sent/received in real time | `ferrisetw` subscribing to `Microsoft-Windows-Kernel-Network` ETW provider |
| **EventBus** | Decouple core producers from frontend consumers; broadcast scan results, process events, traffic updates | `tokio::sync::broadcast` channel (multi-producer, multi-consumer) |
| **DataStore** | Persist port history, favorites, labels, settings | `rusqlite` with WAL mode; `Arc<Connection>` for shared access |
| **ConfigManager** | Load/save user preferences (themes, keybinds, whitelists, i18n locale) | `config` crate with TOML format |
| **I18nEngine** | Localized string lookup for both frontends | `fluent`/`fluent-bundle` crate loading `.ftl` resource files |
| **PlatformLayer** | Abstract OS-specific APIs behind traits for future Linux/macOS support | Rust traits with `#[cfg(target_os = "windows")]` conditional compilation |
| **TUI App** | Terminal interface: tabs, keyboard navigation, streaming updates | Ratatui + crossterm, Elm Architecture (TEA) with component tabs |
| **GUI App** | Desktop window: Svelte components, system tray, context menus | Tauri v2 core process routing IPC; Svelte in WebView2 rendering |

## Recommended Project Structure

```
Portunity/
├── Cargo.toml                      # Workspace root
│   [workspace]
│   members = ["port-core", "port-tui", "port-gui/src-tauri"]
│   resolver = "2"
│
├── port-core/                      # SHARED CORE LIBRARY — no UI, no CLI parsing
│   ├── Cargo.toml                  #   depends on: windows-rs, tokio, rusqlite, ferrisetw, serde
│   └── src/
│       ├── lib.rs                  #   Public API: re-exports, module declarations
│       ├── scanner/                #   Port enumeration & classification
│       │   ├── mod.rs
│       │   ├── tcp.rs              #     GetExtendedTcpTable wrapper
│       │   ├── udp.rs              #     GetExtendedUdpTable wrapper
│       │   └── resolver.rs         #     PID → process name resolution
│       ├── process/                #   Process management
│       │   ├── mod.rs
│       │   ├── info.rs             #     Process details (path, args, signature, start time)
│       │   ├── kill.rs             #     Smart kill: graceful → force escalation
│       │   └── whitelist.rs        #     System-critical + user-defined whitelist
│       ├── firewall/               #   Windows Firewall management
│       │   ├── mod.rs
│       │   ├── rules.rs            #     CRUD operations on firewall rules
│       │   └── profiles.rs         #     Domain/Private/Public profile handling
│       ├── monitor/                #   Traffic monitoring
│       │   ├── mod.rs
│       │   ├── etw.rs              #     ETW kernel network event subscription
│       │   └── poller.rs           #     2s polling fallback via IP Helper
│       ├── events/                 #   Event bus
│       │   ├── mod.rs
│       │   └── bus.rs              #     broadcast channel wrapper, event enum definitions
│       ├── store/                  #   Data persistence
│       │   ├── mod.rs
│       │   ├── connection.rs       #     SQLite connection pool / Arc wrapper
│       │   ├── migrations.rs       #     Schema migrations (table creation, versioning)
│       │   ├── history.rs          #     Port occupation change log queries
│       │   ├── favorites.rs        #     Bookmarked ports CRUD
│       │   └── labels.rs           #     Custom port/process labels CRUD
│       ├── config/                 #   Configuration
│       │   ├── mod.rs
│       │   ├── settings.rs         #     App settings struct + load/save
│       │   ├── themes.rs           #     Theme preset definitions (One Dark, Dracula, etc.)
│       │   └── keybinds.rs         #     Keyboard shortcut mappings
│       ├── i18n/                   #   Internationalization
│       │   ├── mod.rs
│       │   └── engine.rs           #     Fluent bundle loader, locale switch
│       ├── platform/               #   OS abstraction layer
│       │   ├── mod.rs              #     Trait definitions
│       │   ├── windows.rs          #     Windows implementation (primary)
│       │   └── linux.rs            #     Stub for future Linux support
│       ├── models/                 #   Shared data types
│       │   ├── mod.rs
│       │   ├── port.rs             #     PortEntry, PortState, Protocol enum
│       │   ├── process.rs          #     ProcessInfo, DigitalSignature
│       │   └── firewall.rs         #     FirewallRule, RuleAction, RuleDirection
│       └── export/                 #   Data export
│           ├── mod.rs
│           ├── json.rs             #     JSON export
│           └── csv.rs              #     CSV export
│
├── port-tui/                       # TERMINAL UI — thin adapter over port-core
│   ├── Cargo.toml                  #   depends on: port-core, ratatui, crossterm, tokio, clap
│   └── src/
│       ├── main.rs                 #   Entry point: init terminal, start event loop
│       ├── app.rs                  #   App state, main loop (Elm Architecture)
│       ├── message.rs              #   Message enum (all user actions + system events)
│       ├── update.rs               #   Update function: Message → State mutation
│       ├── components/             #   Tab panels (Component trait)
│       │   ├── mod.rs
│       │   ├── overview.rs         #     Dashboard summary tab
│       │   ├── ports.rs            #     Port list with search/filter
│       │   ├── history.rs          #     Change history timeline
│       │   ├── traffic.rs          #     Real-time traffic stats
│       │   └── firewall.rs         #     Firewall rule management
│       ├── widgets/                #   Reusable TUI widgets
│       │   ├── mod.rs
│       │   ├── table.rs            #     Sortable/filterable data table
│       │   ├── search.rs           #     Search input bar
│       │   ├── status.rs           #     Status bar (selected count, admin state)
│       │   └── confirm.rs          #     Confirmation dialog
│       ├── theme.rs                #   Theme system (color palette from config)
│       └── keybind.rs              #   Keybinding dispatcher
│
├── port-gui/                       # DESKTOP GUI — Tauri v2 + Svelte
│   ├── src-tauri/                  #   Rust backend
│   │   ├── Cargo.toml              #     depends on: port-core, tauri v2, tauri-plugin-sql
│   │   ├── tauri.conf.json         #     Tauri configuration
│   │   ├── capabilities/           #     Permission capabilities
│   │   │   └── default.json
│   │   ├── icons/                  #     App icons
│   │   └── src/
│   │       ├── main.rs             #     Tauri entry point: build, plugin registration
│   │       ├── lib.rs              #     Command registration, app setup
│   │       ├── commands/           #     #[tauri::command] functions
│   │       │   ├── mod.rs
│   │       │   ├── ports.rs        #       scan_ports, kill_process, get_process_info
│   │       │   ├── firewall.rs     #       list_rules, create_rule, delete_rule
│   │       │   ├── monitor.rs      #       start_monitor, stop_monitor, get_traffic
│   │       │   └── settings.rs     #       get_config, update_config, switch_theme
│   │       ├── events.rs           #     AppHandle::emit event forwarding from core EventBus
│   │       ├── tray.rs             #     System tray setup (icon, menu, popup panel)
│   │       └── elevate.rs          #     UAC elevation detection and prompt logic
│   │
│   └── src/                        #   Svelte frontend (WebView2)
│       ├── app.html                #     HTML entry point
│       ├── lib/                    #     Svelte components
│       │   ├── App.svelte          #       Root layout (sidebar + content)
│       │   ├── PortTable.svelte    #       Sortable/filterable port grid
│       │   ├── PortDetail.svelte   #       Process detail panel (slide-out)
│       │   ├── FirewallPanel.svelte#       Firewall rule editor
│       │   ├── TrafficGraph.svelte #       Real-time traffic chart
│       │   ├── HistoryTimeline.svelte#     Port change log view
│       │   ├── SearchBar.svelte    #       Faceted search (port, PID, name, protocol)
│       │   ├── TrayPopup.svelte    #       Mini port list for tray popup
│       │   ├── Settings.svelte     #       Settings/preferences panel
│       │   ├── ConfirmDialog.svelte#       Confirmation modal (kill whitelisted process)
│       │   └── StatusBar.svelte    #       Admin status, refresh indicator
│       ├── stores/                 #     Svelte stores (reactive state)
│       │   ├── ports.ts            #       Port list store
│       │   ├── settings.ts         #       Settings store
│       │   └── events.ts           #       Tauri event listeners → store updates
│       └── i18n/                   #     Frontend i18n
│           ├── en.json             #       English strings
│           └── zh.json             #       Chinese strings
│
└── resources/                      # SHARED RESOURCES
    ├── i18n/                       #   Fluent translation files
    │   ├── en-US/
    │   │   └── main.ftl            #     English translations
    │   └── zh-CN/
    │       └── main.ftl            #     Chinese translations
    ├── themes/                     #   Theme preset files
    │   ├── one-dark.toml
    │   ├── dracula.toml
    │   ├── solarized.toml
    │   ├── nord.toml
    │   ├── monokai.toml
    │   └── high-contrast.toml
    └── whitelist/                  #   Default system-critical process whitelist
        └── default.toml            #     (smss.exe, csrss.exe, wininit.exe, services.exe, lsass.exe, svchost.exe, System, etc.)
```

### Structure Rationale

- **`port-core/`:** Single source of truth. Every business rule, data model, and OS interaction lives here. Both frontends consume it, never duplicate it. The platform abstraction layer lives here because future Linux/macOS support touches scanner, process, and firewall modules -- not UI.
- **`port-tui/`:** Thin adapter. It translates keyboard events into `Message` enum variants, calls `port-core` functions, and renders results as Ratatui widgets. No business logic. Its `components/` directory follows the Ratatui Component Architecture pattern where each tab encapsulates its own state, event handling, and rendering.
- **`port-gui/src-tauri/`:** Thin adapter. It registers `#[tauri::command]` functions that delegate to `port-core`. Its `events.rs` subscribes to the core EventBus and forwards events to the WebView via `AppHandle::emit`. The `elevate.rs` module handles Windows-specific UAC detection.
- **`port-gui/src/`:** Pure frontend. Svelte components render data received via Tauri IPC. Svelte stores hold reactive state. No direct OS access -- all system interaction goes through Tauri commands.
- **`resources/`:** Shared between both frontends. Theme definitions, i18n files, and default whitelists are read by `port-core::config` and `port-core::i18n` regardless of which frontend is running.

## Architectural Patterns

### Pattern 1: Trait-Based Platform Abstraction

**What:** Define Rust traits for OS-dependent operations (port scanning, process management, firewall). Each platform gets its own implementation behind `#[cfg(target_os)]` gates. The core library calls through the trait, never directly to `windows-rs`.

**When to use:** Any operation that differs by OS. Required here because Linux/macOS support is an explicit extension point.

**Trade-offs:** Adds indirection but prevents Windows API leakage into business logic. Trait objects add minor overhead -- acceptable for operations called at most a few times per second.

**Example:**
```rust
// port-core/src/platform/mod.rs
#[async_trait]
pub trait PortScanner: Send + Sync {
    async fn scan_tcp(&self) -> Result<Vec<PortEntry>>;
    async fn scan_udp(&self) -> Result<Vec<PortEntry>>;
}

// port-core/src/platform/windows.rs
#[cfg(target_os = "windows")]
pub struct WindowsPortScanner;

#[cfg(target_os = "windows")]
#[async_trait]
impl PortScanner for WindowsPortScanner {
    async fn scan_tcp(&self) -> Result<Vec<PortEntry>> {
        // windows-rs GetExtendedTcpTable calls via tokio::task::spawn_blocking
    }
}
```

### Pattern 2: Elm Architecture (TEA) for TUI

**What:** The TUI uses a single `App` struct holding all state, a `Message` enum for all possible actions, an `update()` function transforming state, and per-component `render()` methods. The main loop: render current state, poll for events, map events to Messages, call update(), repeat.

**When to use:** Terminal UIs where all state is centrally managed and the event loop is single-threaded. Ratatui's official documentation recommends TEA as the starting pattern.

**Trade-offs:** Single global state can become large. Mitigated here because tab components each own a focused subset (ports, history, traffic, firewall). `Message` enum variants are namespaced by domain (e.g., `Message::Ports(PortAction)`).

**Example:**
```rust
// port-tui/src/message.rs
pub enum Message {
    Quit,
    Tick,                       // Periodic refresh
    SwitchTab(usize),           // Tab index
    Ports(PortAction),          // Delegated to ports component
    Firewall(FirewallAction),   // Delegated to firewall component
    SearchInput(char),          // Search bar input
    ConfirmKill(Pid),           // Kill confirmation
    // ...
}

// port-tui/src/update.rs
pub fn update(app: &mut App, msg: Message) -> Option<Message> {
    match msg {
        Message::Tick => {
            app.ports.refresh();     // Calls port-core::PortScanner::scan_tcp()
            app.traffic.poll();      // Calls port-core::TrafficMonitor::snapshot()
            None
        }
        Message::ConfirmKill(pid) => {
            if app.whitelist.contains(pid) {
                app.show_confirmation = true;  // Require explicit confirmation
                None
            } else {
                app.process_manager.kill(pid); // Instant kill
                Some(Message::Tick)            // Refresh after kill
            }
        }
        // ...
    }
}
```

### Pattern 3: Tauri Command + Event Bridge

**What:** The Tauri GUI exposes core functionality through typed `#[tauri::command]` functions. The frontend calls them via `invoke()`. For streaming data (port changes, traffic stats), the Rust side subscribes to the core EventBus and forwards events via `AppHandle::emit`; the frontend listens via `listen()` and updates Svelte stores.

**When to use:** Whenever the GUI needs to call Rust code or receive push updates. This is Tauri's standard IPC pattern.

**Trade-offs:** IPC serialization overhead (JSON). Acceptable for data volumes at port management scale (hundreds of ports, not millions of events).

**Example:**
```rust
// port-gui/src-tauri/src/commands/ports.rs
#[tauri::command]
async fn scan_ports(state: State<'_, AppState>) -> Result<Vec<PortEntry>, String> {
    state.scanner.scan_tcp().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn kill_process(pid: u32, state: State<'_, AppState>) -> Result<(), String> {
    state.process_manager.smart_kill(pid).await.map_err(|e| e.to_string())
}

// port-gui/src-tauri/src/events.rs
pub fn start_event_forwarding(app_handle: AppHandle, core_bus: BroadcastReceiver<CoreEvent>) {
    tokio::spawn(async move {
        let mut rx = core_bus.subscribe();
        while let Ok(event) = rx.recv().await {
            let _ = app_handle.emit("core-event", event);
        }
    });
}
```

```typescript
// port-gui/src/stores/ports.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const ports = writable<PortEntry[]>([]);

export async function refreshPorts() {
    ports.set(await invoke<PortEntry[]>('scan_ports'));
}

listen<CoreEvent>('core-event', (event) => {
    if (event.payload.type === 'ports-changed') {
        refreshPorts();
    }
});
```

### Pattern 4: Event Bus for Cross-Cutting Updates

**What:** Core components (scanner, monitor) publish typed events to a `tokio::sync::broadcast` channel. Frontend adapters subscribe and forward to their respective rendering pipelines. Components never call each other directly -- they go through the bus.

**When to use:** Decoupling producers (port scanner, ETW listener, process killer) from consumers (TUI tabs, GUI panels, history recorder). Multiple consumers need the same event stream.

**Trade-offs:** broadcast channels drop messages when all receivers lag. Use a ring buffer (`ringbuf` crate) for the history recorder to guarantee delivery. Use bounded broadcast capacity (256 events) -- acceptable for port monitoring at human-visible timescales.

**Example:**
```rust
// port-core/src/events/bus.rs
#[derive(Clone, Debug, Serialize)]
pub enum CoreEvent {
    PortsScanned(Vec<PortEntry>),
    PortChanged { old: PortEntry, new: PortEntry },
    ProcessKilled { pid: u32, success: bool },
    TrafficUpdate { pid: u32, bytes_sent: u64, bytes_recv: u64 },
    FirewallRuleChanged { action: RuleAction, rule: FirewallRule },
}

pub struct EventBus {
    tx: broadcast::Sender<CoreEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }
    pub fn publish(&self, event: CoreEvent) {
        let _ = self.tx.send(event);
    }
}
```

## Data Flow

### Primary Scan-and-Display Flow

```
User presses 'r' (TUI)         Timer tick / user clicks "Refresh" (GUI)
    │                                      │
    ▼                                      ▼
Message::Tick / scan_ports()        invoke('scan_ports')
    │                                      │
    ▼                                      ▼
port-core::PortScanner              port-core::PortScanner
    │                                      │
    ▼                                      ▼
Windows IP Helper API ──→ Vec<PortEntry>
(GetExtendedTcpTable)         │
                              ├──→ EventBus::publish(PortsScanned(entries))
                              │         │
                              ├──→ DataStore::record_history(entries)
                              │         │
                              ▼         ▼
                    TUI: Message::Tick → update → render
                    GUI: listen('core-event') → Svelte store → reactive render
```

### Kill Process Flow

```
User selects port, presses 'k' (TUI)    User right-clicks port → "Kill" (GUI)
    │                                              │
    ▼                                              ▼
Message::KillProcess(pid)                 invoke('kill_process', { pid })
    │                                              │
    ▼                                              ▼
Whitelist check ←──────────────────── Whitelist check
    │                                              │
    ├── Protected → ConfirmDialog      ├── Protected → ConfirmDialog (Svelte)
    │       │                                  │
    │       └── User confirms                 └── User confirms
    │                                              │
    ▼                                              ▼
ProcessManager::smart_kill(pid)         ProcessManager::smart_kill(pid)
    │                                              │
    ├── 1. SIGTERM / TerminateProcess (graceful)
    │       ├── Success → done
    │       └── Timeout (2s)
    │
    └── 2. TerminateProcess(force) / taskkill /F
            │
            └── EventBus::publish(ProcessKilled { pid, success })
                    │
                    ├──→ TUI: refresh port list
                    └──→ GUI: refresh port list
```

### ETW Traffic Monitoring Flow

```
Windows Kernel TCP/IP Stack
    │
    ▼
ETW Kernel-Network Provider ──→ ferrisetw::KernelTrace callback
    │                                    │
    │                              TrafficMonitor::on_event()
    │                                    │
    │                              Parse: PID, local port, bytes sent/recv
    │                                    │
    │                              Accumulate per-PID counters
    │                                    │
    │                              Every ~1s: EventBus::publish(TrafficUpdate)
    │                                    │
    ▼                                    ▼
2s Polling Fallback              TUI traffic tab / GUI traffic graph
(when ETW unavailable or         updates incrementally via subscription
elevation insufficient)
```

### Configuration Change Flow

```
User presses 't' (theme switch) / 'l' (language switch)
    │
    ▼
port-core::ConfigManager::set_theme() / set_locale()
    │
    ├──→ Save to TOML config file
    ├──→ Reload theme palette / Fluent bundle
    └──→ EventBus::publish(ConfigChanged)
            │
            ├──→ TUI: re-render with new colors/strings
            └──→ GUI: emit event → Svelte stores → reactive UI update
```

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single user (local desktop) | Everything in-process. SQLite with WAL mode. Single tokio runtime. Monorepo workspace. Default target. |
| Hundreds of ports | Current design handles this. Port table rendering with virtualization (render only visible rows). |
| Thousands of ports (unusual) | Paginate port table. Debounce ETW events (batch per-second instead of per-event). SQLite indexes on timestamp + PID. |
| Multi-tenant (future) | Would require process isolation per user. Not in scope for v1-v3. |

### Scaling Priorities

1. **First bottleneck:** Port table rendering at 1000+ entries. Fix with virtual scrolling (render only viewport rows + overscan).
2. **Second bottleneck:** ETW event volume under heavy network load. Fix with ring buffer aggregation (per-second snapshots, not per-packet).
3. **Third bottleneck:** SQLite write contention during rapid port changes. Fix with write batching (accumulate changes for 100ms, commit in one transaction).

## Anti-Patterns

### Anti-Pattern 1: Business Logic in Frontend

**What people do:** Put port filtering, whitelist checking, or kill strategy logic in the Svelte components or TUI update functions.

**Why it's wrong:** Logic duplicated across two frontends. Inconsistent behavior. Cannot test without UI. Breaks the "thin adapter" contract.

**Do this instead:** All logic in `port-core`. Frontends call core functions and render results. If you need a new scan mode, add it to `PortScanner`, not to the Svelte search bar.

### Anti-Pattern 2: Direct Windows API Calls from Frontend Crates

**What people do:** `port-tui` or `port-gui/src-tauri` importing `windows-rs` directly to call `TerminateProcess` or `GetExtendedTcpTable`.

**Why it's wrong:** Bypasses the platform abstraction layer. Linux/macOS support becomes impossible. Testing requires Windows. Core library loses its reason to exist.

**Do this instead:** All OS calls go through `port-core::platform` traits. Frontends never import `windows-rs`.

### Anti-Pattern 3: Synchronous Blocking in Async Context

**What people do:** Calling `GetExtendedTcpTable` (which involves FFI and kernel transitions) directly from an async tokio task without `spawn_blocking`.

**Why it's wrong:** Blocks the tokio runtime worker thread. Under load, all async tasks stall. UI freezes.

**Do this instead:**
```rust
let entries = tokio::task::spawn_blocking(move || {
    // Windows API calls here
    get_extended_tcp_table()
}).await??;
```

### Anti-Pattern 4: Duplicate State Between Core and Frontend

**What people do:** Keep a `Vec<PortEntry>` in `port-core` and a separate copy in the Svelte store and a third in the TUI `App` struct. Manual sync.

**Why it's wrong:** Drift between copies. "Which one is authoritative?" bugs. State reconciliation complexity.

**Do this instead:** The core is the single source of truth. Frontends hold a read-only snapshot that's replaced wholesale on each scan, sourced from the EventBus or IPC command return value.

### Anti-Pattern 5: Assuming Admin Rights

**What people do:** Writing code that always calls admin-requiring APIs without checking privilege level first. Silent failures with opaque error codes.

**Why it's wrong:** User runs without admin, operations silently fail, user doesn't know why. Or: app crashes with access-denied errors the user can't interpret.

**Do this instead:** Explicit privilege detection at startup. Graceful degradation (e.g., show ports but mark "admin required to kill" on system processes). Trigger UAC elevation only when user initiates an admin-requiring action. The `elevate.rs` module in `port-gui/src-tauri` encapsulates this pattern.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Windows IP Helper API (`iphlpapi.dll`) | FFI via `windows-rs` crate, called through `spawn_blocking` | Two-call pattern: first call gets buffer size, second fetches data. Port numbers in network byte order (`ntohs` conversion). |
| Windows Firewall API (COM) | `INetFwPolicy2` via `windows-rs` COM interop | Requires admin rights for modifications. `windows-rs` provides generated COM bindings. |
| ETW Kernel-Network Provider | `ferrisetw::KernelTrace` real-time session | Kernel trace requires admin/elevated session. Callback must not block. Buffer configuration prevents event loss. |
| Windows Process API (`kernel32.dll`) | FFI via `windows-rs`: `OpenProcess`, `QueryFullProcessImageNameW`, `TerminateProcess` | Process handle must be closed. Access rights vary by process protection level. |
| SQLite Database | `rusqlite` synchronous in `Arc<Connection>` | WAL mode for concurrent reads. `busy_timeout = 5000ms`. Write access serialized via `Mutex` or dedicated write task. |
| Configuration Files (TOML) | `config` crate with file source | Read at startup, write on change. Default config compiled into binary as fallback. |
| i18n Resource Files (.ftl) | `fluent`/`fluent-bundle` loaded from `resources/i18n/` | Bundle cached after load. Locale switch triggers reload. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `port-core` ↔ `port-tui` | Direct function calls (same process, linked crate) | TUI calls `port-core` APIs directly. No serialization. Returns are in-memory Rust types. |
| `port-core` ↔ `port-gui/src-tauri` | Direct function calls (same process, linked crate) | Tauri commands wrap core APIs. Return types must be `Serialize + Deserialize` for IPC to WebView. |
| `port-gui/src-tauri` ↔ Svelte WebView | Tauri IPC: `invoke()` for commands, `emit()`/`listen()` for events | JSON serialization boundary. Command arguments and return values are typed via TypeScript interfaces. |
| Scanner ↔ EventBus | `EventBus::publish()` call after each scan | Fire-and-forget. Scanner does not know or care who subscribes. |
| EventBus ↔ History Recorder | `EventBus::subscribe()` → write to SQLite | Dedicated subscriber that persists `PortChanged` events. Uses ring buffer to avoid loss. |
| EventBus ↔ Frontends | `EventBus::subscribe()` → forward to render pipeline | TUI receives in main loop tick. GUI receives via Tauri event emit. |
| Tauri Core ↔ System Tray | `tauri::tray::TrayIconBuilder` | Left-click opens popup panel (mini port list). Right-click opens context menu. Events routed through Tauri's built-in tray API. |

## Build Order Implications

Components have clear dependency chains. Suggested build order:

```
Phase 1: Foundation
  port-core::models     (data types with serde)
  port-core::config     (TOML loading, settings struct)
  port-core::store      (SQLite schema, connection, migrations)
      │
Phase 2: Core Capabilities
  port-core::platform   (trait definitions)
  port-core::scanner    (Windows port scanning — first working feature)
  port-core::process    (process info + kill)
      │
Phase 3: First Frontend (TUI)
  port-tui              (terminal UI — fastest to iterate, validates core API)
      │
Phase 4: Event System + Monitoring
  port-core::events     (EventBus)
  port-core::monitor    (ETW + polling)
      │
Phase 5: Second Frontend (GUI)
  port-gui              (Tauri + Svelte — richer UX, system tray)
      │
Phase 6: Advanced Features
  port-core::firewall   (Firewall management)
  port-core::export     (JSON/CSV)
  port-core::i18n       (fluent bundles)
```

**Rationale:**
- Phase 1 must come first: everything depends on data types, config, and storage.
- Phase 2 validates the platform abstraction layer against real Windows APIs. The scanner is the most critical component -- if this doesn't work, nothing does.
- Phase 3 (TUI) before Phase 5 (GUI): The TUI is faster to build and iterate. It validates the core API surface without the Tauri/Svelte toolchain complexity. If the core API is awkward, you find out in Phase 3, not Phase 5.
- Phase 4 (events/monitor) can start after Phase 2 but is listed after Phase 3 because a working TUI provides a visualization target for event streams. You can see ETW events arrive in real time.
- Phase 6 (firewall, export, i18n) has no hard dependency on Phase 5 but is logically separated as "advanced/auxiliary" features that build on the solid core.
- `port-core::whitelist` is embedded in `process/` (Phase 2) because process termination cannot be safely implemented without it.

## Sources

- Ratatui Official Documentation: Application Patterns (The Elm Architecture, Component Architecture, Flux Architecture) -- https://ratatui.rs/concepts/application-patterns/
- Ratatui Official Documentation: Event Handling -- https://ratatui.rs/concepts/event-handling/
- Tauri v2 Official Documentation: Process Model -- https://v2.tauri.app/concept/process-model/
- ferrisetw crate (ETW consumer for Rust) -- https://docs.rs/ferrisetw/latest/ferrisetw/
- Microsoft rust_win_etw (ETW provider for Rust) -- https://github.com/microsoft/rust_win_etw
- rustnet crate (Windows IP Helper API in Rust) -- https://github.com/domcyrus/rustnet
- netstat2-rs crate (cross-platform socket information) -- https://github.com/ohadravid/netstat2-rs
- Windows Process Termination Pitfalls (System Informer issue #2629) -- https://github.com/winsiderss/systeminformer/issues/2629
- Arkenar Project Architecture (Rust workspace with core + CLI + Tauri GUI) -- https://github.com/realozk/arkenar
- Tauri v2 Monorepo Structure -- https://deepwiki.com/tauri-apps/tauri/1.3-project-structure

---
*Architecture research for: Windows Port Management Tool (Portunity)*
*Researched: 2026-07-26*

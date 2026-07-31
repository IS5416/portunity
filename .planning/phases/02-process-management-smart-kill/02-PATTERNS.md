# Phase 2: Process Management & Smart Kill — Pattern Map

**Mapped:** 2026-07-31
**Files analyzed:** 22 (9 port-core, 11 port-tui, 1 workspace Cargo.toml, 1 filter.rs compile-fix)
**Analogs found:** 19 with match / 3 no-analog (2 integration test files, 1 absent Help overlay)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `port-core/src/process.rs` (M) | service (trait + module decls) | request-response | `port-core/src/scanner.rs` | exact |
| `port-core/src/process/handle.rs` (N) | service (Win32 wrapper) | request-response | `port-core/src/scanner/resolver.rs` + `scanner/tcp.rs` | role-match |
| `port-core/src/process/info.rs` (N) | service (Win32 fetchers) | request-response | `port-core/src/scanner/tcp.rs` | role-match |
| `port-core/src/process/kill.rs` (N) | service (pipeline) | request-response | `port-core/src/scanner/tcp.rs` (spawn_blocking + error map) | role-match |
| `port-core/src/process/whitelist.rs` (N) | utility (pure logic) | transform/query | `port-core/src/filter.rs` | exact |
| `port-core/src/config/settings.rs` (M) | config | file-I/O | itself (self-extension) | exact (self) |
| `port-core/src/models/process.rs` (M) | model | data | itself (add field) | exact (self) |
| `port-core/src/scanner.rs` (M) | service (batch transform) | transform | itself (`scan_all` post-pass) | exact (self) |
| `port-core/src/filter.rs` (M — compile fix only) | utility | transform | itself (test constructor) | exact (self) |
| `port-core/tests/kill_integration.rs` (N) | test (integration) | — | none — no `tests/` dir exists | no analog |
| `port-core/tests/process_handle_integration.rs` (N) | test (integration) | — | none — no `tests/` dir exists | no analog |
| `Cargo.toml` (workspace) (M) | config | — | itself (windows feature list, lines 21-32) | exact (self) |
| `port-tui/src/main.rs` (M) | controller (keys + render) | event-driven | itself (map_key_event, render_app, status bar, footer) | exact (self) |
| `port-tui/src/app.rs` (M) | store (App state) | state | itself (search/filter overlay state precedent) | exact (self) |
| `port-tui/src/message.rs` (M) | model (Message enum) | event-driven | itself (FilterField `next()`/`prev()` cycle) | exact (self) |
| `port-tui/src/update.rs` (M) | controller (state transitions) | event-driven | itself (search/filter handler blocks) | exact (self) |
| `port-tui/src/components.rs` (M) | config (registration) | — | itself (`pub mod` + `pub use`) | exact (self) |
| `port-tui/src/components/detail_panel.rs` (N) | component (overlay) | render | `port-tui/src/components/filter_panel.rs` | exact |
| `port-tui/src/components/kill_confirm.rs` (N) | component (popup) | render | `port-tui/src/components/filter_panel.rs` + `search.rs` | exact |
| `port-tui/src/components/whitelist_overlay.rs` (N) | component (overlay) | render | `filter_panel.rs` + `search.rs` + `ports.rs` scrollbar | role-match (no `List` precedent in codebase) |
| `port-tui/src/components/ports.rs` (M) | component (table) | render | itself (process column, SYSTEM_NAMES, truncate, scrollbar) | exact (self) |
| `port-tui/src/theme.rs` | — | — | itself — **no change needed**: `accent_secondary` already exists (line 22) | exact (self) |

**Legend:** M = modify, N = new. "exact (self)" = the file itself is the closest analog; modifications extend its own existing patterns.

---

## Pattern Assignments

### `port-core/src/process.rs` (modify — trait reshape + module decls)

**Analog:** `port-core/src/scanner.rs` (68 lines) — parent module with trait + submodule decls + re-exports.

**Module declaration + re-export pattern** (scanner.rs lines 9-17):
```rust
pub mod tcp;
pub mod udp;
pub mod resolver;

pub use resolver::ProcessResolver;
pub use tcp::scan_tcp;
pub use udp::scan_udp;
```
Apply identically: `pub mod handle; pub mod info; pub mod kill; pub mod whitelist;` + `pub use` of `ProcessSnapshot`, `KillOutcome`, `Protection`, etc. (CLAUDE.md rule: no `mod.rs` files; `process.rs` becomes parent of `process/` dir).

**Trait shape to replace** (current process.rs lines 3-6 — stub being reshaped):
```rust
pub trait ProcessManager {
    fn details(&self, pid: u32) -> crate::Result<crate::models::ProcessInfo>;
    fn terminate(&self, pid: u32, force: bool) -> crate::Result<()>;
}
```
New signature per RESEARCH Pattern 2: `details(&self, snapshot: ProcessSnapshot)` / `terminate(&self, snapshot, strategy)` — snapshot is Send-safe, HANDLE stays in `spawn_blocking` scope.

**Async trait pattern** (scanner.rs lines 25-32) if `ProcessManager` stays async:
```rust
#[async_trait]
pub trait PortScanner: Send + Sync {
    async fn scan(&self) -> crate::Result<Vec<Connection>>;
}
```

---

### `port-core/src/process/handle.rs` (new — ProcessSnapshot, open+verify+drop)

**Analog:** `port-core/src/scanner/resolver.rs` (113 lines) — cache struct + PID 0/4 special-case + `Default` impl; `scanner/tcp.rs` for Win32 unsafe-call style.

**Struct + constructor pattern** (resolver.rs lines 14-25):
```rust
pub struct ProcessResolver {
    cache: HashMap<u32, String>,
}

impl ProcessResolver {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }
```

**PID 0/4 special-case pattern** (resolver.rs lines 45-51) — copy for whitelist/handle layers:
```rust
if pid == 0 {
    self.cache.insert(pid, "System Idle Process".to_string());
} else if pid == 4 {
    self.cache.insert(pid, "System".to_string());
}
```

**Default impl** (resolver.rs lines 109-113):
```rust
impl Default for ProcessResolver {
    fn default() -> Self { Self::new() }
}
```

**Win32 unsafe-call style** (tcp.rs lines 36-83) — `const` error codes, `unsafe { ... }`, `crate::Error::Platform` mapping:
```rust
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
let result = unsafe { GetExtendedTcpTable(...) };
if result == NO_ERROR { ... }
return Err(crate::Error::Platform(format!("... error code {}", result)));
```

**For `ProcessSnapshot`:** pure data struct (RESEARCH Pattern 1, lines 234-243) — `#[derive(Debug, Clone)]`, `pid: u32`, `creation_time: Option<FILETIME>`, `executable_path: Option<String>`; the `OpenProcessHandle { pid, handle }` wrapper with `impl Drop` is internal-only, never crosses the channel (HANDLE is `!Send` in windows-rs 0.62 — verified by researcher).

---

### `port-core/src/process/info.rs` (new — detail fetchers)

**Analog:** `port-core/src/scanner/tcp.rs` — async entry point wrapping sync Win32 work in `spawn_blocking` (lines 345-403).

**async-first + spawn_blocking pattern** (tcp.rs lines 345-346, 401-402) — THE core async discipline (Pitfall #9):
```rust
pub async fn scan_tcp() -> crate::Result<(Vec<Connection>, Vec<u32>)> {
    tokio::task::spawn_blocking(move || {
        // ... all Win32 calls here ...
    })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking failed: {}", e)))?
}
```
Every detail fetcher (`query_full_path`, `query_command_line`, `query_start_time`, `query_parent_pid`, `verify_signature`) follows this exact shape: `pub async fn` → `spawn_blocking` → `crate::Error` mapping.

**Failure → Option** convention for per-field rendering: research mandates failures render `—` (UI-SPEC detail states), so fetchers return `Option<T>`/`Option<String>` inside a `Result`-free blocking scope where possible; only the top-level async fn returns `crate::Result`.

**Unit-test target** (RESEARCH validation map PROC-06): FILETIME→SystemTime conversion is pure (Code Example 6: `ft_u64 / 10_000_000 - 11_644_473_600`); UNICODE_STRING extraction is pure-buffer math — both unit-testable without Windows, in-module `#[cfg(test)]` per filter.rs precedent (see whitelist.rs assignment).

---

### `port-core/src/process/kill.rs` (new — escalation pipeline)

**Analog:** `scanner/tcp.rs` (spawn_blocking + error-code mapping + `crate::Error::Platform`); pure routing function pattern from `filter.rs` (free functions, no trait).

**Pure routing fn — unit-testable without Windows** (RESEARCH Pattern 2, lines 279-287):
```rust
pub fn route_strategy(has_visible_windows: bool, has_console: bool) -> Strategy {
    match (has_visible_windows, has_console) {
        (true, _)    => Strategy::WmClose,
        (false, true) => Strategy::ConsoleCtrlC,
        (false, false) => Strategy::ForceDirect,
    }
}
```
This mirrors the filter.rs free-function style (lines 27-86: `pub fn apply_filters(...) -> Vec<Connection>` — no trait, pure logic).

**KillOutcome enum** (RESEARCH Pattern 2, lines 269-277): `Graceful | ForceKilled | Direct | AlreadyExited | AccessDenied | HardBlocked(&'static str) | Failed(String)` — the TUI maps this to status-bar strings + `Message::KillOutcome`.

**Execution skeleton** (all inside one `spawn_blocking`, RESEARCH Pattern 2 steps 1-8): re-read settings → protection check (before OpenProcess — Pitfall #11) → `OpenProcess` with minimal rights const (RESEARCH Code Example 1):
```rust
const RIGHTS: PROCESS_ACCESS_RIGHTS =
    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE;
```
→ verify `GetProcessId` + `GetProcessTimes` creation FILETIME (Pitfall #1) → route → `WaitForSingleObject` → `TerminateProcess` → map errors.

**Error mapping** — lib.rs `Error` variants already exist (lines 8-18): `Platform(String)`, `NotFound(String)`, `PermissionDenied(String)` (maps ERROR_ACCESS_DENIED → `KillOutcome::AccessDenied` per D-03), `Io(#[from] std::io::Error)`.

---

### `port-core/src/process/whitelist.rs` (new — BUILTIN, matchers, Protection)

**Analog:** `port-core/src/filter.rs` (259 lines) — EXACT: pure free-function module + inline `#[cfg(test)]` tests.

**Free-function module pattern** (filter.rs lines 1-9, 27):
```rust
//! Multi-dimensional port/process filtering engine.
//! No trait needed — the filter module has no platform-specific variants.

use crate::models::{Connection, Filter};

pub fn apply_filters(connections: &[Connection], filter: &Filter) -> Vec<Connection> {
```
whitelist.rs: `pub const BUILTIN: &[BuiltinEntry]` + `pub fn builtin_match(pid: u32, basename: &str) -> Option<&'static str>` (returns reason) + `pub fn user_match(path: &str, entries: &[String]) -> bool` (case-insensitive, normalized) + `pub enum Protection { None, UserConfirm, HardBlocked(&'static str) }`.

**Inline test module pattern** (filter.rs lines 123-259) — the unit-test home for PROC-03/04/05:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_range_filter() {
        // arrange / act / assert with no external deps
    }
}
```
Test contract to lock (RESEARCH validation map): built-in list ≥25 entries / lowercase / unique / reason present (PROC-04); path normalization — quotes, case, trailing `\`, 8.3 (PROC-05); PID 0/4 special case; built-in checked BEFORE user tier (Pitfall 6).

---

### `port-core/src/config/settings.rs` (modify — whitelist + kill_timeout_secs)

**Analog:** itself (99 lines). The extension pattern is already established — copy the existing serde-default pattern exactly.

**Serde default field pattern** (settings.rs lines 10-18):
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// Whether the current session detected administrator privileges.
    #[serde(default)]
    pub admin_detected: bool,

    /// Schema version for forward-compatibility.
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
}
```
Add: `#[serde(default)] pub whitelist: Vec<String>` + `#[serde(default = "default_kill_timeout_secs")] pub kill_timeout_secs: u64` (default 5, D-02). **Must also update `default_settings()`** (lines 25-30) — it constructs every field explicitly.

**TOML error mapping pattern** (settings.rs lines 57-59, 92-94) — reuse for new save path:
```rust
let toml_str = toml::to_string_pretty(&defaults).map_err(|e| {
    crate::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
})?;
```

**Backward-compat test target** (RESEARCH Wave 0): serde round-trip on a Phase-1-era TOML fixture proving defaults apply — settings.rs has no test module today; add `#[cfg(test)]` per filter.rs precedent.

**Instant-effect (D-15):** no cache in core — `load_settings()` is called fresh inside `kill.rs` before every attempt (<1ms). The `w` overlay calls `save_settings()` (existing fn, lines 85-99) after add/remove.

---

### `port-core/src/models/process.rs` (modify — add `user_protected: bool`)

**Analog:** itself (14 lines). Add field per RESEARCH Pattern 4 / Assumption A4:
```rust
pub is_system_critical: bool,
pub user_protected: bool,   // NEW — user whitelist confirmation gate (A4)
pub parent_pid: Option<u32>,
```

**⚠ Compile-fix blast radius — 5 explicit struct-literal constructors MUST add `user_protected: false`:**
- `port-core/src/filter.rs:136-145` (test `make_conn`)
- `port-core/src/scanner/tcp.rs:282-291` and `tcp.rs:321-330`
- `port-core/src/scanner/udp.rs:146-155` and `udp.rs:177-186`

Note: tcp.rs/udp.rs currently set `is_system_critical: pid == 0 || pid == 4` (tcp.rs:289, udp.rs:153) — Phase 2 replaces this heuristic with whitelist membership via the `scan_all` post-pass (keeps the PID 0/4 special case inside `builtin_match`). The Phase 1 `SYSTEM_NAMES` heuristic lives in **TUI** ports.rs:364-382 and is superseded by the core whitelist (see ports.rs assignment).

---

### `port-core/src/scanner.rs` (modify — scan-time protection markers)

**Analog:** itself. `scan_all()` (lines 39-68) gets a post-pass after name resolution, per RESEARCH Pattern 4. The existing loop structure is the insertion point:

**Name-application loop pattern** (scanner.rs lines 59-65) — the marker post-pass appends to this same loop (or a parallel one):
```rust
for conn in &mut tcp_conns {
    let pid = conn.process.pid;
    if let Some(name) = resolver.get(pid) {
        conn.process.name = name.to_string();
    }
}
```
Post-pass per RESEARCH Pattern 4 (lines 329-343): build `user_entry_basenames: HashSet<String>` from `settings.whitelist`; for each conn, `builtin_match(pid, basename)` → `is_system_critical = true`; else if basename in user set → `QueryFullProcessImageNameW` (in the existing spawn_blocking scan scope) → `user_match(path)` → `user_protected`. Settings load must also happen here (D-15 also covers scan-time markers).

---

### `port-core/src/filter.rs` (modify — compile fix only)

No behavior change (SRCH-02 already implemented, verification + traceability only). The single edit: add `user_protected: false` to the `make_conn` test constructor (line 136-145). See models/process.rs assignment for exact site.

---

### `Cargo.toml` (workspace, modify — 2 feature flags)

**Analog:** itself, lines 21-32. Add to the existing `windows = { version = "0.62", features = [...] }` list:
```toml
"Win32_Security_WinTrust",   # WinVerifyTrustEx (signature, D-07)
"Win32_System_Console",      # AttachConsole/GenerateConsoleCtrlEvent (Ctrl+C helper)
```
(RESEARCH verification: all other needed modules — Threading, WindowsAndMessaging, ToolHelp, Foundation — already enabled. Zero new crates.)

---

### `port-tui/src/main.rs` (modify — keys, overlays, helper mode, status bar, footer)

**Analog:** itself (676 lines). Five distinct edit zones, each extending an existing pattern.

**1. Overlay key dispatch** — copy the search-mode dispatch block (lines 219-237) for each new overlay:
```rust
// --- Search mode dispatch ---
if app.search_active {
    match key.code {
        KeyCode::Esc => return Some(Message::SearchDeactivate),
        KeyCode::Enter => return Some(Message::SearchDeactivate),
        KeyCode::Backspace => return Some(Message::SearchBackspace),
        ...
        // Pass-through: all other keys (j, k, r, s, etc.) continue to work
        _ => {}
    }
}
```
New blocks (per UI-SPEC Keyboard Contract): `app.detail_active` (intercept `d`/`Esc`, pass through j/k/↑/↓/r/s/g/G//f), `app.whitelist_active` (intercept j/k/↑/↓/d/Tab/BackTab/Enter/Esc/printable/←/→, pass r/s/tabs), `app.confirm_active` (intercept `y`/`n`/`Enter`/`Esc`/`x` — `x` no-op prevents double-kill fire). **Key registration order matters:** overlay checks precede the default-mode match (lines 275-316) exactly as search/filter checks precede it today.

**2. Default-mode new keys** (lines 276-316) — add to the existing match:
```rust
KeyCode::Char('d') => Some(Message::ToggleDetailPanel),
KeyCode::Char('x') => Some(Message::Kill(...)),
KeyCode::Char('w') => Some(Message::ToggleWhitelistOverlay),
```
`k` → MoveUp at line 281 stays untouched (D-01 conflict resolved by choosing `x`).

**3. spawn_blocking intercept in the event loop** — the ElevateRequest pattern (lines 156-179) is the EXACT template for Kill + DetailData fetch + SignatureVerify (all Win32, all must leave the async runtime):
```rust
if matches!(m, Message::ElevateRequest) {
    if !app.elevating {
        app.elevating = true;
        let tx_elevate = tx.clone();
        tokio::task::spawn_blocking(move || {
            match elevate::elevate_to_admin() {
                Ok(()) => { let _ = tx_elevate.send(Message::ElevateDeclined); }
                Err(e) => { let _ = tx_elevate.send(Message::ScanError(format!("Elevation failed: {}", e))); }
            }
        });
    }
} else {
    update(app, m);
    app.needs_render = true;
}
```
For Kill: guard on `app.kill_in_flight` (mirrors `app.elevating` guard), send `Message::KillOutcome(...)` back through the same `tx` clone. For detail fetch: guard on `app.detail_loading` / cache check, send `DetailDataLoaded` / `SignatureVerified`.

**4. Overlay render placement** — copy the search/filter overlay rect pattern (lines 395-415); new overlays are Clear-over (no table squeeze, D-05), rendered AFTER the table in stack order (UI-SPEC overlay stack):
```rust
if app.search_active {
    let search_overlay = Rect { height: 3, ..content_area };
    SearchComponent.render(app, f, search_overlay, theme);
}
```
Phase 2 stack (UI-SPEC): table → search → filter → detail (12 rows, top-anchored `Rect { height: 12, ..content_area }`) → whitelist (content height − 1) → confirm (centered: `Rect { x: (w-60)/2, y: content_area.y + (h-7)/2, width: 60, height: 7 }`). Confirm must render last and `x` is intercepted while open.

**5. Status bar kill outcomes** (D-04) — copy the error-state block (lines 519-532); add a `kill_message: Option<KillStatus>` branch rendering the 8 locked strings from UI-SPEC Kill Flow Copy (error color for failures, normal for info, `✓`/`✗` symbols). Status bar messages must respect the ⚠ unresolved overflow rule (Assumption A9): truncate preserving the actionable tail ("Press w to review the whitelist"), ≤80 cols.

**6. Footer** — copy the context-sensitive footer dispatch (lines 583-676); add branches for `detail_active`, `confirm_active`, `whitelist_active` with the locked strings from UI-SPEC Footer table; the Ports-tab default footer (lines 641-673) loses `[a]Elevate`, `[s]Sort`, `[w]List`... — wait, there is no `[w]List` today; the Phase 1 footer has `[s]Sort` + `[a]Elevate` conditional — per UI-SPEC both drop from the Ports footer, leaving the locked 73-col string `[jk]Move [/]Search [f]Filter [d]Detail [x]Kill [r]Refresh [q]Quit [?]Help`. `{name}` truncation rule: `…` to `term_width − L` per declared budget.

**7. `--ctrl-c <pid>` helper mode** — clap derive (port-tui/Cargo.toml already depends on clap 4.6):
```rust
#[arg(long, hide = true)]
ctrl_c_pid: Option<u32>,
```
At the very top of `main()` (before terminal init/raw mode), if `Some`, run the helper routine (SetConsoleCtrlHandler(NULL,true) → FreeConsole → AttachConsole(pid) → GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0)) and `std::process::exit(code)` — exit 0 = delivered, 1 = no console (Pitfall 7 guard). The helper spawns via `port-core` kill.rs: `Command::new(current_exe).arg("--ctrl-c").arg(pid).creation_flags(0x08000000)`.

---

### `port-tui/src/app.rs` (modify — new state)

**Analog:** itself (140 lines). Copy the search/filter overlay state pattern exactly.

**Overlay state fields pattern** (app.rs lines 45-73):
```rust
// --- Search state ---
pub search_query: String,
pub search_active: bool,
pub search_cursor_pos: usize,
// --- Filter state ---
pub active_filter: Filter,
pub filter_active: bool,
pub filter_applied: bool,
pub filter_focused_field: FilterField,
pub filter_field_text: HashMap<FilterField, String>,
```
Phase 2 additions (per UI-SPEC Message additions + RESEARCH): `detail_active: bool`, `detail_data: Option<ProcessInfo>`, `detail_loading: bool`, `signature_cache: HashMap<u32, Option<bool>>` (D-07 cache, invalidated on scan), `kill_message: Option<KillStatus>`, `kill_in_flight: bool` (mirror `elevating` guard), `confirm_pid: Option<u32>` (which process is awaiting y/n), `whitelist_active: bool`, `whitelist_focus: WhitelistFocus` (List | Input), `whitelist_selected: usize`, `whitelist_input: String` (+ cursor pos), `whitelist_settings: AppSettings` (working copy for the overlay), `pending_kill_snapshot: Option<ProcessSnapshot>`.

**`App::new()` must initialize every field** (lines 100-127 — exhaustive struct literal, same blast-radius rule as ProcessInfo).

**Helper accessor pattern** (lines 133-139):
```rust
pub fn display_data(&self) -> &[Connection] { ... }
```
Add `pub fn selected_process(&self) -> Option<&ProcessInfo>` (kill target + detail data source — `selected_index` is the source per CONTEXT integration points).

---

### `port-tui/src/message.rs` (modify — new Messages)

**Analog:** itself (177 lines). Extend the `Message` enum (lines 86-177) with the UI-SPEC list (line 316): `Kill {pid}`, `KillConfirmed {pid}`, `KillCancelled`, `KillStart`, `KillOutcome {result}`, `ToggleDetailPanel`, `DetailDataLoaded {process_info}`, `SignatureVerified {is_signed: Option<bool>}`, `ToggleWhitelistOverlay`, `WhitelistFocusNext`, `WhitelistFocusPrev`, `WhitelistSelectMove {dir}`, `WhitelistDeleteSelected`, `WhitelistInput(char)`, `WhitelistBackspace`, `WhitelistCursorMove {dir}`, `WhitelistAdd {path}`, `WhitelistSaved`, `ProcessExited {pid}`.

**Cycle-enum pattern for focus** (FilterField lines 50-84 — `next()`/`prev()` + doc comments) — reuse for `WhitelistFocus { List, Input }` (only 2 states; `Tab`/`Shift+Tab` maps to the same `next()`/`prev()` idiom).

**Import convention:** message.rs imports `port_core::models::Connection` (line 7); new imports: `port_core::models::ProcessInfo` + `port_core::process::KillOutcome`.

---

### `port-tui/src/update.rs` (modify — new handlers)

**Analog:** itself (426 lines). Copy the search/filter handler block structure (lines 92-202):
```rust
Message::SearchActivate => {
    app.search_active = true;
    app.search_query.clear();
    app.search_cursor_pos = 0;
    app.filtered_ports = app.ports.clone();
}
```
New handlers follow the same "mutate App state only" discipline (pure state transitions; async work happens in main.rs intercepts — see ElevateRequest at update.rs:210-214, which is a no-op here and handled in the event loop):
```rust
Message::ElevateRequest => {
    // Handled by the main event loop (triggers spawn_blocking).
    // The update function just records the intent.
}
```
Same comment idiom for `Message::Kill` / `Message::DetailRequest` (intercept-owned). `KillOutcome` handler sets `app.kill_message`, clears `kill_in_flight`, triggers one `app.scanning = true` for the post-kill auto-refresh (D-04). `ScanComplete` handler (lines 28-51) gains cache invalidation (signature cache + detail cache per D-07/D-08) and `ProcessExited` detection while detail panel is open (compare `pid` presence in new list).

---

### `port-tui/src/components.rs` (modify — registration)

**Analog:** itself (37 lines). Add `pub mod detail_panel; pub mod kill_confirm; pub mod whitelist_overlay;` (lines 9-15) + `pub use` re-exports (lines 17-23). The `Component` trait (lines 29-37) is unchanged — new components implement it identically (stateless renderers, all state in `App`).

---

### `port-tui/src/components/detail_panel.rs` (new — 12-row Clear-over overlay)

**Analog:** `filter_panel.rs` (198 lines) — EXACT: multi-row Clear-over overlay, `bg_overlay` styling, row-per-field rendering.

**Row-layout + Clear pattern** (filter_panel.rs lines 25-41):
```rust
let rows = Layout::vertical([
    Constraint::Length(1), // header
    Constraint::Length(1), // port range
    ...
]).split(area);

for row_area in rows.iter() {
    f.render_widget(Clear, *row_area);
}
let base = Style::default().bg(theme.bg_overlay);
```
Detail panel = 12 rows (UI-SPEC detail panel internal layout): title (Bold `fg_emphasis` + protection badge), 9 field rows (label `fg_muted`, value `fg_default`, unavailable `—` dim `fg_muted`), hint row (`accent_secondary`), border bottom. `accent_secondary` used ONLY for kill-action hints (UI-SPEC accent split).

**Labeled field row pattern** (filter_panel.rs lines 114-151 — `render_field_row` with muted label + focus styles) — adapt: 17-char fixed label column, `Layout::horizontal([Constraint::Length(17), Constraint::Min(0)])`.

**States to render** (UI-SPEC detail panel states): no selection copy, `Loading details…`, `Verifying…` (signature), Signed/Unsigned/Unknown, strikethrough + `Exited` on `ProcessExited` (Modifier::CROSSED_OUT — ratatui 0.30 SGR 9; UI-SPEC Typography delta).

**Truncation:** command line right-truncates `…`; path keeps right segment (`…\dir\name.exe`) — reuse `truncate()` from ports.rs lines 385-392 as the shared helper (extract or copy).

---

### `port-tui/src/components/kill_confirm.rs` (new — centered 60×7 popup)

**Analog:** `filter_panel.rs` (Clear + rows + Paragraph) + `ports.rs` (bordered `Block` pattern — the only `Borders` usage in the codebase):

**Bordered block pattern** (ports.rs lines 38-40 — borders are NONE there; use `Borders::ALL` for the dialog):
```rust
let block = Block::default()
    .borders(Borders::NONE)
    .style(Style::default().bg(theme.bg_base));
```
For the popup: `Block::default().borders(Borders::ALL).style(Style::default().bg(theme.bg_overlay))` + title (ratatui `Block::title`). The area arrives pre-centered from main.rs (UI-SPEC geometry `x=(w-60)/2, y=content.y+(h-7)/2`). Internal layout: 3 rows — title, `{name} (PID {pid})`, plain-language reason, button row `[y] Confirm kill` (accent_secondary, underlined per UI-SPEC) · `[n] Cancel` (muted). `{name}` truncates `…` to `term_width − 63`.

---

### `port-tui/src/components/whitelist_overlay.rs` (new — 20-row overlay, 2 lists + input)

**Analog:** `filter_panel.rs` (multi-row Clear-over + focus + input-buffer display), `search.rs` (block-cursor input row), `ports.rs` (scrollbar helper).

**Cursor input row pattern** (search.rs lines 55-73) — reuse verbatim for the `Path: >_` input (row 17):
```rust
if query.is_empty() {
    spans.push(Span::styled("type to search...", muted_style));
} else {
    for (i, ch) in query.char_indices() {
        if i == cursor_pos {
            spans.push(Span::styled("\u{2588}", text_style));  // block cursor
        }
        spans.push(Span::styled(ch.to_string(), text_style));
    }
    if cursor_pos >= query.len() {
        spans.push(Span::styled("\u{2588}", text_style));
    }
}
```

**Scrollbar helper** (ports.rs lines 395-435 `render_scrollbar` — `│` track / `█` thumb) — copy for both lists (UI-SPEC whitelist overlay overflow rule). Note: no `List` widget usage exists in the codebase (grep verified — ports/overview use `Table`); the two `List` widgets here are the first usage. `ratatui::widgets::List` + `ListItem` is a core widget (already compiled); state/selection are driven from `App` (`whitelist_selected`, `whitelist_focus`) per the stateless-Component discipline. `List` selection highlighting via `ListState` is NOT available (stateless) — use `List::new(items).highlight_style(...)` with the selected index passed from `App`, same as the table's `is_selected` logic (ports.rs lines 183-192).

**Row layout** (UI-SPEC whitelist overlay internal layout, 20 rows): title `Length(1)`, built-in label `Length(1)`, built-in list `Length(9)`, user label `Length(1)`, user list `Min(5)`, input `Length(1)`, hint `Length(1)`. Row format: built-in `◆ {basename}  {short reason}` (◆ error color for built-in, warning for user entries), user `→ {full path}` (truncate `…`). Empty user list copy per UI-SPEC.

---

### `port-tui/src/components/ports.rs` (modify — ◆ marker, dimming, strikethrough)

**Analog:** itself (435 lines). Three edit zones:

**1. Process cell** (lines 196-266) — prepend `◆` marker: built-in → `status.error` color, user → `status.warning` (UI-SPEC Protection Semantics). The marker comes from the model (`is_system_critical` / `user_protected`, populated by the scan post-pass — render stays O(1), no per-row lookups).

**2. Dimming heuristic superseded** (lines 361-382) — DELETE `SYSTEM_NAMES` const + `is_system_process()`; replace with whitelist membership: dim protected rows only when non-admin:
```rust
let protected = conn.process.is_system_critical || conn.process.user_protected;
let system_dim = !app.is_admin && protected;
```

**3. Exited strikethrough** — add `Modifier::CROSSED_OUT` to the process cell style when the row's process exited (D-04 post-kill state, pending scan removal). `truncate()` (lines 385-392) stays and may be shared with the new components.

**No change:** sort (update.rs owns it), scrollbar, header cells, state/protocol cells.

---

### `port-tui/src/theme.rs` — NO CHANGE

`accent_secondary` already exists (line 22, `Color::Rgb(198, 120, 221)`). Phase 2 only assigns it (kill-action highlights per UI-SPEC accent split).

---

## Shared Patterns

### 1. spawn_blocking discipline (all Win32 in port-core + TUI intercepts)
**Source:** `port-core/src/scanner/tcp.rs:345-402` (async fn wrapping spawn_blocking); `port-tui/src/main.rs:158-174` (event-loop intercept)
**Apply to:** `process/handle.rs`, `process/info.rs`, `process/kill.rs`, `scanner.rs` post-pass, `main.rs` Kill/detail/signature intercepts
```rust
tokio::task::spawn_blocking(move || { /* Win32 only */ })
    .await
    .map_err(|e| crate::Error::Platform(format!("spawn_blocking failed: {}", e)))?
```
Rule (PITFALLS #9, #11): HANDLE never crosses the mpsc channel (`!Send`); `ProcessSnapshot` (pure data) is the channel payload. Every TerminateProcess/PostMessageW result must be mapped, never swallowed.

### 2. Error mapping to `crate::Error`
**Source:** `port-core/src/lib.rs:8-18`
**Apply to:** all new core modules
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("platform error: {0}")] Platform(String),
    #[error("not found: {0}")] NotFound(String),
    #[error("permission denied: {0}")] PermissionDenied(String),
    #[error("io error: {0}")] Io(#[from] std::io::Error),
}
```
ERROR_ACCESS_DENIED → `PermissionDenied` → `KillOutcome::AccessDenied` (D-03 message). PID-reuse mismatch → `NotFound` (Pitfall #1).

### 3. Soft overlay pattern (all 3 new components)
**Source:** `port-tui/src/components/search.rs:21-92` and `filter_panel.rs:23-110`
**Apply to:** detail_panel.rs, kill_confirm.rs, whitelist_overlay.rs
```rust
f.render_widget(Clear, row_area);          // overwrite table behind
let base = Style::default().bg(theme.bg_overlay);
```
Stateless renderer (all state in `App`), keyboard-interruptible via overlay dispatch in `map_key_event` (main.rs), pass-through rules per UI-SPEC table.

### 4. TEA message flow (async results → status bar)
**Source:** `port-tui/src/main.rs:156-190` (channel drain) + `port-tui/src/update.rs:16-226` (handlers)
**Apply to:** Kill outcome, detail fetch, signature verify, whitelist save
Result via `tokio::sync::mpsc` (unbounded, main.rs:61) → `Message::Xxx` → `update()` → `app.needs_render = true`. Status bar rendering in `render_status_bar` (main.rs:483-580) — kill outcomes reuse the existing error/info branch structure.

### 5. Serde-default backward compatibility (settings.toml)
**Source:** `port-core/src/config/settings.rs:10-18`
**Apply to:** `AppSettings.whitelist` + `kill_timeout_secs` (D-13/D-02). `#[serde(default)]` on new fields; `default_settings()` updated; old TOML files parse unchanged. Unit-test with a Phase-1-era fixture.

### 6. Inline unit tests for pure logic
**Source:** `port-core/src/filter.rs:123-259`
**Apply to:** whitelist.rs (PROC-03/04/05), kill.rs `route_strategy` (PROC-02), handle.rs FILETIME verify (PROC-07), info.rs conversions (PROC-06), settings.rs serde round-trip (PROC-05). Windows-gated behavior goes to `port-core/tests/` integration files (spawn real child processes per RESEARCH validation map).

---

## No Analog Found

| File | Role | Data Flow | Reason / Guidance |
|------|------|-----------|-------------------|
| `port-core/tests/kill_integration.rs` (N) | test | — | No `tests/` dir exists (only inline `#[cfg(test)]` precedent). Use RESEARCH validation map: spawn `cmd.exe /c ping -t 127.0.0.1` children, exercise WM_CLOSE/Ctrl+C/force; Windows-gated. Structure: standard `#[cfg(test)] mod tests` per filter.rs precedent, but as an integration crate (no `#[cfg(test)]` needed — top-level fns). |
| `port-core/tests/process_handle_integration.rs` (N) | test | — | Same as above; spawn/kill churn (10 iterations) asserting no wrong-process kill (PROC-07). |
| Help overlay (`port-tui/src/components/help.rs` — referenced by UI-SPEC §Help Overlay) | component | render | **Does not exist in the codebase** — grep verified: no `?` key binding in `map_key_event` (main.rs:218-317), no Help component registered in components.rs. UI-SPEC says "Help overlay gains [d] Detail, [x] Kill, [w] Whitelist" as if it exists. Planner decision needed: (a) create a minimal Help overlay in Phase 2 (it is the canonical reference for footer-dropped `s`/`w` keys per UI-SPEC Footer note), or (b) document as a Phase 1 gap and defer. |

---

## Metadata

**Analog search scope:** `port-core/src/**/*.rs` (22 files), `port-tui/src/**/*.rs` (17 files), workspace + crate Cargo.toml; grep-verified `List` widget usage, Help overlay existence, `ProcessInfo` constructor sites
**Files scanned:** 14 read fully (process.rs, models/process.rs, lib.rs, config/settings.rs, scanner.rs, scanner/resolver.rs, filter.rs, main.rs, app.rs, message.rs, update.rs, elevate.rs, theme.rs, components.rs, components/search.rs, components/filter_panel.rs, components/ports.rs, scanner/tcp.rs 2 ranges, models/connection.rs) + targeted greps
**Pattern extraction date:** 2026-07-31

**Cross-file dependencies the planner must sequence:**
1. `models/process.rs` `user_protected` field first — breaks 5 constructor sites (filter.rs:136, tcp.rs:282/321, udp.rs:146/177) until all updated.
2. `config/settings.rs` fields before `whitelist.rs`/`kill.rs`/`scanner.rs` compile.
3. `message.rs` enum before `update.rs`/`main.rs` handlers.
4. `process.rs` parent module + `process/` submodules before any `port_core::process::*` imports in TUI.
5. Workspace Cargo.toml feature flags before any WinTrust/Console import.

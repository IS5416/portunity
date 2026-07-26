# Feature Research

**Domain:** Windows Port Management Tool
**Researched:** 2026-07-26
**Confidence:** MEDIUM (cross-referenced across 6+ existing tools, web search corroboration, but no primary user interviews)

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete. Every tool from `netstat` to `portio` has these.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Real-time port listing** — display all active TCP/UDP ports with owning process | The entire point of a port manager. `netstat -ano` is the baseline every developer already uses. | LOW (IP Helper API) | `GetExtendedTcpTable`/`GetExtendedUdpTable` provides this. Must show protocol, local address:port, remote address:port, state, PID, process name. |
| **Port-to-process mapping** — show which executable owns each port | Users need to answer "what's on port 3000?" instantly. TCPView, CurrPorts, Resource Monitor all do this. | LOW | PID from Windows API, process name via `CreateToolhelp32Snapshot`. Must show full executable path; just `node.exe` is not enough. |
| **Search/filter** — filter by port number, process name, PID, protocol, state | TCPView lacks filtering entirely — that is its single biggest user complaint and why many switched to CurrPorts. | MEDIUM | Must support substring and exact match. CurrPorts-style filter syntax (`include:process:firefox.exe`) is table-stakes-level at this point. |
| **Process termination** — kill the process owning a port | "Port in use" is the #1 trigger for opening a port tool. Killing the offending process is the expected action. | LOW | `TerminateProcess` API. Must validate PID is still current before acting (PID reuse hazard). |
| **Protocol and state display** — TCP/UDP, LISTENING/ESTABLISHED/TIME_WAIT | Needed to distinguish a server that's actively listening from a stale connection in TIME_WAIT. | LOW | Direct from Windows socket table. Color-coding connection state (like TCPView's green/yellow/red) is table stakes for visual tools. |
| **Sortable columns** — click header to sort by port, process, state | UX baseline for any table-based tool. Every GUI/TUI port tool supports this. | LOW | Sortable by numeric (port, PID) and lexicographic (process name, state). |
| **Admin elevation** — detect and request admin rights when needed | `TerminateProcess` on system-owned processes requires admin. `netstat -b` requires admin. Auto-detection + UAC prompt is the expected modern pattern. | MEDIUM | Must auto-detect when elevation is needed, not require "always run as admin." Show non-owned processes read-only until elevated. |
| **Copy/export data** — copy selected rows or export to clipboard/file | Users need to share findings ("port 5432 is held by PID 8840, here's the output"). TCPView and CurrPorts both support copy/export. | LOW | Clipboard copy as tab-delimited text is minimum. |
| **Auto-refresh** — periodically refresh the port list | Ports change constantly. A static snapshot is useless after 5 seconds. TCPView default is 1s; CurrPorts 2-10s configurable. | LOW | Configurable interval. ETW-driven pushes interval toward "as events happen"; 2s polling is the standard fallback across all tools. |

### Differentiators (Competitive Advantage)

Features that set Portunity apart. None of the existing tools (TCPView, CurrPorts, Resource Monitor, portio, Porter) combine more than 2-3 of these.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Auto-labeled ports** — recognize common dev ports and show human-readable labels ("Vite Dev Server" for 5173, "PostgreSQL" for 5432) | No existing tool does this. Developers memorize port numbers; novices don't know them. Labeling bridges this gap and makes the tool instantly useful to non-experts. | LOW (lookup table) | Static mapping for ~50 common ports. Extensible via user settings. Does not require heuristics or active detection. |
| **Custom labels** — user-assignable names to ports ("my production DB tunnel", "staging API") | Makes the port list personal and meaningful. No competitor offers this. Users remember their own labels. | LOW | Store in SQLite. Display alongside or in place of the port number. Searchable. |
| **Combined/faceted search** — search across port, process name, PID, protocol, state, labels, favorites simultaneously with AND/OR logic | CurrPorts has filter syntax but no faceted UI. TCPView has zero filtering. Faceted search lets users narrow results progressively (e.g., "TCP + LISTENING + port 3000-9000 + node.exe"). | HIGH | Requires a query model that can combine dimensions. SQLite-backed with FTS5 for text dimensions, indexed columns for numeric ranges. |
| **Smart kill** — graceful termination (WM_CLOSE → wait → TerminateProcess) with configurable timeout | Nearly every tool force-kills (`TerminateProcess` with no warning). Graceful shutdown lets servers flush buffers, close connections cleanly, and delete PID files. | MEDIUM | Requires process cooperation (WM_CLOSE for GUI processes, `GenerateConsoleCtrlEvent` for console processes). Fallback to `TerminateProcess` after timeout. |
| **Instant kill as default, whitelist confirmation for protected processes** | Users want speed (one click to kill their own dev server), but they want safety (don't accidentally kill `svchost.exe`). Smart defaults: fast path for user-owned processes, confirmation gate for system processes. | MEDIUM | Whitelist includes ~30 system-critical processes (smss.exe, csrss.exe, wininit.exe, services.exe, lsass.exe, svchost.exe, winlogon.exe, System, Idle, etc.). User-customizable in settings. |
| **Port occupation change history** — timeline of when ports were opened/closed, by which process, with timestamps | No existing tool has persistent port history. CurrPorts has basic change logging but no queryable timeline. Valuable for: "when did that dev server crash and release port 3000?", "what process briefly held port 8080 at 14:32?" | HIGH | SQLite-backed with append-only log. Requires polling or ETW-driven event capture. Table: `port_events(timestamp, event_type, local_addr, remote_addr, pid, process_name)`. Prune old entries to bound storage. |
| **Network traffic statistics** — bytes sent/received per port and per process | ProcessTCPSummary and NetLimiter provide this, but no dedicated port manager does. Developers want to see if a connection is actually transferring data or just idle. | HIGH | Requires ETW or IP Helper API counter reads. Per-process stats via `GetPerTcpConnectionEStats`. Not real-time packet capture — aggregate counters refreshed periodically. |
| **Windows Firewall rule management** — list, create, delete, enable/disable rules from within the port tool | Only ConnectionMgr does this, and it's a niche GitHub project. The alternative workflow is "find the port → open Windows Defender Firewall → Advanced Settings → manually create rule." Collapsing this into right-click → "Block this port" is a massive workflow improvement. | HIGH | Uses Windows Filtering Platform (WFP) or `netsh advfirewall` COM API. Rule CRUD with immediate effect. Show only user-created rules by default (not the 200+ default rules). |
| **Quick block/allow actions** — right-click any port or process → "Block in Firewall" or "Allow in Firewall" | No existing port tool integrates firewall actions this way. It turns firewall management from a separate admin task into a natural extension of port discovery. | MEDIUM | Depends on firewall rule management feature. Needs admin elevation. Should show confirmation dialog with rule details before committing. |
| **Favorites/bookmarks** — save commonly monitored ports for quick access | Few port tools have this. Developers always check the same ports (3000, 8080, 5432). A saved list eliminates repeated search. | LOW | Store in SQLite. Display as a sidebar section or filter preset. Toolbar quick-jump buttons. |
| **Process detail panel** — executable path, start time, command line args, digital signature status | Most tools show only process name + PID. Full process context answers: "is this my instance or someone else's?", "is this a signed binary or malware?", "what command line flags is it running with?" | MEDIUM | `GetProcessTimes`, `QueryFullProcessImageName`, `WinVerifyTrust`, reading PEB for command line. Side panel that opens on click/select. |
| **ETW event-driven refresh** — Windows Event Tracing for connection change events, 2s polling fallback | Every other tool either polls (CPU burn when idle) or has a fixed refresh rate (misses rapid changes). ETW pushes events only when connections change — near-zero CPU when idle, instant when active. | HIGH | Subscribe to `Microsoft-Windows-TCPIP` ETW provider. Fallback to polling catches UDP and edge cases ETW doesn't cover. |
| **Export as JSON/CSV** — structured export with all fields | TCPView only exports text dumps. CurrPorts has HTML/XML. JSON/CSV is what developers actually want for scripting, log analysis, and CI integration. | LOW | Leverage serde serialization. CSV for spreadsheets; JSON for programmatic consumption. |
| **Dual interface — TUI + GUI from shared core** | No existing port manager offers both a terminal TUI (for power users and SSH scenarios) and a desktop GUI (for visual exploration and system tray integration). This is a structural differentiator, not just a feature. | HIGH | Shared `port-core` Rust crate. TUI (Ratatui) and GUI (Tauri + Svelte) are thin presentation layers over the same logic. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems. Documented to prevent scope creep.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Active TCP port scanning** (connect-scan range probing like nmap) | "I want to see if port X is open on remote host Y." | This is a security/network reconnaissance tool, not a port manager. It triggers IDS alarms, can be illegal against hosts you don't own, adds massive complexity (raw sockets, SYN crafting, timing). Port scanning is a fundamentally different product category. | Keep a `PortScanner` trait as an extension point. Document that scanning is out of scope for v1. Users who need this should use `nmap` or `rustscan`. |
| **Always-on-top real-time network throughput graph** (live bandwidth chart) | "I want to see traffic spikes in real time." | High CPU/GPU overhead for continuous rendering. Adds visual noise that distracts from the core port listing. Real-time graphs are a dashboard feature, not a port management feature. | Provide traffic *statistics* (aggregate bytes sent/received, refreshed every few seconds). If graphing is desired, make it a collapsible panel, not the default view. |
| **Automatic process killing** without any user action ("watch mode" that kills anything that binds to a watched port) | "I don't want to manually kill processes. Just auto-kill anything that takes my port." | Extremely dangerous on shared machines. A colleague starts a server on port 3000 — auto-killed with no warning. A legitimate system service binds to a port in the watched range — auto-killed. Users will blame the tool. | Provide a "Watch" feature that *notifies* (toast, tray alert) when a watched port is occupied. Let the user decide to kill. portzap's watch mode (`portzap watch 3000`) is the dangerous pattern to avoid. |
| **Raw packet capture / deep packet inspection** (Wireshark-like) | "I want to see what data is being sent over the connection." | Requires kernel-level packet capture (WinPcap/Npcap driver installation), adds driver signing complexity, and is a fundamentally different product (packet analyzer, not port manager). Massive scope expansion. | Show connection metadata (protocol, state, process, traffic counters). For packet inspection, users should use Wireshark. |
| **"Kill all" button** without whitelist protection | "I want to free all ports at once." | Will terminate system-critical processes (svchost.exe hosts dozens of Windows services). Blue screen or system instability risk. Even with a whitelist, "kill all non-system processes" can nuke the user's other work (IDE, database, browser). | No "kill all" button. Provide batch actions with explicit selection: user must check boxes next to ports they want to act on. Whitelist gating always applies. |
| **Built-in HTTP server** for remote access ("manage ports from a web browser") | "I want to check ports from my phone." | Adds an entire web server with authentication, TLS, CORS, and remote attack surface to a local system tool. Port management requires admin privileges — exposing that over a network-accessible interface is a security nightmare. | Keep port management local. The TUI works over SSH natively. If remote access is needed in the future, use a dedicated API with strong auth — not a bundled HTTP server. |
| **Service management** (start/stop/restart Windows services) | "If I see a service on a port, I want to restart it." | Services have complex dependency trees. Restarting a service can cascade to other services (restarting RPC causes dozens of dependent services to restart). A port manager is not a service manager — `services.msc` exists for that. | Show the service name associated with a process (if any) as informational context. Do not offer service control actions. |
| **DNS/WHOIS on every remote connection** by default | "I want to see where every connection goes." | DNS reverse lookups are slow (blocking). WHOIS queries hit rate limits. Doing this automatically on every connection creates a 2-5 second delay before the port list appears and may look like malware beaconing to network monitoring tools. | Provide DNS resolution and WHOIS as opt-in actions (right-click → "Resolve Hostname" or "WHOIS Lookup"), not automatic defaults. TCPView already demonstrates this pattern correctly. |

## Feature Dependencies

```
[Quick block/allow actions]
    └──requires──> [Firewall rule management]
                       └──requires──> [Admin elevation]

[Combined/faceted search]
    └──enhances──> [Port listing]
    └──requires──> [SQLite FTS5 for text search]

[Smart kill]
    └──requires──> [Process termination]
    └──enhances──> [Whitelist confirmation]

[Whitelist confirmation]
    └──requires──> [Process termination]
    └──requires──> [Process detail panel] (need full path to match whitelist entries)

[Port occupation history]
    └──requires──> [Port listing]
    └──requires──> [ETW event-driven refresh] (for accurate timestamps)

[Traffic statistics]
    └──requires──> [Port listing]

[Custom labels]
    └──enhances──> [Port listing]
    └──enhances──> [Search/filter]

[Favorites/bookmarks]
    └──enhances──> [Port listing]
    └──enhances──> [Search/filter]

[ETW event-driven refresh]
    └──requires──> [Port listing]
    └──conflicts with──> [High-frequency polling] (choose one as primary, use other as fallback)

[Dual interface (TUI + GUI)]
    └──requires──> [Shared core crate (port-core)]
    └──is_orthogonal_to──> [All other features] (presentation layer concern)
```

### Dependency Notes

- **Firewall rule management blocks quick actions:** Firewall CRUD must be implemented before right-click block/allow makes sense. Both require admin elevation.
- **Faceted search requires SQLite/FTS5:** Multi-dimensional search with text + numeric predicates needs a query engine. A simple in-memory `Vec` filter chain works for basic filter but doesn't scale to faceted AND/OR logic across dimensions.
- **Smart kill requires process termination to already work:** The smart kill strategy (graceful→force) is a policy layer on top of basic `TerminateProcess`. Build the basic kill first, then add the escalation logic.
- **Whitelist confirmation requires process detail:** To check if a process is whitelisted, the tool must resolve the full executable path. A PID alone is not enough because the same PID can be reused.
- **ETW and polling are complementary, not conflicting:** ETW is the primary path for TCP connection events. Polling (2s) catches UDP and edge cases. They coexist — ETW drives, polling verifies.
- **Dual interface is orthogonal to features:** The shared core crate means every backend feature works identically in both TUI and GUI. The interface choice affects presentation (table layout, keyboard vs mouse) but not capability.

## MVP Definition

### Launch With (v1)

Per PROJECT.md: "All v1+v2+v3 features in one milestone — user wants full-featured tool from day one." This is not a traditional MVP; it's a full-featured v1 launch. However, within the single milestone, features should be organized into waves by dependency:

**Wave 1 — Core Engine (must be first):**
- [x] Real-time port listing with process mapping
- [x] Protocol and state display with color coding
- [x] Sortable columns
- [x] Admin elevation
- [x] Copy/export data (clipboard)
- [x] Auto-refresh (polling-based initially)

**Wave 2 — Essential Actions (depends on Wave 1):**
- [x] Process termination (basic kill)
- [x] Smart kill (graceful escalation)
- [x] Whitelist confirmation
- [x] Search/filter (basic)
- [x] Process detail panel

**Wave 3 — Intelligence Layer (depends on Wave 1+2):**
- [x] Auto-labeled ports
- [x] Custom labels
- [x] Favorites/bookmarks
- [x] Combined/faceted search
- [x] Export as JSON/CSV

**Wave 4 — Advanced Capabilities (depends on Wave 1+2, some on Wave 3):**
- [x] Port occupation change history (SQLite)
- [x] Network traffic statistics
- [x] ETW event-driven refresh
- [x] Firewall rule management
- [x] Quick block/allow actions

**Wave 5 — Dual Interface (parallelizable after Wave 1 core is stable):**
- [x] Shared core crate (port-core)
- [x] TUI (Ratatui)
- [x] GUI (Tauri + Svelte)

### Future Consideration (v2+)

These are intentionally deferred — the PROJECT.md marks them as out of scope:

- [ ] Linux/macOS support — platform abstraction layer in place, implementations deferred
- [ ] Active TCP port scanning — `PortScanner` trait extension point reserved
- [ ] Mobile support — not applicable

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Real-time port listing | HIGH | LOW | P1 |
| Port-to-process mapping | HIGH | LOW | P1 |
| Protocol and state display | HIGH | LOW | P1 |
| Sortable columns | MEDIUM | LOW | P1 |
| Admin elevation | HIGH | MEDIUM | P1 |
| Copy/export data | MEDIUM | LOW | P1 |
| Auto-refresh | HIGH | LOW | P1 |
| Process termination | HIGH | LOW | P1 |
| Search/filter (basic) | HIGH | MEDIUM | P1 |
| Smart kill | HIGH | MEDIUM | P2 |
| Whitelist confirmation | HIGH | MEDIUM | P2 |
| Process detail panel | MEDIUM | MEDIUM | P2 |
| Auto-labeled ports | HIGH | LOW | P2 |
| Custom labels | MEDIUM | LOW | P2 |
| Favorites/bookmarks | MEDIUM | LOW | P2 |
| Export as JSON/CSV | MEDIUM | LOW | P2 |
| Combined/faceted search | HIGH | HIGH | P3 |
| Port occupation history | MEDIUM | HIGH | P3 |
| Network traffic statistics | MEDIUM | HIGH | P3 |
| ETW event-driven refresh | MEDIUM | HIGH | P3 |
| Firewall rule management | HIGH | HIGH | P3 |
| Quick block/allow actions | HIGH | MEDIUM | P3 |
| Dual interface (TUI + GUI) | HIGH | HIGH | P3 |

## Competitor Feature Analysis

| Feature | TCPView | CurrPorts | Resource Monitor | portio | Porter | Portunity (Target) |
|---------|---------|-----------|-----------------|--------|--------|-------------------|
| Port listing | YES | YES | YES | YES | YES | YES |
| Process mapping | name+PID | name+PID | name+PID | name+PID | name+PID | name+PID+path+signature |
| Color-coded states | YES (5 colors) | YES (pink/green) | NO | NO | YES (3 colors) | YES (TCPView-level) |
| Search/filter | NO | YES (syntax) | NO (manual scan) | YES (basic) | YES (basic) | YES (faceted) |
| Process kill | YES | YES | YES | YES | YES | YES (smart kill) |
| Connection close | YES | YES | NO | NO | NO | NO (not needed — kill process instead) |
| Admin elevation | manual | auto-detect | N/A (built-in) | auto (admin fallback) | YES (detect+warn) | auto-detect + UAC prompt |
| Export | basic text | HTML/XML/TXT | NO | JSON mode | NO | JSON + CSV |
| History/logging | NO | change log | NO | NO | NO | SQLite timeline |
| Traffic stats | NO | NO | NO | NO | NO | per-port + per-process |
| Firewall rules | NO | NO | shows status only | NO | NO | CRUD + quick actions |
| Port labels | NO | NO | NO | NO | NO | auto + custom |
| Favorites | NO | NO | NO | NO | NO | YES |
| System tray | NO | YES | NO | NO | NO | YES (GUI only) |
| TUI | NO (CLI: tcpvcon) | NO | NO | YES | NO | YES (Ratatui) |
| Desktop GUI | YES | YES | YES | NO | YES | YES (Tauri+Svelte) |
| Cross-platform | NO | NO | NO | NO | YES | extension points only |

## Key Insights from Competitor Analysis

1. **Filtering is the single biggest gap in the market.** TCPView's complete lack of filtering is the #1 reason users switch to CurrPorts. Portunity's faceted search directly addresses this.
2. **No tool combines port management with firewall management.** The workflow "find port → block it" currently crosses tool boundaries (TCPView → Windows Firewall MMC). Portunity can own this end-to-end.
3. **History and labeling are unserved needs.** No existing tool helps users remember "what was on port 3000 yesterday" or "which of these 5 node.exe processes is my API server." Custom labels + change history solve this.
4. **Traffic statistics are in a separate tool category.** ProcessTCPSummary and NetLimiter own traffic stats but show no port context. Portunity can bridge this by showing bytes alongside the port listing.
5. **The kill experience is universally bad.** Most tools force-kill. Some lack confirmation. None offer graceful escalation. Smart kill + whitelist is a genuine UX differentiator.

## Sources

- Microsoft TCPView Documentation (learn.microsoft.com/sysinternals/downloads/tcpview) — Primary source for TCPView features
- NirSoft CurrPorts Documentation (documentation.help/CurrPorts/cports.html) — Primary source for CurrPorts features
- SaaSHub CurrPorts vs TCPView comparison — Community feature comparison
- Windows netstat documentation (learn.microsoft.com) — Primary source for netstat capabilities
- Automata Labs "Ultimate netstat Guide for Windows Devs" — Developer workflow analysis
- Resource Monitor capabilities (windowsforum.com, automatalabs.ca) — Resource Monitor feature analysis
- GitHub repositories: portio, Porter, pttr, portzap, PortPal, PortProcessManager, Harbor Sweep, PortKiller — Modern port tool feature analysis
- ManageEngine OpUtils documentation — Enterprise port management challenges
- Fluke Networks LAN troubleshooting guide — Network monitoring pitfalls
- Netdata "Anti-patterns" documentation — Flow monitoring anti-patterns

---
*Feature research for: Portunity — Windows Port Management Tool*
*Researched: 2026-07-26*

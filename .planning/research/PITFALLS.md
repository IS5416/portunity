# Pitfalls Research

**Domain:** Windows port management tool (Rust, Tauri + Ratatui dual-frontend)
**Researched:** 2026-07-26
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: PID Reuse Race Condition (The TerminateProcess Time Bomb)

**What goes wrong:**
The user selects a process by PID, your tool calls `OpenProcess(PID)` to get a handle, then calls `TerminateProcess`. Between those calls, the original process exits and Windows recycles the PID to a completely unrelated new process. `TerminateProcess` kills the wrong process — potentially a system-critical one.

Unlike Unix, Windows has no zombie state. A PID is invalidated immediately on exit and can be reused within milliseconds. The classic broken pattern: enumerate PIDs via `GetExtendedTcpTable`, close the handle, later call `OpenProcess(PID)` to reacquire.

**Why it happens:**
The IP Helper API returns PIDs, not handles. Developers naturally store the PID and later pass it to `OpenProcess`. This two-step "PID → later OpenProcess" pattern is the root cause. The `GetExtendedTcpTable` API gives you a snapshot — by the time you act on it, the world may have changed.

**How to avoid:**
The fundamental rule on Windows: **hold the HANDLE, never re-derive it from a PID.** After `OpenProcess`, retain the `HANDLE` in your process tracking structure alongside the PID. Use the retained handle directly for `TerminateProcess`, `GetExitCodeProcess`, and `WaitForSingleObject`. Only call `CloseHandle` after confirming the process has terminated (via `GetExitCodeProcess` returning `STILL_ACTIVE = false`).

For extra defense, before killing, verify the PID matches via `GetProcessId(handle)` and cross-check the process creation time via `GetProcessTimes` against your snapshot to confirm it is the same process you originally identified.

**Warning signs:**
- Storing raw `u32` PIDs in data structures passed between async tasks
- Calling `OpenProcess` in a different code path from where the PID was obtained
- "Access Denied" errors on TerminateProcess that are actually PID-reuse masking
- Intermittent wrong-process-killed bugs that cannot be reproduced

**Phase to address:**
Phase covering process management (`port-core` process module). Must be designed into the data model from the start — a `ProcessHandle` wrapper struct that bundles PID + HANDLE + creation time, not a bare PID.

---

### Pitfall 2: GetExtendedTcpTable/GetExtendedUdpTable Buffer Allocation Pattern

**What goes wrong:**
The API requires a two-call pattern: first call with `NULL` buffer to get the required size (returns `ERROR_INSUFFICIENT_BUFFER`), then allocate and call again. Getting this wrong produces either an empty result set that looks like "no connections" or a buffer overflow that silently truncates data. Common failure modes: passing `size = 0` on the second call, using a stack buffer that is too small, or not handling the case where the table grows between the two calls.

**Why it happens:**
The documentation buries the two-call requirement in prose. Developers see a function that takes a buffer pointer, pass a fixed-size allocation, and get back `ERROR_INSUFFICIENT_BUFFER` with no rows. Others allocate exactly the returned size on the first call, but the table grows between calls and rows get silently truncated (the function returns `ERROR_SUCCESS` even with partial data; check `dwNumEntries` against expected count).

**How to avoid:**
Always use the two-call pattern with a retry loop. Request the size first, allocate with that size, call again. If the second call also returns `ERROR_INSUFFICIENT_BUFFER`, double the allocation and retry (up to a reasonable bound). After the call succeeds, validate that `dwNumEntries` is non-zero if you expected data. In Rust with the `windows` crate, use `Vec<u8>` for the buffer and cast to the appropriate table struct pointer.

Never assume the table is static — a retry loop is not optional, it is required for correctness under load.

**Warning signs:**
- Single-call pattern with a fixed-size stack buffer
- Not checking return value for `ERROR_INSUFFICIENT_BUFFER`
- Not comparing `dwNumEntries` against expected count
- "No open ports" result when `netstat -ano` shows many

**Phase to address:**
Phase covering `port-core` port enumeration (`port_scanner` module). This is the foundation of the entire tool.

---

### Pitfall 3: IPv4/IPv6 Dual-Stack Enumeration Gap

**What goes wrong:**
The tool shows only IPv4 connections because the developer calls `GetExtendedTcpTable` once with `AF_INET`. On modern Windows (Vista+), IPv6 sockets with `IPV6_V6ONLY` disabled handle both IPv4 and IPv6 traffic, and those connections appear ONLY in the `AF_INET6` table. The IPv4 table will miss dual-stack connections entirely. Users see "port 3000 is free" when in fact a dual-stack socket is listening on it.

Additionally, the `AF_INET6` parameter does NOT support `TCP_TABLE_BASIC_*` table classes — those return `ERROR_NOT_SUPPORTED`. You must use `TCP_TABLE_OWNER_PID_*` or `TCP_TABLE_OWNER_MODULE_*` classes for IPv6, which also give you the PID for free.

**Why it happens:**
The API makes IPv4 the "default" path — it is listed first in documentation, `AF_INET` works with all table classes, and IPv6 feels like a "later" feature. Developers test on IPv4-only setups and never notice the gap.

**How to avoid:**
Always call `GetExtendedTcpTable` twice: once with `AF_INET` + `TCP_TABLE_OWNER_PID_ALL`, and once with `AF_INET6` + `TCP_TABLE_OWNER_PID_ALL`. Merge the results into a unified list. Deduplicate by (protocol, local addr, local port) — an IPv4-mapped IPv6 address `::ffff:192.168.1.1:3000` is the same endpoint as `192.168.1.1:3000`. Use `TCP_TABLE_OWNER_PID_ALL` (value 5) for both calls for consistency.

Same pattern applies to `GetExtendedUdpTable`.

**Warning signs:**
- Only one call to `GetExtendedTcpTable` in the codebase
- `AF_INET` hardcoded without an `AF_INET6` counterpart
- IPv6 connections from `netstat -ano` not appearing in the tool
- Using `TCP_TABLE_BASIC_*` classes at all (they lack PID — useless for a process-aware tool)

**Phase to address:**
Phase covering `port-core` port enumeration. Must be designed into the scanner from day one.

---

### Pitfall 4: Port Number Byte Order Reversal

**What goes wrong:**
`MIB_TCPROW_OWNER_PID.dwLocalPort` and `dwRemotePort` are in **network byte order** (big-endian). If you treat them as native integers without calling `u16::from_be()` (or the C `ntohs()` equivalent), port 80 displays as 20480 and port 443 displays as 47873. Every port number in the tool is wrong.

**Why it happens:**
The field is named `dwLocalPort` (the `dw` prefix conventionally means `DWORD` in Win32), but it is actually a `u16` port number stored in network byte order inside a `u32` field. The type name is misleading and the byte-order requirement is easy to miss in documentation.

**How to avoid:**
Apply `u16::from_be()` to every port field extracted from IP Helper API structures. Wrap this in a dedicated conversion function (e.g., `fn port_from_network(port: u32) -> u16`) and use it everywhere. Add a unit test that verifies port 80 → 80, port 443 → 443, port 8080 → 8080 using known expected values.

**Warning signs:**
- Port numbers in the thousands or tens of thousands for well-known services
- Raw `.dwLocalPort` usage without conversion
- Port column in UI showing nonsensical values

**Phase to address:**
Phase covering `port-core` port enumeration. Caught immediately by unit tests on known port values.

---

### Pitfall 5: ETW Session Orphaning and Resource Exhaustion

**What goes wrong:**
ETW trace sessions are machine-wide kernel objects that **persist beyond process lifetime**. If your tool crashes, is killed from Task Manager, or `Dispose`/`Drop` never runs, the session stays active. Each orphaned session consumes non-paged kernel memory and counts against Windows' limited session quota. After enough crashes during development, the system runs out of ETW sessions and everything fails with cryptic "Insufficient system resources" errors.

**Why it happens:**
Windows does not clean up ETW sessions on process exit. The `StopOnDispose` pattern only works if `Dispose` actually executes. Abrupt termination (kill -9 equivalent, debugger stop, power loss) skips cleanup. The sessions are invisible to normal users — there is no Task Manager tab for them.

**How to avoid:**
Use a fixed, well-known session name (e.g., `Portunity-TCP-Monitor`). On startup, attempt to stop and clean up any pre-existing session with that name before creating a new one (handle the case where it does not exist). This makes crashes self-healing — the next launch cleans up the previous session.

Also: register a Windows console control handler (`SetConsoleCtrlHandler`) to catch `CTRL+C` and `CTRL+BREAK` and explicitly stop the session. In the Tauri GUI, handle the `close_requested` event to stop ETW before exit.

Use `logman query -ets` or `xperf -stop` during development to manually clean up sessions. Add a diagnostic command to the tool that lists and can stop orphaned Portunity sessions.

**Warning signs:**
- `StartTrace` failing with `ERROR_ALREADY_EXISTS` or out-of-resources errors
- Sessions named `Portunity-*` appearing in `logman query -ets` output when the tool is not running
- Development crashes followed by "ETW failed to start" errors on next launch

**Phase to address:**
Phase covering real-time monitoring (ETW integration). Must include startup cleanup logic.

---

### Pitfall 6: ETW Callback Thread Blocking

**What goes wrong:**
The ETW `EventRecordCallback` runs on ETW's internal thread. If the callback performs blocking work — calling `GetExtendedTcpTable`, querying process info via `OpenProcess`, doing synchronous I/O, acquiring a `std::sync::Mutex` — it starves ETW's event processing. Events are dropped. In high-throughput scenarios (busy server with many connections), the tool silently loses events and shows stale data.

**Why it happens:**
The callback pattern in Windows SDK examples looks like a normal function. Developers add "just one more thing" — a process lookup, a timestamp, a database write. Each addition increases the time the callback holds ETW's thread. The event loss is silent — ETW increments an `EventsLost` counter but does not surface it to the callback.

**How to avoid:**
The callback must do exactly one thing: push the raw event data into a lock-free queue (`crossbeam::channel` or `tokio::sync::mpsc` with sufficient capacity) and return. All processing — process lookup, state updates, database writes — happens on the consumer side in the async runtime.

The callback must NOT:
- Call any Win32 API that could block
- Acquire any mutex (use lock-free structures only)
- Perform any allocation beyond the channel push
- Call `println!` or any I/O

**Warning signs:**
- Callback body growing beyond ~20 lines
- Any `Mutex::lock()` or `RwLock::write()` inside the callback
- `EventsLost` counter increasing during load
- Stale data under high connection churn

**Phase to address:**
Phase covering real-time monitoring (ETW integration). Architecture decision: lock-free channel between ETW callback and async runtime.

---

### Pitfall 7: COM Resource Management with Windows Firewall API

**What goes wrong:**
`INetFwPolicy2` is a COM interface. Every call to `get_Rules`, `get__NewEnum`, `Item`, or `QueryInterface` returns a new COM pointer with an incremented reference count. Failing to release these pointers leaks COM objects. In Rust, even with the `windows-rs` crate's RAII wrappers, it is possible to hold onto references too long, call `Clone` excessively, or drop wrappers in the wrong order. In extreme cases, the Windows Firewall service becomes unresponsive because your tool leaked thousands of COM references.

Additionally, write operations (`put_FirewallEnabled`, rule add/remove) require administrator privileges. Without elevation, the COM call returns an `HRESULT` access-denied error that looks like a generic COM failure.

**Why it happens:**
The `windows-rs` crate provides RAII wrappers (`Interface::drop` calls `Release`), which mitigates the worst leaks. But the API surface is large, nested (enumerator → rule → rule properties), and developers working at arm's length from COM do not naturally think about reference counts.

**How to avoid:**
- Always go through `windows-rs` crate types — never raw `*mut c_void` pointers
- Keep COM object lifetimes as short as possible — extract what you need, drop the COM wrapper
- For rule enumeration: collect rule names/properties into a plain Rust `Vec<FirewallRule>`, drop all COM wrappers, then work with the Rust struct
- Always check if the process is elevated before attempting write operations. If not, trigger UAC elevation or return a clear "Administrator privileges required" error
- Run COM operations inside `tokio::task::spawn_blocking` to avoid stalling the async runtime

**Warning signs:**
- Holding COM wrapper types across `.await` points
- Rule list stored as COM wrapper types rather than plain Rust structs
- Access Denied errors with no admin-privilege check
- Growing memory usage during repeated firewall operations

**Phase to address:**
Phase covering firewall management. COM interaction layer should be a thin, well-tested abstraction.

---

### Pitfall 8: `std::sync::Mutex` Across `.await` in Tauri Commands

**What goes wrong:**
Tauri v2 uses Tokio under the hood. A `#[tauri::command]` that holds a `std::sync::MutexGuard` across an `.await` point blocks the Tokio worker thread. The guard is not `Send`, so Tokio cannot move the task to another thread. The result: that worker thread is pinned indefinitely. In the best case, the UI freezes. In the worst case, all worker threads get blocked this way and the entire application deadlocks.

The project shares state between GUI (Tauri) and TUI (Ratatui) via a common `port-core` crate. Both will have their own async contexts accessing shared data like the port snapshot cache, settings, and ETW event buffer.

**Why it happens:**
The Tauri documentation recommends `std::sync::Mutex` for "most cases." This is correct only when the lock is never held across `.await`. The compiler catches some cases (`MutexGuard` is not `Send`), but not all — especially when the guard is held indirectly through a struct method.

**How to avoid:**
Use `tokio::sync::Mutex` for any shared state that is accessed from async Tauri commands or async TUI event loops. The rule: if the lock scope could contain an `.await`, use `tokio::sync::Mutex`. If the lock is only held for synchronous operations (quick read-modify-write), `std::sync::Mutex` is fine and faster.

Pattern: extract the needed data under a short-lived `std::sync::Mutex` lock in a synchronous helper, then pass the owned data to the async portion.

For the shared `port-core` crate: expose both sync and async accessors. The sync accessor uses `std::sync::RwLock` and returns cloned data. The async wrapper uses `tokio::sync::RwLock`.

**Warning signs:**
- `std::sync::Mutex` in a struct accessed from `#[tauri::command]` functions
- Compiler warnings about `Send` bounds on futures
- Intermittent UI freezes that reproduce under load
- `tokio-console` showing worker threads stuck for seconds

**Phase to address:**
Phase covering Tauri GUI integration. Architecture decision for state management pattern.

---

### Pitfall 9: Blocking Win32 API Calls on Async Runtime Threads

**What goes wrong:**
`GetExtendedTcpTable`, `OpenProcess`, `QueryFullProcessImageNameW`, COM calls — these are all synchronous Win32 APIs. Calling them directly inside a `#[tauri::command]` or Tokio task body blocks the Tokio worker thread. Under load (rapid refresh, many ports), the entire async runtime stalls. The UI becomes unresponsive. Other async tasks (ETW event processing, UI updates) queue up but never execute.

**Why it happens:**
The `windows-rs` crate presents these as synchronous function calls. The natural inclination is to call them where they are needed. The `#[tauri::command(async)]` attribute uses `tokio::spawn`, NOT `spawn_blocking` — so marking a command as async does not solve the problem.

**How to avoid:**
Wrap every blocking Win32 API call in `tokio::task::spawn_blocking`. Create a dedicated blocking thread pool (not the default Tokio blocking pool, which is unbounded and can spawn too many threads under contention). Use `tokio::task::spawn_blocking` for one-off calls.

Pattern: the `port-core` crate's public API should be async-first. Internally, it calls `spawn_blocking` around Win32 calls. The frontends only see async functions.

Example for port scanning:
```rust
pub async fn scan_ports() -> Result<Vec<PortEntry>, PortError> {
    tokio::task::spawn_blocking(|| {
        // All IP Helper API calls here
        get_extended_tcp_table(AF_INET, TCP_TABLE_OWNER_PID_ALL)?;
        get_extended_tcp_table(AF_INET6, TCP_TABLE_OWNER_PID_ALL)?;
        // ...process results
    }).await?
}
```

**Warning signs:**
- Any Win32 FFI call outside a `spawn_blocking` block
- `#[tauri::command]` functions that call Win32 APIs directly
- UI stutter during port refresh
- `tokio-console` showing poll times > 1ms in async tasks

**Phase to address:**
All phases interacting with Windows APIs. Design decision: `port-core` exposes only async APIs.

---

### Pitfall 10: Separate Tauri Workspace Causing Double Compilation

**What goes wrong:**
If `src-tauri/` has its own `Cargo.toml` with a `[workspace]` declaration (separate from the root workspace), Cargo treats it as an independent workspace. This creates:
1. A duplicate `target/` directory (`src-tauri/target/` alongside `target/`)
2. Two `Cargo.lock` files
3. Shared crates (like `port-core`) compiled twice with different fingerprints
4. Build times effectively doubled (~2x)
5. Dependency versions potentially diverging between frontends

**Why it happens:**
Historically, Tauri v1 had bugs with workspace detection that made a separate workspace seem necessary. Those bugs were fixed years ago (Tauri 1.x and newer). Old tutorials and templates still show the separate-workspace pattern. Tauri v2 fully supports being a workspace member.

**How to avoid:**
Single Cargo workspace at the project root. `src-tauri/` is a regular workspace member:

```toml
# Root Cargo.toml
[workspace]
members = [
    "crates/port-core",
    "crates/port-tui",     # Ratatui binary
    "src-tauri",            # Tauri app (GUI binary)
]
```

`src-tauri/Cargo.toml` must NOT have its own `[workspace]` section. Remove any `[workspace]` header from the Tauri-side Cargo.toml. Use `workspace = true` for shared dependency declarations.

Also apply release profile optimization from day one:
```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
```

**Warning signs:**
- Two `target/` directories appear after `cargo build`
- Two `Cargo.lock` files in the project tree
- Build time increases super-linearly with each new crate
- Dependency version mismatches between Tauri and TUI binaries

**Phase to address:**
Project scaffolding phase (first phase). Set up correctly before any code is written.

---

### Pitfall 11: Silent Failure When Killing Protected Processes

**What goes wrong:**
Windows protects critical system processes (`services.exe`, `csrss.exe`, `wininit.exe`, `smss.exe`, `lsass.exe`, and others) via Protected Process Light (PPL). `TerminateProcess` on these returns `ERROR_ACCESS_DENIED` even when running as Administrator with `SeDebugPrivilege`. If your tool's "kill" action silently ignores this error, the user sees no feedback — they click kill, nothing happens, they do not know why. If the tool reports "killed successfully" without checking the return code, it lies to the user.

**Why it happens:**
The developer tests killing their own test processes (Node.js servers, Python scripts) and it always works. They never encounter protected processes until a user tries to kill something from `services.exe` that is holding a port. The `ERROR_ACCESS_DENIED` from `TerminateProcess` looks like a generic permissions error, not the specific "this process is protected" condition it actually is.

**How to avoid:**
Build a **system-critical process whitelist** that ships with the tool. This is distinct from the user-customizable whitelist. The shipped whitelist contains known protected processes (`services.exe`, `csrss.exe`, `wininit.exe`, `smss.exe`, `lsass.exe`, `winlogon.exe`, `System`, `svchost.exe`) and is checked BEFORE calling `TerminateProcess`. If the target is on the list, show a clear, non-technical message: "This is a Windows system process and cannot be terminated. Terminating it would crash your system."

For processes not on the whitelist that return `ERROR_ACCESS_DENIED`:
1. Check if the process requires admin rights to kill (try with elevation)
2. If already elevated and still denied, it may be a PPL process — log and inform the user
3. Never silently swallow the error

The user-customizable whitelist extends this — users can add their own databases, Docker Desktop, or other services they want to protect from accidental termination.

**Warning signs:**
- `TerminateProcess` return value not checked
- "Kill" action with no error feedback path
- Test suite that never tests against protected processes
- User reports: "Kill button does nothing sometimes"

**Phase to address:**
Phase covering process management. Must include the shipped whitelist and error handling for all `TerminateProcess` outcomes.

---

### Pitfall 12: SQLite WAL Mode Omission Causing Concurrent Access Failures

**What goes wrong:**
The tool has two frontends (Tauri GUI and Ratatui TUI) that may both run simultaneously, both opening connections to the same SQLite database file for port history, favorites, labels, and settings. Without WAL mode enabled, SQLite uses the default delete-mode journal, which serializes all access. One connection's write blocks all other connections' reads. Under concurrent use, you get `SQLITE_BUSY` errors, slow queries, and potential data corruption if connections are not properly synchronized.

**Why it happens:**
SQLite's default journal mode is "delete" (rollback journal), which does not support concurrent readers during a write. WAL mode must be explicitly enabled with `PRAGMA journal_mode=WAL;`. Many Rust SQLite examples skip this because single-connection scenarios work fine without it.

**How to avoid:**
Execute `PRAGMA journal_mode=WAL;` as the first command after opening every database connection. Also set a reasonable `busy_timeout` (e.g., 5000ms): `PRAGMA busy_timeout=5000;`. These are connection-level pragmas and must run on each new connection.

For the dual-frontend scenario, consider:
- One writer connection (serialized through `tokio::sync::Mutex`) for all inserts/updates
- Reader pool for queries, with connections set to read-only via `PRAGMA query_only=ON;`
- All connections use the same database file path, opened with `SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_NO_MUTEX` flags

Do NOT mix different SQLite wrappers on the same database file (e.g., `rusqlite` in Rust code and `better-sqlite3` in a Node.js sidecar). Different wrappers manage WAL state differently and can corrupt the database.

**Warning signs:**
- `SQLITE_BUSY` errors in logs
- Queries hanging for seconds
- "database is locked" messages
- Only testing with one frontend at a time

**Phase to address:**
Phase covering storage/persistence (`port-core` database module). WAL mode must be enabled on first connection creation.

---

### Pitfall 13: Ratatui Large Table Rendering Degradation

**What goes wrong:**
A system with many open ports (e.g., a busy server with hundreds of connections) produces a table with thousands of rows. Ratatui renders the entire frame on every `draw()` call, computes diffs for every cell, and calls `unicode_width::width()` per cell. At ~15,000 rows, scrolling becomes a 1-2 second lag per keystroke. The TUI becomes unusable precisely when the user needs it most (debugging a busy server).

**Why it happens:**
Ratatui's architecture is full-frame redraw with cell-level diffing. The `Table` widget converts its entire dataset to `Vec<Cell>` on every render, even for rows outside the visible viewport. The `unicode_width` computation during diffing is CPU-intensive at scale.

**How to avoid:**
Use **virtualization**. Only compute and render rows visible in the current viewport (typically 20-40 rows depending on terminal size). When the user scrolls, only compute the newly visible rows. The `ratatui-layout` crate provides `VirtualTable` and `VirtualList` for this purpose.

Additionally:
- Only trigger re-render on actual state changes (user input, ETW event arrival), not on a fixed timer
- For the port table specifically: maintain a sorted, indexed data structure in `port-core` that supports efficient range queries (visible window)
- Cache cell widths where possible — port numbers and IP addresses have predictable widths
- Use `StatefulWidget` pattern with a state that tracks the scroll offset and selection

**Warning signs:**
- `draw()` called on a tick timer rather than event-driven
- `Table::new()` receiving the entire port list every frame
- Scrolling lag perceptible at ~1000+ rows
- CPU fan spinning during TUI usage

**Phase to address:**
Phase covering TUI frontend (Ratatui). Virtualization must be in the initial TUI design.

---

### Pitfall 14: Windows-rs Allocator Mismatch

**What goes wrong:**
Various Windows APIs allocate memory with different allocators: `CoTaskMemAlloc` for COM, `HeapAlloc` for the process heap, `LocalAlloc` for local heap, and so on. Each allocator requires its matching deallocation function. If you wrap a Windows-returned buffer in a Rust type and call `Drop` with the wrong deallocator, you get a heap corruption crash — often a silent process termination with no useful error message.

Concrete example for this project: `GetExtendedTcpTable` allocates with `HeapAlloc` (process heap). But `ConvertInterfaceLuidToNameW` for network interface names may use a different allocator. Mixing them causes corruption.

**Why it happens:**
The `windows-rs` crate generates RAII wrappers, but they are generic and the deallocation function is selected at the binding level. If the binding metadata is wrong (which happens — it is machine-generated from Win32 metadata), the wrong deallocator is called. Additionally, manual `unsafe` blocks that bypass `windows-rs` wrappers must choose the right deallocator by hand.

**How to avoid:**
- Use `windows-rs` generated types and APIs wherever possible — do not write raw FFI
- For `GetExtendedTcpTable`, use the `windows` crate's binding rather than manual FFI
- If you must allocate manually, document which allocator was used and implement a custom `Drop` wrapper
- Run the project under Application Verifier (Windows SDK tool) during development — it detects allocator mismatches in debug builds
- Add a CI step that runs debug builds with page heap enabled (`gflags.exe /p /enable portunity.exe`)

**Warning signs:**
- Intermittent crashes with no stack trace (heap corruption)
- Heap corruption detected by Application Verifier
- Different Windows API data sources requiring different allocation wrappers
- Manual `unsafe` blocks with raw pointer manipulation

**Phase to address:**
All phases with Windows API interaction. Audit all FFI boundaries during the `port-core` implementation.

---

### Pitfall 15: ETW Provider PID Inaccuracy

**What goes wrong:**
The `Microsoft-Windows-Kernel-Network` ETW provider fires events in an **arbitrary thread context** — the `ProcessID` in the ETW event header is often `0` (System Process), not the actual process that initiated the connection. Even the `Microsoft-Windows-TCPIP` provider, which is more accurate, has a PID inaccuracy rate of approximately 30% in some scenarios. If the tool trusts ETW events to identify which process opened a port, it will misattribute connections.

**Why it happens:**
Kernel-level network processing happens in arbitrary context (DPC, worker threads). The TCP/IP stack processes packets in the context of whichever thread happened to be running when the NIC interrupted. The internal `PID` field in the TCPIP provider event payload is populated from the TCB, which is more accurate than the event header but still not 100% reliable for all connection types.

**How to avoid:**
Treat ETW events as a **notification that something changed**, not as an authoritative data source. When an ETW event fires:
1. Use it only as a trigger to refresh the port list via `GetExtendedTcpTable` / `GetExtendedUdpTable`
2. The API call results are the ground truth — always display API data, not ETW data
3. ETW provides "a change happened" + "roughly which process"; the polling refresh provides the accurate snapshot

This is why the project's architecture of "ETW for event-driven refresh + polling as fallback" is correct. ETW is the trigger, not the data source.

**Warning signs:**
- Port entries attributed to PID 0 or System when another process is the true owner
- Process name shown as "System" for user-initiated connections
- Relying on ETW event payload to populate port list entries

**Phase to address:**
Phase covering real-time monitoring. Architecture: ETW as trigger only, `GetExtendedTcpTable` as ground truth.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Storing raw PIDs instead of HANDLE-wrapped process references | Fewer lines of code, simpler data model | Race condition that randomly kills wrong processes | Never — this is a correctness issue, not a tradeoff |
| Single AF_INET call for port enumeration | Half the API calls, simpler code | Misses all dual-stack and IPv6 connections silently | Only during initial prototyping, replaced before any user-facing build |
| Hardcoding table class constants (magic numbers like `5` for `TCP_TABLE_OWNER_PID_ALL`) | No need to import Windows types | Unreadable, unmaintainable, fragile across Windows versions | Never — use named constants from `windows` crate |
| `expect()` / `unwrap()` on Windows API results | Less error handling code | Crashes on transient conditions (momentary buffer too small, handle temporarily invalid) | Never in production code paths |
| Blocking Win32 calls directly in async context | Simpler code flow | Runtime stalls, UI freezes | Only in CLI-only prototypes without async runtime |
| No retry loop on `GetExtendedTcpTable` buffer allocation | Simpler scanning logic | Silent data loss when table grows between calls | Never — required for correctness |
| Using `TCP_TABLE_BASIC_*` table classes (no PID) | Smaller buffer, faster call | No process attribution — useless for the tool's core purpose | Never — this tool needs PIDs |
| Skipping `PRAGMA journal_mode=WAL` | One less line of DB init | Concurrent access failures when both frontends run | Only when only one frontend will ever exist |

## Integration Gotchas

Common mistakes when connecting to external services and Windows subsystems.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `GetExtendedTcpTable` | Single-call with fixed buffer | Two-call pattern with retry loop; validate `dwNumEntries` |
| `GetExtendedUdpTable` | Assuming states like TCP (LISTEN, ESTABLISHED) | UDP has no connection states — entries are ephemeral endpoints |
| `OpenProcess` | Opening with `PROCESS_ALL_ACCESS` | Request minimum rights needed: `PROCESS_QUERY_INFORMATION \| PROCESS_TERMINATE \| PROCESS_VM_READ` |
| `TerminateProcess` | Calling without checking if process still exists | Check `GetExitCodeProcess` → `STILL_ACTIVE` before terminating |
| `INetFwPolicy2` COM API | Calling from non-elevated process for write operations | Check token elevation first; trigger UAC if needed; fail gracefully with clear message |
| ETW `StartTrace` | Creating session without checking for orphaned sessions | Stop pre-existing session with same name on startup; handle `ERROR_ALREADY_EXISTS` |
| ETW `OpenTrace` + `ProcessTrace` | Calling `ProcessTrace` on the main thread (it blocks until session ends) | Run `ProcessTrace` on a dedicated thread; use channel to communicate events to main logic |
| `QueryFullProcessImageNameW` | Assuming buffer size is sufficient (MAX_PATH = 260) | Use extended-length path syntax (`\\?\` prefix); allocate larger buffer (32,767 chars) |
| `CreateToolhelp32Snapshot` | Forgetting to call `CloseHandle` on the snapshot | RAII wrapper that calls `CloseHandle` on drop |
| `windows-rs` COM wrappers | Holding COM objects across async boundaries | Extract data into plain Rust structs; drop COM wrappers before any `.await` |
| Ratatui backend | Rendering on fixed timer rather than event-driven | Only call `draw()` when state actually changes (user input or data update) |
| Tauri IPC | Returning complex nested types from commands | Use `serde::Serialize` types with well-defined schemas; avoid deeply nested enum variants |
| SQLite across two frontends | Using delete-mode journal (default) | Enable WAL mode on first connection; set `busy_timeout = 5000ms` |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full port table rerender every draw cycle | TUI scroll lag, CPU at 100% | Virtualization; only render visible viewport rows | ~500+ open ports |
| Calling `GetExtendedTcpTable` on the render loop | UI thread blocked during refresh | `spawn_blocking` + cached results + ETW-triggered refresh | Any port count above trivial |
| Building `String` for every cell in a Ratatui `Table` | Frame time increases linearly with row count | Pre-computed `Text` spans; reuse allocations | ~2000+ rows |
| Full process enumeration on every port scan refresh | 2+ second refresh time with many processes | Snapshot-based diff; only re-query changed PIDs | ~500+ processes |
| Synchronous disk I/O in ETW callback path | Events dropped under load | Async ring buffer; batch writes on a separate task | ~100+ events/second |
| No connection pooling for SQLite | Connection open/close per query | Connection pool or single persistent connection with mutex | More than trivial query volume |
| `unicode_width::width()` in Ratatui diff path | Diff time dominates frame time for large tables | Pre-compute widths; skip diff for known-width columns | ~5000+ cells per frame |
| `serde_json::to_string` on every IPC response | Serialization overhead per command call | Return structured types that Tauri serializes natively; cache serialized representations for repeated data | Frequent IPC calls with large payloads |
| 2-second polling with full table rebuild | CPU churn during idle periods | ETW-driven refresh eliminates polling during idle; polling as absolute fallback only | Always-on monitoring |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Using `PROCESS_ALL_ACCESS` for `OpenProcess` | Unnecessary privilege escalation; flagged by security software | Use minimum required rights: `PROCESS_QUERY_INFORMATION`, `PROCESS_TERMINATE`, `PROCESS_VM_READ` |
| Killing processes without verifying ownership | Terminating wrong process due to PID reuse | Hold `HANDLE` from `OpenProcess`; verify PID + creation time before terminating |
| No whitelist for system-critical processes | User accidentally kills `lsass.exe` → immediate system reboot | Shipped whitelist of known critical processes; checked BEFORE `TerminateProcess` |
| Running always-as-admin | Attack surface expansion; all child processes inherit elevated token | Auto-detect admin requirement; trigger UAC only when needed for kill/firewall operations |
| Exposing process command-line arguments in UI/export | Command lines often contain secrets (passwords, tokens, connection strings) | Optional redaction flag in export; tooltip warning in UI that command lines may contain secrets |
| No integrity check on firewall rule operations | Malicious rule injection could disable Windows Firewall | Validate rule parameters before COM call; log all firewall mutations |
| Storing process handle in global mutable state without access control | Any part of the code could terminate any process | Process handle owned by the process manager module; only exposed via controlled API |
| SQLite database world-readable | Port history reveals user's application usage patterns | Database file inherits user directory permissions; no additional world-readable flags |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| "Kill" button with no confirmation for non-whitelisted processes | Accidental termination of important user applications | Instant kill for most processes; confirmation dialog only for whitelist-protected processes |
| Silent failure when kill requires admin rights | User clicks kill, nothing happens, no feedback | Detect `ERROR_ACCESS_DENIED`; show "Administrator privileges required — elevate?" prompt |
| Hiding TIME_WAIT and CLOSE_WAIT connections | User does not see why port is "in use" after closing app | Show all connection states; add a "State" column; color-code TIME_WAIT (will release soon) vs ESTABLISHED (actively in use) |
| No indication that data is stale | User acts on outdated port list | Timestamp of last refresh in status bar; "Last updated: 2s ago" or "Live (ETW)" |
| IPv6 addresses displayed in full uncompressed form | `2001:0db8:0000:0000:0000:ff00:0042:8329` is unreadable | Use RFC 5952 canonical representation (`2001:db8::ff00:42:8329`) |
| Port numbers displayed as raw integers without service name | User sees `5432` instead of `postgresql` | IANA service name lookup for well-known ports (optional, toggleable) |
| Inconsistent sort order between refreshes | User loses track of which port they were looking at | Stable sort (by port, then PID); preserve sort choice across refreshes |
| No "jump to process" from port view | User must manually search for the process in a different view | Click/enter on a port row navigates to the owning process detail panel |
| Firewall rule list with no search/filter | Scrolling through hundreds of rules | Search by name, port, program path; filter by allow/block |
| Export without column headers | CSV file is unusable without context | Always include headers; JSON export includes schema version |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **IPv6 enumeration:** Calls both `AF_INET` and `AF_INET6` — verify by enabling IPv6 on test machine; all dual-stack connections appear
- [ ] **PID-to-HANDLE safety:** `ProcessHandle` struct bundles PID + HANDLE + creation time — verify by stress-test with rapid process creation/destruction
- [ ] **ETW cleanup:** Orphaned session stopped on startup — verify: kill process from Task Manager, relaunch, ETW starts successfully
- [ ] **Port byte order:** All port numbers run through `u16::from_be()` — verify by checking port 80 displays as 80, not 20480
- [ ] **WAL mode:** `PRAGMA journal_mode` returns `wal` — verify by running both TUI and GUI simultaneously, both can read/write
- [ ] **Admin elevation:** Non-admin operations work without elevation — verify: port listing works as standard user; kill prompts for elevation
- [ ] **Blocking calls:** All Win32 calls inside `spawn_blocking` — verify with `tokio-console`: no poll time > 1ms in async tasks
- [ ] **COM cleanup:** Firewall rules extracted to plain Rust structs — verify: repeated rule list/add/delete, memory usage stable
- [ ] **TUI large table:** VirtualTable rendering only visible rows — verify: 5000+ ports, scroll is instantaneous
- [ ] **System process whitelist:** `services.exe`, `csrss.exe` etc. in shipped whitelist — verify: attempt to kill any of them, tool refuses with explanation
- [ ] **Stale data indicator:** Last-refresh timestamp in TUI status bar and GUI status bar — verify: stop ETW, wait, timestamp increments via polling
- [ ] **Export validity:** Exported JSON/CSV contains same data as UI — verify: round-trip comparison
- [ ] **Dual-frontend concurrency:** Both TUI and GUI can run and access SQLite — verify: open both, perform port scan in each, no `SQLITE_BUSY`
- [ ] **Single Cargo workspace:** Only one `Cargo.lock`, one `target/` — verify: `find . -name Cargo.lock` returns exactly one file
- [ ] **Release binary size:** TUI binary < 10MB stripped — verify: `cargo build --release`, check binary size

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| PID reuse race (wrong process killed) | HIGH (user data loss) | 1. Restore HANDLE-holding pattern in process module 2. Add PID + creation-time verification before all TerminateProcess calls 3. Audit all OpenProcess call sites |
| ETW session orphaned (system out of sessions) | LOW | 1. `logman query -ets` to find orphaned session 2. `logman stop Portunity-TCP-Monitor -ets` 3. Add startup cleanup logic |
| IPv6 table not enumerated | MEDIUM (silent feature gap) | 1. Add AF_INET6 call path to scanner 2. Add IPv4-mapped address deduplication 3. Test on dual-stack machine |
| Port byte order wrong | LOW | 1. Add `u16::from_be()` conversion 2. Add unit test with known values 3. Audit all `.dwLocalPort` and `.dwRemotePort` access sites |
| Separate Tauri workspace detected late | MEDIUM | 1. Remove `[workspace]` from `src-tauri/Cargo.toml` 2. Add `src-tauri` to root workspace members 3. Delete `src-tauri/Cargo.lock` and `src-tauri/target/` 4. Rebuild |
| Blocking Win32 calls on async threads discovered late | MEDIUM | 1. Audit with `tokio-console` to find slow tasks 2. Wrap each blocking call in `spawn_blocking` 3. Add regression test that verifies poll times |
| SQLITE_BUSY under dual-frontend load | MEDIUM | 1. Enable WAL mode 2. Add `busy_timeout = 5000` 3. Implement reader/writer connection split |
| COM reference leak in firewall module | MEDIUM | 1. Audit all COM wrapper lifetimes 2. Ensure COM wrappers dropped before `.await` 3. Extract data to plain structs immediately |
| Ratatui table lag at scale | MEDIUM | 1. Switch to VirtualTable/VirtualList 2. Make rendering event-driven 3. Cache cell dimensions |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| PID Reuse Race Condition | Process Management phase | Stress test: spawn/kill 1000 processes rapidly while the tool is running; verify no wrong-process kills |
| GetExtendedTcpTable Buffer Pattern | Port Scanning core phase | Unit test: call with intentionally small buffer, verify retry loop works |
| IPv4/IPv6 Dual-Call Requirement | Port Scanning core phase | Integration test: verify both `AF_INET` and `AF_INET6` tables are queried; check dedup logic |
| Port Number Byte Order | Port Scanning core phase | Unit test: assert port 80 → 80, 443 → 443, 8080 → 8080 |
| ETW Session Orphaning | Real-time Monitoring phase | Manual test: kill process, relaunch, verify ETW starts; orphaned session cleanup on startup |
| ETW Callback Thread Blocking | Real-time Monitoring phase | Load test: 100+ connections/sec; verify `EventsLost` = 0 |
| COM Resource Management | Firewall Management phase | Memory profiling: repeated rule operations, verify stable memory usage |
| `std::sync::Mutex` Across `.await` | Tauri GUI phase | Load test with rapid IPC calls; verify no deadlocks |
| Blocking Win32 on Async Threads | All Windows API phases | `tokio-console` audit: verify all poll times < 1ms |
| Separate Tauri Workspace | Project Scaffolding phase | File check: exactly one `Cargo.lock`, one `target/` directory |
| Protected Process Silent Failure | Process Management phase | Test: attempt to kill `services.exe`, verify clear refusal message |
| SQLite WAL Mode Omission | Storage/Persistence phase | Concurrent test: TUI + GUI both open, both read/write |
| Ratatui Large Table Lag | TUI Frontend phase | Performance test: 5000+ ports, scroll delay < 50ms |
| Windows-rs Allocator Mismatch | All Windows API phases | Application Verifier run in CI: no heap corruption reports |
| ETW PID Inaccuracy | Real-time Monitoring phase | Cross-reference: compare ETW-reported PID with `GetExtendedTcpTable` PID for same connection |

## Sources

- Windows IP Helper API documentation: `GetExtendedTcpTable` function (learn.microsoft.com)
- Stack Overflow: "Not getting correct port number by GetExtendedTcpTable" — port byte order issue
- Stack Overflow: "How can I monitor new IPv4 connections" — ETW TCPIP provider usage
- GitHub: olafhartong/PockETWatcher — ETW TCPIP provider event examples (Event ID 1033)
- GitHub: ratatui/ratatui Issue #1004 — Table performance with 15000 items
- GitHub: ratatui/ratatui Issue #1116 — Bypass diff for performance
- GitHub: ratatui/ratatui Discussion #409 — Partial rendering
- GitHub: mmogr/gglib Issue #106 — Tauri workspace double compilation
- GitHub: tauri-apps/tauri Discussion #10329 — `#[tauri::command(async)]` and `spawn_blocking`
- dev.to: "Rust Async in Tauri v2 — What Tripped Me Up and How I Fixed It" — Mutex across await
- GitHub: magiash/magia Issue #305 — `std::sync::Mutex` vs `tokio::sync::Mutex` in Tauri commands
- GitHub: swiftlang/swift-subprocess PR #93 — PID reuse mitigation by holding HANDLE
- GitHub: rust-lang/rust Issue #112423 — TerminateProcess after process exit
- Stack Overflow: "INetFwPolicy2 COM cleanup" — COM reference counting for firewall API
- docs.rs: zlayer-overlay firewall module — COM initialization and error handling patterns
- Microsoft Docs: TCPIP ETW provider events (learn.microsoft.com)
- TrainSec: "Protected Processes & PPL" — System process protection levels
- Rust users forum: "Windows-rs FFI ergonomic issues" — Allocator mismatch, FAM structs, Send/Sync
- docs.rs: rappct ADR 0001 — FFI safety and ownership patterns
- SQLite forum: "Can multiple applications access a single database file?" — WAL mode concurrent access
- GitHub: optave/ops-codegraph-tool Issue #696 — Cross-library SQLite corruption
- Microsoft Docs: Network Tracing in Windows 7 Architecture — ETW provider comparison

---
*Pitfalls research for: Windows port management tool (Portunity)*
*Researched: 2026-07-26*

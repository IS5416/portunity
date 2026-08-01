//! port-core Ctrl+C helper binary (WR-03).
//!
//! Owned by port-core so ANY frontend (TUI, GUI, tests) that links the
//! kill pipeline gets a correct graceful-signal helper — no dependency
//! on an undocumented `--ctrl-c` flag inside a specific binary.
//!
//! `kill_blocking` (port-core/src/process/kill.rs) re-executes this
//! binary when the kill target has no visible window: the helper detaches
//! from its own (hidden) console, attaches to the target's console, and
//! broadcasts CTRL_C_EVENT to every process on that console (group 0).
//!
//! Exit-code contract (documented in `kill.rs`):
//!   0 = event delivered
//!   1 = no console to attach to
//!   2 = attach succeeded but GenerateConsoleCtrlEvent failed (WR-05:
//!       truthful status — the pipeline force-kills instead of waiting
//!       the full graceful timeout for a signal that was never sent)
//!
//! Caveats: the event broadcasts to ALL processes on the target's console;
//! CREATE_NEW_PROCESS_GROUP children ignore Ctrl+C; a pending ReadConsole
//! may not be interrupted — delivery ≠ exit, the kill pipeline's
//! WaitForSingleObject timeout is the real arbiter.

fn main() {
    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let pid = args.next().and_then(|s| s.parse::<u32>().ok());

    let code = match (flag.as_deref(), pid) {
        (Some("--ctrl-c"), Some(pid)) => deliver_ctrl_c(pid),
        _ => {
            eprintln!("usage: ctrl_c_helper --ctrl-c <pid>");
            2
        }
    };
    std::process::exit(code);
}

/// Deliver CTRL_C_EVENT to the given console process.
///
/// Returns the exit code per the contract above.
fn deliver_ctrl_c(pid: u32) -> i32 {
    use windows::Win32::System::Console::{
        AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
        CTRL_C_EVENT,
    };

    unsafe {
        // Ignore CTRL_C in the helper so the broadcast does not kill it.
        let _ = SetConsoleCtrlHandler(None, true);
        // Detach the helper's own (hidden) console — never the caller's terminal.
        let _ = FreeConsole();

        match AttachConsole(pid) {
            Ok(()) => {
                // WR-05: report the event result truthfully — a failed
                // GenerateConsoleCtrlEvent is NOT a delivery.
                if GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0).is_ok() {
                    0 // delivered
                } else {
                    2 // event delivery failed
                }
            }
            Err(_) => 1, // no console
        }
    }
}

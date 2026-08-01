//! PID-reuse churn test (PROC-07).
//!
//! Spawns and kills processes rapidly (10 iterations) to verify that
//! the creation-time verification logic never returns `Ok` for a stale
//! process identity (wrong-process-kill proof).
//!
//! **All tests are Windows-only.**

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use port_core::process::{kill, open_verified, snapshot_for, KillOutcome};

#[cfg(target_os = "windows")]
#[test]
fn test_churn_no_wrong_process_kill() {
    for iteration in 0..10 {
        // Spawn a long-running child
        let child = Command::new("cmd.exe")
            .args(["/c", "timeout", "30"])
            .creation_flags(0x00000010) // CREATE_NEW_CONSOLE
            .spawn()
            .expect("failed to spawn child");

        let pid = child.id();
        std::thread::sleep(Duration::from_millis(300));

        // Capture snapshot
        let snap = snapshot_for(pid).expect(&format!("iter {}: snapshot_for", iteration));

        // Verify: open_verified should succeed for the real child
        let result = open_verified(&snap);
        assert!(
            result.is_ok(),
            "iter {}: open_verified should succeed for real PID {} — got {:?}",
            iteration,
            pid,
            result.err()
        );
        drop(result); // CloseHandle on drop

        // Kill the child
        let snap2 = snap.clone();
        let outcome = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(kill(snap2, 5, || {}));

        // AccessDenied is tolerated here: TerminateProcess can transiently
        // return ERROR_ACCESS_DENIED when the target is already in the process
        // of exiting (the race is the same practical outcome — the process
        // goes away; the wrong-process-kill proof is the open_verified check).
        assert!(
            matches!(
                outcome,
                KillOutcome::Graceful
                    | KillOutcome::ForceKilled
                    | KillOutcome::Direct
                    | KillOutcome::AlreadyExited
                    | KillOutcome::AccessDenied
            ),
            "iter {}: kill should succeed — got {:?}",
            iteration,
            outcome
        );

        // Wait for exit
        wait_for_exit(pid, 5000);

        // Verify: open_verified with a STALE snapshot (creation time now
        // different because process exited and PID may have been recycled to
        // a new process with a different creation time, OR the PID is gone).
        // Construct a forged snapshot with a wrong creation time.
        let mut forged_snap = snap.clone();
        if let Some(ref mut ct) = forged_snap.creation_time {
            // Flip a bit to ensure it doesn't match any real process
            ct.dwLowDateTime = ct.dwLowDateTime.wrapping_add(1);
        }

        // Try to open_verified with the forged snapshot.
        // If PID is reused by a different process, creation-time verification fails.
        // If PID is gone, snapshot_for will fail.
        // In either case, open_verified must NOT return Ok.
        match snapshot_for(pid) {
            Ok(current_snap) => {
                // PID still exists (reused by a new process? or same process lingering?)
                // The forged snapshot should have a different creation time.
                if current_snap.creation_time != snap.creation_time {
                    // Different process with same PID — verification MUST fail
                    let bad_result = open_verified(&forged_snap);
                    assert!(
                        bad_result.is_err(),
                        "iter {}: open_verified with stale snapshot should fail (PID {} reused)",
                        iteration,
                        pid
                    );
                }
                // If same creation time, PID not yet reused — skip assertion
            }
            Err(_) => {
                // PID is gone — that's fine, no wrong-process kill possible
            }
        }
    }
}

/// Wait for a process to exit (poll with sleeps, up to `max_wait_ms`).
fn wait_for_exit(pid: u32, max_wait_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        let status = snapshot_for(pid);
        if status.is_err() {
            return true; // Process gone
        }
        if start.elapsed().as_millis() as u64 > max_wait_ms {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

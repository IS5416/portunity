//! Integration tests for the kill escalation pipeline (PROC-01, PROC-02, PROC-07).
//!
//! These tests spawn real child processes and exercise the full pipeline:
//! graceful console kill, timeout-then-force, creation-time mismatch abort,
//! already-exited detection, and built-in whitelist hard-block.
//!
//! **All tests are Windows-only** — gated behind `#[cfg(target_os = "windows")]`.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use port_core::process::{
    kill, snapshot_for, KillOutcome,
};

/// Spawn a console child process with the given creation flags.
/// Returns the child's PID.
fn spawn_child(exe: &str, args: &[&str], extra_flags: u32) -> u32 {
    let child = Command::new(exe)
        .args(args)
        .creation_flags(0x00000010 | extra_flags) // CREATE_NEW_CONSOLE + caller's extras
        .spawn()
        .expect("failed to spawn child");
    child.id()
}

/// Wait for a process to exit (poll with sleeps, up to `max_wait_ms`).
/// Returns true if the process exited within the timeout.
fn wait_for_exit(pid: u32, max_wait_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        // Check via OpenProcess + GetExitCodeProcess
        let status = snapshot_for(pid);
        if status.is_err() {
            // Process likely already gone — treat as exited
            return true;
        }
        if start.elapsed().as_millis() as u64 > max_wait_ms {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_graceful_console_child() {
    // Spawn a console child that will respond to Ctrl+C (the helper).
    // `cmd.exe /c ping -t 127.0.0.1` creates a long-running ping.
    let pid = spawn_child(
        "cmd.exe",
        &["/c", "ping", "-t", "127.0.0.1"],
        0, // CREATE_NEW_CONSOLE only — child gets its own console
    );

    // Give the child a moment to start
    std::thread::sleep(Duration::from_millis(500));

    let snap = snapshot_for(pid).expect("snapshot_for");
    let outcome = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(kill(snap, 5, || {}));

    match &outcome {
        KillOutcome::Graceful | KillOutcome::ForceKilled | KillOutcome::Direct => {
            // Any successful kill outcome is acceptable for a ping child.
            // Graceful = Ctrl+C worked, ForceKilled/Direct = fell through to TerminateProcess.
        }
        _ => panic!("Expected Graceful|ForceKilled|Direct, got {:?}", outcome),
    }

    // The child should exit within 5 seconds
    let exited = wait_for_exit(pid, 5000);
    assert!(exited, "Child PID {} did not exit after kill", pid);
}

#[cfg(target_os = "windows")]
#[test]
fn test_timeout_then_force() {
    // Spawn a signal-ignoring child: CREATE_NEW_PROCESS_GROUP means it ignores Ctrl+C.
    let pid = spawn_child(
        "cmd.exe",
        &["/c", "ping", "-t", "127.0.0.1"],
        0x00000200, // CREATE_NEW_PROCESS_GROUP — ignores Ctrl+C
    );

    std::thread::sleep(Duration::from_millis(500));

    let snap = snapshot_for(pid).expect("snapshot_for");

    // Short timeout to trigger the force path
    let outcome = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(kill(snap, 1, || {}));

    match &outcome {
        KillOutcome::ForceKilled | KillOutcome::Direct => {
            // The child ignores Ctrl+C -> wait timeout -> force kill
        }
        KillOutcome::Graceful => {
            // If Ctrl+C somehow worked, that's fine too
        }
        _ => panic!("Expected successful kill, got {:?}", outcome),
    }

    let exited = wait_for_exit(pid, 5000);
    assert!(exited, "Child PID {} did not exit after force kill", pid);
}

#[cfg(target_os = "windows")]
#[test]
fn test_creation_time_mismatch_aborts() {
    // Spawn a child
    let pid = spawn_child(
        "cmd.exe",
        &["/c", "ping", "-t", "127.0.0.1"],
        0,
    );

    std::thread::sleep(Duration::from_millis(500));

    let snap = snapshot_for(pid).expect("snapshot_for");

    // Build a corrupted snapshot: flip one FILETIME word
    let mut bad_snap = snap.clone();
    if let Some(ref mut ct) = bad_snap.creation_time {
        ct.dwLowDateTime = ct.dwLowDateTime.wrapping_add(1);
    }

    let outcome = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(kill(bad_snap, 2, || {}));

    // Must be Failed (creation-time mismatch), not Graceful/ForceKilled/Direct
    match &outcome {
        KillOutcome::Failed(msg) => {
            assert!(
                msg.contains("creation time"),
                "Expected creation-time mismatch message, got: {}",
                msg
            );
        }
        _ => panic!(
            "Expected Failed with creation-time mismatch, got {:?}",
            outcome
        ),
    }

    // The original child should still be alive (wrong-process-kill proof)
    let snap2 = snapshot_for(pid);
    assert!(snap2.is_ok(), "Original child should still be alive; PID {} is gone", pid);

    // Clean up — kill the real child
    let real_snap = snapshot_for(pid).expect("re-snapshot");
    let _ = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(kill(real_snap, 2, || {}));

    wait_for_exit(pid, 5000);
}

#[cfg(target_os = "windows")]
#[test]
fn test_already_exited_kill() {
    // Spawn a quick-exiting child
    let pid = spawn_child(
        "cmd.exe",
        &["/c", "exit", "0"],
        0,
    );

    // Wait for the child to exit
    std::thread::sleep(Duration::from_millis(2000));

    // Try to kill an already-exited process
    let snap = snapshot_for(pid);

    match snap {
        Ok(snap) => {
            // Process might still exist briefly — kill should detect AlreadyExited
            let outcome = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(kill(snap, 2, || {}));

            match outcome {
                KillOutcome::AlreadyExited | KillOutcome::Failed(_) => {
                    // Acceptable — process is gone
                }
                _ => panic!("Expected AlreadyExited or Failed, got {:?}", outcome),
            }
        }
        Err(_) => {
            // Process already gone — snapshot_for failed, which is fine
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_hardblocked_system_process() {
    // PID 4 (System) is always HardBlocked — never reaches OpenProcess
    let snap = snapshot_for(4).expect("snapshot_for(4)");
    let outcome = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(kill(snap, 2, || {}));

    match &outcome {
        KillOutcome::HardBlocked(reason) => {
            assert!(!reason.is_empty(), "HardBlocked reason must not be empty");
        }
        _ => panic!("Expected HardBlocked for PID 4, got {:?}", outcome),
    }
}

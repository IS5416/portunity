---
status: testing
phase: 02-process-management-smart-kill
source: [02-VERIFICATION.md]
started: 2026-08-01
updated: 2026-08-01
---

## Current Test

number: 1
name: BUILTIN constant domain review
expected: |
  Every entry in BUILTIN (port-core/src/process/whitelist.rs) is a real system-critical
  Windows process with a plain-language reason; Tier 1 grounded in Microsoft Restart
  Manager Critical System Services; 25 entries; explorer.exe absent; securesystem.exe
  spelled correctly.
awaiting: user response

## Tests

### 1. BUILTIN constant domain review
expected: Every entry in BUILTIN (port-core/src/process/whitelist.rs) is a real system-critical Windows process with a plain-language reason; Tier 1 grounded in Microsoft Restart Manager Critical System Services; 25 entries; explorer.exe absent; securesystem.exe spelled correctly.
result: [pending]

### 2. x-key kill UX
expected: Press x on a selected row → status bar shows kill outcome strings per UI-SPEC (graceful/force/direct/already-exited); row disappears after auto-refresh; hard-blocked process shows "✗ {name} is protected — ... Press w to review the whitelist." with NO dialog; repeat x on exited process shows "already exited".
result: [pending]

### 3. d-key detail panel
expected: Press d → 12-row panel with all fields (name/PID, status, owning port, path, command line, start time, parent PID, signature, protection, reason, hint); j/k move refreshes panel content; after process dies, name renders strikethrough + Status "Exited"; ◆ markers show for whitelisted processes (red built-in, yellow user); non-admin shows dimmed limited fields.
result: [pending]

### 4. w-key whitelist overlay
expected: Press w → overlay shows built-in section (read-only, ◆ name + short reason) + user section; add path via input (validated — nonexistent path rejected); remove selected entry; status strings match UI-SPEC (Added/Removed); kill of a user-listed process shows confirm dialog; whitelist change takes effect on next kill without restart.
result: [pending]

### 5. ?-key Help overlay
expected: Press ? → full key reference (all Phase 1 + d/x/w + y/n confirm keys + s/w footer-dropped keys); Esc closes; renders above whitelist overlay, below confirm dialog in stack.
result: [pending]

## Summary

total: 5
passed: 0
issues: 0
pending: 5
skipped: 0
blocked: 0

## Gaps

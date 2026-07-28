---
status: testing
phase: 01-tui-port-viewer
source: [01-VERIFICATION.md]
started: 2026-07-28
updated: 2026-07-28
---

## Current Test

number: 1
name: Launch TUI and verify visual rendering
expected: |
  Terminal opens with full frame (tab bar + content + status bar + footer).
  Overview tab shows Port Summary stats, Connection States counts, Top 10 mini-table, Admin Status card.
  Ports tab (Tab 2) shows live TCP/UDP port table with color-coded states and text labels.
awaiting: user response

## Tests

### 1. Launch and Visual Rendering
expected: Full frame renders immediately. Overview tab default. Scanning spinner then live data. Ports tab shows real TCP/UDP ports with color-coded state symbols + text labels.
result: [pending]

### 2. Keyboard Shortcuts — Ports Tab
expected: s=sort cycles (none→▲→▼→none), j/k/↑/↓=row navigation with reverse-video highlight, r=refresh, q=clean exit
result: [pending]

### 3. Search and Filter
expected: /=fuzzy search bar opens, real-time filtering across all fields, Esc clears. f=filter panel opens, Tab cycles fields, Enter applies, Esc clears.
result: [pending]

### 4. Tab Navigation
expected: 1-5 switch tabs. Active tab = Bold + accent_primary bg. Inactive = Dim + fg_muted. Tab/Shift+Tab cycle forward/backward. Tabs 3-5 show "Coming later" placeholder.
result: [pending]

### 5. Resize Gate
expected: Terminal < 80x24 → centered "Terminal too small" message. Resize back ≥ 80x24 → normal layout returns immediately.
result: [pending]

### 6. Non-Admin Mode
expected: All ports visible. System processes dimmed. Status bar: "Admin needed — press a to elevate" (yellow). 'a' triggers UAC prompt. Footer includes "[a]Elevate".
result: [pending]

### 7. Admin Mode
expected: Status bar: "Admin ✓" (green). No dimming. Footer removes "[a]Elevate". 'a' is no-op.
result: [pending]

### 8. Color Map Verification
expected: LISTENING=green ●, ESTABLISHED=blue ●, TIME_WAIT=gray ○, CLOSE_WAIT=yellow ◉, UDP=gray —
result: [pending]

### 9. Auto-Refresh
expected: Port table refreshes every 5 seconds when idle. Status bar clock updates. No interruption to keyboard interaction.
result: [pending]

### 10. Exit Cleanliness
expected: 'q' exits to normal terminal. No raw mode artifacts. Cursor visible. Shell prompt intact.
result: [pending]

## Summary

total: 10
passed: 0
issues: 0
pending: 10
skipped: 0
blocked: 0

## Gaps

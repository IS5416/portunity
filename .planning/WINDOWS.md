---
schema_version: 1
open_count: 4
waived_count: 0
fixed_count: 0
total_count: 4
last_updated: 2026-07-28T10:44:27.947Z
---

# Broken Windows Ledger

> Cross-phase defect register. `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 01 | stub | port-tui/src/components/history.rs |  | Placeholder tab: 'Coming later' — content deferred to Phase 3 | open |  | 2026-07-28T10:44:11.488Z |  |
| 2 | 01 | stub | port-tui/src/components/traffic.rs |  | Placeholder tab: 'Coming later' — content deferred to Phase 3 | open |  | 2026-07-28T10:44:27.197Z |  |
| 3 | 01 | stub | port-tui/src/components/firewall.rs |  | Placeholder tab: 'Coming later' — content deferred to Phase 4 | open |  | 2026-07-28T10:44:27.539Z |  |
| 4 | 01 | deviation | port-tui/src/components/overview.rs |  | Rule 1: Replaced 'Local Addr' column with 'Proto' in top ports mini-table — Connection model lacks local_address field | open |  | 2026-07-28T10:44:27.947Z |  |

````json
[
  {
    "id": 1,
    "kind": "stub",
    "phase": "01",
    "file": "port-tui/src/components/history.rs",
    "line": null,
    "description": "Placeholder tab: 'Coming later' — content deferred to Phase 3",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-07-28T10:44:11.488Z",
    "resolved_at": null
  },
  {
    "id": 2,
    "kind": "stub",
    "phase": "01",
    "file": "port-tui/src/components/traffic.rs",
    "line": null,
    "description": "Placeholder tab: 'Coming later' — content deferred to Phase 3",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-07-28T10:44:27.197Z",
    "resolved_at": null
  },
  {
    "id": 3,
    "kind": "stub",
    "phase": "01",
    "file": "port-tui/src/components/firewall.rs",
    "line": null,
    "description": "Placeholder tab: 'Coming later' — content deferred to Phase 4",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-07-28T10:44:27.539Z",
    "resolved_at": null
  },
  {
    "id": 4,
    "kind": "deviation",
    "phase": "01",
    "file": "port-tui/src/components/overview.rs",
    "line": null,
    "description": "Rule 1: Replaced 'Local Addr' column with 'Proto' in top ports mini-table — Connection model lacks local_address field",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-07-28T10:44:27.947Z",
    "resolved_at": null
  }
]
````

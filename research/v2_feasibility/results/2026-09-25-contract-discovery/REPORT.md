# Discovering contract balances after calls and reorganization

The daemon now indexes retained public terms by their immutable program and
spending rules. `walletListObjectStates` returns bounded pages of known counters
and checks each balance against one selected chain tip. The GUI uses these
pages to find other funded states, then reloads their actual instances before
reviewing a call. This does not change the contract ABI or matrices.

The index does not imply confirmation. It retains pending, spent and orphaned
terms, allowing a predecessor to become usable again after a reorganization.
It streams index names with bounded memory and reads at most 64 openings per
page. It never scans complete proof receipts. Existing watched openings are
indexed once on first use. Discovery covers local public terms, not unknown
programs or calls missed beyond the retained body window.

## Real daemon checks

`scripts/live_v2_contract_discovery_scenario.py` continued copies of the stopped
H81 shared-receipt fixture in a loopback-only network namespace:

- H82 funded a counter watched by both participants.
- Before mining, the prepared successor was listed without a balance, while
  the original remained funded.
- H83 confirmed a call from one authority. The other participant discovered
  the resulting counter and balance automatically through P2P processing.
- A branch starting from the common H82 funding block accumulated 524,288 work,
  exceeding the call branch's 262,144. At H84, the real P2P reorganization
  restored the original funded state and removed the successor balance.
- The old receipt was rejected as belonging to an orphaned call. Browsing
  from the orphaned successor still found its funded predecessor.
- The same discovery results survived receiver restart. Pagination used a
  one-entry limit to exercise continuation and prevent repeated entries.

All processes stopped successfully. This functional run was not a constrained
receiver performance measurement.

## Real GUI interaction

A separate virtual display used the release GUI, a real isolated daemon and
the resulting H84 data. Only the saved orphaned successor was initially listed.
Using GUI buttons, the operator opened those terms, found and selected the
funded predecessor, reviewed the exact next call, and confirmed it.

The transaction shown in the GUI review entered the mempool and then the normal
externally mined H85 block. The GUI subsequently displayed counter zero changed
to one and the balance changed from 10 to 9.9942 NOID, matching the reviewed
0.0058 NOID fee. An independent RPC check verified the call receipt and current
State instance. No RPC call was used to submit the tested GUI transaction.

The interactive check exposed a stale submission notice remaining after balance
refresh. The final UI change clears that notice when loading a fresh balance
view; it does not infer transaction confirmation from a local candidate.

Validation passed 202 node tests, 42 RPC tests and 66 GUI tests, plus release node
and GUI builds. The final notice cleanup repeated the affected GUI checks.
The earlier node compilation attempt required fixing a missing trait-dependent
zero constant before these successful checks; no failed result is counted.

[Measurements](measurements.json) retain binary/script hashes, both branch work
values, every discovery observation, GUI transaction and receipt identity, and
hashes of the privately retained GUI screenshots. The tested bank remained
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.

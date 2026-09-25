# Retired-history node: cold sync, contract production and restart

An empty daemon built with `retired-history` synchronized over P2P and then
produced two accepted v2 blocks, including an integer-contract call. Its
executable contains both new matrices and the independently pinned legacy
preprocessing keys, with no embedded old matrices. The boundary certificate
authenticates both old classes; the last pre-fork block uses B255.

This run uses the [both-class origin](../2026-09-25-full-legacy-origin/REPORT.md)
and the same H10 joint candidate bank. It exercises the real daemon, wallet
authorization, external mining, transport, durable State and portable receipts
inside a loopback-only network namespace.

## Workload and result

A transition daemon first funded a custom counter and changed it from 0 to 1
at H20. The chain then reached H63, beyond the full-body serving window for
that call. Its body was absent, while the watched portable receipt remained
available and valid.

The retired node started with empty data. It received the 15,093,850-byte
certificate through P2P, installed snapshot H42, applied the 21-block tail
and reached the exact H63 tip. It verified the pruned H20 receipt and found
the counter's current funded output. After restarting as an external-template
producer, it built empty H64 and the H65 call changing the counter from 1 to 2.
The normal peer accepted both blocks and verified the new receipt.

Finally all peers stopped. The retired node restarted offline at the unchanged
H65 tip and verified both the old pruned receipt and its newly produced one.
All daemons stopped cleanly. No legacy matrix cache files appeared.

## Resource measurements

The retired node was constrained to four logical CPUs, the PCLMUL backend,
8 GiB RAM and no swap. Its two-thread nonce worker ran outside that cgroup;
the table's memory figures cover the node rather than that separate worker.

| Operation | Measured time |
| --- | ---: |
| Empty-node startup and cold synchronization to H63 | 28.487 s |
| Snapshot terminal verification, including initial setup | 3.222 s |
| H63 suffix terminal verification | 2.662 s |
| Empty H64 proof preparation | 76.052 s |
| One-call H65 proof preparation | 78.885 s |

Cold synchronization and receipt verification peaked at 625,864,704 bytes
(596.87 MiB). The highest observed node peak across production was
3,369,639,936 bytes (3.14 GiB). Every recorded `max`, `oom` and `oom_kill`
counter remained zero. The offline restart peaked at 620,707,840 bytes.

This proves operation within the tested memory envelope. **The four-CPU
PCLMUL producer does not meet a 30-second mining interval.** Its verification
cost is a separate measurement. The transition producer used eight logical
CPUs and AVX2+VPCLMUL; its per-template timings are recorded separately in
[measurements.json](measurements.json). These are laptop measurements with a
small live State, not a server distribution or an extrapolation to every
State size.

## Reproduction and harness correction

Run [the scenario](../../../../scripts/live_v2_retired_mining_scenario.py) with
the transition and retired binaries, completed `joint-produce` fixture and
matching retirement certificate identified by the recorded hashes. The
environment variables are `NOID_V2_LIVE_DIR`, `NOID_V2_FULL_ORIGIN_SOURCE`,
`NOID_V2_FULL_RETIREMENT`, `NOID_V2_TRANSITION_NODE` and
`NOID_V2_RETIRED_NODE`. Invoke it through
`unshare --user --map-root-user --net python3`.

The initial harness stopped at H45 and incorrectly expected H20's body to be
pruned. The full-body serving window is 42 blocks, distinct from the 18-block
finality and snapshot suffix. That attempt failed its assertion and stopped
both nodes cleanly; no consensus or retention setting was changed. The
corrected run copied the stopped H45 data and wallets, verified the receipt,
and continued through H65. Its report preserves the failed checkpoint's
source and executable hashes and the complete accepted-block records.
`NOID_V2_RETIRED_RESUME_SOURCE` is restricted to that specific harness failure.
Without it, the corrected scenario runs the full 43-block pruning tail.

The final tip is
`52cb96a86ac60553139041cfcaf67406d9e66ea08c05eb77a08b91f1b9f114f3`.
The final mainnet capacities and source-scheduled matrix pack remain separate
release decisions. This run does not claim retired-node B255 production.

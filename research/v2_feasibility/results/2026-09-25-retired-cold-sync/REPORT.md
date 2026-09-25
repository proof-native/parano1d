# Cold synchronization after legacy matrix retirement

The transition binary and a `retired-history` binary both synchronized from
empty directories to the same pruned v2 chain through ordinary P2P. The retired
binary contains no legacy matrix blobs. It authenticated the fork certificate,
installed the snapshot at H66, applied the 19-block suffix to H85, and verified
the contract receipt produced by the earlier real GUI call.

This qualifies the current isolated joint bank, not a mainnet release artifact.
The chain's legacy history used B25. The both-legacy-class boundary is a separate
qualification. Legacy verification metadata and independently pinned retirement
keys remain necessary; retirement removes the old matrix rows.

## Method and results

Source revision: `86d07d660910a010463a6d73b30d43309e940950`. Run:
`v2-live-retired-sync-20260925-b`; scenario:
[`live_v2_retired_sync_scenario.py`](../../../../scripts/live_v2_retired_sync_scenario.py).
The source is a stopped copy of the previously qualified GUI chain. Only the
copy's transport certificate was replaced. Every receiving node independently
verified the replacement against its embedded pins.

Receivers used four logical CPUs, the PCLMUL backend, an 8 GiB memory limit and
no swap. The network namespace contained loopback only. Proof/build jobs did
not overlap this run. Measurements are single-run observations, not percentiles.

| Check | Startup, synchronization and RPC checks | Peak cgroup memory |
|---|---:|---:|
| Transition binary, empty directory | 32.473 s | 1,640,566,784 B |
| Retired binary, empty directory | 33.883 s | 1,637,761,024 B |
| Retired offline restart and receipt verification | — | 1,654,394,880 B |
| Recovery of missing certificate | — | 1,637,306,368 B |
| Recovery of legacy-only certificate | — | 1,643,991,040 B |
| Recovery of corrupted certificate | — | 1,647,779,840 B |

All cases preserved the exact H85 tip and current contract instances. Every
memory limit/OOM counter was zero. No legacy packed-matrix cache files appeared
on the retired receiver. All processes stopped cleanly.

The offline checks stopped the serving node as well as every other receiver.
The valid retained certificate allowed independent receipt verification after
restart. Missing bytes, a valid legacy-only `O1V2OR01` certificate, and a
one-byte corruption in `O1V2OR02` each caused verification to fail. After the
source restarted, the receiver fetched and authenticated the replacement and
verified the same receipt **without advancing the selected tip**.

## Exact-origin certificate

The certificate is bound to this chain's actual H9 predecessor and the current
joint bank. It cannot be reused for a different predecessor or bank.

- Encoded certificate: **7,353,469 B**, retained once per origin.
- Preparation: **463.599 s**, peak RSS **7,143,460 KiB**, no swap, 20 GiB cap.
- Independent verification with four CPUs/PCLMUL: **935 ms** for certificate
  verification, **6.94 s** for the whole process, peak RSS **86,472 KiB**.
- Observed transition binary: **87,358,432 B**; retired binary: **70,216,896 B**.

The certificate uses its own bounded P2P transport; it is not appended to each
block terminal. The existing 1,100,000-byte terminal limit is unchanged.
The measurements cover a small retained State and a 19-block suffix; they are
not a bandwidth benchmark or a full mainnet State-size prediction. Machine
readable pins, hashes, phase timings and negative outcomes are recorded in
[`measurements.json`](measurements.json).

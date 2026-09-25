# Live integer-contract lifecycle qualification

The candidate passed a fresh two-daemon P2P run from H1 through H64, with
v1.1 at H5 and v2 at H10. All six templates and a custom persistent counter
used the normal wallet, authorization, mempool, external mining, recursive
proof, State and RPC paths. This is an isolated qualification result; it does
not select final release capacities.

The bank was
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`:
Small P63/I504/C63 and Large P255/I1020/C26. This run mined Small after v2.
Its largest block contained nine ordinary payments or seven contract calls.
Full-capacity and alternating-class daemon tests are separate work.

## Coverage

- Nine deposits covered payment collection, expiry refund, vault, allowance,
  period budget, recurring payment, tranche vesting, counter and overflow cases.
- Eighteen confirmed calls exercised repeated counter updates, budget exhaustion
  and reset, due-height restrictions, fixed recurring amounts, anchored vesting,
  recovery and closing.
- Ten rejected API operations covered pre-activation funding, wrong authority,
  early release, arithmetic overflow, wrong incarnation, excessive payment,
  exhausted budget, premature repeated claims and a stale reviewed height.
- The receiver was offline for H19–H21, then recovered the exact suffix over P2P.
  Its H19 receipt used the later proof with an authenticated header path.
- All 18 calls exported and verified on both nodes: 36 receipt checks before
  pruning and another 36 after 43 additional real blocks. Both nodes reported
  the original call bodies absent after pruning. Retained receipt sizes were
  912,970–917,770 bytes.
- The receiver restarted at H64 and recovered the identical selected tip. Both
  processes shut down successfully.

The negative API cases complement the earlier circuit mutation tests; they do
not replace them. A reorg crossing activation was not exercised in this run.

## Host and observations

One shared Intel Core i7-1365U laptop ran both processes. The producer had eight
logical CPUs in its affinity set, six proof workers, and the AVX2+VPCLMUL
backend. The receiver had four logical CPUs, four workers, forced PCLMUL,
an 8 GiB cgroup limit and no swap. The external PoW worker used two threads.
No builds or other proof benchmarks ran alongside the scenario. These affinity
sets are not independent physical servers.

| Block workload | Producer template preparation | Receiver terminal verification |
| --- | ---: | ---: |
| H10, fork block | 23.794 s | 2.561 s |
| H11, nine payments | 18.877 s | 2.675 s |
| H12, six contract calls | 21.208 s | 2.726 s |
| H15, budget reset and recurring claim | 20.028 s | 2.731 s |
| H18, seven closing calls | 24.839 s | 2.671 s |
| H22–H64, 43 empty blocks, median | 22.334 s | 3.328 s |

Preparation excludes wallet authorization, PoW and network delivery. Verification
excludes subsequent State application and wallet artifact retention. Individual
rows are single observations, not latency distributions. Empty-block preparation
ranged from 18.422 to 29.066 s; verification ranged from 2.936 to 4.872 s.

The highest measured receiver cgroup peak was **806,973,440 bytes (769.59 MiB)**,
including its database and wallet activity. Both measured process lifetimes had
zero memory-limit or OOM events. This is evidence for the tested light workload,
not a peak-memory result for a full Large block.

## Reproduction and evidence

Run `scripts/live_v2_contract_scenario.py` in a loopback-only network namespace
after building the separately named `isolated_v2_node`, `parano1d-cli` and
`parano1d-miner` with the pinned candidate pack. Set `NOID_V2_LIVE_DIR` to a fresh
directory; the script refuses an existing destination. It needs a user systemd
manager with the stated cgroup controls. Do not set `NOID_V2_SKIP_PRUNING` for
this qualification.

The measured binaries used source base `f5b99401` plus the external worker's
configurable timeout/finite block count and the node's scheduled startup reward.
Later wallet durability and miner heartbeat changes were not in these binaries.
[Machine-readable measurements](measurements.json) retain binary hashes, every
accepted block's preparation time, receiver verification observations, negative
results, receipt sizes and resource samples.

Earlier attempts remain recorded privately: one had an invalid receiver launch
argument; another began controlled mining before its second remote call arrived.
The successful scenario explicitly waits for producer mempool delivery before
starting each call block.

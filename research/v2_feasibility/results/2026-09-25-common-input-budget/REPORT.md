# Shared input and contract limits

The two-class candidate fits 63 user pages in m23 and 206 user pages
in m24, with the same limits of 504 live inputs and 63 contract calls in both
classes. Both the isolated and mainnet-scheduled matrices have been frozen
and checked. This preserves the standard class and makes the larger class a
capacity superset. It trades five pages from the 211-page, 384-input Large
candidate for 120 additional input positions.

## Capacity and interpretation

| Class | User pages | Live inputs | Contract calls | Useful rows | Row headroom |
|---|---:|---:|---:|---:|---:|
| Small m23 | 63 | 504 | 63 | 6,936,668 | 1,451,940 |
| Large m24 | 206 | 504 | 63 | 16,776,698 | 518 |

This table uses the source-pinned mainnet schedule at H210537. The H10
isolated schedule uses eleven fewer rows per matrix.

These are simultaneous limits. Calls use the same page budget as payments;
they are not an additional allowance. The primary coinbase has its own page.
Any additional mandatory system page consumes part of the remaining budget.

| Workload | Small | Large |
|---|---:|---:|
| Ordinary one-page payments, one or two inputs each | 63 | 206 |
| Ordinary one-page payments, eight inputs each | 63 | 63 |
| Contract calls | 63 | 63 |
| Full-call mixed block | 63 calls | 63 calls + 143 one-page payments |

The mixed example also fits when each payment consumes two inputs. The common
504-input budget applies to the sum of all transaction inputs in the block,
not to each transaction separately. Multi-page transactions consume several
page positions.

At the 30-second target, the one-page capacity ceilings are 2.1 and about
6.87 transactions per second respectively. Both classes allow at most 2.1
contract calls per target second. These are arithmetic per-class ceilings,
not measurements of sustained network throughput. Larger-class mining remains
an explicit server option; the standard class already supports the same
contract interpreter and templates.

## Complete isolated matrix construction

The runner authenticated the existing H1–H9 fixture chain, then constructed
both complete recursive relations for v2 at H10. This includes the annual
height-based emission rule, both predecessor-class branches and the full
contract interpreter. The matrices converged over two passes, satisfied their
witnesses and were rebuilt with their final bank pin. Compact runtime recipes
round-tripped without changing that pin.

The adjacent larger candidate does not fit:

| Large pages, with 504 inputs and 63 calls | Useful rows | Result against m24 |
|---|---:|---:|
| 206 | 16,776,687 | Fits, 529 spare |
| 207 | 16,778,193 | 977 over |
| 209 | 16,781,223 | 4,007 over |
| 210 | 16,782,703 | 5,487 over |
| 211 | 16,784,497 | 7,281 over |

Compressed matrices occupy 10,464,568 and 21,475,021 bytes. Their terminal
upper bounds are 1,014,132 and 1,081,396 bytes, both below the 1,100,000-byte
transport limit. Matrix-file sizes are not network terminal sizes.

The isolated bank is
`be82f3bec102f03c63715dac9bb4a939cb9aa5b21d013e38402b321fd8a41fa9`.
See [candidate records](comparison.json) for matrix identities, commands,
executable hashes and construction results. The [source patch](isolated-source.patch)
reconstructs the successful isolated measurement source on its recorded base.

## Mainnet-scheduled pack

The normal mainnet build froze both matrices with the existing v1.1 boundary
at H95125 and v2 at H210537. It then rebuilt each matrix with the final bank
pin and two distinct hypothetical boundary states. All four witnesses were
satisfied and retained the same class digests. The pack loader authenticated
both compressed matrix files and checked their terminal bounds.

The mainnet bank is
`c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e`.
Its compressed matrices occupy 10,465,911 and 21,478,342 bytes. Terminal bounds
are the same as for the isolated bank. [Mainnet records](mainnet.json) contain
the exact pins, file hashes, build identity and successful checks; the
[mainnet source patch](mainnet-source.patch) reconstructs that build's source.
Ten targeted tests passed for configuration identities, complete registry
certificates, miner resource budgets and pre/post-fork RPC class reporting.

The matrix-construction runs above do not produce v2 terminals. The mainnet
freezer creates no verified fork origin and does not claim to know the future
predecessor block.

## Full isolated proofs

A subsequent run used the exact archived isolated executable and reproduced
the same `be82f3be…` bank. It verified the H1–H9 legacy fixtures and produced
27 accepted v2 blocks, H10–H36, including all predecessor-class transitions,
full ordinary and contract blocks, and Small blocks after Large. Each block
passed native execution and recursive terminal verification. Complete matrix
witness audits passed for the transition cases and both full-call classes.
Terminal mutation checks rejected 243–245 variants per audited block.

These are individual samples on an Intel Core i7-1365U laptop, with 12 Rayon
threads and AVX2+VPCLMUL. Preparation is input construction, recursive assembly
and proving. Wallet authorizations were created separately; one-time matrix
authentication, witness audits, PoW and negative controls are also excluded
from the preparation column. Verification here ran in the producer process.

| Workload | Height | Preparation | Verification | Terminal bytes |
|---|---:|---:|---:|---:|
| Small: 63 ordinary payments | 27 | 17.565 s | 0.830 s | 913,108 |
| Small: 63 calls | 30 | 18.161 s | 0.774 s | 913,012 |
| Large: 206 ordinary payments | 31 | 42.583 s | 2.017 s | 981,396 |
| Large: 63 calls + 143 payments | 32 | 46.294 s | 2.469 s | 979,828 |
| Small: 63 calls immediately after Large | 33 | 19.631 s | 1.521 s | 912,884 |

The three subsequent empty Small blocks had median preparation of 16.584 s
and median verification of 1.011 s. This sequence checks continued operation
after Large; it is not a server speed prediction. Large preparation on this
laptop exceeds the 30-second target. Its optional production flag remains
an operator choice for suitable hardware.

All observed terminals fit their class bounds and the transport limit. The
whole run peaked at 8,171,596 KiB RSS, including full matrix audits and both
provers; this is not the memory requirement of a receiving seed. Exact
commands, executable identity, timing scope, terminal hashes and records are
in [full proof measurements](full-proofs.json). The existing
[isolated source patch](isolated-source.patch) reconstructs this executable's
source.

The production artifact loader also authenticated and staged this exact bank,
verified H36 against its checked legacy origin and matched native replay. It
rejected five malformed/unpinned metadata cases and both swapped matrix
classes. [Loader records](production-loader.json) preserve the command,
artifact hashes and origin binding. This exercises the embedded runtime path;
it is separate from running a complete node.

The corresponding admission changes derive limits from the authenticated
bank. Wallet preflight and mempool checks reject unmineable intents before
authorization, repeat the checks after authorization, and remove entries
invalidated by a fork or changed candidate-height capacity. Tests cover the
504/505-input boundary, activation during authorization, eviction and rollback
to legacy admission. The mempool/RPC suites passed 96 tests on each of the
mainnet and H10 profiles; the isolated node passed 202 tests, and the mempool
API example compiled. These tests do not change the frozen matrices.

## Maximum inputs across State segments

The same archived executable continued the verified H36 fixture to H54 with
18 further accepted proofs. Each maximum-input workload spent 504 inputs from
256 State segments. Complete frozen-matrix witness checks passed for all four
layouts. Both classes also accepted an empty successor after their maximum
input workloads.

| Class and layout | Pages | Inputs | Outputs | Preparation | Verification | Terminal bytes |
|---|---:|---:|---:|---:|---:|---:|
| Small, eight inputs per page | 63 | 504 | 126 | 18.099 s | 0.706 s | 915,700 |
| Large, four inputs per page | 126 | 504 | 252 | 48.032 s | 2.583 s | 983,732 |
| Large, eight inputs per page | 63 | 504 | 126 | 41.818 s | 2.867 s | 981,076 |
| Large, all page positions used | 206 | 504 | 412 | 43.545 s | 1.776 s | 979,764 |

These are individual samples on the same laptop and use the preparation timing
scope described above. Verification ran in the producer process. The last row
checks that Large's page and input limits can be reached simultaneously.

A deliberately constructed Large block with 505 inputs failed both native
candidate validation and a direct check against the unchanged frozen matrix.
The latter check bypassed the narrow native limit to confirm that the circuit
itself rejects the excess input. Small's 63 pages already bound its inputs to
504 at eight inputs per page.

The run performed seven complete matrix witness audits and peaked at
9,651,792 KiB RSS, including the prover and audits. This is not a receiving
node memory measurement. [Maximum-input records](input-budgets.json) contain
the command, source identity, terminal hashes, timing records and negative
control. This executable predates the later wallet/mempool admission change;
the admission suites and daemon workloads qualify that separate code path.

## Verification with four CPUs and 8 GiB

A separate process verified saved terminals with four Rayon threads, CPU
affinity `0,2,4,6`, a four-CPU cgroup quota, an 8 GiB memory limit and no swap.
It forced the PCLMUL backend instead of using this laptop's faster VPCLMUL.
The driver checked the actual cgroup limits before starting and retained its
final memory and CPU counters after the child exited.

Each row below has three samples per cache condition. A cold check starts with
an empty checked-claim cache after the matrices have been authenticated and
loaded. A repeated check verifies the **same terminal** again in that context;
it is not a measurement of the next distinct block in a running daemon.

| Workload | Height | Cold median | Repeated-terminal median |
|---|---:|---:|---:|
| First v2 Small block | 10 | 1.647 s | 1.670 s |
| Small: 63 calls | 30 | 5.740 s | 2.867 s |
| Large: 63 calls + 143 payments | 32 | 7.699 s | 6.489 s |
| Small: 63 calls immediately after Large | 33 | 5.851 s | 2.810 s |
| Small: 504 inputs across 256 segments | 42 | 5.680 s | 2.827 s |
| Large: 206 pages, 504 inputs across 256 segments | 53 | 7.502 s | 6.407 s |

All 36 timed checks passed. Six independent native fixture replays matched
the verified accumulators, and 1,468 malformed or altered terminal checks
were rejected. The slowest individual timed check took 9.787 s. The cgroup
peaked at 2,820,214,784 bytes (2.63 GiB), with no memory-limit or OOM events
and no swap. Process peak RSS was 2,749,524 KiB. These peaks include fixture
replay and negative controls as well as proof verification.

Standalone setup took 767.574 s, including full matrix-file authentication
and legacy-origin verification; it is excluded from the per-terminal samples.
It is not an embedded-daemon startup measurement. The complete run took
979.572 s. [Receiver records](receiver-4cpu8g.json) preserve all samples,
enforced limits, final counters, source and executable identity, and the
precise timing and memory scopes.

This qualifies standalone proof verification under the stated local hardware
profile. Daemon workloads, transaction admission, P2P delivery, sequential
cache use and startup are separate checks. The following run covers the light
contract lifecycle with this bank; earlier daemon results used different banks.

## Contract lifecycle through real daemons

A fresh loopback-only network mined and accepted H1 through H64, crossing v1.1
at H5 and v2 at H10 with the exact isolated bank above. Six templates and custom
integer programs used normal CLI/RPC wallet authorization, mempool admission,
external mining, recursive proofs, P2P delivery and persistent State. All post-
fork blocks used Small. The largest workload in this run was nine ordinary
payments or seven contract calls; full-capacity daemon checks are separate.

Eighteen confirmed calls exercised collection, expiry recovery, timed release,
allowance limits, budget resets, recurring payments, tranche vesting, persistent
counters and closing. Ten rejected operations covered pre-activation funding,
wrong authority, early release, overflow, wrong incarnation, excessive payment,
exhausted or premature claims and a stale reviewed inclusion height. At H9,
the mining RPC reported the scheduled next-block reward and stopped offering
legacy cache controls for v2.

The receiver was offline for H19–H21 and recovered the exact selected tip over
P2P. The missed H19 call exported with a later authenticated proof. All 18 calls
exported and verified on both nodes before pruning and again after 43 further
real blocks: 72 receipt checks in total. Both nodes confirmed that the original
call bodies had been removed. The receiver restarted at H64, reopened the
identical tip, and both processes stopped successfully. This run did not reorg
across the activation boundary.

Both daemons shared the same laptop. The producer used affinity
`1,3,5,7,8,9,10,11`, six proof workers and AVX2+VPCLMUL; the external nonce worker
used two threads. The receiver used affinity `0,2,4,6`, four proof workers,
forced PCLMUL, an enforced four-CPU quota, 8 GiB memory and no swap. No builds
or independent proof benchmarks ran alongside the scenario.

| Workload | Height | Producer preparation | Receiver terminal verification |
|---|---:|---:|---:|
| First v2 block | 10 | 20.110 s | 2.212 s |
| Nine ordinary payments | 11 | 23.738 s | 3.457 s |
| Six contract calls | 12 | 21.508 s | 2.737 s |
| Budget reset and recurring claim | 15 | 25.322 s | 3.482 s |
| Seven closing calls | 18 | 19.106 s | 2.764 s |
| 43 empty blocks, median | 22–64 | 22.665 s | 3.256 s |

Preparation excludes wallet authorization, PoW and delivery. Verification is
the running receiver's check of each distinct terminal before State application,
using its normal sequential cache. Empty-block preparation ranged from 18.781
to 32.092 s; verification ranged from 2.708 to 4.791 s. The individual workload
rows are single observations on shared hardware.

The highest sampled receiver cgroup peak was 806,969,344 bytes (769.59 MiB).
Both measured process lifetimes had zero memory-limit or OOM events. Initial
embedded receiver startup to responsive RPC took 10.524 s; restarts at H18 and
H64 took 11.010 and 11.512 s. These are actual node startup measurements, unlike
the standalone matrix-authentication setup above. The complete scenario took
1,843.533 s. This light workload does not establish the full Large memory peak.

[Daemon records](contract-daemons.json) retain the actual resource limits,
startup times, all block preparation and received-terminal observations,
negative cases, receipt sizes and source/binary/log hashes. The node source is
equivalent to commit `cb9504f`; the record also preserves its build base and
patch identity. Reproduce with `scripts/live_v2_contract_scenario.py` in a
fresh loopback-only namespace, using the pinned isolated node and a new
`NOID_V2_LIVE_DIR`, with pruning enabled.

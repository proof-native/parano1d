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

## Full-capacity daemon workloads

The stopped H64 fixture continued through H83 with the same node, bank and
enforced receiver profile. Large was permitted on the producer throughout.
The 63-payment and 63-call workloads still selected Small; the larger payment
workloads selected Large. Every submitted intent reached both mempools before
mining and appeared in the accepted block on both nodes.

| Accepted workload | Class | Height | User pages | Live inputs | Preparation | Receiver verification | Receiver application |
|---|---|---:|---:|---:|---:|---:|---:|
| 63 ordinary payments | Small | 65 | 63 | 63 | 20.755 s | 2.700 s | 0.294 s |
| 63 counter calls | Small | 66 | 63 | 63 | 22.790 s | 2.774 s | 2.391 s |
| 206 ordinary payments | Large | 70 | 206 | 206 | 50.180 s | 6.305 s | 1.254 s |
| 63 counter calls + 143 payments | Large | 71 | 206 | 206 | 67.326 s | 6.723 s | 6.050 s |
| One 341-input spend + 163 payments | Large | 72 | 206 | 504 | 54.619 s | 8.394 s | 1.619 s |
| 63 payments immediately after Large | Small | 73 | 63 | 63 | 23.904 s | 5.696 s | 1.094 s |
| One 504-input spend | Small | 75 | 63 | 504 | 19.634 s | 2.867 s | 1.254 s |

These are individual observations. Preparation uses six proof workers on the
shared laptop and excludes wallet authorization, PoW and delivery. Large took
longer than the 30-second target on this producer; these results do not estimate
AVX-512 server performance. Receiver application is measured after terminal
verification and includes State, watched-wallet and receipt work.

The two full-call blocks advanced all 63 counters from zero to one and then
two. The receiver found every successor and independently verified receipts
from both ends of each call prefix. Calls use the same page budget as payments:
Large increases ordinary payment space without changing the 63-call limit.
The maximum-input Large layout contained 164 logical transactions occupying
206 pages. The maximum-input Small layout contained one logical transaction
occupying 63 pages. Page capacity is not a transaction count for multi-page
spends. Both wallet planning and submission rejected a 505-input spend with
`InputLimitExceeded` and `max_inputs: 504`.

The first Small block after Large required 5.696 s for terminal verification.
The next two distinct Small blocks took 2.729 and 2.867 s, followed by eight
empty Small blocks with a 2.890 s median and a 2.587–4.156 s range. Their
preparation median was 21.253 s. This sequence did not retain a doubled Small
verification cost after Large. The receiver then restarted and reopened the
exact H83 tip. Its initial RPC startup took 10.023 s and its restart 12.512 s.

The receiver cgroup peaked at 1,775,669,248 bytes (1.65 GiB), with no memory-
limit or OOM events and no swap. This includes startup, mempool admission,
full blocks, receipt checks and the subsequent Small sequence. It excludes
producer memory and is a local qualification workload, not a measurement of
a production seed under sustained public traffic.

For admission, serialized wallet submission of the 206-payment batch until
both mempools held every intent took 187.275 s; the receiver used 19.041 CPU
seconds over that interval. The mixed batch took 179.643 s and 19.691 receiver
CPU seconds. These wall times include the producer's authorization work and
are not saturated transaction-relay throughput measurements. The receiver
CPU deltas also include its background activity during each interval.

The complete scenario took 1,656.438 s and shut down successfully.
[Capacity records](capacity-daemons.json) retain all 19 accepted blocks,
six admission workloads, exact page/input totals, both over-limit errors,
resource counters, source identities and log hashes. Reproduce with
`scripts/live_v2_capacity_scenario.py` in a fresh loopback-only namespace,
using this bank and the stopped successful contract-lifecycle fixture.

## Reorganization across the activation boundary

A separate two-node network shared a real legacy prefix through H8. Branch A
mined its own H9 predecessor, crossed v2 at H10, funded a counter at H11 and
confirmed its first call at H12. Branch B independently mined another H9
predecessor and continued to H13 with strictly greater cumulative work.
The scenario compared work, rather than assuming that greater height wins.

After reconnection, normal P2P synchronization replaced branch A's selected
origin and rolled back the orphaned contract balance and successor. Its
receipt failed with `receipt is not on the selected chain`, export failed
because the call was no longer confirmed, and resubmitting the stale reviewed
call was rejected. Retained public terms remained available. Both nodes agreed
on every canonical block hash through H13. After restarting the reorganized
node, it mined H14 successfully and both nodes again selected the exact same
tip. Both processes shut down successfully.

This functional run took 335.515 s; observed P2P convergence during the reorg
took 1.786 s. It did not use the constrained receiver profile and is not a
four-CPU reorg performance claim. [Reorg records](reorg-daemons.json) retain
the old and new origin headers, cumulative work, rejected operations, restart
times and source/binary/log hashes. Reproduce with
`scripts/live_v2_boundary_reorg_scenario.py` and the pinned isolated bank in
a fresh loopback-only namespace.

## Shared receipt storage on the selected bank

The capacity fixture continued from H83. At H84, another 63-call Small block
advanced the existing counters from two to three. Each node exported all 63
portable receipts, with matching hashes across nodes; the first and last were
independently verified. H85 exercised the retained-window reader. After a
receiver restart, all 63 exports were byte-identical and the first and last
verified again. Four earlier retained receipts still exported and verified
after their original call bodies had been pruned.

| Receipt payload storage per node for H84 | Bytes |
|---|---:|
| 63 individual references and inclusion data | 110,754 |
| One shared recursive proof | 914,004 |
| Combined local payload | 1,024,758 |
| Equivalent 63 separate portable receipts | 57,690,486 |

Sharing reduced this block's local receipt payload by a factor of 56.30.
These are file content sizes, excluding directory metadata and separately
retained public terms. Every complete portable export was still 915,722 bytes;
this storage change does not reduce export size or proof creation time.

The receiver retained the enforced four-CPU, PCLMUL, 8-GiB/no-swap profile. Its
highest sampled cgroup peak was 1,753,653,248 bytes (1.63 GiB), with zero memory-
limit or OOM events across both process lifetimes. Startup took 12.527 s and
restart 12.512 s. The 197.490 s scenario shut down successfully.
[Receipt records](receipts-daemons.json) retain all export hashes, older
receipt checks, local sizes, actual resource counters and source identities.
Reproduce with `scripts/live_v2_shared_receipts_scenario.py`, using the stopped
capacity fixture and its pruned contract-lifecycle ancestor in a new isolated
network namespace.

## Wallet discovery and a real GUI call

A separate copy continued the H85 receipt fixture. One participant funded a
watched counter at H86 and called it at H87; the other participant discovered
the funded successor through its own wallet. Discovery used one-entry pages
and a stable tip per query. Pending successor terms did not count as a balance.
A strictly heavier branch retained the original funded counter at H88. After
reorganization and restart, discovery again reported the original balance and
the orphaned successor as unfunded; the orphaned receipt was rejected.
All seven recorded discovery observations passed in a 149.060 s run.
[Discovery records](discovery-daemons.json) retain the terms, selected tips,
pagination observations, work comparison and exact source identities. This is
discovery of locally retained public terms, not a global contract registry.

The native GUI then opened that stopped H88 fixture in an isolated X11 display,
using the real node without mock RPC. Starting from saved orphaned successor
terms, the interface found and selected the restored funded predecessor. Its
review showed inclusion at H89, a 0.005800 NOID fee, a 9.994200 NOID successor
balance and counters `1, 0`. The GUI's confirm button authorized and submitted
the call. A normal external miner included it at H89. Selecting the candidate
balance then showed the funded successor. A separate read-only RPC check
confirmed the transaction and independently verified its 914,634-byte receipt.
Both public openings remained in the saved GUI library.

This inspection also exposed a display-only issue: while a contract RPC was
pending, the activation caption treated disabled controls as inactive rules.
The caption now checks protocol activation independently of the busy flag;
button authorization rules are unchanged. All 66 GUI tests passed. A rebuilt
GUI was checked against the same real H89 node: the waiting message appeared
without a false activation notice, and the terms loaded successfully. No new
block was mined during that caption check.

[GUI records](gui-daemons.json) distinguish the call-test binary from the later
caption-only build, and retain the source patch identity, test results, receipt
hash, selected State, and screenshot and log hashes. Both GUI/node runs stopped
successfully. The original H89 fixture is preserved for cold synchronization
and retirement-certificate qualification.

## Native production after both legacy classes

A separate native production run replayed the saved H1–H9 legacy fixture that
had used both B25 and B255, then accepted eight new Small blocks at H10–H17
with this bank. It used the real mempool, template builder, block prover, local
commit and inbound verifier. Both producer and receiver MDBX contexts reopened
at the exact H17 header and State root.

H11 funded all six templates. H12 made six contract calls and one ordinary
payment; H14 and H16 each made four calls and one payment. The intervening
heights checked that recurring and vesting payments could not execute early.
The final block followed closure of all test objects. Every terminal remained
below 1,100,000 bytes; observed sizes ranged from 912,948 to 916,916 bytes.

This was a standalone producer/receiver process using 12 proof and verifier
workers on the laptop. Legacy replay and setup took 184.473 s. The first v2
preparation took 67.515 s; subsequent preparations took 14.743–21.588 s.
These timings exclude separately measured wallet authorization, template
selection, PoW and inbound verify/apply. The complete run took 400.872 s and
peaked at 4,875,344 KiB RSS, including the prover and setup. These are not
constrained receiving-daemon measurements.

[Native production records](native-production.json) preserve the accepted
layouts, exact legacy-origin hash, executable identity, command, terminal
hashes and timing scopes. The executable was built with
`noid_chain/isolated-v2-fork-testnet`, as required by `joint-produce`. This run
uses the legacy matrices; qualification of certificates and binaries that omit
them is a separate stage.

## Certificates for retiring legacy matrices

Two new certificates bind the exact legacy predecessors to this bank. The
first uses the fresh daemon fixture's H9 origin, where only legacy B25 was
active. The second uses the H9 origin of the native H10–H17 production run,
where both legacy classes had accumulated obligations. For each live class,
preparation recomputed its preprocessing key from the authenticated canonical
legacy matrix and matched its independent release pin.

| Legacy classes with live obligations | Certificate bytes | Offline preparation | Certificate verification operation |
|---|---:|---:|---:|
| B25 | 7,353,789 | 457.203 s | 0.924 s |
| B25 and B255 | 15,093,850 | 2,522.807 s | 1.461 s |

Preparation used 12 workers on the laptop. The two-class run peaked at
20,924,676 KiB RSS. Its 20-GiB/no-swap cgroup reached its memory cap and
reclaimed memory; the saved observations show no OOM or killed process.
These are one-time certificate-generation requirements. This preparation was
not an 8-GiB receiving-node workload or recurring block production.

Both verification commands enforced four CPUs, PCLMUL, four Rayon workers,
an 8-GiB memory cap and no swap. The table's verification operation excludes
input loading and key initialization. The complete B25-only command took
7.213 s and peaked at 95,387,648 cgroup bytes.

The two-class verification command also authenticated the new matrices and
verified the H17 terminal without a legacy-row source. It then exercised
origin retention and disk restart, rejecting a missing certificate, old-format
transport without rows, and a tampered terminal. That complete command took
205.168 s, including 185.790 s for the first terminal verification and new
matrix setup, and peaked at 628,113,408 cgroup bytes. Both verification scopes
recorded zero memory-limit and OOM events. Receiving-daemon builds authenticate
their embedded matrices during compilation; their cold synchronization is
qualified separately from this standalone tool.

For scale, the two compressed legacy matrix files total 17,105,552 bytes;
the two preprocessing-key files total 608 bytes. A later node omits the old
matrix bytes and retains the authenticated certificate together with the new
matrices. These are individual artifact sizes, excluding database caches,
filesystem metadata and executable code.

The isolated `retired-history` executable built successfully, and the ordinary
isolated executable was restored afterward. [Certificate records](retirement-certificates.json)
retain both origin bindings, the precise bank and key pins, binary and file
hashes, commands, timing scopes and resource counters. The certificate tool
checks the ancestry against those inputs; a future mainnet origin certificate
requires the actual selected predecessor and has not been created here.

## Cold synchronization and certificate recovery

A stopped copy of the real H89 GUI fixture served its new B25-only origin
certificate. Both receiving nodes started with empty data and used ordinary
P2P snapshot bootstrap to reach the exact source tip. Each checked the active
contract limits, the GUI call's successor State and its portable receipt, and
retained the received certificate byte-for-byte.

| Receiving executable | Startup, sync and State/receipt checks | Cgroup peak bytes |
|---|---:|---:|
| Transition, legacy matrices embedded | 33.453 s | 1,561,047,040 |
| Retired, legacy matrices omitted | 35.131 s | 1,558,216,704 |

Both receivers enforced four CPUs, PCLMUL, 8 GiB and no swap. This is a local
fixture measurement, including startup and the checks above, rather than a
prediction for a seed synchronizing over a public connection. Neither
receiver copied a source database. The retired node materialized no legacy
matrix-cache files.

The retired node then restarted without a running provider and verified the
same State and receipt from its retained certificate. Three separate restarts
tested missing evidence, old-format evidence and a one-byte-corrupted
certificate while no provider was available. Each receipt verification failed
without changing the selected H89 tip. Reconnecting a provider recovered the
authenticated certificate and restored receipt verification at that same tip;
the logs confirmed recovery without a new block.

All six positive observations and all three offline rejection/recovery cases
passed. The six resource observations peaked at 1,579,155,456 bytes (1.47 GiB);
each recorded zero memory-limit or OOM events. The
209.488 s scenario shut down all nodes successfully.
[Cold-sync records](retired-sync-daemons.json) retain the source and binary
identities, actual enforced limits, exact tips, rejection messages, timings
and log hashes. Reproduce with `scripts/live_v2_retired_sync_scenario.py`,
the stopped GUI fixture and the certificate bound to its selected origin.

## Retired-node production after both legacy classes

A separate network continued the native H17 fixture using its certificate for
both legacy classes. An ordinary transition node funded a counter at H19 and
called it at H20. After 43 more real blocks, both peers agreed on H63 and the
old call body was pruned. Its portable receipt still exported successfully.

A fresh retired node obtained the 15,093,850-byte certificate over P2P and
reached the exact H63 tip through snapshot bootstrap. Startup and convergence
took 28.616 s. It then verified the old receipt and found the funded successor.
That receiving phase peaked at 623,550,464 cgroup bytes. This fixture differs
from the H89 GUI fixture above; its convergence timer stops before the
additional State and receipt checks.

After restarting in external-miner mode, the retired executable produced an
empty Small block at H64. A separately authorized call reached it through the
mempool, and its H65 block advanced the counter from one to two. The transition
peer accepted both blocks and independently verified the new receipt.

| Retired-node workload | Production and delivery | Cgroup peak bytes |
|---|---:|---:|
| Empty Small block, H64 | 81.614 s | 2,794,541,056 |
| One-call Small block, H65 | 86.566 s | 3,348,955,136 |

The node enforced four CPUs, PCLMUL, 8 GiB and no swap; its configured proof
pool used three workers. The external nonce worker ran separately with two
threads. Timings include template preparation, nonce work and peer acceptance.
These constrained production samples exceed the 30-second target; the earlier
AVX2 producer measurements have a different hardware and worker profile. The
peak after H65 is cumulative over that producer process lifetime.

No legacy matrix-cache files appeared. After all providers stopped, the
retired node restarted at the exact H65 tip and verified both old and new
receipts. Its offline startup took 10.510 s. All four recorded resource samples
had zero memory-limit or OOM events, and every process stopped successfully.
The complete scenario took 1,423.569 s, including the 43-block pruning interval.

[Retired-production records](retired-mining-daemons.json) preserve both calls,
all six receipt checks, accepted tips, actual resource limits, worker count,
source identities and log hashes. Reproduce with
`scripts/live_v2_retired_mining_scenario.py`, the stopped native H17 fixture
and its exact two-class retirement certificate.

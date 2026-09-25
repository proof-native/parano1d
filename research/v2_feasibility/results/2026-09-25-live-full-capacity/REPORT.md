# Full candidate blocks through real daemons

The Small P63/I504/C63 and Large P255/I1020/C26 candidate passed real wallet,
mining, P2P, verification, State application and receiver restart checks.
The bank was
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
These results qualify this candidate; they do not freeze release capacities.

The run continued copies of the stopped H64 contract-lifecycle fixture. Every
new input and transaction came from the wallet RPC. No State, block or header
was injected. Large mining was enabled explicitly on the producer. A block
containing 63 contract calls correctly selected Small, because this candidate's
Large class supports only 26 calls.

## Observations

| Workload | Template preparation | Receiver terminal verification | Following suffix application |
| --- | ---: | ---: | ---: |
| H65: 63 ordinary one-input/two-output payments | 22.777 s | 2.845 s | 0.303 s |
| H66: 63 persistent-counter calls | 20.517 s | 2.731 s | 2.668 s |
| H70: 255 ordinary one-input/two-output payments | 54.645 s | 6.848 s | 1.441 s |
| H71: 63 ordinary payments immediately after Large | 28.267 s | 5.851 s | 0.753 s |
| H72–H79: eight empty Small blocks, range | 18.716–32.580 s | 2.733–3.613 s | 0.479–0.620 s |

The receiver's cgroup peak was **1,892,683,776 bytes (1.763 GiB)**, including
database and wallet work. Before Large it peaked at 780,091,392 bytes.
All memory-limit and OOM event counters remained zero. It reopened the same
H79 tip after restart. Both nodes exited successfully.

The first Small successor still paid an extra carried-claim evaluation cost.
The following eight Small blocks returned to the earlier verification range;
the exact verified-claim cache retained the unchanged Large claim. This result
does not eliminate the cold-cache cost after a restart or a different claim.

All 63 independent counters advanced from zero to one. The receiver found all
63 resulting balances and independently verified receipts for the first and
last calls. The call-block suffix application includes retaining separate full
receipts in the measured binary. A later local shared-proof storage change was
not present in this run.

## Measurement limits and reproduction

One shared Intel Core i7-1365U host ran both daemons. The producer used eight
logical CPUs in its affinity set, six proof workers and AVX2+VPCLMUL. The
receiver used four logical CPUs, four workers, forced PCLMUL, an 8 GiB cgroup
limit and no swap. External PoW used two threads. No build or other proof
benchmark overlapped this run. Logical affinity sets on this laptop are not
independent physical servers.

Template preparation excludes wallet authorization, PoW and delivery. Terminal
verification excludes subsequent State application and wallet retention. Each
full workload is one observation, not a latency distribution. In particular,
Large preparation exceeded the 30-second target on this host. These measurements
do not establish Large profitability or an AVX-512 performance claim.

Build `isolated_v2_node` with the stated bank and published legacy pins, plus
`parano1d-miner`. Run `scripts/live_v2_capacity_scenario.py` in a loopback-only
network namespace with `NOID_V2_CAPACITY_SOURCE` pointing to the completed,
stopped contract-lifecycle fixture and `NOID_V2_LIVE_DIR` to a fresh destination.
It copies the isolated data, prepares independent inputs through ordinary
wallet sends, and requires a user systemd manager for the receiver limits.

The binary source base was `73bebb04`; the script records exact binary and
source-file hashes. [Machine-readable measurements](measurements.json) include
every workload, resource samples, timings, candidate budgets and final tip.

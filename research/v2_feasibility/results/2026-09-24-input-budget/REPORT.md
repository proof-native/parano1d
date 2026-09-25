# m23 with 96 pages and a shared input budget — 2026-09-24

The complete recursive 96-page candidate fits m23 when the block-wide input
budget is 384. A page still permits eight inputs and two outputs. This is an
explicit research bank, not a mainnet parameter selection. The contract core
retains its sixteen call slots and eight instructions per call.

## Matrix and consensus isolation

The frozen relation uses **8,166,783 of 8,388,608 positions**, leaving 221,825
positions. The earlier 96-page layout reserved 768 inputs and needed 8,839,013
positions. Reducing the input budget saves 672,230 positions, including a
589,824-position reduction in the committed Merkle-path region.

The compressed matrix is **9,511,133 bytes**. Matrix digest:
`53feb8306a2b00ab63b3f49f290138c50d312ba4bca660493e7c38432e483719`.
Bank digest:
`eed8ac63ae5833bf51cb527504db94d8cd112187b33048e56a80850e5e456795`.

The reduced budget is enforced by native preparation and by the R1CS integer
input sum, independently of touched-slot occupancy. It is included in the
bank identity and the `O1V2PT03` compact recipe. Existing `O1V2PT02` candidate
recipes retain their original geometry and identities. Authorization bindings
consume the allocator's exact geometry, including the input budget.

The run reverified the actual legacy chain through its B255 block at height
nine, including both accumulated legacy matrix obligations. It then proved
and verified 29 candidate blocks through height 38. v1.1 activates at height
five and the candidate at height ten in this isolated fixture. The target
interval of 30 seconds is a measurement setting.

A second run starts from a verified legacy **B25** tip. Its first candidate
block and recursive successor also pass, and independently freezing that
boundary produces the same matrix and bank identities above.

## Construction measurements

Host: Intel Core i7-1365U, twelve logical CPUs, ten physical cores, roughly
31 GiB RAM, AVX2 + VPCLMULQDQ. Release build, twelve Rayon threads. One benchmark
ran at a time, without concurrent compilation or tests; the host remained a
normal desktop. Each ordinary payment in this table uses one input and one
output. Every contract call exercises all eight instructions.

Three separate blocks were measured for each case:

| Pages | Calls | Input preparation + assembly + proving, p50 | Range | Terminal verification, p50 |
|---:|---:|---:|---:|---:|
| 0 | 0 | 22.741 s | 18.110–23.652 s | 0.871 s |
| 4 | 0 | 18.231 s | 17.335–23.201 s | 0.856 s |
| 96 | 0 | 27.835 s | 21.267–28.026 s | 1.210 s |
| 96 | 1 | 21.401 s | 20.954–27.678 s | 0.855 s |
| 96 | 4 | 27.910 s | 21.348–28.175 s | 1.211 s |
| 96 | 16 | 21.977 s | 21.397–28.160 s | 0.947 s |

Construction excludes client authorization generation, fixture-template work,
nonce search, encoding, verification and State application; raw records keep
those phases separate. The initial boundary block took 15.904 s, so its cost
must not stand in for ordinary recursive blocks. Timing varies substantially
and does not establish a linear cost per call or a tail-latency bound.

The full sixteen-call recurrent witness rebuilt the exact frozen matrix and
satisfied it. Terminals remained approximately 0.93 MB. Peak process RSS before
the distributed-State cases was 4,176,012 KiB, approximately 3.98 GiB.

## Resource boundaries

All notes were created and spent by actual authorized blocks. No State root
or balance was injected into the fixture. The two boundary blocks below both
touch **all 256 segments** of the depth-24 fixture:

| Height | Pages | Inputs | User outputs | Construction | Native State application |
|---:|---:|---:|---:|---:|---:|
| 34 | 96 | 384, four per page | 192 | 27.358 s | 335.832 ms |
| 37 | 48 | 384, eight per page | 96 | 19.787 s | 166.914 ms |
| 38 | 0 | 0 | 0 | 20.884 s | 3.085 ms |

Each boundary case is one sample. The empty successor retains the preceding
recursive obligation and also verifies successfully. The complete process
peaked at **5,377,064 KiB, approximately 5.13 GiB**, including matrix freezing,
client proofs, prior blocks and distributed State. This is not a fresh
receiver's memory requirement.

A separate invalid block contains 385 inputs and 96 outputs: its 482 touched
slots still fit the allocation of 577. Native preparation rejects it. Preparing
its otherwise valid witness under the original wider budget and assembling it
against the bounded runtime produces the **same matrix digest and an
unsatisfied witness**. The invalid block is never applied to the chain.
Six altered budget/recipe/registry combinations are rejected. The first two
terminals also each reject 138 structural and public-input mutations.

## Four-core receiver with State

A separate process used the **PCLMUL backend**, four Rayon threads and CPU
affinity `0,2,4,6`: four distinct physical laptop cores, two P cores and two
E cores. A live systemd scope enforced **8 GiB memory and zero swap**. This
does not reproduce the exact processor or virtualization of a seed server.

For each saved block, the receiver authenticates its endpoint, reconstructs
its parent State through checked fixture bodies, then measures five complete
terminal verifications and native State applications. Each sample starts from
the same parent; a separate warm-up is excluded. Columns are independent
medians and therefore need not add exactly.

| Height / workload | Proof verification, p50 | State application, p50 | Combined, p50 | Combined range |
|---|---:|---:|---:|---:|
| 30 / 96 pages, 16 calls | 3.288 s | 0.072 s | 3.352 s | 3.336–3.378 s |
| 34 / 384 inputs, 192 outputs, 256 segments | 3.330 s | 0.374 s | 3.706 s | 3.660–3.717 s |
| 37 / eight inputs per page, 256 segments | 3.352 s | 0.240 s | 3.586 s | 3.574–3.607 s |
| 38 / empty successor | 3.372 s | 0.005 s | 3.377 s | 3.327–3.410 s |

All samples passed. Peak process RSS, including both legacy matrices during
setup and the distributed parent State, was **approximately 1.73 GiB**.
Post-sample RSS was about 431 MiB for the mixed block and 1,654–1,752 MiB for
the later distributed-State cases. These results leave memory headroom inside
the enforced limit. They cover proof verification and in-memory State work;
persistent database, retained fork history, RPC and network load still need
daemon measurements.

The separate initial authentication phase took **779.959 seconds**. The
runner fully checks supplied canonical matrix contents, including both old
matrices for this B255 origin. This is not the packaged-node startup path:
production has a separate build-authenticated artifact loader, and v2 release
packaging remains to be integrated. Fixture-State reconstruction is also
reported separately from each accepted block's application.
[Raw receiver records, live resource limits and source hashes](receiver-pclmul-state.json)
preserve the measurements.

## Interpretation and remaining qualification

The [earlier 63-page candidate](../2026-09-24/REPORT.md) remains a comparison
point. At an illustrative 30-second interval, single-page capacity is 3.2
transactions/second for 96 pages and 2.1 for 63 pages. These are arithmetic
ceilings. The higher capacity also costs more proving work; these measurements
do not establish a safe target interval or a free throughput improvement.
On this laptop the slow full-block observations leave little margin within
30 seconds for the miner's later PoW phase.

Current evidence covers the fixed contract core and the depth-24 State
fixture. Qualification still requires deeper State, sustained workloads,
epoch boundaries, reorgs, restart/cold sync, and the actual wallet, node and
pool interfaces. The legacy-origin verifier still uses authenticated old
matrices. Capacity and interval remain joint decisions.

The recursive and block library tests passed **417 tests**, with three
pre-existing ignored tests. Rebuilding the legacy B25 relation produced its
unchanged 4,185,273 positions, a satisfied witness and published digest
`9be6ed118dc0df56d670161569dc8283a2ed5b441bb9476911ead230968b3d8b`.
Both compressed legacy artifact pins also remain unchanged.
The rebuilt receiver also accepts the original 63-page `O1V2PT02` recipe and
its saved full sixteen-call proof under the original external bank pin.
[B25 transition and regression evidence](b25-transition-and-regressions.json)
records these checks. [Raw mining records and source/binary hashes](mining.json)
include the complete sequence. The exact original prover source delta and
binary are archived beside the generated matrix; subsequent harness changes
add receiver State measurements without changing the frozen relation.
[Reproduction instructions](../../CAPACITY_MEASUREMENTS.md) describe both modes.

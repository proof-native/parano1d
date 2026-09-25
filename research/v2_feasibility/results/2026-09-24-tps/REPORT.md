# Independent-payment capacity measurements — 2026-09-24

A complete m23 candidate fits **112 independent one-page payments**, with one
input and two outputs per payment, and retains the existing contract core in
the matrix. Three full blocks from different owners were proved, verified and
applied to distributed State, followed by an empty recursive successor.
This increases measured page capacity by **16.7%** over the 96-page candidate.
The 112-page / 384-input bank is retained as the current working candidate.
It does not establish a doubling of throughput or select a release, activation
height or block interval.

The ordinary wallet → miner → block flow, before-fork B25/B255 rules and the
live node's activation schedule are unchanged.

## Full relation and capacity boundaries

The row budget is 2^23 = 8,388,608. The input budget is block-wide; each page
still permits up to eight inputs. Different budgets are different authenticated
research banks, not runtime choices within a frozen bank.

| Pages | Input budget | Complete recursive positions | m23 result |
|---:|---:|---:|---|
| 96 | 384 | 8,166,783 | Previous measured candidate |
| 112 | 384 | 8,350,239 | Fits, 38,369 positions spare |
| 120 | 384 | 8,428,375 | Exceeds by 39,767 |
| 127 | 256 | 8,422,615 | Exceeds by 34,007 |
| 127 | 384 | 8,471,819 | Exceeds by 83,211 |

For 223 pages / 384 inputs, even the **direct** relation occupies 13,995,339
positions. The m23 attempt therefore fails while deriving the committed slices,
before a complete recursive relation can be assembled. This count must not be
reported as a full recursive matrix or as a proof measurement.

## Genuine independent payments

The fixture begins with a verified legacy B25 parent at height nine, activates
the candidate at height ten, creates funding notes, then assigns one note to
each of 112 distinct owners. Each payment spends its owner's note, sends one
output to another owner and retains change. Authorizations are generated
independently, with fresh proof randomness. No authorization is reused between
live transactions. The full blocks touch 254, 240 and 240 State segments.

The first full block rebuilds the complete recursive matrix, checks equality
with the frozen matrix and checks witness satisfaction. Independent transition
and payment runs produce byte-identical compressed matrices and runtime
recipes. The fork and first successor also reject 138 terminal mutations each.

Host: Intel Core i7-1365U laptop, twelve Rayon workers, AVX2 + VPCLMUL backend.
No compiler or other benchmark ran concurrently. Three interleaved samples
per occupancy are exploratory, not a p95 or a server prediction.

| Workload | Construction p50 | Observed range | Verification p50 | Native State p50 |
|---|---:|---:|---:|---:|
| 2 payments, 1 input / 2 outputs each | 19.768 s | 18.312–23.073 s | 0.949 s | 9.976 ms |
| 112 payments, 1 input / 2 outputs each | 22.601 s | 22.580–23.527 s | 0.864 s | 343.393 ms |

Construction means input preparation + recursive assembly + proving. PoW,
wallet proof creation, fixture construction, wire encoding and State application
are separate phases. The fixed matrix explains why lightly occupied blocks
still require substantial proving work. Differences between occupancy medians
must not be interpreted as a controlled speedup; temperature, scheduling and
three-sample variance remain relevant.

The whole process peaks at 5,141,316 KiB RSS, including setup and all preceding
blocks; this is not the memory required by a fresh receiver. The empty H22
successor constructs in 19.119 s, verifies in 0.823 s, and has a 931,540-byte
terminal. Its State application takes 3.993 ms.

The 112-page matrix retains sixteen contract calls with up to eight instructions
per call. This payment sequence contains **no live contract calls**. Maximum
input occupancy and mixed contracts need qualification for this bank; those
measurements from the 96-page bank do not transfer automatically.

## Receiver and terminal size

A separate process used CPU affinity `0,2,4,6` (four distinct physical laptop
cores), four Rayon workers, the forced `pclmul` backend, an enforced 8 GiB memory
limit and no swap. Each height has one warm-up and four measured repetitions.
This is a constrained laptop measurement, not an actual seed-server benchmark.

| Workload | Verify + State p50 | Observed range | Proof verification p50 |
|---|---:|---:|---:|
| Full H17, 112 payments / 254 segments | 3.590 s | 3.503–3.670 s | 3.079 s |
| Empty H22 successor | 3.179 s | 3.152–3.298 s | 3.175 s |

Full-block State application alone has a 511.615 ms median. The process peaks
at 1,890,180 KiB RSS (1.80 GiB), including matrix authentication and fixture
replay. The separate matrix/origin authentication phase takes 324.999 seconds;
replaying each parent State is also outside the per-block timing. This harness
authenticates supplied matrix bytes; it does not measure packaged node startup,
database I/O, networking or sustained catch-up under concurrent traffic.

The full-payment terminals measure **929,236–932,692 bytes**. The runtime-derived
unshared terminal bound is **1,032,212 bytes**, leaving **67,788 bytes** under the
current **1,100,000-byte** transport cap. The body occupies 36,716 bytes. H17's
body plus terminal totals 969,408 bytes, or 969,420 bytes with accepted-bundle
framing. These fit the existing byte budgets.

The production bundle constructor nevertheless rejects both saved terminals
with `UnsupportedVersion { actual: 6 }`: the node currently admits versions
4/5, while the research harness uses the direct v2 decoder. Byte compatibility
does not establish production P2P, storage, RPC or synchronization support.
The [network audit](../../NETWORK_BUDGETS.md) records the other admission limits.

## What occupies the budget

The authorization axis includes the primary coinbase and rounds up to a power
of two. Up to 127 user pages it has 128 tiles; at 128 user pages it becomes 256.
The current four authorization table families occupy the following positions:

| Family | Formula per tile | 128 tiles | 256 tiles |
|---|---:|---:|---:|
| Owner transcript | 6 × 128 | 98,304 | 196,608 |
| Main transcript | 6 × 256 | 196,608 | 393,216 |
| Wallet leaf/transcript hashes | 6 × 2,048 | 1,572,864 | 3,145,728 |
| Wallet Merkle paths | 9 × 1,024 | 1,179,648 | 2,359,296 |
| Authorization algebra and bindings | 16,536 | 2,116,608 | 4,233,216 |
| **Subtotal** | | **5,164,032** | **10,328,064** |

Sources: [geometry](../../../../noid_recursive/src/region_sidecar/block.rs),
[authorization row ledger](../../../../noid_recursive/src/acceptance/trace/zk_authorization_candidate.rs),
[Phase-B algebra](../../../../noid_recursive/src/acceptance/trace/zk_phase_b_composition.rs).

This subtotal excludes State, body constraints, fees, contract execution,
recursive verification and alignment. Thus merely reducing input counts,
changing transaction serialization or optimizing the small execution core
cannot make 200+ independent authorizations fit this m23 relation.

The expensive work must remain proved. Moving calculations into an internal
batched relation would require complete input/output bindings, transcript and
opening checks, recursive verification, and accounting for its terminal bytes.
It is not enough to delete outer constraints or count only the smaller trace.

Phase-B algebra alone costs 13,926 × 256 = 3,565,056 positions. Removing even
that entire cost for free would not make the current 223-page direct relation
fit m23. A viable larger design needs to reduce both authorization tables and
algebra. Longer internal hash chains can reduce stored intermediate states,
but add proof layers and verifier work. These are unimplemented design avenues,
not measured gains or selected changes. No query count, challenge width or
ownership check was weakened in this investigation.

## Scope and remaining decisions

At 112 independent one-page payments, nominal capacity is 3.73 / 3.11 / 2.80
transactions per second for hypothetical 30 / 36 / 40-second intervals.
These are page-count divisions, not observed network throughput. No interval
has been selected. Proving, PoW, propagation and receiver load must fit together.

The current result is a reproducible candidate and a cost breakdown. It is
not a production-ready v2 release. Full mixed-contract cases, distributed input
boundaries, sustained node traffic, both legacy parents, reorgs, restart, cold
sync, mining interfaces and exchange-facing RPC remain part of qualification.
The live decoder also does not yet admit the research v2 terminal version;
see the [network budget audit](../../NETWORK_BUDGETS.md).

## Reproducibility

The [raw measurements](measurements.json) retain every timed sample, both freeze
passes, mutation results, failed-shape ledgers, binary hashes and candidate pins.
The compressed matrix is 9,951,178 bytes. Its digest is
`f4d1c8bd58e1d510483bf716d4d245f8f6b911d5b760607322eaa44a9a64e7a6`;
the candidate bank is
`57a104e68cc9b26b7ff1f147dbc4ba9d7c4d7420b773ba1f4617fb01d0fdc3b9`.
This fixture's H10 activation and 30-second spacing are part of that research
bank identity. A different schedule requires rebuilding the candidate.

Use the [measurement guide](../../CAPACITY_MEASUREMENTS.md) for build, proof and
receiver commands. Larger matrix artifacts remain outside the source tree.

Release library checks passed four block tests and 413 recursive tests on the
first run; one existing registry negative exposed the new research-tier alias
described below. After correction, all five allocator/registry checks passed,
including that negative and the legacy B25/B255 certificates: **418 distinct
checks successful** across the full run and focused rerun, with three ignored.
Formatting and diff checks also pass.

Some research tiers share B255's table dimensions. The legacy registry
constructor now explicitly allows only tiers 25 and 255; it can no longer
infer permission for a research tier from dimensions alone. The 112-page
object constructor and measured matrix are unchanged by this correction.

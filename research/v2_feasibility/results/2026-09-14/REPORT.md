# Initial v2 contract measurements

**Ignotus Nemo** · 14 September 2026 · Research record 01

These results justify a recursive-integration experiment, not an architecture
selection or a claim that contracts fit the production budget. The proposed
bytecode and object formats remain hypotheses.

## Environment and boundaries

- Source: `edb998f745ca5092fee1abd4cf52eabddcbfc9e5`, with unchanged production
  files and a separate measurement harness.
- Intel Core i7-1365U, 12 logical CPUs, 31 GiB RAM, AVX2+VPCLMUL backend.
- Rust 1.96.0 / Cargo 1.96.0. Optimized thin-LTO, one-codegen-unit builds.
  Research dependency versions were seeded from the production Cargo.lock.
- HistoryStep uses its production CPU pool. The standalone harness requests
  12 Rayon threads and uses the libraries' native prover/verifier entry points.
- No benchmark ran concurrently with another benchmark or compiler. The
  desktop and browsers stayed open. Thermal/power state was not controlled.
  These are exploratory same-host samples, not production latency tails or a
  regression comparison against older published measurements.
- B25: three samples. B255: one sample, no wire audit. Proxy and capsule cases:
  one unmeasured warm-up followed
  by three samples. Repeated proxy samples use the same deterministic relation.
  Proving includes the configured transcript grinding. Small-case timing is
  not a linear measure of arithmetic-operation throughput.
- Peak RSS is the entire process high-water mark, including fixture/matrix
  setup and warm-up. It is not a steady-state full-node requirement or an
  incremental contract memory cost.

## 1. Existing production HistoryStep

| Case | User pages | HistoryStep | Terminal verification | Wire bytes |
|---|---:|---:|---:|---:|
| B25 after B25, median of 3 | 0 | 13,917 ms | 848 ms | 872,724 shared / 971,732 expanded |
| B255 after B25, one sample | 26 | 51,395 ms | 2,137 ms | 1,081,108 expanded only |

B25 construction ranged from 13,898 to 14,260 ms. Verification ranged from
793 to 866 ms. Process peak RSS: 2,358,228 KiB (about 2.25 GiB).
The wire audit verified both encodings, exact round trips, eight fork-format
checks and 530 malformed-input rejections per audit. Audit work is outside the
reported proving interval. [Raw B25 output](b25-baseline.txt).

B255 construction included 751 ms of input preparation, 13,013 ms of staged
assembly and 37,625 ms of proving/encoding. Its entire process peaked at
5,486,268 KiB (about 5.23 GiB). The 7:19 process duration includes fixture and
matrix setup and is NOT block construction time. This case verifies the
expanded terminal only. No shared-encoding measurement is inferred from it.
[Raw B255 output](b255-baseline.txt).

These fixtures are neither a matched-occupancy comparison nor saturated-class
tests: B25 has zero user pages, B255 has 26, not 255. They establish the cost of
these exact existing paths, not a per-transaction slope or the cost of a future
interpreter. This single B255 sample cannot establish latency tails.

The decisive structural observation is **4,185,273 useful rows out of
4,194,304**, leaving **9,031 rows, 0.2153%**. That is raw padding slack, not a
guaranteed budget for a new verifier. A new relation may need to remove other
work, use a different representation or change geometry. It is not valid to
assume that a small extra application means a small extra block cost.
B255 used 16,360,489 out of 16,777,216 rows, leaving 416,727 (2.4839%) raw
padding rows under the same caveat.

Construction excludes wallet proving and fixture setup, nonce search and
network propagation. The existing 20-second block target includes production
and propagation, so subtracting 13.917 from 20 does NOT give a free contract
budget. No source change was made to the existing mining policy.
That policy only permits B255 automatically when the session's first completed
B25 preparation sample times four is at most 20 seconds. A session with these
B25 timings would not qualify. Forcing the benchmark's B255 path does not mean
the live network forces this laptop to produce B255 blocks. See
[session capacity policy](../../../../docs/architecture/mining.md).

## 2. Existing independent wallet capsules

Medians across three samples. Each participant's proof is built sequentially
on this one laptop for this diagnostic. It is not a multi-owner transaction.

| Capsules | Sequential proving | Decode + native verification | Authorization bytes |
|---|---:|---:|---:|
| 1 | 796.5 ms | 25.71 ms | 89,316 |
| 4 | 3,142.5 ms | 101.99 ms | 358,480 |
| 16 | 15,619.1 ms | 435.69 ms | 1,431,872 |

Separate owners could prove concurrently on their own hardware. Do not charge
the sequential client total to block construction. Native admission and wire
cost still need accounting, while recursive inclusion is NOT measured here.
The 16-capsule authorization payload alone would take at least 1.15 seconds on
one ideal 10 Mbit/s link, before framing, RTT, gossip fanout or other payloads.
That is a byte-rate calculation, not a network measurement.

Every case rejected a changed recipient. Exact sizes vary with randomized
capsule openings and are retained in each JSON sample:
[one](wallet-1.json), [four](wallet-4.json), [sixteen](wallet-16.json).

## 3. Closed native C1 hash-heavy proxy proofs

These use 133 queries, rate 1/4, 32-lane initial leaves and wide C1 challenges.
Each copy is an independent chain of full Poseidon2b permutations through the
ordinary field-R1CS gadget. This deliberately excludes FROST-GKR batching.

| Permutations per copy x copies in ONE proof | Useful / padded rows | Prove median | Verify median | Commitment + expanded proof | Process peak RSS |
|---|---:|---:|---:|---:|---:|
| 8 x 1 | 2,889 / 4,096 | 134.7 ms | 19.9 ms | 212,396 B | 9.7 MiB |
| 64 x 1 | 23,049 / 32,768 | 111.4 ms | 28.8 ms | 245,676 B | 40.3 MiB |
| 64 x 4 | 92,193 / 131,072 | 352.0 ms | 86.4 ms | 287,660 B | 145.5 MiB |
| 64 x 16 | 368,769 / 524,288 | 688.6 ms | 301.5 ms | 384,796 B | 543.4 MiB |
| 512 x 1 | 184,329 / 262,144 | 289.9 ms | 140.5 ms | 367,740 B | 290.8 MiB |
| 1,024 x 1 | 368,649 / 524,288 | 641.2 ms | 297.1 ms | 384,796 B | 532.9 MiB |

All rows are backed by `substrate-*.json` with raw samples, geometry, setup
costs, serialized bytes and memory. Each case accepted honest proofs, rejected
a changed proof and wrong transcript domain, and detected a corrupted witness.

Setup is material: for 64 x 16, matrix/witness construction was 488.5 ms and
shape preparation was 3,554.8 ms, separately from the 688.6 ms warm proof.
Those costs cannot be ignored for an uncached program or fresh verifier.

Batching the public proxy computations gave sublinear proof-byte growth: the
16-copy proof was about 1.57 times the single-copy proof, not 16 times. This
does NOT implement aggregation of independent secret-bearing owner proofs.
Nor does a 111 ms native proof imply adding 111 ms to HistoryStep: recursive
verification has a different constraint and authentication cost.

The prover inputs and outputs are fixed in each reference matrix. There is no
program-authenticated interpreter, arbitrary matrix/VK admission, integer
contract semantics, State witness or recursive inclusion in this experiment.
Generic C1 proofs are not asserted to hide arbitrary contract witnesses.
These measurements are neither lower bounds nor a forecast for finished v2.

## Decision after this stage

Proceed with research, **do not freeze the contract ABI or select bytecode**.
The first decisive next test is the same tiny contract under two candidates:
a bounded fixed interpreter and an authenticated compiled-circuit route. Each
must actually bind program identity and public effects, close every deferred
matrix/PCS claim, and be included in a recursive HistoryStep prototype.

Measure zero calls, one call, then multiple calls and mixed parent/child
classes. Independently test public-data availability, hot-object contention
and admission under invalid-proof load. The full gate list is in
[EXPERIMENTS.md](../../EXPERIMENTS.md).

Feasibility for the existing 20-second network cadence remains **unestablished**.
No consensus rules, release pins, query counts, certificate or production docs
were changed by this stage of the experiment.

## Artifact identities

SHA-256:

```text
harness src/main.rs
4840534a410a427e7f004587044d47542286f5f749946f95201b46e68c36272c
research Cargo.lock
6281e1402e420dd3f0bbb4ea4a7c85b3d8292e4241314c43d154d3ad62e1f79c
production Cargo.lock
3df8edf9c09ebafb07ab506cfae7febb6b607033996854e0caf3bb0823abecfc
harness executable
02fbdfca2f6037181f3f7bd60fa757d4d695003cf35d572e6043a046d1d5f31f
existing HistoryStep benchmark executable
5111192915f42081f902031d2574249417daf12167788f52d7ccf7cc5f0ba732
```

The existing HistoryStep benchmark was rebuilt with
`cargo bench --locked -p bench_prover --bench history_step_proof --no-run`.
Its runtime pack was `parano1d-artifacts/mainnet-v1/history-step-pack-v1`.
The runtime metadata release digest was
`ad463bd76e27df3c0f414f4fd7640cfb5c45cc7f3a09e44a2f7bc7a8b869485b`.
See [performance methodology](../../../../docs/reference/performance.md)
for pack initialization and the timing boundary.

The research crate's two unit tests and formatting check passed. The nine JSON
records were parsed and checked for sample counts and expected C1 query counts.

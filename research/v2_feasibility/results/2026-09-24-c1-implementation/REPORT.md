# C1 implementation measurements — 2026-09-24

Precomputing repeated matrix-column weights reduced complete verification of
the same saved m23 terminal from **3.333 s to 2.855 s**, a **14.34% reduction
in elapsed time** in this run. The zerocheck folding change from experimental
commit `3859e7e7` reduced copying and retained buffers, but did not establish
a speedup for complete block proving.

The baseline is `0bf546ec342cb473914e31e6913d9739d91f0aa0`. Only two proof-core
implementation files change. Neither capacity nor block interval is selected
by this work.

## Complete terminal verification

The before and after processes read exactly the same saved H11 terminal from
the 96-page, 384-input m23 candidate after a legacy B25 parent. H11 is an empty
recursive successor. Both processes perform bounded decoding and full proof
verification, including fresh and accumulated matrix claims. State application,
database, networking and daemon load are outside this measurement.

Host: Intel Core i7-1365U laptop. CPU affinity `0,2,4,6` selects four distinct
physical cores, with four Rayon workers and the forced `pclmul` backend. Each
process runs under an enforced 8 GiB memory limit with no swap. This is not an
actual seed server. One benchmark runs at a time, without compilation or tests.
Each binary has one warm-up followed by seven measured verifications.

| Measurement | Before | After |
|---|---:|---:|
| Complete verification, p50 | 3,332.522 ms | 2,854.637 ms |
| Observed range | 3,253.805–3,369.583 ms | 2,817.773–2,915.081 ms |
| Core proof protocol, p50, excluding final matrix evaluation | 337.193 ms | 329.937 ms |
| Process peak RSS, including setup | 622,244 KiB | 621,940 KiB |
| Separate matrix/origin authentication | 301.909 s | 296.983 s |

All verifications passed. Initial authentication checks the supplied canonical
matrix contents and is reported separately; this harness does not measure the
packaged node's build-authenticated startup path. The RSS peaks are dominated
by setup and do not establish a reduction in verifier memory.

The optimization caches the product of the 64-entry fresh-claim prefix and
the low equality table. A column lookup then needs one fewer extension-field
multiplication per matrix entry. The m23 low table grows from 256 to 16,384
field elements, adding **504 KiB** of table payload, shared by the workers.
It is rebuilt for each claim and does not reuse values from another proof.

## Zerocheck folding

The previous implementation creates a temporary vector of pairs, copies it
back serially, and retains both original buffers. The new implementation writes
directly to two half-sized buffers and releases the previous allocations.

The ignored A/B benchmark uses the same deterministic, satisfiable m23 input
and alternates the order of the old and new implementations. It measures seven
pairs after warming both implementations, and checks byte-identical proofs
and the following Fiat–Shamir challenge on every run.

| Host configuration | Old folding, p50 | New folding, p50 | Retained buffers after first fold |
|---|---:|---:|---:|
| 12 threads, AVX2 + VPCLMUL | 3.198 ms | 2.091 ms | 8 MiB → 4 MiB |
| 4 physical cores, PCLMUL | 4.414 ms | 3.525 ms | 8 MiB → 4 MiB |

These are folding-phase timings, not block construction times. Complete
zerocheck medians were 218.212/222.241 ms before/after on twelve threads and
428.785/435.908 ms on four cores, with overlapping ranges. No complete-prover
speedup is claimed. The 4 MiB saving applies to these two retained buffers,
not to the entire proving process or its peak RSS.

The new fork and successor proofs provide a full-prover phase check:
zerocheck took 208.5 and 199.0 ms out of 11.793 and 11.856 seconds of proving.
Most of those proving times were PCS commitment and post-commit auxiliary
proofs. These two empty-block samples are compatibility checks, not a
controlled before/after block-proving benchmark.

Reproduce the A/B comparison without running another CPU workload:

```sh
RAYON_NUM_THREADS=12 NOID_C1_FOLD_BENCH_M=23 NOID_C1_FOLD_BENCH_SAMPLES=7 \
  cargo test --locked --release -p noid-ivc-core --lib bench_c1_direct_folding \
  -- --ignored --nocapture --test-threads=1
```

## Correctness

Release library qualification passed **873 tests**: 454 core, two prover,
four block and 413 recursive tests; fifteen tests were ignored. Added checks
compare complete zerocheck proofs and transcripts against the previous
implementation, exercise buffer release, compare prefixed weights against
dense weights, and compare compact/resident matrix evaluation across all claim
combinations. Existing malformed-proof and recursive replay tests also pass.

Independently rebuilding the complete candidate still produces 8,166,783
positions and matrix digest
`53feb8306a2b00ab63b3f49f290138c50d312ba4bca660493e7c38432e483719`.
The compressed matrix, runtime-parts recipe and H10/H11 block bodies are
byte-identical to the previous artifacts. The bank remains
`eed8ac63ae5833bf51cb527504db94d8cd112187b33048e56a80850e5e456795`.
Only timing and memory observations differ in the generated pins JSON.

Both newly proved H10/H11 terminals pass the archived **old verifier**; the
optimized verifier passes the **old H11 terminal** used in the A/B measurement.
The new run also rejects 138 mutated terminals at each height. Independently
generated complete terminals may differ because wallet ghost authorization
uses fresh randomness. Exact proof-byte parity is tested on the controlled
zerocheck inputs, not asserted across those randomized terminal constructions.

[Raw measurements, source/binary hashes, live resource limits and artifact
comparisons](measurements.json) preserve the evidence. The
[capacity harness instructions](../../CAPACITY_MEASUREMENTS.md) describe the
saved-terminal receiver and transition modes. These implementation changes
preserve the proof format, transcript rules and frozen matrices; they do not
require a separate consensus activation.

The experimental branch also reviewed
[round batching for sumcheck](https://www.cs.cmu.edu/~csd-phd-blog/2026/sumcheck-proving/).
That is separate research, not an implemented speedup in this patch. Its
reported improvements must not be applied as a multiplier to NOID block times.

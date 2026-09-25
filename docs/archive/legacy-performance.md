# Historical performance — v1 / v1.1

This page preserves measurements from the legacy profiles. Current v2
measurements are in [Performance](../reference/performance.md).

Performance belongs to one source revision, proof profile, authenticated matrix
pack, build profile and host. It is not a consensus constant and cannot be
inferred from core count alone.

The historical construction measurements below use Parano1d revision
`39626b22d53cf2f2c480a7e28446c197dca68043`, the production C1 profile and the
authenticated B25/B255 matrix pack. They predate v1.1 shared-path encoding.
They remain a hardware baseline for later capacity experiments, including v2;
new proof shapes and complete block production still need their own measurements.

| Host | Class | `HistoryStep` construction | Statistic |
|---|---|---:|---|
| Low-cost AVX2 laptop, 12 threads | B25 / `m=22` | **10.734 s** | p50 of 3 samples |
| Low-cost AVX2 laptop, 12 threads | B255 / `m=24` | **34.938 s** | 1 isolated sample |
| AVX-512 PC, 24 threads | B25 / `m=22` | **6.905 s** | p50 of 3 samples |
| AVX-512 PC, 24 threads | B255 / `m=24` | **21.053 s** | p50 of 3 samples |

The [original measurement record](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/two_class/results/2026-08-06-history-step-b25-b255.md)
preserves the expanded terminal sizes from that revision. Those sizes are not
the v1.1 network payload, and these timings exclude shared-path codec overhead.

PoW nonce search is not included in the table. ASERT targets the complete
elapsed interval between accepted blocks. It does not assign a separate
20-second budget to nonce search. Proof preparation, nonce search and network
propagation all occupy the same observed block interval, and ASERT adjusts the
nonce target against that complete cadence.

## Terminal size in v1.1

Shared-path encoding stores and transmits each shared authentication node once.
A fresh codec audit on 2026-09-24 measured the same verified proof in both
representations, using the production C1 profile and authenticated B25/B255 pack:

| Class | Expanded paths | v1.1 shared paths | Reduction |
|---|---:|---:|---:|
| B25 / `m=22` | 971,732 B | **874,516 B** | 10.00% |
| B255 / `m=24` | 1,081,108 B | **982,100 B** | 9.16% |

Both samples are below 1 MB (1,000,000 bytes). These are one-proof examples,
not fixed sizes or upper bounds: shared-path size depends on query openings.
An independently verified mainnet B25 terminal at height 137191 was 872,500 B.
The consensus cap remains 1,100,000 bytes for encoded and expanded terminals.
Block bodies, transport framing and RPC hex text are excluded from these sizes.

The [codec measurement record](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/two_class/results/2026-09-24-terminal-shared-paths.md)
contains reproduction instructions and validation results. This audit measures
representation savings; it does not remeasure the historical AVX-512 host.

## Native state processing

The node retains a bounded cache of authenticated segment columns and their
exact Merkle trees. The payload budget is 64 MiB per state view, including the
columns: nine production-size segments fit. Copies share immutable data until
modified. Active block scratch is separate from this retained-cache budget.

Hot slot updates recalculate changed paths. Cold loads authenticate the complete
segment against its exact root before entering the cache. After commit, cold
payloads are evicted; restart and snapshot installation rebuild the cache on
demand. The same policy applies across the whole slot domain. Workloads with
poor locality still pay for cold authentication, so measure them separately
from repeated updates to one hot segment.

The ignored state benchmark covers a dense segment, eight and sixteen touched
segments, and repeated misses across thirty-two segments. Each case compares
its final root with the streaming reference. It measures authenticated loads
and native root updates; disk commits, HistoryStep verification and PoW are
outside the measured intervals. The first iteration starts cold.

```sh
RAYON_NUM_THREADS=4 cargo test --locked --release -p noid_chain --lib \
  bench_exact_state_cache_cycles -- --ignored --nocapture --test-threads=1
```

For comparisons, fix CPU affinity, backend and build profile for both binaries.
Keep a separate result for each workload and for cold versus warmed iterations.

## Wallet authorization

The wallet harness measures page construction, logical hashing, one
authorization capsule, complete intent encode/decode and local capsule
admission. It excludes network latency and block `HistoryStep` proving.

```sh
NOID_WALLET_BENCH_SAMPLES=20 cargo run --release --locked \
  --manifest-path research/two_class/Cargo.toml \
  --bin two-class-wallet-bench
```

The production C1 wallet uses 65 Fiat–Shamir queries. One `PagedSpend` contains
one authorization capsule whether it occupies one page or the full 128 pages.
The canonical serialized authorization has a 92,696-byte worst-case bound.

## HistoryStep

The isolated production benchmark requires a completed and authenticated
matrix pack. Run each class separately so the output identifies the exact
parent and child class.

```sh
NOID_PACK_ROOT=../parano1d-artifacts/history-step-pack-v1
source "$NOID_PACK_ROOT/pins.env"
export NOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST
export NOID_HISTORY_STEP_PACK_LEAF_DIGESTS
export NOID_HISTORY_STEP_PACK_DIR="$NOID_PACK_ROOT"

NOID_HISTORY_STEP_BENCH_FILTER=B25 \
NOID_HISTORY_STEP_BENCH_SAMPLES=20 \
cargo bench --locked -p bench_prover --bench history_step_proof

NOID_HISTORY_STEP_BENCH_FILTER=B255 \
NOID_HISTORY_STEP_BENCH_SAMPLES=20 \
cargo bench --locked -p bench_prover --bench history_step_proof
```

`cargo bench` uses the optimized bench profile. Transaction construction,
wallet proving, block-template construction and matrix authentication are
setup. `history_step_ms` covers parent-terminal decoding, bounded input and
authorization preparation, recursive assembly, nonce sealing, proof
construction and terminal encoding. `verify_ms` covers bounded wire decoding
and complete terminal verification.

## End-to-end block production

The isolated proof measurement is not the complete mining latency. Capacity
decisions must measure:

```text
select intents
  + assemble the current block trace
  + replay and bind the parent terminal
  + prove HistoryStep
  + search the nonce
  + submit and accept the block
```

Nonce search and network propagation vary independently from proof
construction. End-to-end comparisons must use the complete production path on the final host.
The miner's automatic B255 permission uses only the first completed B25
preparation and a four-times timing estimate, as described in
[Mining architecture](../architecture/mining.md). Official binaries keep a portable baseline and select the
`pclmul`, `avx2+vpclmul`, `avx512bw+vpclmul` or `neon+pmull` backend at runtime.

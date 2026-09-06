# Performance measurement

Performance belongs to one source revision, proof profile, authenticated matrix
pack, build profile and host. It is not a consensus constant and cannot be
inferred from core count alone.

The measurements below use Parano1d revision
`39626b22d53cf2f2c480a7e28446c197dca68043`, the production C1 profile and the
authenticated B25/B255 matrix pack. The table contains isolated production
benchmarks only.

| Host | Class | `HistoryStep` construction | Statistic | Expanded terminal |
|---|---|---:|---|---:|
| Low-cost AVX2 laptop, 12 threads | B25 | **10.734 s** | p50 of 3 samples | 971,732 B |
| Low-cost AVX2 laptop, 12 threads | B255 | **34.938 s** | 1 isolated sample | 1,081,108 B |
| AVX-512 PC, 24 threads | B25 | **6.905 s** | p50 of 3 samples | 971,732 B |
| AVX-512 PC, 24 threads | B255 | **21.053 s** | p50 of 3 samples | 1,081,108 B |

The terminal sizes are for expanded authentication paths. Shared-path encoding
reduces stored and transmitted bytes; its exact size depends on the openings.
These measurements do not include shared-path codec overhead.

PoW nonce search is not included in the table. ASERT targets the complete
elapsed interval between accepted blocks. It does not assign a separate
20-second budget to nonce search. Proof preparation, nonce search and network
propagation all occupy the same observed block interval, and ASERT adjusts the
nonce target against that complete cadence.

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

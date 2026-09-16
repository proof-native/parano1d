# v2 contract feasibility research

Status: integrated feasibility. The generic recursive-verifier and direct-VM
routes were eliminated. A bounded object-contract relation is now assembled
inside the actual HistoryStep path with sixteen fixed contract-capable
positions. Zero, one, four and sixteen calls share one matrix, the complete
B25 composition remains in 2^22 with 5,505 rows available, and B255 retains all
255 live authorization positions. No final ABI, frozen matrix pack, production
consensus rule or new soundness claim has been selected. See the
[research note](results/2026-09-15/REPORT.md) and the
[integrated prototype](https://git.parano1d.org/ignotusnemo/parano1d/commit/590b489a2bc5c7c78dfa16241b534fd921227beb).

## Question

Can permissionless proof-native contracts preserve independent production and
verification on ordinary hardware? Measure before selecting the execution model.

## First measurements

1. Unchanged production B25 and B255 HistoryStep benchmarks using the release
   matrix pack. Separate class, occupancy, parent, construction, verification,
   transmitted encoding and whole-process peak RSS.
2. Unchanged wallet authorization capsules, one versus multiple independent
   capsules. This is not an atomic multi-owner implementation.
3. Closed native C1 proofs for hash-heavy proxy relations, including several
   independent chains inside one proof. Measure the fixed proof overhead,
   power-of-two row boundaries, prover/verifier time and bytes.

The proxy deliberately uses the ordinary 360-row Poseidon gadget, not the
production FROST-GKR batching construction. It is neither a lower bound nor a
prediction for a finished contract. Inputs and outputs are pinned to each
reference matrix, so this is not a reusable program-authenticated interpreter.
Generic C1 proofs do not inherit the wallet capsule's zero-knowledge property.
Using C1 entry points and 133 queries is not a new Category 1 certificate.

## Reproduce

The integrated prototype lives in the main workspace. Its direct B25 and B255
relations can be rebuilt without a frozen pack:

```sh
cargo build --release --locked -p bench_prover --bin noid_pack_pins
NOID_V2_DIRECT_ONLY=1 ./target/release/noid_pack_pins
```

The complete B25 composition also needs the existing authenticated HistoryStep
pack so it can replay the parent relation:

```sh
NOID_V2_FULL_ONLY=1 ./target/release/noid_pack_pins \
  /path/to/history-step-pack-v1
```

These modes build and scan the research matrices. They do not freeze a new
terminal proof.

The earlier route-comparison harness builds separately from the root workspace.
Its release profile matches the root thin-LTO, one-codegen-unit profile.
Execute CPU measurements one at a time, with no compiler or other benchmark
running concurrently.

```sh
CARGO_TARGET_DIR=target cargo build --release --locked \
  --manifest-path research/v2_feasibility/Cargo.toml
RAYON_NUM_THREADS=12 target/release/paranoid-v2-feasibility substrate 64 1 3
RAYON_NUM_THREADS=12 target/release/paranoid-v2-feasibility substrate 64 4 3
RAYON_NUM_THREADS=12 target/release/paranoid-v2-feasibility substrate 64 16 3
RAYON_NUM_THREADS=12 target/release/paranoid-v2-feasibility wallet 0 1 3
RAYON_NUM_THREADS=12 target/release/paranoid-v2-feasibility wallet 0 4 3
```

For the unchanged production baselines, initialize the pinned pack as described
in [performance methodology](../../docs/reference/performance.md), then run:

```sh
cargo bench --locked -p bench_prover --bench history_step_proof --no-run
NOID_HISTORY_STEP_BENCH_FILTER=B25 \
NOID_HISTORY_STEP_BENCH_SAMPLES=3 \
NOID_HISTORY_STEP_BENCH_WIRE_AUDIT=1 \
cargo bench --locked -p bench_prover --bench history_step_proof
env -u NOID_HISTORY_STEP_BENCH_WIRE_AUDIT \
NOID_HISTORY_STEP_BENCH_FILTER=B255 \
NOID_HISTORY_STEP_BENCH_SAMPLES=1 \
cargo bench --locked -p bench_prover --bench history_step_proof
```

The saved measurements invoked the built benchmark executable directly under
`/usr/bin/time -v` so that process RSS and elapsed time exclude Cargo. The
executable hash, authenticated pack and source revision are in the
[dated report](results/2026-09-14/REPORT.md).

The standalone harness accepts an optional last argument to save a NEW JSON
file. Existing data is never overwritten. Each harness process has one
unmeasured warm-up and emits every measured
sample, median/min/max, source revision and Linux process high-water RSS.
Three samples are exploratory, not a p95 or production capacity estimate.
Whole-process RSS includes setup and warm-up and is not a per-proof delta.

The binding and timed-policy checks are reproducible separately:

```sh
cargo run --release --manifest-path research/v2_feasibility/Cargo.toml \
  --bin contract_abi_binding
cargo run --release --manifest-path research/v2_feasibility/Cargo.toml \
  --bin timed_covenant_rows
```

## Required next gates

The heterogeneous HistoryStep feasibility gate has passed for zero, one, four
and sixteen calls. The next decisive gate is a frozen terminal proof for the
selected ABI, followed by a fresh end-to-end soundness calculation. Before
that freeze, the instruction semantics, object receipt, construction rules,
code migration and canonical wire representation must be fixed.

Also measure cold sync/current-data transfer, live storage and reclamation,
transaction admission under invalid-proof load, stale-proof retries and
end-to-end production including propagation. Inspect how proof geometry affects
blocks with zero contract calls. The present matrix measurements establish
shape and satisfiability, not production proving latency or throughput.

Preserve ordinary payments and existing security assumptions. These artifacts
do not define a mainnet activation, release pin, query-count reduction or
documentation promise.

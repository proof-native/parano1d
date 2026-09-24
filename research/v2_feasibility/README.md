# v2 contract feasibility research

Status: the [joint-bank measurements](results/2026-09-24-banked-integer-core/REPORT.md)
now prove a candidate m23 class with 63 user pages and 63 contract-call positions,
alongside an m24 class with 255 pages and 26 call positions. Both contain the
expanded integer core. All four class transitions and filled call envelopes
have actual recursive proofs; production integration and qualification remain.
The earlier [112-page baseline](results/2026-09-24-tps/REPORT.md) used a different
single-class core and does not establish capacity for this bank. The
[carried-claim measurements](results/2026-09-24-carried-claims/REPORT.md)
retain the original legacy-bank control experiment.
The [fork-origin check](results/2026-09-25-fork-origin/REPORT.md) verifies the
same post-fork history without loading either legacy matrix, including recovery
of the authenticated certificate from disk in a fresh protocol context.
The [core candidate](CORE_CANDIDATE.md) lists the operations and policies that
are included in the joint-bank measurements. The development branch
schedules 30-second v2 blocks at H210537, estimated for October 10, 2026 at
23:59 PDT. Running mainnet remains on its existing v1.1 release. The final
joint matrix bank and call capacities still require selection and qualification.

The September 15 integrated prototype established feasibility for an earlier
object relation; its row counts are historical and do not describe the current
core. See the [dated research note](results/2026-09-15/REPORT.md).
The current measurement path builds a scheduled recursive relation and actual
terminal proofs for medium-capacity candidates. See the
[recursive measurement procedure](CAPACITY_MEASUREMENTS.md); successful
freezing alone does not qualify a candidate for mainnet.

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

The legacy B25 relation can be rebuilt and checked against its published matrix:

```sh
NOID_LEGACY_FULL_MATRIX_AUDIT=1 ./target/release/noid_pack_pins \
  /path/to/history-step-pack-v1
```

The historical `NOID_V2_FULL_ONLY` genesis probe has been removed. It must not
be mistaken for a scheduled v2 transition or a new terminal proof.

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

Complete production integration while preserving the published legacy matrix
identities. Extend the joint-bank qualification to both legacy boundary classes,
resource-distribution limits and sustained whole-node verification under a
4 CPU / 8 GiB budget. The first joint sequence retains both accumulated claims
after the large class appears; cold-cache, restart and real transport tests must
retain that requirement too.

Capacity, interval and class count are joint decisions after measurements.
The final pack requires a fresh soundness calculation and transition checks
covering reorgs, restart, cold sync, miners and existing RPC integrations.

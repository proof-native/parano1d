# Scheduled single-class measurements

The working hypothesis is one m23 class after the v2 boundary. The existing
B25/B255 bank remains responsible for blocks before that boundary. Capacity,
target interval and activation height have not been selected for mainnet.
The [September 24 measurements](results/2026-09-24/REPORT.md) contain actual
matrix bounds, proofs after both legacy classes and constrained receiver runs.

`noid_v2_capacity` builds the complete recursive relation, including the
contract core described in [CORE_CANDIDATE.md](CORE_CANDIDATE.md). It checks
the frozen witness, resolves the integrated registry slices, rebuilds with
the final bank identity, writes a canonical matrix artifact, and constructs
actual terminal proofs. Direct-relation row counts alone are insufficient.

Build the isolated fixture runner:

```sh
cargo build --locked --release -p bench_prover --bin noid_v2_capacity \
  --features noid_chain/isolated-v1-1-testnet
```

The input fixture directory must contain real blocks and terminals named
`h000001.block` / `h000001.terminal` through height nine. Its v1.1 activation
is height five. Every supplied block is checked against native rules and its
complete legacy proof before its state is materialized. Saved state or a
caller-supplied origin hash is never accepted as authority.

For example, to measure an explicit research candidate at 63 pages and a
30-second interval, activating at height ten:

```sh
RAYON_NUM_THREADS=12 /usr/bin/time -v target/release/noid_v2_capacity \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/verified-legacy-fixtures /path/to/NEW-output \
  23 63 30 3
```

The runner requires a new output directory. Use `--freeze-only` as the last
argument to check and serialize a complete matrix without running the block
sequence. `--transition-only` instead constructs the first new block followed
by `SAMPLES` empty recursive successors. Supply a legacy fixture ending in
B255 to exercise a boundary with both legacy accumulated claims live.
Tested capacities must include both sides of body-table boundaries:
63/64 and 127/128, as well as 96. The primary coinbase occupies an additional
body position, so rounding only the user-page count gives an incorrect domain.

The sequence begins with an empty fork block, then creates spendable notes
and sixteen contract objects through ordinary authorized transactions. Each
sample contains an empty block, a four-page ordinary block, and full blocks
with zero, one, four and sixteen contract calls. Ordinary measurement payments
use one input and one output. Contract calls retain a successor and exercise
all eight instructions. This is not a maximum-input or maximum-segment test.

Reported phases distinguish wallet authorization, fixture construction,
input preparation, recursive assembly, proving, nonce search, encoding,
terminal decoding/verification and receiver state materialization. Matrix
authentication and freezing belong to setup. The process high-water RSS
includes setup and earlier proofs; it is not the memory required by a fresh
receiver. Three samples provide exploratory medians, not a tail-latency bound.
The runner generates independent client authorizations with up to four workers
and records that worker count separately. This fixture generation time is not
part of the miner's proving time.

Run receiver measurements in a separate process, supplying the candidate bank
pin independently of the downloaded matrix and recipe:

```sh
RAYON_NUM_THREADS=4 /usr/bin/time -v target/release/noid_v2_capacity verify \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/verified-legacy-fixtures /path/to/candidate \
  CANDIDATE_BANK_PIN HEIGHT 3
```

The receiver authenticates both the matrix and legacy origin before timing
repeated complete terminal verification. Its first sample is marked as
warm-up. Apply an actual CPU affinity and an 8 GiB memory limit when evaluating
the seed envelope; a Rayon thread count alone is not a hardware limit.
Also repeat with `NOID_CPU_BACKEND=pclmul` for machines without VPCLMULQDQ.
CPU affinity on a laptop does not reproduce a server's core speed or sustained
thermal envelope; record the host and selected backend with the measurements.

Qualification still requires maximum resource distributions, transitions
from both old classes, long recursive sequences, reorgs, restart and cold
sync, and integration with the node, wallet and mining interfaces. The
current origin verifier uses authenticated legacy matrices. Replacing that
dependency with a compact certificate is separate cryptographic work.

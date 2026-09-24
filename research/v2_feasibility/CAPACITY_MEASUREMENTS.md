# Scheduled single-class measurements

The measured baseline is one m23 class with 112 user pages and a
384-input block budget after the v2 boundary. The existing B25/B255 bank
remains responsible for blocks before that boundary. Target interval and
activation height have not been selected; this is not a release-qualified bank.
The [September 24 measurements](results/2026-09-24/REPORT.md) contain actual
matrix bounds, proofs after both legacy classes and constrained receiver runs.
The [96-page input-budget investigation](results/2026-09-24-input-budget/REPORT.md)
adds an explicit 384-input candidate and real distributed-State boundary blocks.
The [independent-payment measurements](results/2026-09-24-tps/REPORT.md)
record the 112-page candidate, larger failed shapes and constrained receiver
results. The [network budget audit](NETWORK_BUDGETS.md) distinguishes terminal,
body, bundle and response limits from production protocol admission. A proposed
second class is investigated in the
[carried-claim report](results/2026-09-24-carried-claims/REPORT.md); its legacy
control experiment does not establish two-class v2 capacity.

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

The runner requires a new output directory. Use `--freeze-only`
to check and serialize a complete matrix without running the block
sequence. `--transition-only` instead constructs the first new block followed
by `SAMPLES` empty recursive successors. Supply a legacy fixture ending in
B255 to exercise a boundary with both legacy accumulated claims live.
Tested capacities must include both sides of body-table boundaries:
63/64 and 127/128, as well as 96. The primary coinbase occupies an additional
body position, so rounding only the user-page count gives an incorrect domain.

Use `--payments-only` to fund distinct owners and alternate two-payment and
full-payment blocks, each transaction with one input and two outputs. The first
full block also rebuilds and checks the complete frozen matrix; the sequence
ends with an empty recursive successor. For the current candidate:

```sh
RAYON_NUM_THREADS=12 target/release/noid_v2_capacity \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/verified-legacy-fixtures /path/to/NEW-output \
  23 112 30 3 --payments-only --inputs=384
```

The 30-second interval in this command is a fixture parameter. This mode retains
the contract core in the matrix but does not generate live contract calls.
It reports independent wallet-proof creation separately from miner construction.
The three execution modes are mutually exclusive.

The sequence begins with an empty fork block, then creates spendable notes
and sixteen contract objects through ordinary authorized transactions. Each
sample contains an empty block, a four-page ordinary block, and full blocks
with zero, one, four and sixteen contract calls. Ordinary measurement payments
use one input and one output. Contract calls retain a successor and exercise
all eight instructions. This is not a maximum-input or maximum-segment test.

An additional explicit research profile uses 96 pages with a block-wide
384-input budget, while preserving up to eight inputs per page:

```sh
RAYON_NUM_THREADS=12 /usr/bin/time -v target/release/noid_v2_capacity \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/verified-legacy-fixtures /path/to/NEW-output \
  23 96 30 3 --inputs=384
```

Its input budget is part of the authenticated bank identity and the versioned
compact recipe. The native preparation boundary and the R1CS integer input
sum enforce it independently. Legacy rules and earlier candidate recipes
retain their original capacities.

After the ordinary samples, this profile creates real notes across the
fixture's 256 segments and measures blocks with 384 inputs: 96 pages with
four inputs and two outputs each, then 48 pages with eight inputs and two
outputs each. It also constructs a 385-input block with spare touched slots,
checks native rejection, deliberately bypasses that native budget during
witness preparation, and requires the *same frozen matrix* to be unsatisfied.
The invalid block is never added to the chain. Logs include actual segment
counts; distributing output addresses is not itself evidence of maximum
segment coverage.

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
warm-up. A comma-separated height list measures several saved blocks after
one authentication step, with a separate warm-up for each block.
Apply an actual CPU affinity and an 8 GiB memory limit when evaluating
the seed envelope; a Rayon thread count alone is not a hardware limit.
Also repeat with `NOID_CPU_BACKEND=pclmul` for machines without VPCLMULQDQ.
CPU affinity on a laptop does not reproduce a server's core speed or sustained
thermal envelope; record the host and selected backend with the measurements.

Use `verify-state` in place of `verify` to also measure native State application
after each accepted terminal. This first verifies the endpoint and reconstructs
its parent State by replaying the bounded fixture bodies, checking every header
link, native transition and root. Reconstruction is a separate setup phase.
Each timed application starts from a clone of that parent and must reproduce
the authenticated header's root and counters. This includes State work but is
still not a full daemon, persistent database or retained-fork workload.

Further qualification requires other resource distributions and deeper State,
transitions from both old classes, long recursive sequences, reorgs, restart and cold
sync, and integration with the node, wallet and mining interfaces. The
current origin verifier uses authenticated legacy matrices. Replacing that
dependency with a compact certificate is separate cryptographic work.

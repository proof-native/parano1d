# Carried matrix claims after a large block

The proposed direction is a primary m23 class and a manually selected m24
class. The existing 112-page / 384-input m23 measurement remains a **single-class
baseline**. It does not establish that 112 pages still fit when both parent
classes are supported. The 112 user pages exclude the primary coinbase.
No block interval or activation height is selected.

## What persists after B255

The legacy two-class bank provides a working control for this mechanism:

1. A B255 terminal has a fresh claim against its large matrix.
2. Its immediate small successor folds that claim into the large accumulated
   lane. This successor performs one large matrix fold.
3. Later small successors fold their small parent. The large accumulated
   point and value pass through unchanged.
4. Before this change, terminal acceptance scanned every live matrix again,
   including the unchanged large lane, on every later small terminal.

Thus the large fold is not repeated forever by miners. The repeated large
matrix evaluation by receivers was real and avoidable.

The runtime now retains up to eight exact, successfully evaluated accumulated
claims. The key includes the bank, class, matrix shape and digest, and the
complete evaluation point and value. Only a carried lane can reuse a result;
the current terminal's fresh matrix obligation and proof are still checked.
The cache stores no matrices, accepts no external entries and is not persisted.
Restart, eviction, a changed claim or a different bank takes the ordinary
authenticated evaluation path. Returning to an earlier branch can reuse an
identical mathematical claim independently of whether that branch is selected.

This changes local verification work, not the relation, proof encoding,
consensus rules or published legacy matrix identities. It is connected to the
existing long-lived `HistoryStepRuntime`; the current single-class v2 runtime
has no inactive matrix lane to optimize.

## Real proof sequence

The control starts from a verified legacy B255 at H9 and generates four actual
B25 successors, H10–H13. The isolated v1.1 activation is H5; these successors
**do not activate v2**. Each proof is decoded, verified against its header and
epoch, checked against native block rules, and applied to State. This is a
mechanism measurement using the published m22/m24 bank, not an m23/m24 v2
capacity qualification.

Host: Intel Core i7-1365U, twelve Rayon workers, AVX2 + VPCLMUL. CPU measurements
ran without a concurrent compiler or another benchmark. Every height has three
interleaved samples with and without the checked-claim cache. The comparison
uses independent verifier runtimes sharing only authenticated matrices;
matrix loading and runtime construction are outside those per-proof timings.

| Small successor | Parent fold | Construction | Verification, empty claim cache p50 | Verification, populated claim cache p50 |
|---|---|---:|---:|---:|
| H10, immediately after B255 | m24, 3.925 s | 13.581 s | 1.283 s | 0.543 s |
| H11 | m22, 0.981 s | 10.841 s | 1.371 s | 0.545 s |
| H12 | m22, 0.973 s | 10.610 s | 1.584 s | 0.549 s |
| H13 | m22, 1.234 s | 13.650 s | 1.896 s | 0.786 s |

Construction is input preparation + recursive preparation + proving, excluding
PoW, final nonce sealing, encoding and State application. Each construction is
one sample. Verification includes terminal decoding in the interleaved samples.
All samples, including the slower H13, belong in the result; this is not a p95
or a server prediction. The roughly 2.4–2.9× reduction is in **verification of
this control after B255**, not overall proving or v2 block construction.

The receiver's first H10 verification, with no previously checked large
accumulated claim, took 1.419 s. Its first verification of each later distinct
terminal took 0.582, 0.494 and 0.772 s. Consequently the benefit is present on
new successors, not only when repeatedly checking one terminal.

Terminals measured 871,732–873,876 bytes. Cache hits do not change their bytes
or remove the large accumulated lane from them. Whole-process peak RSS was
5,575,184 KiB, including matrix authentication and proving; it is not a receiver
memory measurement. Fixture/matrix authentication took 178.071 s separately.
This research loader authenticates canonical matrix bytes; it does not model
the node's packaged startup path and persistent packed matrix cache.

A separate B25-only control, with no live large accumulated lane, measured
0.531 / 0.545 s without populated claims and 0.525 / 0.553 s with them at
H10 / H11. There is no useful speedup to claim there; the cache only avoids
an extra carried-lane scan when that lane exists and has been checked.

## Constrained receiver

A separate process verified the same four terminals with affinity `0,2,4,6`
(four distinct physical laptop cores), four Rayon workers, forced `pclmul`,
an enforced 8 GiB memory limit and no swap. These are constrained laptop
measurements, not measurements on an actual seed server.

| Small successor | Empty claim cache p50 | Populated claim cache p50 | First verification of this distinct terminal |
|---|---:|---:|---:|
| H10 | 4.958 s | 1.858 s | 4.717 s, empty cache |
| H11 | 5.095 s | 1.876 s | 1.913 s |
| H12 | 5.415 s | 1.927 s | 1.891 s |
| H13 | 5.446 s | 1.931 s | 2.016 s |

The interleaved medians improve by 2.67–2.82×. The first new successor after
H10 also benefits: it inherits the exact large accumulated claim already
checked at H10. The current small proof and its fresh matrix obligation are
verified on every call. Native State work is outside these verification
timings; this is not an end-to-end node throughput test. First-verification
timings start after decoding; the interleaved cache comparison includes decoding.

Peak RSS was **1,459,360 KiB (1.39 GiB)**, including initial authentication of
both legacy matrices. The research authentication/fixture replay took
611.700 s separately and is excluded from all table entries. This cold setup
does not represent packaged node startup. No swap was used.

## Standalone m24 shape

The complete single-class m24/255 relation with a 384-input block budget and
the existing sixteen-call contract core freezes at **15,596,345 of 16,777,216
positions**. Two independent runs converge to the same matrix digest,
`ef76c009d656867638e0c3d384e90b5014ae90f20ef1b066daa87e99be846a58`.
The compressed matrix is 18,307,025 bytes. Its runtime-derived terminal bound
is **1,079,764 bytes**, leaving **20,236 bytes** below the current 1,100,000-byte
transport cap. These parameters are a research control; a large-class input
budget has not been selected, and the original 1,020-input budget is not
qualified by this run.

An empty v2 fork block and two empty m24 successors were proved and verified
from an authenticated legacy B25 origin. The H10 activation and 30-second
spacing are fixture parameters. Construction took 40.790, 48.589 and 36.521 s;
verification took 1.559, 2.639 and 2.040 s on the twelve-thread laptop.
The actual terminals were 981,556, 978,644 and 979,924 bytes. Each is one sample;
these timings do not select an interval or predict server performance. The
process peaked at 7,058,908 KiB RSS, including freezing and proving. This is not
the constrained receiver's memory requirement.

The first two terminals each rejected 142 mutations. This sequence contains
no live user payments or contract calls. It does not qualify full occupancy,
the two-class bank or production admission of research terminal version 6;
see the [network audit](../../NETWORK_BUDGETS.md).

The first freeze attempt stopped in a harness negative check after the matrix
was built. The harness incorrectly assumed that every changed input budget
must change the padded VK layout and therefore fail recipe parsing. A different
self-consistent recipe can parse while still failing the independently supplied
bank pin. The corrected harness records five decode rejections and one bank-pin
rejection across six substitutions. Core decoding and authentication were not
relaxed. The failed attempt remains in the raw measurements.

## Two-class capacity is a separate unresolved constraint

The single-class m23/112 relation uses 8,350,239 of 8,388,608 positions, leaving
38,369. Supporting another parent changes the relation and its authenticated
bank. The existing two-class design allocates both parent verifier arms even
though only the selected arm's checks are active. A straightforward duplicate
of an m24 verifier costs hundreds of thousands of positions, beyond this
reserve. Shared parent verification or a jointly selected capacity adjustment
needs a new complete freeze and real alternating-class proofs.
The joint v2 bank and manual miner selector still require implementation;
this change implements and measures the carried-claim optimization.

Manual selection is a miner policy. Every receiver must still accept valid
large blocks, including consecutive ones. It does not remove their verification
cost, the first small successor's large fold, or the need to make the large
matrix available after a restart. Qualification must include sustained large
blocks, alternating classes, reorgs, cold synchronization and the terminal
transport bound. The cache addresses the repeated unchanged-lane scan only.

## Validation and reproduction

All 420 recursive library tests passed, with three ignored. Six new checks
cover reuse, fresh-claim rejection, changed or false accumulated claims,
bank/class/matrix binding, simulated branch changes, restart, eviction and
poisoned cache fallback. A 32-successor claim sequence also checks that the
unchanged large result survives small-claim churn beyond the cache capacity.
All six cache tests passed again after adding this sequence; formatting passed.
The ordinary uncached decision entry point remains available.
The [raw measurements](measurements.json) retain all samples, artifact hashes,
the binary hash and the hashes of the changed source files.

Build the isolated measurement runner using the
[capacity measurement guide](../../CAPACITY_MEASUREMENTS.md). Generate a new
tail from an authenticated legacy fixture ending at H9:

```sh
RAYON_NUM_THREADS=12 NOIDH_MATRIX_FOLD_TIMING=1 \
  target/release/noid_v2_capacity legacy-tail \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/legacy-B255-fixtures /path/to/NEW-tail 4
```

For a separate receiver process, replace `legacy-tail` with
`legacy-tail-verify`, supply the existing tail directory, and apply the intended
CPU affinity, backend and memory limit. The runner shares authenticated matrix
storage between A/B verifier runtimes, not their checked-claim caches.

The standalone large-class control used a new directory and a legacy B25
fixture with the same isolated activation schedule:

```sh
RAYON_NUM_THREADS=12 target/release/noid_v2_capacity \
  /path/to/history-step-pack-v1 LEGACY_METADATA_PIN \
  /path/to/legacy-B25-fixtures /path/to/NEW-candidate \
  24 255 30 2 --transition-only --inputs=384
```

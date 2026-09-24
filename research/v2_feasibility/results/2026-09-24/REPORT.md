# Scheduled m23 capacity measurements — 2026-09-24

The current complete single-class relation accepts real 63-page blocks,
including sixteen calls to the eight-instruction contract core. Transitions
from both legacy B25 and B255 have been proved and verified. Neither a
mainnet capacity, interval nor activation height has been selected.

A [follow-up with an explicit 384-input budget](../2026-09-24-input-budget/REPORT.md)
fits 96 pages in m23. The capacity table below describes the original input
budgets, including 768 inputs for its 96-page layout.

## Capacity

These figures include the recursive parent verifier, authorization, exact
State and contract rules. The limit is 8,388,608 witness positions for m23.
The unsuccessful candidates were rejected after assembling the full relation;
they have no frozen matrix or proof. This is a bound on the current layout,
not a theoretical limit on future optimizations.

| Page budget | Positions required | Result |
|---:|---:|---|
| 63 | 5,339,938 | Frozen matrix and actual recursive proofs |
| 64 | 8,538,565 | Exceeds m23 |
| 96 | 8,839,013 | Exceeds m23 |
| 127 | 9,376,384 | Exceeds m23 |
| 128 | 14,579,605 | Exceeds m23 |

The body and authorization tables have dyadic dimensions. Crossing 63/64 or
127/128 therefore has a discontinuous cost. The measurement work also fixed
an ambiguous authorization overflow lookup: equal padded tile counts do not
imply equal metadata offsets. The binding now uses the allocator's exact
physical capacity. Both legacy capacities retain their existing offsets.

The 63-page compressed matrix occupies 6,388,784 bytes. Its identity is
`44d405f1e5609c2bd4f4622fcf15ebb68997466b22d6cf363f6d0aaeb2485579`;
the candidate bank is
`fb3e9c9ba518d563373bd7f6732bb96acd6463ee46adfec369d784b2867e8825`.
Both legacy-parent fixtures produced these same identities. The full
recurrent sixteen-call witness also rebuilt this exact satisfied matrix.

## Proving on the laptop

Host: Intel Core i7-1365U, twelve logical CPUs, ten physical cores,
approximately 31 GiB RAM, AVX2 + VPCLMULQDQ, Rust 1.96.0 release build.
Only one benchmark ran at a time, without a concurrent compiler or test suite.
The host remained a normal desktop; these are exploratory measurements.

The following are three samples per case after a verified legacy B255 parent.
The chain activates v1.1 at height five and v2 at ten, and continues to height
thirty. Thirty seconds is an explicit fixture setting, not a release decision.

| Pages | Contract calls | Preparation + assembly + proving, p50 | Range | Terminal verification, p50 |
|---:|---:|---:|---:|---:|
| 0 | 0 | 14.406 s | 14.216–14.417 s | 664 ms |
| 4 | 0 | 16.463 s | 15.847–19.301 s | 843 ms |
| 63 | 0 | 18.195 s | 17.651–22.974 s | 562 ms |
| 63 | 1 | 16.851 s | 16.733–17.114 s | 669 ms |
| 63 | 4 | 22.019 s | 21.981–22.453 s | 865 ms |
| 63 | 16 | 16.290 s | 16.263–16.775 s | 645 ms |

The construction column includes native proof-input preparation, recursive
assembly and proving. It excludes client authorization generation, the fixture
template constructor, nonce search, encoding and subsequent verification.
Those phases remain separate in the raw records. Calls and ordinary payments
in these measured blocks use one input and one successor/output; they are
not maximum-input or maximum-segment workloads. Every call exercises all eight
instructions. The timings do not imply a linear cost per contract call.

Peak RSS for the complete process was 4,019,636 KiB (about 3.83 GiB), including
authentication of both legacy matrices, candidate freezing and earlier blocks.
This is not a fresh verifier's memory use. Three samples do not establish a
tail-latency bound or determine a safe network block interval.

## Receiver

A separate verifier was limited by CPU affinity to four distinct physical
cores (two P cores and two E cores), four Rayon threads, 8 GiB memory and no
swap. It authenticated the candidate bank/matrix and legacy origin before
timing complete terminal decoding and verification. Sample zero is warm-up.
This process is not a full node and does not retain the network's State.

For the full 63-page, sixteen-call proof with AVX2 + VPCLMULQDQ, five measured
verifications had a median of 1.129 s and a range of 1.108–1.148 s. RSS after
verification was 273,800 KiB; process peak including setup was 518,264 KiB.
Authenticating the supplied artifacts and origin took 69.129 s separately.
The laptop's constrained cores do not reproduce the speed of a seed server.

With `NOID_CPU_BACKEND=pclmul`, the full 63-page, four-call proof had a median
verification time of **2.249 s**, range **2.223–2.269 s**, over five samples.
RSS after verification was 273,676 KiB; process peak including setup was
518,380 KiB (about 506 MiB). The separate matrix/origin authentication phase
took 243.405 s. This deliberately verifies supplied canonical matrix artifacts
from their contents; it is not a measurement of packaged-node startup.
The two receiver runs use different mixed blocks and therefore are not a
controlled comparison of CPU backends.

## Evidence and remaining work

- [Initial B25-parent run and four-core receiver](m23-63-initial.json).
- [Three samples per case after B255](m23-63-after-b255.json).
- [Four-core PCLMUL receiver](m23-63-receiver-pclmul.json).
- [Capacity failures and source/binary hashes](m23-capacity-boundaries.json).
- [Reproduction procedure](../../CAPACITY_MEASUREMENTS.md).

The base and first recurrent terminals each rejected 138 structural/public-IO
mutations. The recursive regression after the overflow correction passed
411 tests, with three pre-existing ignored tests.
Reassembling the legacy B25 matrix after that correction still produced
4,185,273 useful rows and the published digest
`9be6ed118dc0df56d670161569dc8283a2ed5b441bb9476911ead230968b3d8b`,
with a satisfied witness.

Further qualification must cover maximum inputs and State-segment spread,
long sequences and epoch edges, reorgs, restart and cold synchronization,
and the actual node, wallet and mining interfaces. Legacy matrix retirement
and the final parameter-specific soundness calculation remain separate work.
Capacity and timing choices remain joint decisions after measurement.

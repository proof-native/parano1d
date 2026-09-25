# V2 performance measurements

Measurements belong to a source revision, authenticated bank, build, backend
and workload. Capacity is not throughput, and proof construction is not the
whole block interval. Older B25/B255 and AVX-512 results are kept in the
[archive](../archive/legacy-performance.md); they are not v2 timings.

## Frozen profile

Small is m23 / 63 pages / 504 inputs / 63 calls; Large is m24 / 206 / 504 / 63.
The mainnet bank is
`c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e`.
The qualification network uses the same capacities with early activation and a
different schedule-bound bank. An isolated bank cannot substitute for mainnet.

Class-specific terminal upper bounds are 1,014,132 B and 1,081,396 B;
the network cap is 1,100,000 B. Actual shared-path sizes depend on openings.
Bodies, framing and RPC hex are additional. The one-time fork-origin material
is separate from the per-block terminal.

## Isolated proof construction

Intel i7-1365U, 12 proof threads, AVX2+VPCLMUL. Each row is an individual
observation using the final isolated profile; preparation excludes wallet
proving, PoW and initial artifact authentication.

| Workload | Construction | Verification | Encoded terminal |
| --- | ---: | ---: | ---: |
| Small: 63 payments | 17.565 s | 0.830 s | 913,108 B |
| Small: 63 calls | 18.161 s | 0.774 s | 913,012 B |
| Large: 206 payments | 42.583 s | 2.017 s | 981,396 B |
| Large: 63 calls + 143 payments | 46.294 s | 2.469 s | 979,828 B |

An almost empty block still proves the same fixed relation. Three later empty
Small blocks had a construction median of 16.584 s and verification median of
1.011 s. Few transactions therefore do not reduce proving time in proportion
to page occupancy. No final-bank AVX-512 measurement is claimed.

## Full daemons, constrained receiver

Two daemons shared the laptop. The producer used six proof workers and a
separate two-thread nonce worker. The receiver had an enforced **four-CPU quota,
8 GiB memory, no swap**, four proof workers and forced PCLMUL. The following
are individual accepted blocks, not sustained public-network throughput:

| Workload | Producer preparation | Receiver verification | Receiver application |
| --- | ---: | ---: | ---: |
| Small: 63 payments | 20.755 s | 2.700 s | 0.294 s |
| Small: 63 calls | 22.790 s | 2.774 s | 2.391 s |
| Large: 206 payments | 50.180 s | 6.305 s | 1.254 s |
| Large: 63 calls + 143 payments | 67.326 s | 6.723 s | 6.050 s |
| Large: 504 inputs / 206 pages | 54.619 s | 8.394 s | 1.619 s |
| Small after Large: 63 payments | 23.904 s | 5.696 s | 1.094 s |

Preparation excludes wallet authorization, PoW and delivery. Application follows
verification and includes State, watched-wallet and receipt work. The receiver
peaked at **1.65 GiB** for this full-capacity scenario, including startup,
mempool admission, receipt checks and the following Small sequence. No OOM or
memory-limit events occurred. Initial RPC startup took 10.023 s, restart 12.512 s.

After the first Small block following Large, the next two distinct Small
terminals verified in 2.729 and 2.867 s. Eight subsequent empty Small blocks
had a 2.890 s verification median. The larger accumulated obligation remains
cryptographically covered, but this measured sequence did not sustain twice
the Small verification cost. Cold standalone replay has a different cost and
must not be substituted for daemon cache behavior.

Large preparation exceeds the 30-second target on this producer. Manual
permission is intended for hosts where the tradeoff is useful; the flag cannot
make the hardware faster. The receiver test qualifies this workload, not a
production seed under arbitrary sustained traffic. Serialized batch submission
also includes wallet proof generation and is not a saturated relay benchmark.

## Native State and interpretation

Authenticated segment payloads use a bounded 64 MiB retained cache per State
view; active block scratch is separate. Hot writes update changed paths; cold
segments must first be authenticated. Test locality, restarts and multi-segment
workloads separately. A block can touch up to 256 segments.

The full interval is selection, assembly, recursive proof, nonce search,
transport and acceptance. ASERT targets their combined cadence. Report each
component and its hardware scope instead of equating proof seconds with TPS.

All sample records, source identities, limits, negative controls and
reproduction scripts are linked in the [qualification report](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/v2_feasibility/results/2026-09-25-common-input-budget/REPORT.md).

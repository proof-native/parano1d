# v1.1 terminal shared-path sizes

Measured on 2026-09-24 with the production C1 profile and the authenticated
mainnet B25/B255 pack. The source is v1.1 based on `ae920530` with the native
state cache update recorded alongside this report; terminal codecs, proof
parameters and matrices are unchanged from that baseline.

| Condition | Value |
|---|---|
| Host | Intel Core i7-1365U |
| CPU affinity | `0,2,4,5`, four distinct cores |
| Runtime backend / threads | `avx2+vpclmul` / 4 |
| Build | Optimized Cargo bench profile, locked dependencies |
| Parent class | B25 / `c00` for both child classes |
| Size samples | One verified terminal per child class |
| Child workload | B25: coinbase only; B255: 26 user pages |
| Codec timing repetitions | 32 per operation, alternating order |

Authenticated runtime metadata digest:
`ad463bd76e27df3c0f414f4fd7640cfb5c45cc7f3a09e44a2f7bc7a8b869485b`.

Pack leaf digests, in B25/B255 order:

```text
3e4b60852aa67803670f67f79d0451b6b3f56635cfa4e7093748cc4b1b3060f8
1bd7ab3ee54ccc2f98032ae0a3a473b0ef9a7a5a5cfce096430cd4f61cca8edf
```

## Size results

| Class | Expanded paths | Shared paths | Saved | Reduction |
|---|---:|---:|---:|---:|
| B25 / `m=22` | 971,732 B | **874,516 B** | 97,216 B | 10.00% |
| B255 / `m=24` | 1,081,108 B | **982,100 B** | 99,008 B | 9.16% |

These sizes include terminal metadata, and exclude the block body, transport
framing and RPC hex expansion. Shared-path size varies with query positions;
these are examples, not universal bounds. The v1.1 consensus cap remains
1,100,000 bytes for both the encoded terminal and expanded decoding.

The diagnostic encodes each verified proof in both formats without changing
the actual activation schedule. Expanded B255 is a comparison representation;
its size exceeds the pre-v1.1 terminal cap and does not establish pre-fork
block admissibility. Each class passed exact round trips, native verification
of both decoded representations, eight activation-format checks and rejection
of 530 malformed inputs.

An independent mainnet sample, read from a synchronized node on the same day:
B25 terminal at H137191, wire version 5, **872,500 B**. Its canonical block hash
was `2921dff565d686e9b10564d8af5978b8185baf67b1cab7abeed25727ef09b402`.

## Reproduce

Use the authenticated mainnet pack matching the pins above. This command runs
both child classes after a B25 parent; leave the class filter and all-parent
option unset. Choose a valid CPU affinity for the measurement host.

```sh
NOID_PACK_ROOT=../parano1d-artifacts/mainnet-v1/history-step-pack-v1
source "$NOID_PACK_ROOT/pins.env"
export NOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST
export NOID_HISTORY_STEP_PACK_LEAF_DIGESTS
export NOID_HISTORY_STEP_PACK_DIR="$NOID_PACK_ROOT"

RAYON_NUM_THREADS=4 \
NOID_HISTORY_STEP_BENCH_SAMPLES=1 \
NOID_HISTORY_STEP_BENCH_WIRE_AUDIT=1 \
taskset -c 0,2,4,5 cargo bench --locked -p bench_prover --bench history_step_proof
```

Read the `wire_audit` lines for both representations. The benchmark's ordinary
`terminal_bytes` field follows its fixture height and can still report the
legacy encoding; it is not the shared-path size column.

This measurement qualifies representation sizes, not a full-block latency or
throughput distribution. Historical AVX2/AVX-512 construction timings remain in
the [original report](2026-08-06-history-step-b25-b255.md) as a baseline for later
capacity experiments, including v2. No new AVX-512 measurements were made here.

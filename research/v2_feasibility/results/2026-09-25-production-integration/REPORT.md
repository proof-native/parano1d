# Scheduled v2 producer and receiver integration

The production `TemplateBuilder` and `PreparedBlockAttempt` now construct the
two-class v2 candidate after the isolated activation at H10. The driver uses
real wallet authorization, `AsyncMempool` admission, independent producer and
receiver MDBX contexts, serialized accepted bundles, full inbound recursive
verification, native State replay, confirmation cleanup and durable reopening.
This is component integration, not whole-daemon/P2P qualification.

Candidate bank:
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
Small: m23, 63 pages, 504 live inputs, 63 contract calls. Large: m24, 255 pages,
1,020 live inputs, 26 contract calls. These remain candidate resource limits;
they do not establish final release parameters. Both classes are verified;
production defaults to Small and offers Large only with the server option
`--v2-large-blocks`. Large is not a superset of the Small call budget.

## Real production paths

The fork run replayed authenticated legacy blocks H1–H9 and produced H10–H17.
It funded and called six templates: refundable payment, timelocked vault,
per-call allowance, period budget, recurring payment and tranche vesting.
It checked repeated counter transitions, early-call rejection, recovery/closing
and ordinary payments alongside contract calls. Both databases reopened at the
same validated boundary. Raw results: [fork.json](fork.json),
[source and binary hashes](fork-source.json).

The capacity run replayed the saved mixed-class fixture through H33 and then
produced the following blocks. Each ordinary phase admitted 255 wallet-signed
payments; disabling Large selected 63 of them. At H37, Large remained enabled,
but the producer selected the more valuable 63-call Small block rather than
discarding calls to fit Large's 26-call ceiling.

| Height | Selected block | Prepare/prove, excluding PoW | Receiver verify + apply | Terminal |
|---|---|---:|---:|---:|
| 34 | Small, 63 ordinary pages | 20.303 s | 1.532 s | 914,836 B |
| 35 | Large, 255 ordinary pages | 49.032 s | 2.354 s | 981,908 B |
| 36 | Small, 63 ordinary pages after Large | 20.407 s | 1.659 s | 912,724 B |
| 37 | Small, 63 integer contract calls | 23.271 s | 0.980 s | 913,684 B |

These are individual observations on the AVX2 i7-1365U laptop with a shared
12-thread proof pool, not percentiles or AVX-512 measurements. The proof inputs
retain their normal authorization and native checks. Wallet signing and mempool
admission are recorded separately from block proving. No builds or other test
jobs ran concurrently. PoW was measured separately at the isolated difficulty.
Every terminal stayed below the existing 1,100,000-byte transport ceiling.

The capacity process used a 20-GiB/no-swap scope and peaked at 8,376,608 KiB
(7.99 GiB). That peak includes wallet batches, the prover, legacy replay and
both MDBX contexts. It is **not** a receiver-only or 4-CPU/8-GiB daemon result.
Initial file-pack authentication and replay took 276.732 seconds; this setup is
separate from warm block production and the embedded-release startup path.
Both contexts reopened successfully at H37. Raw results:
[capacity.json](capacity.json), [source and binary hashes](capacity-source.json).

## Integration fixes and remaining qualification

The first capacity attempt found a storage assumption that inferred the proof
class from the old 25-page threshold. It rejected an already verified empty
Large block at H12. Normal and reorg commits now retain the class from the
height/header-bound v2 terminal; legacy selection remains unchanged. The repeat
run passed. RPC reports the actual retained class, or explicitly reports missing
class/parameter information when it is unavailable.

The node now fetches and authenticates missing fork-origin certificates on
snapshot, suffix and boundary-verification paths. Same-tip recovery can restore
a missing origin without waiting for another block. Certificate unavailability
does not label the terminal provider as malicious. Full network recovery is
still awaiting daemon qualification. Cancelled mempool submissions keep their
CPU admission permit until the blocking verifier exits.

The 193 node test cases and scoped native template, miner, mempool, storage,
reorg and RPC suites passed. Whole-daemon networking, constrained receiver load,
contract wallet/API/GUI integration and the complete matrix-retirement lifecycle
remain separate work. This report does not claim release readiness.

Reproduction entry point: build `noid_v2_capacity` with feature
`noid_chain/isolated-v2-fork-testnet`, then use
`joint-produce PACK LEGACY_METADATA_PIN LEGACY_FIXTURES CANDIDATE BANK_PIN NEW_OUTPUT fork`
or the same command ending in `capacity`. The candidate and legacy artifacts
must match the recorded pins; the output directory must be new.

# Research gates before choosing a v2 contract architecture

The first object/bytecode model was treated as a hypothesis, not a selected
protocol. The experiments below establish feasibility before any consensus
architecture is frozen.

## Current result

The complete generic recursive-verifier route and a direct per-HistoryStep VM
have been rejected by measurement. The authorization-reuse route has now
passed an integrated heterogeneous HistoryStep build for zero, one, four and
sixteen calls. The complete B25 matrix remains in 2^22 and B255 preserves all
255 live authorization positions. The evidence supports continuing with a
bounded one-object contract design. It does not yet freeze the instruction set,
wire ABI, matrix pack or consensus transition. See
[the dated report](results/2026-09-15/REPORT.md).

## Alternatives to compare

| Candidate | Required experiment | Cost that must not be omitted |
|---|---|---|
| Fixed universal bounded interpreter | Two different, non-whitelisted programs under one unchanged verifier | Code identity, instruction fetch, operand routing, memory consistency, integer ranges, full recursive verification |
| Authenticated compiled circuits | Two different program matrices under one admission rule | Authenticating their structure/VKs and closing their matrix claims without a free or unbounded whitelist |
| Restricted programmable policy language | Equivalent escrow/recovery/composition workloads | Expressiveness limitations, all authorization/effect bindings, future opcode changes |

All candidates must use compatible post-quantum assumptions. A fast foreign
proof system is not a replacement until its recursive composition and security
model have been checked. Identical field names do not establish compatibility.

## Workloads and controls

- Baseline: unchanged B25, then unchanged B255, with exact occupancy and parent
  recorded. An empty B25 is not evidence for a saturated B25.
- Zero contract calls through the proposed v2 path. This exposes mandatory
  interpreter/verifier/recursion overhead paid by ordinary blocks.
- All parent/child class transitions, including an empty small-class block
  AFTER a contract-heavy large-class parent. Measuring only small-to-small
  can hide costs that the recursion must carry after contracts are used.
- One small contract, then 4 and 16 calls. Same program versus different
  programs, separately proved versus aggregated when a real aggregator exists.
- Integer-heavy and hash-heavy contracts, bounded loops and maximum code/data
  sizes, explicit read sets, read-only references and shared mutable objects.
- A multi-party escrow/refund and an atomic exchange of two independently
  controlled objects. Include constructor/origin checks, not only spending.
- Concurrent updates to one hot object versus independent objects. Report
  retries and useful completed operations, not just proof-generation speed.
- Invalid/truncated proofs, wrong program and effect bindings, unauthorized
  output creation, replay, over-budget programs, and deliberately unavailable
  current object data.
- Fresh-node State transfer and verification after old bodies are pruned,
  followed by an actual contract action. Validity and witness availability are
  separate requirements.

## Measure each boundary

Client: execution, witness generation, proof construction, peak memory, bytes,
and reproving after state conflicts. Miner: admission, recursive proof checking,
State witnesses, HistoryStep assembly/proving, and template staleness. Other
nodes: decode, terminal verification, materialization and current-data storage.
Network: admitted transaction bytes, body and terminal bytes, propagation and
invalid-input admission work.

Use release builds with the same CPU policy and security configuration. Run
CPU jobs serially. Record cold/setup costs separately from steady state, raw
samples and the hardware/software context. Three samples are preliminary and
cannot establish tails. Background desktop activity and thermal state must be
acknowledged, then controlled for the final architecture comparison.

## Decision rules

1. No candidate advances solely because its native proof verifies quickly.
   It must be measured inside an actual recursive HistoryStep prototype,
   including program binding and all matrix/PCS claims.
2. Cost must be explicitly bounded per transaction AND per block. Distinct
   programs, slow paths, code size, data size, verifier work and invalid-input
   admission must all be accounted for. Fees alone do not prevent admission
   DoS because an invalid transaction never pays its advertised fee.
3. Proving, nonce search and propagation share the existing 20-second target.
   Twenty seconds is not an independent allowance for each phase. Longer proof
   preparation affects small-miner participation even when difficulty adjusts.
4. Record the zero-call and ordinary-payment regression before choosing shared
   geometry. A larger dyadic relation can affect every block. Raw unused rows
   are not automatically usable capacity for a new verifier.
5. A bounded prototype may establish feasibility for its tested envelope,
   never for arbitrary computation or worst-case transaction contention.
6. Only after performance AND adversarial tests pass: freeze an ABI/resource
   envelope, derive the v2 soundness accounting, and specify the authenticated
   transition from the existing mainnet. Do not change the current certificate
   to cover unimplemented relations.

## Primary references reviewed

- [Parano1d performance methodology](../../docs/reference/performance.md):
  separate setup, HistoryStep construction, verification and total production.
- [Mining geometry and session capacity](../../docs/architecture/mining.md):
  B25/B255, the adaptive production policy and the shared block interval.
- [Decentralization Across Time](https://lab.parano1d.org/research/decentralization-across-time/):
  object commitments and holder-retained witnesses are a published design
  direction, not an implemented contract runtime.
- [o1Labs zkApps](https://docs.o1labs.org/o1js/zkapps/intro): off-chain proving
  does not remove inclusion-proving limits, and mutable-state preconditions
  can invalidate a proof before inclusion. Its timings/limits cannot be
  transferred to the Parano1d proof stack.
- [Cardano CIP-31](https://cips.cardano.org/cip/CIP-0031): consuming an object
  merely to read shared information causes contention. Reference inputs solve
  the read-only case, not arbitrary concurrent writes to one object.

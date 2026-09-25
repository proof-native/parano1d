# Proof-native contracts without a general VM

## First feasibility results for Parano1d v2

**Ignotus Nemo** · 16 September 2026 · Research note 01

**Prototype:** [`590b489a`](https://git.parano1d.org/ignotusnemo/parano1d/commit/590b489a2bc5c7c78dfa16241b534fd921227beb)

## Abstract

Parano1d does not need a contract layer that makes every node replay an
ever-growing machine. Any v2 design has to preserve the reason the network
exists: proof of work orders transitions, HistoryStep carries State validity
forward recursively, and a node can verify the present without replaying old
transaction bodies.

I tested three ways to add programmable State. A general interpreter inside
every HistoryStep exhausted the small-block budget. Recursively verifying a
complete generic proof for every call was much more expensive. A bounded
object-contract model did fit when it reused the authorization and commitment
machinery already paid for by each block.

The integrated research relation supports sixteen contract-capable positions.
Zero, one, four and sixteen real calls use the same matrix. The complete B25
HistoryStep remains inside its existing 2^22 domain with 5,505 rows remaining,
and B255 keeps all 255 live authorization positions. The result establishes a
viable direction. It does not yet define the production ABI, freeze a new
terminal proof, or extend the current soundness certificate.

## Question

Can Parano1d add useful programmable State objects without giving up its fixed
recursive HistoryStep, compact transaction body, or independent verification
on ordinary hardware?

## Decision

Continue with a bounded proof-native object-contract layer.

Reject the two naive alternatives measured during this study:

1. Do not append a general interpreter directly to every HistoryStep.
2. Do not recursively verify one complete generic proof per contract call.

The viable construction reuses the proof machinery already present in each
block. The existing authorization capsule proves the authority selected by an
object policy. The enclosing HistoryStep authenticates the program and object,
executes the bounded transition, binds it to the actual transaction fields and
enforces the resulting State effect.

An integrated research relation now supports a fixed envelope of sixteen
contract-capable positions. Zero, one, four and sixteen real calls all use the
same matrix. The B25 HistoryStep remains inside its existing 2^22 domain with
5,505 rows remaining. B255 preserves all 255 live authorization positions.

This is a feasibility result, not a release decision. No terminal proof has
been frozen for the changed relation, the instruction set and receipt format
are not final, and the end-to-end post-quantum soundness calculation has not
been repeated.

## Prototype architecture

The prototype keeps the current 323-byte Tx8x2 body. It does not add a second
proof type or a per-contract proof payload.

Each contract call consumes one State object. Its owner-sized commitment binds:

- an eight-step program with two field elements per step
- the current and successor object State
- claim and recovery authorities
- a deadline
- claim and recovery recipients
- an object-format version

The existing validity bitmap marks a body as a contract call and distinguishes
continuing from terminal transitions. Contract calls occupy a canonical prefix
of the block's ordinary body positions. A continuing call creates a successor
object. A terminal call closes the object and pays the recipient selected by
the deadline branch. One second output remains available for a public side
effect on a continuing call.

The program is a fixed eight-step field machine with four opcodes. Every step
can read one of eight contexts taken from the actual transaction body. The
matrix therefore proves the program against the transaction that will enter
the block, not against a detached claim supplied by the prover.

Before the committed deadline, the claim authority is selected. At or after
the deadline, the recovery authority is selected. The ordinary wallet capsule
proves knowledge for that selected address. HistoryStep derives the branch
from its authenticated block height and enforces the matching terminal
recipient.

Program, policy and object commitments occupy the unused tail of the existing
64-slot Meta-A transaction allocation:

- slot 0 of the tail remains the Tx8x2 body wrap
- slots 1 through 8 commit the program
- slots 9 through 14 commit the policy
- slots 15 and 16 commit the old object
- slots 17 and 18 commit the successor object
- slots 19 through 31 remain available

The Meta-A domain does not grow. Missing contract calls use canonical ghost
openings, so block contents do not change the proof shape.

## Alternatives eliminated by measurement

### Complete recursive replay

A fixed inner C1 relation containing eight Poseidon2b permutations was proved,
then its complete proof was verified inside another C1 relation. The outer
relation included every PCS leaf and Merkle-path check.

| Item | Result |
|---|---:|
| Inner relation | 2,889 useful / 4,096 padded rows |
| Inner expanded proof | 212,336 bytes |
| Outer complete replay | 2,412,563 useful / 4,194,304 padded rows |
| Outer verifier rows | 2,399,039 |
| Outer proof | 519,568 bytes |
| Outer proving | 3,217 ms |
| Outer native verification | 1,321 ms |
| Whole-process peak RSS | 1,900,560 KiB |

Adding this verifier to the existing B25 relation necessarily crosses from
2^22 to 2^23 rows. Removing leaf and path hashing leaves 202,103 rows but also
leaves 266 leaf obligations and 266 path obligations unresolved. That is not a
proof and still exceeds the available B25 budget by more than twenty times.

[Raw recursive replay](../2026-09-15-recursive-replay-8x1.json)

### Direct interpreter

A deliberately optimistic fixed-matrix interpreter was measured before any
object authorization, origin rule, memory model or HistoryStep integration was
added.

| Steps | Useful rows | B25 arithmetic projection | Remaining before 2^22 |
|---:|---:|---:|---:|
| 8 | 2,956 | 4,188,228 | 6,076 |
| 16 | 5,900 | 4,191,172 | 3,132 |
| 24 | 8,844 | 4,194,116 | 188 |
| 32 | 11,788 | 4,197,060 | crossed |
| 64 | 23,564 | 4,208,836 | crossed |

Even the toy relation exhausts the B25 domain at 24 steps. A separate general
VM paid by every block is therefore the wrong integration point.

Raw results:
[8 steps](interpreter-8-8.json),
[16 steps](interpreter-16-16.json),
[24 steps](interpreter-24-24.json),
[32 steps](interpreter-32-32.json),
[64 steps](interpreter-64-64.json).

## Route exploration

Several smaller experiments established the pieces required by the integrated
relation.

The native authorization-capsule experiment placed an eight-step program in
the unused portion of the existing 2^11-cell private bank. It kept the five
dynamic terminal claims and produced proofs between 88,856 and 90,104 bytes.
This demonstrated available private-proof capacity, but it was not selected as
the final composition boundary. Transaction contexts and State effects are
more naturally visible to HistoryStep, while the capsule retains its original
job of proving authority.

[Raw authorization-capsule experiment](universal-policy-capsule-full-v2.json)

The Meta-A experiments established a fixed program and object commitment tail
without enlarging the existing domain or adding another sponge walk.

[Raw Meta-A schedule](contract-meta-wrap-v2.json)

The transaction and State binding experiment established that the existing
body still encodes to exactly 323 bytes and that every program field, object
field and selected transaction context changes its authenticated commitment.

[Raw transaction and State binding](contract-abi-binding.json)

The timed-policy experiment accepted a claim before deadline, accepted recovery
at and after deadline, and rejected the wrong key, wrong recipient and invalid
terminal shape.

[Raw timed-policy relation](timed-covenant-rows.json)

## Integrated B25 direct relation

The final research path was assembled inside the actual direct-block relation,
including transaction bodies, selected authorization, Meta regions, fees,
exact State and header binding.

| Real calls | Useful rows | Matrix digest | Satisfied |
|---:|---:|---|:---:|
| 0 | 2,226,648 | `8ae7ec37...06584` | yes |
| 1 | 2,226,648 | `8ae7ec37...06584` | yes |
| 4 | 2,226,648 | `8ae7ec37...06584` | yes |
| 16 | 2,226,648 | `8ae7ec37...06584` | yes |

The identical row count and digest establish one content-independent relation
for the measured envelope. The actual number of contract calls is witness data,
not a proof-shape selector.

Five adversarial cases were checked:

| Mutation | Result |
|---|---|
| Invalid opcode opening | matrix unsatisfied |
| Wrong successor State | matrix unsatisfied |
| Wrong transaction context | matrix unsatisfied |
| Wrong terminal recipient | matrix unsatisfied |
| Authorization proof for the wrong deadline branch | rejected during native authorization preparation |

[Raw B25 result](heterogeneous-b25-history-step.json)

## Saturated B255 relation

B255 was tested with 26 live user pages, the minimum occupancy that selects the
complete fixed B255 class.

| Real calls | Useful rows | Matrix digest | Satisfied |
|---:|---:|---|:---:|
| 0 | 14,402,324 | `39c62a8b...21f01` | yes |
| 1 | 14,402,324 | `39c62a8b...21f01` | yes |
| 4 | 14,402,324 | `39c62a8b...21f01` | yes |
| 16 | 14,402,324 | `39c62a8b...21f01` | yes |

The contract-capable prefix replaces no authorization position. The first
sixteen existing positions select either wallet-owner or object-policy
authority semantics. A separate saturation test retained all 255 live B255
positions plus its one dyadic pad for both zero and sixteen contract calls.

[Raw B255 result](heterogeneous-b255-direct.json)

## Complete B25 HistoryStep

The full B25 composition includes the parent-proof replay, current direct
block, recursive matrix claims and outer HistoryStep glue.

| Item | Result |
|---|---:|
| Useful rows | 4,188,799 |
| Fixed domain | 4,194,304 |
| Remaining rows | 5,505 |
| Matrix digest | `f0126ac7...9fb4f` |
| Complete witness | satisfied |

This is an exact integrated matrix build and satisfaction scan. It is stronger
than adding independent row estimates. It is not a newly frozen recursive
terminal proof and does not measure production proving latency. The complete
parent composition used the canonical zero-call launch witness. The sixteen-call
witness was checked in the identical fixed direct relation, whose matrix is the
current-block component of that composition.

## The alignment cliff

The first sixteen-slot layout technically fit, but left only 81 rows. A sweep
over the fixed envelope exposed a discontinuity:

| Fixed slots | Full useful rows | Remaining rows |
|---:|---:|---:|
| 4 | 4,185,503 | 8,801 |
| 8 | 4,185,679 | 8,625 |
| 9 | 4,185,723 | 8,581 |
| 10 | 4,185,767 | 8,537 |
| 11 | 4,194,003 | 301 |
| 12 | 4,194,047 | 257 |
| 16 | 4,194,223 | 81 |

The eleventh slot did not add eight thousand semantic constraints. It shifted
the starting position of six dyadically aligned committed regions and forced
an additional 8,192-row gap.

The dependency graph was then made explicit. Only deadline evaluation and the
selected authorization address must exist before the committed authorization
region. Program execution, transaction contexts, terminal recipient and the
contract-prefix shape can be constrained afterwards. Moving those 2,768 real
constraints recovered 5,424 rows at the sixteen-slot envelope. Nothing was
removed from the relation.

The final split is:

| Contract component | Rows for 16 fixed slots |
|---|---:|
| Deadline policy before committed region | 4,112 |
| Execution and shape after committed region | 2,768 |
| Execution portion per slot | 144 |

This result is also a warning against treating raw row addition as a complete
capacity model. Alignment and recursive composition have to be measured in the
actual HistoryStep.

## Timing boundary

The fixture-preparation timings include deterministic transaction construction
and sequential creation of user authorization proofs. In production those
proofs are supplied by users. These numbers are not miner block latency.

The direct `build_and_scan` measurement constructs the matrix and evaluates all
constraints. It is not recursive proof construction. Normal B25 samples were
about 1.56 to 2.08 seconds and B255 samples about 9.42 to 9.69 seconds. One B25
sample ran under transient contention and was retained in the raw data rather
than used for comparison. Each case has only one sample, so no latency tail or
throughput claim follows.

## Validation

Before scaling the envelope, the shared prototype passed:

- 450 `noid-ivc-core` tests, with 11 explicitly ignored
- 204 `noid_gkr` tests
- 400 `noid_recursive` tests, with 3 explicitly ignored
- 24 `noid_tx` tests
- 3 `noid_block` tests

The large recursive test suite required a 16 MiB test-thread stack. The exact
test that exhausted the default stack passed under that setting, followed by
the complete suite.

After scaling to sixteen slots and reordering the relation, the affected five
authorization-capability tests, all three block tests, all 24 transaction
tests, formatting, static checks, both direct matrices and the complete B25
matrix were rerun successfully.

[Validation record](heterogeneous-validation.json)

## What this establishes

The experiment establishes that a useful, fixed-shape object-contract relation
can coexist with the current B25 domain. It can preserve the 323-byte body,
reuse existing authorization proofs, retain ordinary B25 and B255 capacity,
support continuing and terminal State objects, and enforce an exact
claim/recovery deadline policy.

It does not establish arbitrary computation, dynamic code loading, unbounded
storage, synchronous cross-contract calls or general atomic composition.
Current Tx8x2 has one shared input owner, so two independently controlled
mutable objects still require an authorization or transaction redesign.

## Remaining gates

Before any v2 protocol selection:

1. Freeze the instruction semantics, object receipt and canonical wire format.
2. Define code migration, construction and origin rules.
3. Produce a new frozen matrix pack and recursive terminal proof.
4. Repeat the complete post-quantum soundness calculation for the changed
   relation and query schedule.
5. Measure production proving, verification, peak memory and propagation.
6. Bound invalid-proof admission work before fees can be collected.
7. Measure stale-proof retries and contention on shared mutable objects.
8. Specify current-object witness availability after old transaction bodies
   have been deleted.
9. Define and authenticate the transition from the current mainnet relation.

The first-week result is therefore a selected research direction and an
integrated feasibility proof. It is not yet a consensus specification.

# Joint m23/m24 bank with the integer contract core

This is a local candidate with real recursive proofs, not a released matrix
bank. It includes the [sixteen-instruction integer core](../../CORE_CANDIDATE.md),
both predecessor classes, an authenticated legacy origin and a configurable
contract-call envelope. The earlier 112-page measurements used a different,
single-class relation and do not describe this candidate.

## Complete relation bounds

The isolated schedule activates v1.1 at height 5 and v2 at height 10, with
30-second v2 intervals. These matrices cannot be relabeled as mainnet artifacts:
the schedule belongs to the authenticated relation.

| Class | User pages | Live input budget | Call positions | Useful rows | Row headroom | Compressed matrix | Terminal upper bound |
|---|---:|---:|---:|---:|---:|---:|---:|
| m23 | 63 | 504 | 63 | 6,936,149 | 1,452,459 | 10,458,448 B | 1,014,132 B |
| m24 | 255 | 1,020 | 26 | 16,764,276 | 12,940 | 21,892,176 B | 1,081,396 B |

User pages exclude the primary coinbase. A required additional system record
shares the user envelope. Input and call budgets are independent limits: the
larger page class is not a superset of the smaller class's contract capacity.
The matrix reserves all call positions even in an empty block.

The primary coinbase makes the body/authorization domain cross a power-of-two
boundary at 64 user pages. Spare rows in the 63-page candidate therefore do
not translate linearly into additional pages. This report establishes a fitting
candidate, not an impossibility bound for other designs.

Full joint probes at 255 pages exceeded m24 with 63 calls and either 1,020
inputs (17,272,101 rows) or 384 inputs (17,091,437 rows). The 1,020-input,
32-call relation used 16,846,626 rows, also exceeding m24. Each additional call
in this large-class envelope costs 13,725 rows. Reducing the input budget may
permit a different call limit; no unmeasured alternative is qualified here.

Both terminal upper bounds fit the current 1,100,000-byte terminal limit.
This does not qualify combined body/terminal response budgets or production
transport; see the [network budget audit](../../NETWORK_BUDGETS.md).

## Proof sequence

The driver reverified legacy blocks 1–9, froze both complete matrices, resolved
the common parent and per-class block verifier keys, then reassembled and checked
both matrices against their final bank pins. It constructed and verified blocks
10–37, including all four predecessor/current class combinations, ordinary and
contract funding, full ordinary blocks, full small blocks with 1, 4 and 63 calls,
a full large block with 26 calls, and a small-block tail after that large block.
The complete matrix was rebuilt and checked for each class transition and the
first full call envelope in each class; its digest remained unchanged.

The host was an Intel Core i7-1365U laptop using 12 Rayon threads and the
automatically selected AVX2/VPCLMUL backend. Samples below are individual
observations, not medians or a sustained throughput guarantee. Ordinary
measurement payments use one input and one output. Calls retain a successor
and exercise the integer program; this is not a maximum-input or maximum-segment
workload.

| Height / case | Input preparation | Recursive assembly | Proving | Sum excluding PoW | Verification | Terminal |
|---|---:|---:|---:|---:|---:|---:|
| 11 / empty small | 0.002 s | 5.981 s | 9.865 s | 15.848 s | 0.769 s | 914,452 B |
| 31 / 63 small calls | 1.657 s | 6.984 s | 10.181 s | 18.822 s | 1.227 s | 915,796 B |
| 32 / 255 ordinary pages | 6.188 s | 15.481 s | 29.923 s | 51.592 s | 3.097 s | 978,964 B |
| 33 / 255 pages, 26 calls | 6.294 s | 18.184 s | 27.147 s | 51.625 s | 2.706 s | 981,044 B |
| 34 / 63 calls after large | 1.583 s | 9.076 s | 9.484 s | 20.143 s | 1.488 s | 914,516 B |

The sums exclude independent wallet authorizations, nonce search, matrix freezing
and matrix audits. The first use of each class in this run also included lazy
matrix authentication in assembly; heights 10 and 12 must not be interpreted as
steady-state construction latency. Subsequent driver versions report this
authentication as a separate setup phase.

After the large-to-small transition at height 14, verification took 2.243 s;
the next three small blocks took 0.678, 0.744 and 0.698 s. After the filled large
case and its first small successor, the final three small blocks took 0.755,
0.958 and 1.021 s. The checked-claim cache skips only an exactly unchanged,
previously verified carried matrix claim. Every terminal still checks its fresh
matrix claim and proof. The cache is bounded and process-local; a cold receiver
must establish the carried claim independently.

Process lifetime peak RSS was 8,367,556 KiB. This includes matrix freezing,
complete matrix audits and both provers; it is not a receiver memory requirement.
All observations are retained in [proving.jsonl](proving.jsonl); executable and
source hashes are recorded in [proving-source.json](proving-source.json).
Constrained receiver measurements are separate from these prover-process results.

## Identity and remaining qualification

- Source baseline: `7e04b528`, with the joint-bank changes recorded in the
  accompanying source-hash manifest.
- Proving executable SHA-256:
  `df6a451d56fedac3721ffd7262f80e1c14e82f4edc45c7279d788a9b54edff89`.
- Bank: `e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
- Small matrix: `bbe4c8033ca3e2eb33cdf5b31521cdcf9cd926716857bf0094a8bc6fdef8ea32`.
- Large matrix: `b48f87f12d3faf7f346f2032723038ae6fd1bcec3f7d7403832b281de61b84eb`.

The proof run rejected mutations of every public-IO coordinate, class/version
changes, truncation and trailing data. The subsequent receiver check also
tests combined class substitution, canonical erasure of a live accumulated lane,
and a mismatched sealed header. Receiver qualification independently replays
the saved fixture using the candidate schedule's native ASERT anchor selection;
this replay is a fixture audit, not an archive requirement for proof verification.

Final capacities remain a separate decision. Required production work includes
both legacy boundary classes, resource-distribution boundaries, the live node's
combined caches/database/P2P workload, reorg/restart/cold-sync/pruning, authenticated
legacy retirement and the final bank's soundness inventory. Successful local
proofs do not establish that this bank is ready for release.

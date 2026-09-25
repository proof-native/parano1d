# Large-class input and contract-call tradeoff

The complete isolated joint bank fits 255 user pages, 384 live inputs and
40 calls in m24. A full block containing 215 ordinary payments and 40 integer
program calls was proved and verified. This is an alternative candidate;
neither its input reduction nor its call limit is a release selection.

## Capacity

| Candidate class | User pages | Live inputs | Contract calls | Useful rows | Row headroom |
|---|---:|---:|---:|---:|---:|
| Small m23 | 63 | 504 | 63 | 6,936,161 | 1,452,447 |
| Large m24 | 255 | 384 | 40 | 16,775,762 | 1,454 |

Pages, inputs and calls are simultaneous independent limits. Calls occupy user
pages: Small supports 63 ordinary one-page transactions, 63 calls, or a mixture
totalling 63 pages. Large supports 255 ordinary one-page transactions, at most
40 calls, or a mixture totalling 255 pages with no more than 40 calls. A block
containing only 40 calls leaves 215 Large page positions unused. The additional
primary coinbase is outside these user-page counts.

The [earlier candidate](../2026-09-24-banked-integer-core/REPORT.md) instead
supports 1,020 Large inputs and 26 calls. Both retain 255 Large user pages.
This experiment exchanges 636 input positions for 14 call positions. It does
not increase Small capacity or establish a proving-speed improvement. Large
is not a superset of Small: its input and call limits are both lower here.

Both matrices were rebuilt with the final common bank pin and satisfied their
witnesses. Compressed sizes are 10,453,140 and 21,510,878 bytes. Terminal bounds
remain 1,014,132 and 1,081,396 bytes, within the 1,100,000-byte limit.

## Proof sequence and timing

The runner authenticated legacy blocks 1–9, activated v2 at height 10 and
constructed 28 new blocks through height 37. It covered all four class
transitions, full ordinary blocks, 1/4/63 Small calls, 40 Large calls and Small
successors after Large. Complete matrix audits checked the class transitions
and full call envelopes. The terminal mutation checks rejected 2,691 cases.

Host: Intel Core i7-1365U laptop, 12 Rayon threads, AVX2 + VPCLMUL, a 20 GiB
process-group memory limit and no swap for that group. No compiler or other
project benchmark ran concurrently. The host remained a normal desktop.
Each case below is one sample. Class 0 is Small; class 1 is Large.

| Height | Class | Pages | Calls | Input + assembly + proving | Verification | Terminal |
|---|---:|---:|---:|---:|---:|---:|
| 28 | 0 | 63 | 0 | 17.933 s | 0.761 s | 914,580 B |
| 31 | 0 | 63 | 63 | 23.087 s | 1.096 s | 914,228 B |
| 32 | 1 | 255 | 0 | 55.144 s | 3.333 s | 978,292 B |
| 33 | 1 | 255 | 40 | 49.978 s | 3.068 s | 983,636 B |
| 34 | 0 | 63 | 63 | 20.548 s | 1.559 s | 915,284 B |
| 35 | 0 | 0 | 0 | 15.735 s | 0.720 s | 910,996 B |
| 36 | 0 | 0 | 0 | 18.399 s | 1.012 s | 914,580 B |
| 37 | 0 | 0 | 0 | 21.554 s | 0.942 s | 915,188 B |

Construction excludes wallet authorizations, nonce search, matrix generation,
authentication and audit scans. Payments in this run use one input and one
output; calls consume one object and retain its successor. These are not
maximum-input or maximum-segment cases. The results apply to this exact pinned
bank and isolated schedule.

Whole-run elapsed time was 2,075.068 seconds. Matrix generation took 273.479 s;
subsequent matrix authentication took 192.028 s, both outside the block timing
column. Process peak RSS was 8,286,388 KiB, including generation and proving;
it is not a receiver memory measurement. This alternative has not yet received
the full-daemon or constrained-receiver qualification of the earlier candidate.

## Reproduction and identity

Use the [isolated joint runner](../../CAPACITY_MEASUREMENTS.md) with limits
`63 504 63 384 40`, a fresh output directory and authenticated H1–H9 fixtures.
The final bank is
`8510e012152310a68a2e58a028368a8855e31c658f6a8baae215c1baa6871603`.
Matrix identities, every observation and the completion record are in
[proving.json](proving.json). [source.json](source.json) records the saved
executable hash, its earlier qualification provenance, command and run status.
The H10 bank cannot be relabeled as a mainnet bank.

## Full input envelopes across State segments

A separate continuation authenticated the same bank and H37 terminal from the
legacy origin, replayed the matching fixture State and then produced H38–H52.
It funded outputs through valid transactions and exercised all 256 State
segments. Each maximum-input case rebuilt the complete matrix, checked its
unchanged digest and satisfied the witness before proving and verifying the
terminal. No State entries were injected.

| Height | Class | Pages | Inputs | Outputs | Input + assembly + proving | Verification | Terminal |
|---|---:|---:|---:|---:|---:|---:|---:|
| 43 | 0 | 63 | 504 | 126 | 18.521 s | 0.763 s | 908,980 B |
| 46 | 1 | 96 | 384 | 192 | 49.110 s | 2.891 s | 982,132 B |
| 48 | 1 | 48 | 384 | 96 | 50.700 s | 2.769 s | 980,884 B |
| 51 | 1 | 255 | 384 | 510 | 56.224 s | 3.527 s | 982,996 B |

The full Large case simultaneously uses all 255 pages and all 384 inputs,
with 510 outputs. A 385-input body on valid pages was rejected by native
admission; its complete R1CS witness was unsatisfied under the same matrix.
Empty successors also passed. These are individual samples on the same
12-thread host profile, not latency percentiles or four-CPU receiver results.

The continuation took 1,825.148 seconds including fixture authentication,
wallet authorizations, funding, audits and nonce search. Peak process RSS was
9,734,840 KiB across both producers and the matrix audits; this is not a seed
verification requirement. No compiler ran alongside the benchmark.

[Raw observations](input-budgets.json) and [source identity](input-budget-source.json)
retain the exact executable and source-file hashes. The [qualification patch](input-budget-source.patch)
applies to the recorded base commit and reconstructs the four modified driver
files. This bank and executable predate the subsequent height-only v2 emission
revision; their measurements do not qualify the revised relation.

# Joint v2 and legacy-retirement soundness accounting

The separate `noid_v2_soundness` calculator evaluated the current H10 joint
bank and both independently pinned legacy preprocessing keys. It inventories
wallet authorization, both old History classes, both new History classes and
the two sparse retirement arguments. The published default legacy calculation
and all protocol parameters remain unchanged.

The result is conditional on the correspondence, all-root extraction,
fixed-Poseidon2b and response-price premises in the
[derivation](../../../../noid_soundness/docs/v2-retirement.md). This calculation
does not replace those premises with a claim based on test coverage.

| Conditional quantity | Result |
| --- | ---: |
| Typed failure-event entries | 20 |
| Largest sequential ideal-QROM query budget below half success | 30,103,381,624,534,947,556 |
| Ideal success upper bound at `T = 2^64` | 0.187749539660, rounded upward |
| Dominant gate-depth work floor, descriptive bits | 173.3897612554 |
| Complete ideal Category 1 envelope | 0.049373883734, rounded upward |
| Fixed-Poseidon2b delta headroom | greater than 0.450626116266 |

The limiting resource event is `retirement.b25.query`. Both sparse keys have
four prover-selected initial commitments and seven honestly preprocessed
columns. Initial candidate tuples are bounded by `L^4`. The conservative
memory-polynomial envelopes are 167,772,160 and 671,088,640 roots. The price
uses the cheapest actual column's query response, twelve sequential
permutations, rather than the cost of its longest codeword. Exact rational
values, every column geometry and the complete event inventory are in
[accounting.json](accounting.json).

The release build and all 39 calculator tests passed, including the unchanged
legacy reference results. The CLI rejected an incorrect bank pin, an incorrect
preprocessing-key pin and swapped class keys before printing a certificate.
See [negative-checks.json](negative-checks.json) and [source.json](source.json).

This result identifies candidate bank
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.
The final source-scheduled mainnet pack must be passed through the same tool
with its own bank pin after capacities are chosen and the matrices are frozen.

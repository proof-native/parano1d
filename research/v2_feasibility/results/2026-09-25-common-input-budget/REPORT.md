# Shared input and contract limits

The two-class candidate fits 63 user pages in m23 and 206 user pages
in m24, with the same limits of 504 live inputs and 63 contract calls in both
classes. Both the isolated and mainnet-scheduled matrices have been frozen
and checked. This preserves the standard class and makes the larger class a
capacity superset. It trades five pages from the 211-page, 384-input Large
candidate for 120 additional input positions.

## Capacity and interpretation

| Class | User pages | Live inputs | Contract calls | Useful rows | Row headroom |
|---|---:|---:|---:|---:|---:|
| Small m23 | 63 | 504 | 63 | 6,936,668 | 1,451,940 |
| Large m24 | 206 | 504 | 63 | 16,776,698 | 518 |

This table uses the source-pinned mainnet schedule at H210537. The H10
isolated schedule uses eleven fewer rows per matrix.

These are simultaneous limits. Calls use the same page budget as payments;
they are not an additional allowance. The primary coinbase has its own page.

| Workload | Small | Large |
|---|---:|---:|
| Ordinary one-page payments, one or two inputs each | 63 | 206 |
| Ordinary one-page payments, eight inputs each | 63 | 63 |
| Contract calls | 63 | 63 |
| Full-call mixed block | 63 calls | 63 calls + 143 one-page payments |

The mixed example also fits when each payment consumes two inputs. The common
504-input budget applies to the sum of all transaction inputs in the block,
not to each transaction separately. Multi-page transactions consume several
page positions.

At the 30-second target, the one-page capacity ceilings are 2.1 and about
6.87 transactions per second respectively. Both classes allow at most 2.1
contract calls per target second. These are arithmetic per-class ceilings,
not measurements of sustained network throughput. Larger-class mining remains
an explicit server option; the standard class already supports the same
contract interpreter and templates.

## Complete isolated matrix construction

The runner authenticated the existing H1–H9 fixture chain, then constructed
both complete recursive relations for v2 at H10. This includes the annual
height-based emission rule, both predecessor-class branches and the full
contract interpreter. The matrices converged over two passes, satisfied their
witnesses and were rebuilt with their final bank pin. Compact runtime recipes
round-tripped without changing that pin.

The adjacent larger candidate does not fit:

| Large pages, with 504 inputs and 63 calls | Useful rows | Result against m24 |
|---|---:|---:|
| 206 | 16,776,687 | Fits, 529 spare |
| 207 | 16,778,193 | 977 over |
| 209 | 16,781,223 | 4,007 over |
| 210 | 16,782,703 | 5,487 over |
| 211 | 16,784,497 | 7,281 over |

Compressed matrices occupy 10,464,568 and 21,475,021 bytes. Their terminal
upper bounds are 1,014,132 and 1,081,396 bytes, both below the 1,100,000-byte
transport limit. Matrix-file sizes are not network terminal sizes.

The isolated bank is
`be82f3bec102f03c63715dac9bb4a939cb9aa5b21d013e38402b321fd8a41fa9`.
See [candidate records](comparison.json) for matrix identities, commands,
executable hashes and construction results. The [source patch](isolated-source.patch)
reconstructs the successful isolated measurement source on its recorded base.

## Mainnet-scheduled pack

The normal mainnet build froze both matrices with the existing v1.1 boundary
at H95125 and v2 at H210537. It then rebuilt each matrix with the final bank
pin and two distinct hypothetical boundary states. All four witnesses were
satisfied and retained the same class digests. The pack loader authenticated
both compressed matrix files and checked their terminal bounds.

The mainnet bank is
`c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e`.
Its compressed matrices occupy 10,465,911 and 21,478,342 bytes. Terminal bounds
are the same as for the isolated bank. [Mainnet records](mainnet.json) contain
the exact pins, file hashes, build identity and successful checks; the
[mainnet source patch](mainnet-source.patch) reconstructs that build's source.
Ten targeted tests passed for configuration identities, complete registry
certificates, miner resource budgets and pre/post-fork RPC class reporting.

These runs freeze and check the full matrices; they do not produce new v2
terminals. The mainnet freezer creates no verified fork origin and does not
claim to know the future predecessor block. Earlier full-block and constrained
receiver measurements used different banks and do not qualify this candidate.
Full production qualification is still required.

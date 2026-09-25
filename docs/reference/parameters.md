# Protocol parameter reference

Mainnet switches to the following v2 profile at **H210537**. Activation follows
block height; calendar dates are estimates.

| Parameter | Scheduled v2 value |
|---|---:|
| Block target | 30 seconds |
| ASERT half-life | 180 seconds |
| Small `m=23` | 63 pages, 504 inputs, 63 calls |
| Large `m=24` | 206 pages, 504 inputs, 63 calls |
| Large production | Server opt-in: `--v2-large-blocks` |
| Gross subsidy at activation | 16 NOID |
| Issuance interval | 1,051,200 blocks from activation |
| Issuance floor | 1 NOID |

Inputs are a whole-block budget and calls share the page budget with payments.
Both classes use the same contract interpreter. See the
[scheduled consensus profile](../protocol/parameters.md#scheduled-v2-profile)
for combined limits and unchanged windows, and the
[height-based issuance schedule](../protocol/economics.md#v2-issuance) for all
nine amounts. `getContractProtocol` reports the installed class limits.

The sections below retain the **released v1.1 profile before H210537**, including
its proof-system accounting and physical transaction wire bounds.

## Core

```text
block target             20 seconds
ASERT epoch               6 blocks
ASERT half-life         120 seconds
median-time-past         11 headers
future drift            120 seconds
transaction epoch       144 blocks
hard finality            18 blocks
```

## State

```text
initial domain           2^24 slots
maximum domain           2^32 slots
segment                  2^16 slots
expansion threshold      75%
finalized sample         18 headers
expansion majority       10 / 18
```

## Transactions and blocks

```text
Tx8x2 page               8 inputs / 2 outputs / 323 bytes
PagedSpend               1..128 pages
logical input cap        1,020
logical output cap       256
block body cap           256 fixed bodies
ordinary user pages      255
payout-block user pages  254
block user outputs       510
touched segments         256
```

## Proof system

```text
committed trace field    GF(2^128)
wide challenge field     GF(2^256)
challenge support        2^255
Poseidon2b width         4
S-box                    x^7
full rounds              8
partial rounds           58
B25                      m=22, up to 25 positions
B255                     m=24, up to 255 positions
wallet queries           65
History/BaseFold queries 133
B25 codeword             2^19 at rate 1/4
B255 codeword            2^21 at rate 1/4
serialized authorization max 92,696 bytes
target FRI security      128 bits
Block–Tiwari provable    127 bits
Block–Tiwari conjectured 127 bits
ideal-QROM boundary      64.707407428576 bits
NIST Post-Quantum Cryptography Category 1
Category 1 gate-depth floor 173.391078499301 bits
```

## Monetary

```text
1 NOID                   1,000,000 μNOID
initial subsidy          50 NOID
subsidy floor            1 NOID
base fee                 5,000 μNOID
input fee                100 μNOID
output fee               700 μNOID
base growth fee          2,500 μNOID / net-new slot
```

The explanatory and boundary rules are in
[Consensus parameters](../protocol/parameters.md).

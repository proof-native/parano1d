# Transaction protocol

An ordinary user transaction is one canonical logical `PagedSpend`. It consists
of one to 128 fixed `Tx8x2` pages and exactly one detached authorization capsule.

## Physical page

Each page has the fixed 323-byte body encoding:

| Field | Meaning |
|---|---|
| `epoch_anchor` | Current 144-block replay-protection anchor |
| `fee` | Logical fee on the first page; zero on continuations |
| `input_owner` | Owner shared by every live input |
| `inputs[8]` | Possible `{slot_index, amount, creation_id}` records |
| `outputs[2]` | Possible `{slot_index, amount, owner}` records |
| `validity_bitmap` | Live records and logical start/end markers |
| `is_coinbase` | False for every user page |

Bits 0–7 select inputs and bits 8–9 select outputs. Bits 10 and 11 mark the
start and end of a logical group. No other bitmap bit may be set.

A record whose live bit is zero must be the all-zero canonical dummy. This
removes alternate encodings of the same statement.

## Group validity

A valid `PagedSpend` satisfies all of these rules:

- page count is between one and 128;
- only page zero has the start marker;
- only the final page has the end marker;
- every page has the same `input_owner` and `epoch_anchor`;
- only the first page carries a non-zero fee;
- live inputs and outputs are packed densely in page order;
- the page count is minimal for those records;
- at least one input is live;
- no input appears twice;
- no output slot appears twice;
- an output slot is not also an input slot in the same spend;
- there are at most 1,020 inputs and 256 outputs;
- total input value equals total output value plus the fee;
- the detached authorization is no larger than 256 KiB.

The maximum canonical intent encoding is 303,495 bytes.

These physical bounds remain valid for decoding legacy history. At and after
mainnet H210537, a transaction must also fit an installed v2 class: Small has
63 pages and Large has 206; both share a whole-block limit of 504 live inputs.
Wallet planning and mempool admission enforce the candidate height's limits.

## Logical transaction ID

Page-local body hashes do not independently name the payment. The logical ID
is a domain-separated Poseidon2b sponge over:

```text
version || page_count || ordered_page_hashes
```

The launch version is `1`. Reordering, adding or removing a page changes the
ID. The authorization capsule proves knowledge of the preimage behind
`input_owner` for that exact logical ID.

## State checks

For each input, the current slot must be occupied by the declared owner,
amount and `creation_id`. For each output, the target slot must be empty and
inside the active slot domain.

Output `creation_id` values are not transaction fields. Consensus assigns them
from the post-State allocation sequence. A user cannot choose or reuse one.

The block transition enforces all State checks atomically for the complete
logical group.

## Fees

The minimum fee is:

```text
5,000
+ 100 × live_inputs
+ 700 × live_outputs
+ growth_price × max(0, live_outputs - live_inputs)
```

All amounts are in μNOID. `growth_price` begins at 2,500 μNOID per net-new
slot and is multiplied by 1, 2, 4 or 8 according to parent-State occupancy.

The State-growth component is burned. Any fee above the required minimum is a
miner-claimable tip. State-preserving and State-shrinking spends have no growth
burn.

## Epoch anchor

Every page uses the current transaction-epoch anchor. For child height `C`,
the anchor is the block ID at:

```text
floor((C - 1) / 144) × 144
```

for `C > 0`. Height 144 still consumes the height-zero anchor; height 145
begins using height 144. The mempool removes intents whose anchor is no longer
current.

## Authorization and State are separate

Wallet authorization proves ownership only. It does not prove current slot
membership and contains no Live State Merkle path.

The block `HistoryStep` proves current membership, emptiness, balance,
allocation and the post-State root. Both statements bind the same public
logical transaction.

## V2 contract calls

V2 also accepts a call that supplies the public opening of a committed contract
output. A call consumes one page, spends one exact output incarnation, and can
make a permitted payment, create a successor with updated counters or close.
The opening determines the spending or recovery authority at the candidate
height. Authorization proves knowledge of that authority's secret; the block
relation checks the opening's commitment, program, rules and exact successor.

Calls form a canonical prefix of the selected user pages. Both v2 classes
allow up to 63 calls, sharing page and input budgets with ordinary payments.
The [contract reference](../reference/contracts.md) specifies the integer core,
templates, APIs and retained public terms. The
[scheduled parameters](parameters.md#scheduled-v2-profile) give combined limits.

## System records

Primary reward and scheduled development payout records use the fixed physical
body type but are not user `PagedSpend` intents. Their shape, placement,
recipients and amounts are derived entirely from block context. They carry no
wallet authorization.

See [Transaction architecture](../architecture/transactions.md) for wallet and
mempool flow.

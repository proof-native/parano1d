# Transaction architecture

Parano1d separates the physical transaction page carried by a block from the
logical payment authorized by a wallet.

The physical record is `Tx8x2`: at most eight inputs and two outputs in a
fixed 323-byte encoding. A logical `PagedSpend` joins up to 128 of those pages
into one atomic intent with one transaction ID and one authorization capsule.

| Layer | Limit | Purpose |
|---|---:|---|
| `Tx8x2` page | 8 inputs, 2 outputs | Fixed consensus and proof record |
| `PagedSpend` | 128 pages | One atomic wallet operation |
| Logical transaction wire format | 1,020 inputs, 256 outputs | Maximum live records after page markers |

From mainnet H210537, v2 admission also requires the complete intent to fit
one installed class: Small has 63 pages and Large has 206; both have a
504-input whole-block budget. Physical wire bounds remain available for
legacy history. The [scheduled profile](../protocol/parameters.md#scheduled-v2-profile)
defines the combined limits.

## Fixed physical pages

Each page has one shared `input_owner`, an epoch anchor, a fee field, eight
input records, two output records and a validity bitmap. Dead records must be
canonical zeroes.

An input identifies:

```text
{slot_index, amount, creation_id}
```

An output identifies:

```text
{slot_index, amount, owner}
```

The allocator supplies a fresh `creation_id` when the output is materialized.
This prevents a stale input reference from becoming valid if its numerical
slot is later reused.

## One logical intent

The first page carries the fee and start marker. The final page carries the end
marker. Every page uses the same owner and epoch anchor, and live records must
be densely packed. Page count must be minimal for the declared inputs and
outputs.

The logical transaction ID commits to:

- protocol version;
- page count;
- every ordered physical page hash.

It is computed with a domain-separated Poseidon2b sponge. The wallet
authorization is bound to this logical ID rather than to any one page.

The entire group is admitted, selected, proved and included atomically. A miner
cannot include only the profitable pages of a larger spend.

## Wallet construction

For a normal payment, the wallet spends outputs of the active owner. It chooses
largest values first to reduce the number of proof inputs, creates the recipient
output, and returns any change to the same active owner. Output slots come from
node-provided empty-slot hints.

Construction has two proof boundaries:

1. the wallet proves knowledge of the active owner's secret;
2. the miner proves current input membership, output emptiness, value
   conservation, fee rules and the exact post-State.

The wallet proof runs outside the wallet-state lock. Mining may continue, but
the node gives local transaction work priority so a user is not forced to wait
behind continuous proving.

## Mempool admission

Admission deliberately repeats the cheap checks around expensive proof
verification:

1. decode canonical wire data and validate group semantics;
2. under the mempool lock, check fee, epoch, conflicts and the current State;
3. release the lock and verify the authorization capsule under a bounded CPU
   permit;
4. reacquire the lock and repeat the cheap State and conflict checks;
5. reserve every live input and output slot, then relay the intent.

Both cheap passes check the candidate height's class budgets. This closes the
race between authorization verification and a new block, fork boundary or
competing transaction. Duplicate submission is idempotent.

The mempool holds at most 1,024 logical transactions and 384 MiB of intent
data. Slot reservations make conflicting-input and output detection constant
time.

## Selection

Miners order admissible groups by fee rate, breaking ties by transaction ID.
Groups remain indivisible. Before v2, selection also respects:

- the active proof class, B25 or B255;
- at most 1,020 live inputs per block;
- at most 510 user outputs per block;
- at most 256 distinct State segments touched;
- the available physical page positions.

A scheduled development-reward payout uses one physical page position, leaving
254 user pages in that block. Otherwise up to 255 user pages are available.

From v2, the selected Small or Large class supplies its own page, input and
contract-call budgets. Calls form a canonical prefix of the selected user
pages. They share page and input space with payments, up to 63 calls and
504 inputs in either class. Large adds ordinary payment capacity and is
produced only when the operator permits it. Nodes admit and verify both classes
regardless of their own mining preference. See
[class selection](mining.md#scheduled-v2-profile).

## Confirmation and receipts

Submission means the local node accepted and relayed the complete intent. It
does not mean a block has included it. Once included and confirmed, the wallet
can recover a receipt from the retained block.

Same-owner consolidation is intentionally omitted from receipts. Payments to
any distinct owner, including another inactive address derived by the same
wallet, are retained as outgoing payment evidence.

For normative validity rules, see
[Transaction protocol](../protocol/transactions.md). For user-facing behavior,
see [Send NOID](../wallet/send.md).

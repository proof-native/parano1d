# Mining architecture

**Hashpower alone cannot produce blocks. Mining requires State; nonce search
begins only after the proof is complete.**
The block producer must hold the current State and establish the exact next
transition before nonce search begins. An independent miner is therefore a
proving full node backed by hashpower, not a stateless hashing endpoint.

Mining has two ordered phases:

1. select transactions, construct the State transition and prove the complete
   nonce-independent block;
2. search the 128-bit nonce of the fixed Poseidon2b header.

This order prevents proof of work from being spent on a block whose transition
has not yet been established.

## Template construction

The node starts from its canonical tip and current mempool. It selects
non-conflicting logical transactions by fee rate while respecting State,
segment and proof-class limits. It then:

- computes coinbase and any scheduled system payout;
- assigns fresh output creation identifiers;
- derives canonical slot writes;
- computes transaction and post-State roots;
- builds the `HistoryStep` public input;
- proves the new terminal.

System-mint slot selection prefers not to occupy outputs reserved by other
pending transactions. This is a bounded local preference, not a veto. If no
alternative is found, the previous valid choice is used. Selected transactions
remain strictly protected; fund recipients, payout amounts and schedules do
not change.

Everything except the header nonce is now immutable.

## B25 and B255

The proof stack ships two authenticated matrix classes:

| Class | Hypercube dimension | Effective page capacity |
|---|---:|---:|
| B25 | `m=22` | Up to 25 |
| B255 | `m=24` | Up to 255 |

Every mining session starts with B25, without a startup benchmark. The first
completed B25 proof preparation supplies the timing sample. B255 is permitted
for that session only if `prepare_time_B25 × 4 ≤ 20 seconds`. This is a
prediction, not a measured B255 time, and does not wait for a successful PoW
nonce. Later samples do not change the session's permission.

The producer selects eligible transactions in fee order within its permitted
capacity. A selection of at most 25 pages uses B25; a larger selection uses
B255. A due fund payout occupies one page, leaving up to 24 or 254 user pages
respectively. Transactions remain atomic during selection.

ASERT applies that target to the complete interval between accepted blocks.
Proof preparation, nonce search and propagation share the same interval.

The classes prove the same consensus relation. They are capacity choices, not
different block-validity rules.

## CPU scheduling

The internal miner uses one shared CPU pool across proof construction and
nonce search. It does not run two unrelated all-core jobs at once.

Local wallet work has priority. When a user sends or consolidates while mining
is active, the node pauses or yields mining CPU work at the local boundary,
completes transaction authorization and submission, then continues mining.
No network rule, peer priority or global fee policy changes.

## Refresh policy

Templates are rebuilt on events that change useful work:

- a new canonical tip;
- a mining payout-address change;
- the first transaction entering a coinbase-only template;
- invalidation of selected transactions.

A 100-second heartbeat is a safety net, not the normal refresh loop. Already
proved templates remain bound to their original payout and transaction set;
they are not mutated after proof construction.

## Internal miner

The internal miner runs inside the node process. The node owns transaction
selection, proof construction, PoW and block submission. CPU thread count and
payout address can be controlled through Core or the GUI.

Mining starts only when the node is synchronized and has at least one
authenticated peer. A locally found block is sealed, committed and announced
under the same consensus rules as a received block. Its commit path reuses the
exact locally proved State transition instead of verifying the same proof
again. Received blocks still undergo proof verification.

## External miner

The external miner moves only nonce search out of the node. It requests an
opaque, single-use template containing:

- template identifier;
- exact 16-field Poseidon2b PoW schedule;
- target;
- expiry and display metadata.

It returns one little-endian 128-bit nonce. The node verifies the nonce against
the still-live template, seals the already-proved block and invalidates the
template. External templates expire after 30 seconds and cannot be replayed
after a tip change or successful submission.

The external worker never receives authority to replace transactions, alter
State, change fees or recompute coinbase. Custom coinbase use requires both an
explicit node setting and a bearer key.

## Fork choice

Mining extends the highest-cumulative-work valid chain. Equal-work candidates
use the lexicographically smaller block hash as a deterministic tie-break.
Candidates that would replace the hard-finalized prefix are not eligible.

For deployment, see [Internal mining](../operate/internal-mining.md) or
[External miner](../operate/external-miner.md). The exact nonce relation is in
[Proof of work](../protocol/proof-of-work.md).

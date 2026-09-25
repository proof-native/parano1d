# Receipts and pruned history

Parano1d does not require every node to retain every historical block body.
That removes a growing consensus burden, but users still need a durable way to
prove that a payment was included. A receipt preserves exactly that evidence.

A receipt contains:

- the canonical logical `PagedSpend` without its authorization capsule;
- its logical position and transaction count in the block;
- a fixed-depth Merkle path in the block transaction tree;
- the claimed transaction root and block height;
- the public payment summary needed for display and checking.

The transaction tree has 256 leaves, so the inclusion path always has depth
eight.

## Two levels of verification

Receipt verification answers two different questions.

**Offline inclusion verification** checks that the transaction data, position
and Merkle path reconstruct the claimed transaction root. It needs no node and
continues to work after the block body has been pruned.

**Canonical verification** asks a live node whether the permanent header at
the claimed height has that transaction root. This establishes that the
receipt belongs to the node's current canonical chain.

A valid inclusion proof for a non-canonical block is internally consistent but
does not prove canonical payment. Wallets therefore wait for confirmation
before storing the final receipt.

## Local storage

The wallet creates an outgoing receipt after the payment is confirmed and its
block data is available. Receipts are stored locally beside the wallet data.
They are not derived from the 256-bit master secret and are not restored when
that secret is imported on another computer.

Back up receipt data separately if long-term payment evidence matters.
Exported receipt text is self-contained and can be verified by another
Parano1d node.

## Which transactions receive a receipt

A payment to a distinct recipient receives a receipt. “Distinct” is evaluated
against the input owner of that spend, not against every address the wallet
might derive.

This means:

- sending to another person's address creates a receipt;
- sending from the active address to another, inactive address in the same
  wallet also creates a receipt;
- consolidating outputs back to the same owner does not create a receipt.

The rule keeps consolidation out of the user's payment record without making
the node scan or identify the wallet's other possible addresses.

## Retention timing

Nodes retain the latest 42 complete blocks. A confirmed outgoing payment must
be recovered into the local receipt store while its body remains available.
The wallet scanner performs that recovery during ordinary operation and after
restart.

Once saved, the receipt no longer depends on the body-retention window.

For wallet use, continue with [Receipts](../wallet/receipts.md). For the
consensus transaction tree, see [Blocks and headers](../protocol/blocks.md).

## Contract receipts

The format above describes ordinary payment receipts, including contract
deposits. A contract call uses its own receipt containing public terms, the
call and recursive proof evidence. Verification can restore a successor or
prove closing after the body has been pruned. Import merges local activity
from both parties; the live balance is queried from State. See
[contract receipts and recovery](../contracts/receipts-and-recovery.md).

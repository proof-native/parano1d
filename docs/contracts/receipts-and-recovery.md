# Receipts, shared files and recovery

Current balance, historical evidence and local activity are different data.
A synchronized node authenticates **what is spendable now** from State.
A receipt proves a particular past call. A wallet journal lists the evidence
that this wallet has retained or imported. None of these is a global archive.

| Artifact | What it carries | Where to use it |
| --- | --- | --- |
| Public opening | Program, immutable policy and counters for one commitment | Contract API, CLI, shared contract file |
| Shared contract file | Terms, local description and at most one matching receipt | F7 Open file |
| Contract receipt | Original and optional successor terms, call, inclusion and recursive proof evidence | F7 Open file; contract verify/import API |
| Ordinary payment receipt | Payment inclusion evidence, including a deposit | F4 Receipts; ordinary receipt API |

## What a receipt establishes

Verification checks the exact call, its authorization relation and inclusion
against the node's selected chain and authenticated recursive proof bank.
An exported receipt can include a bounded path of later headers. It survives
pruning of the call's original block body. The original opening is present even
when the call closes the contract and there is no successor.

A receipt proves the call at that height; it does **not** prove that its output
is still unspent. Query current State using the opening and exact
`(slot_index, creation_id)`. Reused slots must never be confused with the old
incarnation. Canonical status can change during an allowed reorganization;
an orphaned journal record is not a current confirmation.

The decoded contract receipt limit is **1,110,624 bytes**, including the bounded
terminal and up to 42 descendant headers. Ordinary payment receipts retain a
separate **128 KiB** limit. These are different formats; do not pass a contract
receipt to `verifyReceipt` or assume that it fits the payment bound.

## Two parties, one evolving right

1. Alice creates and saves terms, funds an instance, and shares the contract
   file with Bob. Creating terms alone is not funding.
2. Bob opens the file, checks his role and current balance, then saves it.
3. An authorized party calls the contract. Online wallets watching its terms
   retain applicable transitions, including calls authorized by the other party.
4. Alice goes offline. Bob calls again and saves a receipt.
5. Alice returns after old transaction bodies have been pruned. State still
   reveals whether a known commitment has live funds, but cannot reconstruct
   unknown programs or a missed successor from a hash alone.
6. Bob sends the receipt or a shared file carrying that update. Alice verifies
   and imports it, obtains the successor terms or closing evidence, and queries
   current State again. Existing local records remain.

The two-wallet test exercises funding, both authorities, offline pruning,
receipt transfer, repeated imports, closing and restart. It validates this
workflow; it does not turn the network into an archival service.

## Merge behavior

Imports add verified evidence and deduplicate repeated transaction IDs. They
preserve the existing local name, your own operations and other known states.
An older imported opening does not replace a newer live state. Journal entries
are not required to arrive in order. A shared contract file is **not a full
journal synchronization format**: export additional receipts individually when
both parties need those records.

`walletListObjectReceipts` returns the locally retained calls of either
party with their current `canonical` status. `walletImportObjectReceipt`
verifies the proof and rechecks canonical inclusion before saving original
and successor terms. Its optional expected opening guards against importing
an unrelated call. `walletWatchObject` saves terms for future observation;
it cannot recover arbitrary calls whose bodies and evidence are already gone.

## Backups

Keep the wallet secret and public contract data. The GUI library lives in
`wallet.contracts.json` in its wallet directory. The node's `objects/` directory
holds watched openings and retained receipts, including shared terminal
material under `objects/terminals/`; `contract-activity/` holds the compact
activity index. Back up the wallet/data directories consistently while stopped.
See [files and ports](../reference/files-and-ports.md) for platform paths.

A private key recovers authority, not arbitrary public terms. If every holder
loses a required opening and no receipt or retained body can supply it, the
commitment alone cannot reconstruct it. Sharing terms and exporting important
receipts is part of operating an application with pruned history.

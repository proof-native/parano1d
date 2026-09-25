# Contract quick reference

Mainnet v2 activates at **H210537**. These instructions describe v2; the earlier
chain follows v1.1. The public contract ABI is **3**: 16 instructions, two
persistent u64 counters and two scratch registers. All programs use the shared
block proof. Small m23 permits 63 pages / 504 inputs / 63 calls; server-opt-in
Large m24 permits 206 / 504 / 63. Calls and payments share these budgets.

The six constructors are refundable payment, timelocked vault, allowance
wallet, period budget, recurring payment and tranche vesting. A custom program
uses the same checked integer core and explicit branch permissions. Terms
commit to authorities, recipients, deadline, fee and payout caps and reserve.
Each funded output is independent; scheduled transfers require calls.

**Saving terms has no creation fee.** Funding is a separate ordinary payment.
A call spends its fee from the contract balance, bounded by the policy ceiling.
Continuation creates a successor; closing transfers the remainder and has no
successor. At the deadline height the recovery branch is active.

GUI: **F7 Contracts**, then Create, My contracts or Open file. The local journal
shows retained interactions from either party. Imports merge verified evidence,
deduplicate transaction IDs and preserve your own records. F4 handles ordinary
payment receipts; contract files and call receipts use F7 Open file.

CLI starts with `parano1d-cli contract protocol` and
`parano1d-cli contract --help`. RPC uses the `paranoid_` namespace and local
owner scope. Discover limits with `getContractProtocol`, create terms with
`createObject`, fund with `walletFundObject`, preview with `previewObjectCall`
and submit with `walletCallObject`. Select an exact slot and creation ID and
bind the reviewed body before signing. Submission still needs confirmation.

Keep the wallet secret, public terms and receipts. State authenticates live
balances; it cannot reconstruct a lost program from a commitment. A receipt
survives body pruning and proves a past call, while present spendability must
be queried separately. Shared contract files carry terms and at most one
matching receipt, so exchange additional receipts when more history is needed.

The detailed [contract documentation](../contracts/index.md) covers the
[core](../contracts/core.md), [templates](../contracts/templates.md),
[API](../contracts/api.md), [GUI](../contracts/gui.md) and
[recovery](../contracts/receipts-and-recovery.md). Release-package readers can
open the same section at [docs.parano1d.org](https://docs.parano1d.org/contracts).

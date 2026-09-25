# Proof-native contracts

Contract transactions become available at the scheduled v2 activation height.
`getContractProtocol` reports that height, whether the next block can include
calls, and the installed proof bank's actual page, input and call limits.
Preparing or sharing public contract terms does not spend coins.

A contract is a funded State output whose spending rules include a committed
program. Its calls are checked inside the block's recursive proof. Creating a
program does not require its own circuit, proving key or matrix. Programs share
the bounded integer core supported by the installed bank.

## Templates

| Template | Rules |
| --- | --- |
| Payment with refund | The payee collects before expiry; the payer recovers from the expiry block onward. |
| Timelocked vault | The owner cannot withdraw before the unlock block. |
| Allowance wallet | A spending key makes payments subject to a per-call cap and minimum reserve; the recovery key can close from the recovery block. |
| Period budget | Payments **and fees** consume the balance's saved period budget. Unused allowance is discarded when the first eligible call starts a new period. |
| Recurring payment | The payee claims a fixed prepaid amount when due. The next due height is the successful call's height plus the period. Missed charges do not accumulate. |
| Gradual unlock | Fixed tranches become available on an anchored block schedule. Missed tranches remain claimable, one per call; the beneficiary can close at maturity. |

Scheduled payments require an authorized transaction; they do not execute on a
timer. Periods and deadlines use block heights. At the v2 target of 30 seconds,
2,880 blocks represent one nominal day; actual wall-clock duration varies.

Each deposit creates a separate funded output with its own counters and limits.
Deposits do not merge, and a budget is not an aggregate limit across independent
outputs. Continuing calls spend one exact output incarnation and create its
successor. A changed counter can change the successor's address. The separate
funded instances are visible through `getObjectInstances`.

## Wallet interface

The Contracts tab provides the six templates, an editor for custom programs,
public terms import/export, a local named list, funding, calls and receipt
verification. Use **Edit as new program** to start from a decoded template.
Changing the draft creates new terms; it does not alter an existing contract.

Before a call, the node computes its exact body, fee, payment, retained balance
and successor counters. Confirmation binds that body, the inclusion height and
the active authority. A changed height, fee or reserved output requires a new
review. The wallet keeps preceding terms alongside an unconfirmed candidate;
submission alone is not confirmation.

Export and back up public terms. A wallet key alone cannot reconstruct an
arbitrary program and its current counter values after history pruning. The
node's wallet also retains watched openings and call receipts. Restoring by
address reads those local saved files; it is not an archive lookup on peers.
A verified receipt can recover its successor terms, whose current balance must
then be checked in State.

## CLI

Commands live under `parano1d-cli contract`. Amount arguments use NOID with up
to six decimal places; fees of zero request the live minimum.

```sh
parano1d-cli contract protocol
parano1d-cli contract vault 220000 --max-fee 0.1 --out vault.json
parano1d-cli contract fund vault.json 10
parano1d-cli contract instances vault.json
parano1d-cli contract call vault.json SLOT CREATION_ID --close --preview
parano1d-cli --rpc-timeout 600 contract call vault.json SLOT CREATION_ID \
  --close --expected-txid REVIEWED_TXID --out withdrawal.json
```

The last command requires the reviewed body to remain valid. `--wait-seconds 0`
returns after submission; otherwise the command waits for confirmation and
saves a verified `.receipt` file. A removed pending call is reported explicitly;
inspect the transaction and current balance before authorizing again.

The call file saves the reviewed transaction ID, request and candidate successor
before sending the authorization request. If the RPC response is lost, its
`submission_status` remains `unknown`; query that transaction before retrying.
Receiving a submission response updates the file atomically to `submitted`.

Additional constructors are `payment`, `allowance`, `budget`, `recurring` and
`vesting`; `create` accepts a definition JSON file. `watch`, `restore`, `status`,
`receipt` and `verify` manage shared terms, live instances and retained evidence.
Run each subcommand with `--help` for its arguments.

## Custom integer programs

The current ABI has two persistent unsigned 64-bit registers, two scratch
registers reset on each call, and at most 16 instructions. The core supports
`keep`, `move`, checked `add` and `subtract`, `min`, `max`, `less_than`, `equal`,
`assert_equal` and `assert_less_or_equal`. Overflow, underflow, a failed assertion
or a non-boolean predicate rejects the call.

Operands include the registers, an immediate, the inclusion height, input,
payment and retained amounts, fee, deadline and call flags, output-owner words
and slot/incarnation identifiers. Predicates select the deadline branch,
closing/payment flags or a scratch boolean, with optional inversion. There are
no jumps, loops, cross-contract calls or external data reads in this ABI.

`custom_program` definitions explicitly specify both authorities, both closing
recipients, deadline, fee/payment/reserve caps and permitted call modes.
`state` and instruction `immediate` values are canonical decimal **strings** in
JSON, preserving all u64 values in browser clients. Program amounts use
micronoid. Trailing instructions omitted from the definition become `keep`.
All sixteen decoded instructions are returned for review; imported descriptions
are derived from the actual opening rather than trusted file labels.

## RPC and receipts

The contract methods are:

- `getContractProtocol`, `createObject`, `getObjectStatus`, `getObjectInstances`;
- `previewObjectCall`, `walletFundObject`, `walletCallObject`;
- `walletWatchObject`, `walletGetObjectOpening`, `walletListObjectStates`;
- `exportObjectReceipt`, `verifyObjectReceipt`.

An instance query takes an inclusive slot cursor and returns its exact tip
identity. Recheck that tip when combining pages. A call specifies the opening,
slot and creation ID, closing flag, optional payment and fee. Its optional review
guards are `expected_authority`, `expected_recovery`, `expected_call_height` and
`expected_txid`; the graphical wallet supplies all of them.

A receipt retains the opening, call page, authenticated transaction inclusion
and recursive proof, with a short header path when a later terminal covers the
call. Verification uses the selected chain's canonical header, the pinned v2
bank and an authenticated fork origin. A receipt proves the recorded call; it
does not prove that its successor remains unspent. Watched calls are retained
locally so exported evidence can survive body pruning.

The daemon stores a shared proof once when several watched calls use it.
Receipt export reconstructs a complete portable receipt. For a backup of the
daemon's local artifacts, copy the entire `objects` directory, including its
`terminals` subdirectory.

The GUI also finds locally saved counters under the same program and spending
rules. Refresh balances, then select another funded state if a participant's
call or a chain reorganization changed the current one. Previous terms remain
available even when the saved GUI entry has advanced to a successor.

Applications can use `walletListObjectStates(opening_hex, after_root, limit)`
for the same discovery. `after_root` is an exclusive hexadecimal root cursor
or `null`; `limit` is 1–64. The response returns terms, a `has_balance` result
for each, the selected height/hash, and the next cursor. Balances are queried
from verified State at one tip. The local index itself is not confirmation,
and results can include pending, spent or orphaned terms. Reload a state's
instances before reviewing a call. Each page has its own tip; restart paging
after a tip change if an application needs one consistent view.

Discovery covers openings retained by this wallet, including successors found
when processing watched calls. It does not recover an unknown program from a
wallet key. Nodes that missed an entire pruned call window still need exported
terms or a verified receipt from a participant. The local index reads compact
public terms without scanning full proof receipts and can be rebuilt from the
saved openings.

The scheduled bank's page limits are 63 for Small and 206 for Large. Both
classes allow 504 inputs and up to 63 calls per block; calls share page space
with ordinary payments. Applications should read `getContractProtocol` for
the active height and installed limits. See the
[scheduled profile](../protocol/parameters.md#scheduled-v2-profile).

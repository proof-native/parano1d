# Contract wallet: both parties, additive imports and offline recovery

Two independent wallets passed the contract file workflow through real isolated
nodes. The run used implementation commit `8f7e1a06`, the existing H10 candidate
bank and real proofs, signatures, external nonce work and P2P synchronization.
No consensus, matrix or retention parameters changed for this test.

## Verified workflow

Alice created and funded a counter contract at H86, then shared its terms and
funding receipt with Bob. Bob imported the file and called it at H87. Alice's
journal found that other-party call, including its signer and exact payout,
without reading each full recursive proof during refresh.

Alice then stopped. Bob made a second call at H88, incrementing the counter to
2, and exported an updated contract file with the call receipt. Bob produced
43 further blocks through H131. The normal 42-block retention policy removed
H88's body: both `getBlock(88)` and `getBlockDetails(88).retained` were null.
No chain files were deleted by the harness.

After reconnecting and synchronizing, Alice still had her funding record and
the first observed call. Her wallet did not know the missed successor opening.
Importing Bob's update restored the missing call and selected the live counter
state, preserving Alice's own records and local contract name. Importing that
same file again produced no duplicate. Importing the older funding file neither
erased the new call nor selected the spent predecessor.

Alice then closed the contract through its recovery authority at H132. Bob
imported the standalone closing receipt twice and kept all four operations.
Those records and their receipts survived his node restart. Alice also exported
the call she had missed at H88 from her locally retained imported proof; full
receipt verification succeeded after the original body had been pruned.

Both a receipt bound to the wrong exact opening and a damaged recursive proof
were rejected. Both nodes stopped cleanly at the same H132 tip:
`cb01151e8a8a1b8e068048c8927d1492de782727c301856a55abd717a8afe023`.

## Scope and reproduction

The [scenario](../../../../scripts/live_v2_contract_wallet_scenario.py) invokes
the actual GUI backend through its explicitly selected live test. It exercises
the same library, journal, preview, acceptance and export methods, substituting
supplied file paths for interactive dialogs. It does not claim GUI rendering
coverage. Contract paths include non-ASCII characters.

Set `NOID_V2_WALLET_SOURCE` to the stopped, passed H85 shared-receipt fixture,
`NOID_GUI_TEST_BINARY` to the recorded GUI test executable and
`NOID_V2_LIVE_DIR` to a fresh directory. Run through
`unshare --user --map-root-user --net python3
scripts/live_v2_contract_wallet_scenario.py`. The record identifies the binary,
script and candidate-bank hashes. This is an explicit local scenario, not a
new CI job.

The initial harness incorrectly required `getTx` to return null after body
pruning. That RPC returns only an index pointer (height, block hash and logical
position), so the assertion failed at H131 after all 43 blocks were mined.
The corrected harness checks the body-serving RPCs instead and records the
remaining pointer. It resumed from copies of the cleanly stopped wallets with
identical executables; the original failed report hash is preserved.
`NOID_V2_WALLET_RESUME_SOURCE` accepts only that specific checkpoint. A fresh
run does not need it and produces the entire pruning tail.

The GUI suite passed 81 tests, the node wallet suite 102, and the RPC object
suite 2. The ordinary GUI suite ignores the explicit live driver; the scenario
ran it 23 times. Commands, counts and evidence hashes are in
[measurements.json](measurements.json).

## User-visible boundaries

The journal merges locally retained or explicitly imported evidence. A shared
contract file contains terms and at most one matching receipt; it is not an
export of every historical operation. Additional missing operations can be
imported as individual receipts. An entirely missed, already-pruned call is
not reconstructed from the chain's transaction index.

The call-fee limit is a policy ceiling for future calls. Saving unfunded terms
does not charge that amount; funding has its own previewed network fee. The
three contract tabs and the ordinary payment-receipt screen keep their separate
purposes.

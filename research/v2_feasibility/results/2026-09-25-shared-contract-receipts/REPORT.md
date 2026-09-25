# Shared local proof storage for contract receipts

The wallet now stores identical recursive proof bytes once. Each local receipt
retains its own opening, transaction, inclusion path, headers and a content hash
of that proof. Export reconstructs the original complete portable receipt.
Consensus, matrices, network framing and receipt verification are unchanged.

A real two-daemon run continued the completed full-capacity fixture from H79.
At H80, 63 independent counter calls advanced their saved state from one to two.
Both nodes exported all 63 receipts; the first and last were independently
verified. After another real block and receiver restart, all 63 receiver exports
were byte-identical, and the first and last again verified.

| Local storage per node | Bytes |
| --- | ---: |
| 63 receipt references and individual inclusion data | 110,754 |
| One shared recursive proof | 911,988 |
| Total | **1,022,742** |
| Equivalent 63 separate complete receipts | 57,563,478 |

That is approximately **56.28 times less receipt payload storage** for this
block. These are file content sizes, not allocated filesystem blocks. The
comparison excludes separately retained public openings and directory metadata.
This change does not reduce proof creation time or portable export size.

Four existing receipts were also exported and verified with the new binary after
their original block bodies had been pruned. They included the earlier complete
direct format and the descendant-header format. Reading them did not rewrite
their files. Both processes exited successfully.

The receiver retained the earlier four-logical-CPU PCLMUL, 8 GiB/no-swap envelope.
No other build or proof run overlapped the scenario. The bank remained
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`.

The full 200-test isolated-node suite passed before the release binary was
built. Storage regressions cover corrupt or missing shared bytes, oversized
references, mismatched transaction filenames, repair from a valid receipt and
backward migration of complete direct and descendant receipts.

Run `scripts/live_v2_shared_receipts_scenario.py` in a loopback-only namespace.
Set `NOID_V2_CAPACITY_SOURCE` to the completed and stopped capacity fixture,
`NOID_V2_PRUNED_RECEIPTS_SOURCE` to its ancestor contract-lifecycle fixture, and
`NOID_V2_LIVE_DIR` to a new destination. The script copies only isolated data.
[Measurements](measurements.json) preserve binary/source hashes, storage sizes,
all export hashes, older format checks and the final selected tip.

# Live reorg through the v2 activation boundary

Two real isolated daemons passed a reorg that changed the last legacy block,
replaced the authenticated v2 origin and rolled back a confirmed integer call.
The scenario used the candidate joint bank
`e1a79797415bb6d8da4935566ed614a9ec616ddee3315686f75eb8090495a0cd`,
with v1.1 at H5 and v2 at H10. Every branch block was mined with its actual
recursive proof; no State or headers were injected.

1. Both nodes received the same legacy chain through H8 over P2P.
2. Branch A independently mined H9 and crossed activation at H10. It funded a
   custom counter at H11, confirmed the transition from 0 to 1 at H12, and
   verified the call receipt against its selected chain.
3. With A stopped, branch B continued from the common H8. It produced a distinct
   H9 and a H13 tip with strictly greater cumulative work. The scenario compares
   the protocol's exact work from each target, not height alone: sequential
   mining can cause ASERT to assign the two branches different difficulties.
4. Both nodes restarted and connected as non-mining peers. A reorganized to B;
   every canonical hash from H1 through H13 matched. Its selected H9 changed.
5. The old call was absent from the confirmed transaction index. Verification
   and export of its orphaned receipt failed. Neither the original funded
   output nor its successor survived in State. Both public openings remained
   locally recoverable, and resubmission of the stale reviewed call failed.
6. A restarted again, mined H14 using the new origin, and B accepted the exact
   same tip. Both nodes shut down successfully.

This is functional qualification on one laptop, not a receiver capacity or
independent-server timing measurement. Both legacy boundaries used B25; the
legacy B255 boundary and omitted-legacy-matrix cold synchronization remain
separate qualification cases.

Run `scripts/live_v2_boundary_reorg_scenario.py` in a loopback-only network
namespace, with `NOID_V2_LIVE_DIR` set to a fresh directory and the same pinned
isolated binaries as the contract lifecycle scenario. The script permits extra
competing blocks, within its bound, if H13 has not yet accumulated greater work.

[Measurements](measurements.json) include binary/script hashes, branch targets
and work, both origin headers, exact rejection results, and the final tip.

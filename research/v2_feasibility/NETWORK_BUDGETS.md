# Size and admission limits relevant to v2 capacity

Audit date: September 24, 2026. These are the existing v1/v1.1 limits and the
current research implementation. No production limit is raised by this audit.
Fitting m23 does not imply fitting transport, and fitting bytes does not imply
that the production node accepts the research v2 protocol.

| Boundary | Current limit | Implication |
|---|---:|---|
| Serialized v1.1 terminal | 1,100,000 bytes | Terminal only; separate from the body. |
| Expanded v1.1 terminal decode | 1,100,000 bytes | Shared-path compression does not permit an oversized expanded proof. |
| Canonical block body | 82,905 bytes | 212-byte header, marker/count and 256 fixed 323-byte pages. |
| Accepted bundle including framing | 1,182,917 bytes | 12-byte framing + body cap + terminal cap. |
| Exact-object response payload | 1,100,000 bytes total | Body and terminal can require separate responses even when each fits. |
| Exact objects requested together | 8 | A count limit, not permission to send eight maximum-size terminals. |
| One wallet authorization bundle | 262,144 bytes | Enforced separately from the containing transaction intent. |
| PagedSpend intent | 303,495 bytes | Up to 128 pages belonging to one logical transaction plus authorization. |
| Active GossipSub message size | 303,495 bytes | Configured from maximum intent/header announce size; not the unused 2 MiB constant. |
| Mempool | 1,024 transactions / 384 MiB | Burst/backlog limits, separate from block capacity. |
| Mempool-sync response | 128 transactions / 16 MiB | Both bounds apply. |
| P2P response allocation budgets | 64 MiB inbound and 64 MiB outbound | Concurrency cannot multiply payload memory without bound. |
| Block resource weight | 64 MiB | Includes weighted transaction, input, output and frontier work; not just wire bytes. |
| User pages / live inputs / user outputs | 255 / 1,020 / 510 | Existing native global limits; research profiles impose tighter page/input limits. |
| Distinct State segments in one block | 256 | A high-TPS candidate must include distributed State cases. |
| Snapshot tail staging | 4,096 bodies / 512 MiB | A separate catch-up backlog bound. |

Sources: [shared wire limits](../../noid_chain/src/consensus/wire_limits.rs),
[accepted bundles](../../noid_chain/src/accepted_block_bundle.rs),
[legacy terminal decode](../../noid_recursive/src/acceptance/history_step/wire.rs),
[object codec](../../noid_p2p/src/object_codec.rs),
[object protocol](../../noid_p2p/src/object_protocol.rs),
[active GossipSub configuration](../../noid_p2p/src/behaviour.rs),
[transaction intent](../../noid_tx/src/paged_spend.rs),
[semantic budgets](../../noid_chain/src/consensus/params.rs),
[snapshot tail staging](../../noid_node/src/snapshot_tail_staging.rs).

## Current candidate caveat

The research [v2 codec](../../noid_recursive/src/acceptance/history_step/v2/wire.rs)
uses version 6 and a runtime-derived `terminal_max_bytes`. It bounds its own
decode, but does **not** impose the production 1,100,000-byte cap independently.
The [production metadata decoder](../../noid_chain/src/history_step.rs) still
selects versions 4/5 by the v1.1 height. The candidate benchmark verifies version
6 directly; that does not exercise production P2P admission, storage, RPC or sync.

The benchmark now reports the runtime-derived terminal bound, actual encoded
length, body/bundle sizes and whether the combined payload fits one current
object response. Receiver runs also exercise the production bundle constructor
and report its rejection of the research wire version. This is a recorded
integration gap, not a reason to relax the production decoder prematurely.

For 112 independent one-page payments, the body is 36,716 bytes. At 223 and 255
such payments it would be 72,569 and 82,905 bytes. The measured proof sizes and
runtime-derived bounds are in the [capacity report](results/2026-09-24-tps/REPORT.md).
An optimized block relation must account for all terminal proof bytes, including
its internal GKR proofs. Reducing the outer row count while making the terminal
oversized would not qualify a candidate.

## Integration requirements after a proof design is selected

Keep admission height-aware across terminal metadata, the bundle constructor,
object and terminal codecs, persistence and snapshot/catch-up readers. Check
serialized and expanded maxima before allocation. Preserve pre-fork decoding and
exercise exact limits, one byte over, malicious length tables and truncated
encodings. Check combined body/terminal responses and the split-response path.

The [network profile](../../noid_p2p/src/network_profile.rs) deliberately retains
a pre-v1.1 baseline of 1 MiB/version 4 in its identity, while active admission
uses the height-selected cap and encoding. Changing those advertised fields
without a transition design could disconnect nodes before activation. They are
not an overlooked active 1 MiB terminal cap.

Measure propagation, mempool authorization traffic, bounded verifier queues and
catch-up concurrently on 4 CPU / 8 GiB. A successful local proof or a byte-size
comparison alone does not qualify these paths. Any necessary limit change must
be tied to the selected fork rules and tested with old/new nodes before and
after the boundary.

# Blocks and headers

Direct block admission uses an atomic pair:

```text
{canonical block bytes, matching HistoryStep terminal}
```

The block contains a fixed header and up to 256 fixed transaction bodies.
The terminal proves the nonce-independent semantic header and complete State
transition.

## Block contents

Transaction position zero is the mandatory primary reward. A scheduled
development payout, when present, follows it. User `PagedSpend` pages occupy
the remaining positions in canonical group order.

| Limit | Value |
|---|---:|
| Total fixed bodies, including system records | 256 |
| User page positions without scheduled payout | 255 |
| User page positions with scheduled payout | 254 |
| Live user inputs | 1,020 |
| Live user outputs | 510 |
| Live user actions | 1,530 |
| Distinct State segments touched | 256 |

The decoder rejects oversize counts before allocating or decoding their
payload.

## Transaction root

Every body occupies one leaf in a universal 256-leaf transaction tree. Unused
positions have one canonical empty value. The tree is wrapped with the exact
body count under the transaction-root domain, so appending or deleting empty-
looking leaves cannot produce an equivalent block.

Receipts use an eight-level inclusion path into this tree. A logical
`PagedSpend` may occupy several consecutive leaves; the receipt binds its
logical position and page count.

## Canonical header

The header has a fixed 212-byte little-endian encoding:

| Field | Size | Meaning |
|---|---:|---|
| `prev_block_hash` | 32 | Nonce-bearing parent block ID |
| `state_root` | 32 | Exact post-State UTXO root |
| `tx_root` | 32 | Count-bound transaction root |
| `timestamp` | 8 | Unix time in seconds |
| `height` | 8 | Child height |
| `miner_address` | 32 | Primary reward recipient |
| `nonce` | 16 | Poseidon2b PoW nonce |
| `difficulty_target` | 32 | Exact little-endian ASERT target |
| `log_slots` | 4 | Slot-domain exponent |
| `active_slot_count` | 8 | Post-State live UTXO count |
| `alloc_counter` | 8 | Post-State allocation counter |

Field order is consensus-locked. Future formats cannot reorder existing fields.

## Two header identifiers

The nonce-bearing block ID hashes every field under the `BLOCKHDR` domain. It
is used for parent links, transaction epoch anchors and canonical block
identity.

The semantic header ID hashes the same fields in the same order but skips the
nonce and uses the `SEMHDR__` domain. `HistoryStep` binds this projection so a
miner can vary the nonce without rebuilding the transition proof.

An accepting node requires the terminal and the native nonce-bearing header to
refer to one identical set of non-nonce fields.

## Primary reward

There is exactly one live primary reward output. Its `creation_id` uses a
disjoint height-tagged namespace:

```text
2^63 | block_height
```

Ordinary output allocation remains below `2^63`, so a reward record can never
collide with a user allocation identifier.

The primary reward may claim only the current miner subsidy and miner-claimable
fees. It cannot reclaim the burned State-growth component.

## Terminal representation

The terminal uses a canonical shared-path encoding. Merkle authentication
nodes shared by several openings are transmitted and stored once. Decoding
reconstructs the complete paths consumed by the recursive verifier; the
mathematical proof, query count and authenticated matrices are unchanged.

Consensus limits the serialized terminal to 1,100,000 bytes, counting the
compressed representation and its terminal metadata. Expansion is independently
bounded to 1,100,000 bytes. The block bytes have a separate 82,905-byte maximum;
block bodies and bundle framing do not consume the terminal limit. Malformed
or non-canonical encodings are rejected before expensive verification.

## Accepted bundle

The terminal includes metadata binding its version, height, semantic-header
hash and proof class. Bundle decoding checks lengths before allocating and
rejects trailing bytes, mismatched metadata or a terminal for another header.

Canonical block bodies are retained for 42 blocks. A directly admitted block
stores its complete bundle. During authenticated catch-up, one verified
terminal at the suffix tip authorizes the exact linked intermediate bodies;
the tip stores the complete bundle, while intermediate rows keep compact local
authorization for that verified suffix. Headers are permanent.

See [Proof of work](proof-of-work.md) for nonce validation and
[Receipts](../concepts/receipts.md) for durable transaction inclusion.

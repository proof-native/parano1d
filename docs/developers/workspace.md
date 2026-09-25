# Workspace

The repository is split by proof and trust boundary rather than by binary.

## Arithmetic and proof foundations

| Crate | Responsibility |
|---|---|
| `noid_core` | Binary tower field, packed kernels and CPU dispatch |
| `noid_poseidon2b` | Poseidon2b permutation, domains, hashes and batch execution |
| `noid_fri` | FRI primitives |
| `noid_fri_binius` | Binary-field FRI-Binius/BaseFold integration |
| `noid_ivc_core` | Recursive public I/O and verifier foundations |
| `noid_ivc_prover` | Recursive prover implementation |
| `noid_gkr` | [FROST-GKR](../research/frost-gkr.md) relations and wallet authorization |
| `noid_recursive` | `HistoryStep`, exact-State relation and recursive acceptance |
| `noid_soundness` | Source-bound Block–Tiwari, QROM and Category 1 certificate |
| `bench_prover` | Matrix generation, pin tools and proof benchmarks |

## Protocol objects

| Crate | Responsibility |
|---|---|
| `noid_tx` | Fixed `Tx8x2`, logical `PagedSpend`, IDs and authorization binding |
| `noid_block` | Block-level proof composition |
| `noid_chain` | Headers, consensus, State, fees, receipts, MDBX and snapshots |

`noid_tx` contains representation-level rules that can be checked without a
chain. `noid_chain` adds current epoch, Live State, issuance, allocation and
fork context.

## Runtime

| Crate | Responsibility |
|---|---|
| `noid_mempool` | Intent admission, CPU permits, conflicts and selection metadata |
| `noid_miner` | Shared CPU plan, template construction, proof and PoW |
| `noid_p2p` | libp2p discovery, gossip, sync and resource limits |
| `noid_rpc` | Typed JSON-RPC API and wallet operations |
| `noid_node` | Daemon, CLI, wallet state and subsystem orchestration |
| `noid_gui` | Native multilingual wallet and private-node supervision |
| `noid_extminer` | External Poseidon2b nonce worker |

## Dependency direction

The intended direction is:

```text
field / hashes / proof primitives
            ↓
transaction and block relations
            ↓
chain consensus and storage
            ↓
mempool / miner / P2P / RPC
            ↓
node and GUI applications
```

Lower crates do not call GUI, RPC or network policy. Consensus types do not
depend on wallet labels or application presentation.

## Consensus-sensitive changes

A change is consensus-sensitive when it alters any of:

- canonical byte encoding;
- Poseidon2b field order or domain tag;
- transaction or block validity;
- State-root derivation;
- issuance, fee burn or allocation;
- difficulty, timestamp, finality or expansion rules;
- `HistoryStep` relation or authenticated matrices.

Such a change requires updated vectors, relation tests, matrix pack and
embedded pins. A new matrix pack without corresponding source semantics is not
an upgrade mechanism.

## Application-only changes

Translation, layout, wallet labels, log presentation and ordinary RPC response
formatting are outside consensus so long as they do not change the serialized
objects submitted to the node.

Wallet coin selection is also policy. The resulting `PagedSpend` must still
satisfy the same consensus rules as one built by another implementation.

## Contract components

`noid_tx::experimental_object` defines the canonical opening, program, policy and call
encoding; `noid_tx::experimental_object::applications` builds the templates. `noid_recursive` proves
their execution in the block relation. `noid_chain` handles live-state checks,
receipts and fork context; `noid_rpc` exposes typed construction and wallet
calls. `noid_gui` owns forms, local names and file import/merge.

A new program expressed in the existing ABI can use the same matrices.
Changing an opcode, operand meaning, authority rule or execution budget
changes the proven relation and requires consensus and matrix review.
Contract terms and receipt journals are application data; they cannot override
the committed program or current State.

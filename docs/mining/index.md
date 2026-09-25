# Mining

**Hashpower alone cannot produce blocks. Mining requires State; nonce search
begins only after the proof is complete.**

Proof of work in Parano1d orders transitions whose validity has already been
established. Before searching a nonce, a block producer must follow the
canonical chain, hold the current State, construct the exact next
transition and complete its recursive `HistoryStep`.

This makes the proving full node—not an individual hash worker—the unit of
independent block production.

![Proof-native block flow](../assets/architecture/proof-native-block-flow.svg)

## What the node and worker do

A mining node owns the block. It:

- follows and independently validates the canonical chain;
- verifies transaction intents before they enter its mempool;
- selects a non-conflicting transaction set;
- fixes the payout, fees, slot writes and post-State root;
- proves the nonce-independent block and preceding terminal;
- validates the winning nonce;
- commits and broadcasts the complete `{block, HistoryStep terminal}` bundle.

Nonce search may run inside that process or in `parano1d-miner`. An external
worker has a much narrower role:

- receive one immutable Poseidon2b header schedule and target;
- search independent values of its 128-bit nonce;
- return a candidate nonce to the node.

The worker does not receive the block body, State witness or `HistoryStep`
witness. It cannot replace transactions, alter the State root or modify a
template after proof construction.

## One block attempt

Block production proceeds in this order:

1. The node waits until it is synchronized and has the required authenticated
   peer quorum.
2. It reads its canonical tip, current State and admissible mempool intents.
3. It selects the Small or Large proof class and fixes every semantic field of the
   candidate block except its nonce.
4. It computes the exact slot writes and resulting UTXO root.
5. It proves the new `HistoryStep`, including recursive continuity from the
   preceding terminal.
6. The completed proof fixes one immutable mining template.
7. The internal miner or an external worker searches the Poseidon2b nonce.
8. The node checks the nonce, seals the prepared terminal, commits the block
   atomically and announces it to peers.

A new canonical tip makes unfinished work stale. The node discards that attempt
and starts from the new State; it never moves an old proof onto a different
parent or transaction set.

Peers accept the result only after independently checking the parent,
`HistoryStep`, PoW target and every consensus commitment. Cumulative work
chooses between valid competing chains.

## Two mining modes

| Mode | Proof construction | Nonce search | Best fit |
|---|---|---|---|
| Internal | Core node | Core node | GUI wallet, solo miner, one server |
| External | Core node | `parano1d-miner` | Separate CPU worker, private mining network or pool |

Both modes use the same consensus rules and produce the same blocks. External
mining moves only nonce search across the RPC boundary.

An ordinary `--mode node` process validates and relays blocks but does not
construct mining templates.

## Mine from the GUI wallet

The native wallet supervises its own full node. Open **Mining** with `F5`,
choose the CPU thread budget and select **Start mining**.

Mining becomes available after the node is synchronized and connected to at
least one authenticated peer. The active wallet address receives newly
constructed payouts. Changing the active address affects the next template;
an existing immutable template keeps its original payout.

The page reports the selected CPU backend, proof preparation, current mining
state and locally found blocks. See
[Mining in the wallet](../wallet/mining.md) for shutdown behavior and the mined
block table.

## Run the internal Core miner

Check the actual host before creating node data:

```sh
parano1d --check-hardware
```

Start Core with its built-in miner:

```sh
parano1d --mode miner --cpu-threads 12
```

Omit `--cpu-threads` to use every logical CPU visible to the process. When no
explicit payout is configured, Core uses the active address in its local
wallet. A separate canonical bech32m payout can be fixed with:

```sh
parano1d --mode miner --miner-address o1...
```

Watch readiness and chain progress from another terminal:

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli mining
```

Core waits rather than mining an isolated local view when it is unsynchronized
or has no authenticated peer. The complete server and systemd
procedure is in [Internal mining](../operate/internal-mining.md).

## Run an external worker

Start a node that owns and proves external-mining templates:

```sh
parano1d \
  --mode extminer \
  --mining-key-file ~/.parano1d/mining.key
```

Run the worker against its loopback RPC endpoint:

```sh
parano1d-miner \
  --rpc http://127.0.0.1:9601 \
  --key-file ~/.parano1d/mining.key \
  --threads 12
```

The node prepares a complete proof before returning a template. The worker
searches its nonce and submits only the result. Templates are single-use,
expire after 30 seconds and become stale immediately after a competing tip is
accepted.

A remote worker should connect through an authenticated private network or a
TLS endpoint with firewall restrictions. A bearer token authenticates the
worker but does not encrypt plain HTTP. The token authorizes only
`getBlockTemplate` and `submitBlock`; it cannot access wallet or node-control
methods. The legacy `--mining-key` and `--key` arguments remain compatible,
but protected key files avoid exposing the token in process arguments.

By default, the node controls the payout. Allowing authenticated workers to
request their own payout is an explicit operator decision. The full remote
configuration and trust boundary are documented in
[External miner](../operate/external-miner.md).

## CPU and proof capacity

Production requires SSE4.1 + PCLMULQDQ on x86-64 or NEON + PMULL on ARM64.
Runtime dispatch selects the best supported backend. Proof and PoW share one
thread budget; leave room for wallet work and P2P service.

The active bank has two jointly authenticated classes:

| Class | Pages | Live inputs | Calls |
| --- | ---: | ---: | ---: |
| Small, m23 | 63 | 504 | 63 |
| Large, m24 | 206 | 504 | 63 |

Small is the default. `--v2-large-blocks` permits Large for both internal mining
and external templates; there is no GUI control. The producer chooses Large
when its eligible set yields more claimable fees, otherwise Small. There is no
v2 automatic timing calibration. Every node verifies both classes.

Calls share page and input budgets with payments. Large can fit 63 calls plus
143 one-page payments, within the 504-input bound. Primary coinbase is separate;
an extra mandatory system record uses one effective page. Programs use the same
interpreter in either class. Query `getContractProtocol` for installed budgets.

ASERT targets the complete 30-second mean interval: proving, nonce search and
propagation all consume it. Benchmark the complete preparation path on the
actual host. [Measured costs](../reference/performance.md) include the sequence
of Small blocks after a Large block.

## Difficulty, rewards and confirmations

ASERT adjusts the Poseidon2b target against the complete interval between
accepted blocks. Proof preparation, nonce search and propagation share that
30-second mean target.
The chain with the greatest cumulative valid work wins; an equal-work tie uses
the canonical block-hash tie-break.

The gross reward follows the [fixed height schedule](../protocol/economics.md#v2-issuance), starting at
16 NOID gross and advancing every 1,051,200 blocks. The mining RPC reports the
next block's gross subsidy:

```sh
parano1d-cli mining
```

During the three-year development allocation, the miner receives 90% of newly
issued block rewards. Claimable transaction fees also belong to the miner
after the consensus State-growth burn. After the allocation ends, 100% of each
new block reward goes to the miner.

A locally found block is not an immediate final balance. Its confirmation depth
increases as valid descendants are accepted, and a shallow higher-work reorg
can replace it inside the retained competition window.

## Mining and decentralization

An autonomous miner cannot operate from hashpower alone. Its node must remain
current, validate incoming work, construct and prove the next exact State transition before
any nonce engine receives useful work. When that node accepts inbound P2P
connections, the same infrastructure also relays transactions and blocks and
serves synchronization data.

External workers and pools remain possible: one proving node can serve more
than one nonce worker. Specialized nonce hardware still
needs a node capable of constructing valid block proofs at the required pace.

For the exact header relation, continue with
[Proof of work](../protocol/proof-of-work.md). For the implementation pipeline,
read [Block production](../architecture/mining.md).

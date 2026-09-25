# Parano1d ①

> **V2 activation: mainnet block 210537.** This README describes the v2 rules.
> Before that height the network uses v1.1. Activation follows height; the
> planning estimate is October 10, 2026, 11:59 PM PDT (October 11, 06:59 UTC).
> Previous profiles and measurements are in the [archive](docs/archive/index.md).

**Proof-native Layer 1 ordered by proof of work. From live value to live rights.**

[Website](https://parano1d.org) ·
[Documentation](https://docs.parano1d.org) ·
[Research](https://lab.parano1d.org) ·
[Releases](https://git.parano1d.org/ignotusnemo/parano1d/releases)

**Source:** [Forgejo](https://git.parano1d.org/ignotusnemo/parano1d) (canonical) ·
[GitHub](https://github.com/proof-native/parano1d) ·
[GitLab](https://gitlab.com/ignotusnemo/parano1d)

Parano1d makes the current State independently verifiable without replaying
historical transactions. The wallet proves authorization with its private
witness. The miner proves public execution and the exact State transition.
Peers verify the resulting recursive proof and materialize the proven writes.

Every accepted block carries a `HistoryStep` that proves its transition and
verifies the preceding terminal. A joining node authenticates Live State and
a bounded recent suffix. Old transaction bodies can be pruned; their validity
remains in the recursive proof. Permanent headers still support cumulative-work
comparison, and State transfer still scales with the live data.

## From live value to live rights

V2 extends that model to **proof-native contracts**. A live output can commit
to a program, authorities, recipients, height conditions and persistent
counters. Spending it proves not only ownership of value but the right to take
a particular action: collect a payment, spend a budget, claim a due installment
or recover a remaining balance.

The authorized transition produces a new committed state or closes the right.
Its validity becomes part of the same block proof. Nodes do not need a growing
contract execution history to validate the present. Applications retain the
public terms and portable receipts they need to use and explain that present.

The shared integer core has 16 instructions, two persistent u64 counters, two
scratch registers, checked arithmetic, conditions and block-height access.
Users can create programs within this ABI without a new circuit or matrix per
application. It is a bounded core: scheduled actions require an authorized
call, and there are no loops or cross-contract calls.

The GUI, CLI and RPC expose six templates plus custom programs:

- refundable payments;
- timelocked vaults;
- delegated allowance wallets;
- budgets per height period;
- recurring payments;
- gradual release in fixed tranches.

This opens application space around prepaid services, delegated spending,
conditional access and staged payments while keeping consensus storage tied
to live rights. Each deposit is an independent instance. The
[concept](docs/concepts/proof-native-contracts.md),
[contract guide](docs/contracts/index.md), [API](docs/contracts/api.md) and
[GUI walkthrough](docs/contracts/gui.md) explain the mechanics and limits.

## How a block is made

1. The wallet builds an atomic intent and proves knowledge of the required
   spending secret, bound to the complete logical transaction.
2. The mempool checks authorization, current spendability, limits and fees.
3. The miner proves the selected payments and contract calls, exact slot writes
   and recursive continuity with the parent.
4. Only then does an internal or external worker search the immutable
   Poseidon2b header's 128-bit nonce.
5. Peers verify the proof, PoW and chain rules, then apply the proven writes.

**Hashpower alone cannot originate a block.** A producer needs current State
and a completed proof. An external worker receives a single-use nonce template;
transaction selection and proving remain in the node.

## V2 network profile

| Parameter | Value |
| --- | --- |
| Mainnet genesis | 2026-08-21 16:00:00 UTC |
| Genesis block ID | `860e70453390bf815718e933aa4927167a13d098b0151391eefd722ee1add610` |
| Target block interval | 30 seconds |
| ASERT | 6-block reference epoch, 180-second half-life |
| Default Small | m23: 63 pages, 504 live inputs, 63 contract calls |
| Optional Large | m24: 206 pages, 504 live inputs, 63 contract calls |
| Hard finality / maximum reorg | 18 / 17 blocks |
| Local body retention | 42 blocks |
| State domain | 2^24 to 2^32 slots |
| Serialized terminal cap | 1,100,000 bytes |

Page, input and call limits apply together. Small fits 63 one-page payments or
63 calls; Large fits 206 one-page payments or 63 calls plus 143 payments,
within the input budget. A multi-page spend remains one logical transaction.
The primary coinbase is separate; an additional mandatory system page consumes
one effective page.

Large production is a server option, `--v2-large-blocks`, with no GUI switch.
It permits fee-based class selection and does not force Large blocks. Every
node verifies both classes. Ordinary one-page capacity ceilings at the target
interval are 2.1 and about 6.87 transactions/s; these are capacity calculations,
not measured sustained network throughput. See [parameters](docs/protocol/parameters.md)
and [measured proving and verification costs](docs/reference/performance.md).

## State and issuance

State is an exact sparse vector of indexed outputs. Spending clears a slot;
new outputs reuse empty positions with fresh `creation_id` values. Empty
2^16-slot segments need no materialized payload. Sustained finalized occupancy
can expand the domain without moving existing outputs.

Issuance follows an exact height schedule from H210537:
**16 → 11.30 → 8 → 5.65 → 4 → 2.83 → 2 → 1.41 → 1 NOID**,
one step every 1,051,200 blocks. The permanent 1 NOID tail means there is no
fixed maximum supply. State expansion does not change the reward.

Occupancy still increases the fee for net-new live slots. That growth component
is burned; consolidation frees slots and avoids growth fees. The
[network economics](docs/protocol/economics.md) explains the schedule, its
rationale and the distinction between gross subsidy and actual supply.

## One proof stack

Committed traces use GF(2^128); C1 Fiat–Shamir challenges and recursive claims
use GF(2^256) with a trace-one support of size 2^255. Poseidon2b underlies
addresses, transactions, trees, transcripts and PoW. FROST-GKR batches its
permutations and Merkle paths; sumcheck, zerocheck, lincheck and FRI-Binius
close the relation without a trusted setup.

Ordinary ownership uses a freshly randomized proof of a 256-bit secret's
preimage, with no public-key transaction signature. Contract calls prove the
active policy authority. The libp2p Ed25519 identity identifies a peer only.
Public addresses, values and relayed transactions are not concealed, and third
parties can archive them. Pruning is not an anonymity guarantee.

The final v2 bank and matrix-retirement construction have
[source-linked soundness accounting](noid_soundness/docs/v2-retirement.md),
including legacy ancestry. Under the stated all-root composition, ideal
compiler, honest public preprocessing, fixed-Poseidon2b and resource-price
premises, the Category 1 assessment gives a dominant gate-depth floor with
log2 value **173.3897612554174** and an ideal success bound of about
**0.04937388373372754** at the Category 1 reference envelope. The premises and exact resource bounds are detailed in the
[security model](docs/protocol/security-model.md) and
[exact final-bank records](research/v2_feasibility/results/2026-09-25-common-input-budget/REPORT.md#accounting-for-the-final-banks).

The fork binds the new proof to the selected last old block. The transition
release includes the historical verification material. A later
`--retired-history` build can omit old matrix bytes using independently
verified ancestry certificates; it does not introduce a trusted checkpoint.

## Run a node or wallet

Official releases contain a Core archive and a native GUI wallet with its own
supervised node. The GUI supports Linux, Windows and macOS. See
[installation](docs/getting-started/wallet.md) and [Core](docs/getting-started/core.md).

```sh
parano1d --check-hardware
parano1d
```

For mining:

```sh
parano1d --miner
parano1d --miner --v2-large-blocks
```

External nonce search keeps proving in the node:

```sh
parano1d --extminer --mining-key-file ~/.parano1d/mining.key
parano1d-miner --key-file ~/.parano1d/mining.key
```

Use only the mining command appropriate to the host. Default ports are P2P
9600 and local JSON-RPC 127.0.0.1:9601. The built-in wallet key is not
password-encrypted; protect it and back up contract terms and receipts too.

```sh
parano1d-cli status
parano1d-cli balance
parano1d-cli contract protocol
parano1d-cli contract --help
```

Addresses use bech32m `o1…`; 1 NOID = 1,000,000 μNOID. F7 opens Contracts in
the GUI, and F8 opens Settings. See [RPC](docs/reference/rpc.md) for owner,
mining and operator access scopes.

## Build and verify

The workspace pins Rust 1.96.0. Native builds need a C/C++ toolchain, CMake,
libclang and platform packaging tools. Production x86-64 requires SSE4.1 and
PCLMULQDQ; ARM64 requires NEON and PMULL. Portable binaries select the available
PCLMUL, AVX2+VPCLMUL, AVX-512+VPCLMUL or NEON+PMULL backend at runtime.

A production build requires the authenticated v2 bank, reviewed pins and
legacy ancestry material. The [build guide](docs/developers/build.md) provides
the complete generation, authentication and packaging procedure. Contract
programs use this shared bank and do not generate new matrices.

```sh
./scripts/build_release.sh \
  --pack PATH_TO_LEGACY_PACK \
  --v2-pack PATH_TO_FROZEN_V2_PACK \
  --v2-pins PATH_TO_REVIEWED_PIN_FILE \
  --retirement-keys PATH_TO_AUTHENTICATED_KEYS
```

Designed and developed by **Ignotus Nemo**. [Apache License 2.0](LICENSE).
Report security issues through the [security policy](.github/SECURITY.md).
Contact: [dev@parano1d.org](mailto:dev@parano1d.org).

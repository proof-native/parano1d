# Troubleshooting

Start with the exact startup error and the last node log lines. Most failures
fall into one of the following boundaries.

## CPU unsupported

```text
CPU UNSUPPORTED
This CPU is too old or does not expose the required instructions.
REQUIRED x86-64 SSE4.1 + PCLMULQDQ
```

On ARM64 the required pair is NEON and PMULL. Production does not fall back to
the scalar reference implementation.

Some virtual-machine plans hide PCLMULQDQ even when the physical host supports
it. Check the guest-visible CPU flags or choose a virtual CPU profile that
exposes the required instructions.

Most Intel and AMD desktop or server processors manufactured since 2012
support SSE4.1 and PCLMULQDQ, but the hypervisor must expose them.

Run the non-mutating check:

```sh
parano1d --check-hardware
```

## No peers

Check:

```sh
parano1d-cli peers
```

Confirm outbound DNS and TCP access, local time, and the P2P listener setting.
If accepting inbound connections, allow TCP `9600` in both host and upstream
firewalls.

Do not open RPC `9601` as a substitute for P2P.

## Mining button or process is unavailable

Mining needs:

- synchronization with the canonical tip;
- one authenticated peer;
- a ready embedded HistoryStep runtime;
- a valid payout owner;
- supported CPU instructions.

Use:

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli mining
```

An ordinary node started with `--mode node` never mines even if an old TOML
file contains `mining.enabled = true`.

## Synchronization appears stuck

Inspect whether the node is validating headers, verifying a terminal,
transferring segments or applying the recent suffix. These phases have
different CPU, network and disk profiles.

Try more than one healthy peer before treating unchanged height as corruption.
An interrupted snapshot staging directory is discarded automatically at
restart.

## Address balance is missing after import

Automatic discovery stops at the first empty derived address and checks at most
20 funded candidates. If an older wallet skipped an unused index, generate the
later addresses explicitly.

Receipts are local artifacts and are not restored by importing the key.

## Send was rejected

Common causes are:

- transaction epoch changed;
- an input was spent by a new block;
- active address changed;
- output-slot hint became occupied;
- fee quote is below current policy;
- insufficient balance after fee;
- too many selected inputs.

Refresh the wallet and construct a new intent. Do not edit serialized intent
bytes; the authorization binds the complete logical transaction.

## Clean shutdown takes time

Mining and proof tasks stop at safe cancellation boundaries. The GUI waits for
the private node to close networking and flush MDBX. A short delay is expected
under all-core work.

If a service does not stop after its configured timeout, preserve the logs
before forcing it. Repeated hard kills can turn an ordinary delay into database
recovery work.

## Port already in use

Find which process owns `9600` or `9601`. Only one node may bind a listener and
one process may own a data directory.

Use distinct listener ports and distinct data directories for independent
instances. A GUI-owned private node can also conflict with a manually started
Core node on the defaults.

## Report a reproducible fault

Include:

- release version and operating system;
- CPU model and `--check-hardware` output;
- exact command or GUI action;
- relevant log lines;
- whether the node was synchronized and mining;
- expected and observed result.

Never include `wallet.key`, the exported master secret, private photo material
or bearer tokens.

## Contract balance or history is missing

Refresh against a synchronized node and inspect related saved states. A call
may have moved value to a successor address or closed the instance. The local
journal contains saved and imported evidence; being offline during a call can
leave a gap after its block body is pruned. Import the other party's contract
receipt through F7 Open file. Existing local records are preserved. A plain
master-secret import cannot reconstruct lost public contract terms. See
[recovery](../contracts/receipts-and-recovery.md).

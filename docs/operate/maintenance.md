# Maintenance

Parano1d persists Live State transactionally. Routine maintenance does not
replay or retain historical block bodies.

## Observe

Use the local CLI:

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli state
parano1d-cli mempool
parano1d-cli mining
```

With systemd:

```sh
systemctl status parano1d
journalctl -u parano1d --since today
```

Monitor disk space for the live MDBX State and proof cache, not an assumed
fixed allocation. State storage follows current UTXO usage.

## Stop before file operations

Always stop the process before copying its database or replacing binaries:

```sh
sudo systemctl stop parano1d
```

Wait for the service to become inactive. Do not copy a live MDBX directory
with a generic file-copy tool and assume the result is a consistent backup.

## Back up wallet authority

The critical file is:

```text
DATA_DIR/wallet.key
```

It contains the unencrypted 256-bit master secret. Copy it to an offline,
access-controlled location and preserve owner-only permissions.

For payment evidence, also back up:

```text
DATA_DIR/wallet.receipts
```

The P2P identity file preserves the node's stable peer ID but has no spending
authority.

## Update

1. download the new archive and matching `SHA256SUMS`;
2. verify its digest;
3. stop the service;
4. replace `parano1d`, `parano1d-cli` and `parano1d-miner` together;
5. start the service;
6. inspect startup and synchronization.

Do not delete the data directory during a normal update.

## Rebuild chain data

If State corruption is suspected and a healthy peer network is available:

```sh
parano1d --purge-state
```

This clears the complete chain database, including headers, retained blocks,
indexes, undo data and Live State, then forces authenticated synchronization.
Wallet files, receipts and peer identity are stored separately and remain, but
backing up the wallet key and receipts first is still prudent. The operation is
recovery, not routine cleanup.

## Snapshot staging

Incoming snapshots are scratch data. An interrupted transfer is discarded on
the next start before a new staging session begins. Do not manually promote
files from `snapshot-staging` into the canonical database.

## Database and finality

Undo data is retained for 36 blocks and complete block bodies for 42. These
windows are managed automatically. Increasing local disk retention does not
change the 18-block consensus finality rule.

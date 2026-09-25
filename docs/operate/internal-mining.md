# Internal mining

Internal mining keeps transaction selection, proof construction, nonce search
and block submission in one Core process.

## Prepare

Run the production hardware check:

```sh
parano1d --check-hardware
```

Obtain a payout address from the node wallet:

```sh
parano1d
parano1d-cli address --list
parano1d-cli stop
```

The active wallet address is used automatically when no explicit payout is
configured.

## Start

In the foreground:

```sh
parano1d --mode miner --cpu-threads 12
```

Or update a systemd unit:

```ini
ExecStart=/usr/local/bin/parano1d \
  --config /etc/parano1d/parano1d.toml \
  --mode miner \
  --cpu-threads 12
```

Then reload and restart:

```sh
sudo systemctl daemon-reload
sudo systemctl restart parano1d
```

## Readiness

Ordinary mining requires one authenticated peer and a synchronized chain.
Check:

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli mining
```

The process selects the supported CPU backend and uses Small m23 by default:
63 pages, 504 inputs, 63 calls. A server can permit Large m24 (206 pages,
the same input/call limits) with:

```sh
parano1d --mode miner --v2-large-blocks
```

Large is selected when the eligible set yields more claimable fees. The flag
permits it; Small remains available. Benchmark the intended host. The GUI has
no Large control. See [class selection](../architecture/mining.md).

## CPU planning

`--cpu-threads` is the total shared budget for proof and PoW phases. Do not set
it higher than the logical CPUs available to the service's cgroup or virtual
machine.

Leave capacity for the operating system and public P2P service on an
infrastructure node. A dedicated miner can use every visible logical CPU.

Wallet transaction proving has local priority over ongoing mining work. This
does not change the transactions or blocks accepted by other nodes.

## Payout changes

The active wallet address is resolved for each new template. Changing it
invalidates or refreshes local work at a safe boundary. An already immutable
template cannot have its payout rewritten.

To pin a separate payout for the process:

```sh
parano1d --mode miner --miner-address o1...
```

Use the complete bech32m address.

## Stop

Stop through RPC or the service manager:

```sh
parano1d-cli stop
```

```sh
sudo systemctl stop parano1d
```

Graceful shutdown cancels mining, closes networking and flushes MDBX. Do not
send repeated hard-kill signals merely because an active proof takes several
seconds to reach its cancellation boundary.

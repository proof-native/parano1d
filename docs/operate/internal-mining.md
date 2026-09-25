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

The process selects the best available CPU backend. Before v2, it prepares
the embedded B25 and B255 matrices and each mining session starts with B25.
Its first completed preparation permits B255 if `prepare_time_B25 × 4 ≤ 20 seconds`.
Eligible page count then determines whether a template needs the larger class.

At mainnet H210537, default production switches automatically to Small `m=23`
with 63 pages, 504 inputs and up to 63 contract calls. A server can permit
Large `m=24`, with 206 pages and the same input/call budgets:

```sh
parano1d --mode miner --v2-large-blocks
```

The flag permits a larger selection when it yields more claimable fees; Small
remains available. The GUI does not expose this flag. Benchmark the intended
host before allowing Large. See
[Mining architecture](../architecture/mining.md#scheduled-v2-profile).

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

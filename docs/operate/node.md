# Run a node on Linux

An ordinary Parano1d node verifies complete blocks, maintains the live UTXO
State, relays transactions and serves synchronization data. It does not mine.

This guide installs the official Core release as a system service. It assumes a
64-bit Linux server with systemd.

## Requirements

The production proof backend requires:

- x86-64 with SSE4.1 and PCLMULQDQ; or
- ARM64 with NEON and PMULL.

Runtime dispatch selects the `pclmul`, `avx2+vpclmul`,
`avx512bw+vpclmul` or `neon+pmull` backend automatically. The scalar reference
backend is not used by a production node.

The node listens for P2P connections on TCP port `9600`. Its JSON-RPC endpoint
should remain bound to `127.0.0.1:9601`.

The dominant mutable storage follows the live UTXO set. Compact 212-byte
headers remain permanent, while complete block bodies are retained only for
the latest 42 blocks.

Read [Hardware and capacity](hardware.md) before ordering a virtual machine or
choosing disk and memory limits.

## Install the Core release

Download the archive for the server architecture and `SHA256SUMS` from the
[release page](https://github.com/ignotusnemo/parano1d/releases). Verify the
archive before extracting it. Replace `VERSION` with the release number:

```sh
grep '  parano1d-core-vVERSION-linux-x86_64.tar.gz$' SHA256SUMS \
  | sha256sum --check
```

The command must report `OK`. For ARM64, replace `linux-x86_64` with
`linux-aarch64`.

Extract the archive and run the hardware check:

```sh
tar -xzf parano1d-core-vVERSION-linux-x86_64.tar.gz
./parano1d --check-hardware
```

A supported machine ends with:

```text
NODE READY
```

Install the node and CLI:

```sh
sudo install -m 0755 parano1d parano1d-cli /usr/local/bin/
```

## Create the service account

Keep node data separate from interactive user accounts:

```sh
sudo useradd --system --home-dir /var/lib/parano1d \
  --create-home --shell /usr/sbin/nologin parano1d
sudo install -d -o parano1d -g parano1d -m 0700 /var/lib/parano1d
sudo install -d -o root -g parano1d -m 0750 /etc/parano1d
```

Create `/etc/parano1d/parano1d.toml`:

```toml
[network]
listen = "0.0.0.0:9600"
seeds = []

[storage]
backend = "mdbx"
path = "/var/lib/parano1d"

[rpc]
listen = "127.0.0.1:9601"

[mining]
enabled = false
miner_address = ""
```

Protect the configuration:

```sh
sudo chown root:parano1d /etc/parano1d/parano1d.toml
sudo chmod 0640 /etc/parano1d/parano1d.toml
```

No seed address is required. The released binary discovers the public network
through its built-in DNS seeds and remembers successful outbound peers.

## Run under systemd

Create `/etc/systemd/system/parano1d.service`:

```ini
[Unit]
Description=Parano1d node
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=parano1d
Group=parano1d
ExecStart=/usr/local/bin/parano1d --config /etc/parano1d/parano1d.toml
Restart=on-failure
RestartSec=5
KillSignal=SIGINT
TimeoutStopSec=45
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

`KillSignal=SIGINT` gives the node time to close its network services and flush
MDBX cleanly.

Load the unit and start the node:

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now parano1d
sudo systemctl status parano1d
```

Follow startup and synchronization:

```sh
sudo journalctl -u parano1d -f
```

## Check the node

The CLI connects to the local RPC endpoint by default:

```sh
parano1d-cli status
parano1d-cli peers
parano1d-cli state
```

`status` should report the current height, `peers` should become non-zero, and
`state` reports the authenticated Live State dimensions.

## Network access

Allow inbound TCP `9600` in the host and provider firewalls. If the server is
behind NAT, forward TCP `9600` to it. A node can synchronize through outbound
connections without an inbound route, but accepting inbound peers makes it
useful to the network.

Do not expose TCP `9601` publicly. Remote administration should use an SSH
tunnel or another authenticated private transport.

## Stop or update

Stop the service before replacing binaries or copying its data:

```sh
sudo systemctl stop parano1d
```

Install the verified replacement binaries, then restart:

```sh
sudo install -m 0755 parano1d parano1d-cli /usr/local/bin/
sudo systemctl start parano1d
parano1d-cli status
```

Do not remove `/var/lib/parano1d` during an ordinary software update. If the
node's wallet receives funds, back up `/var/lib/parano1d/wallet.key`
separately and protect it as a private secret.

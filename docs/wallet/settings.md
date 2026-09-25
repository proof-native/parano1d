# Settings

Open Settings with `F8`.

![Wallet settings](../assets/wallet/settings.png)

## Secret

The Secret tab shows the active master-secret mode and exposes explicit
maintenance actions:

- export the current 64-character master secret;
- import an existing secret;
- replace it from photo pixels;
- generate a new secret.

Replacement changes every derived address. Export the current secret first if
it controls funds you intend to keep.

The secret field is not a normal application preference. Operations stop the
private node, update the owner-only key file atomically and restart wallet
state.

## Node

Node settings control:

- data directory;
- log level;
- node restart when a restart-bound value changes.

The terminal-style log view shows a bounded tail of the real private-node log.
Text can be selected and copied for diagnosis. Viewing it does not create a
second persistent log store.

Changing the data directory points the application at a different database and
wallet. Verify the target before applying.

## Network

Network settings control:

- P2P listen address;
- additional seed endpoints.

The default P2P listener is `0.0.0.0:9600`. Built-in DNS discovery works
without custom entries.

Do not expose the private RPC endpoint as a network setting. The GUI and node
communicate locally.

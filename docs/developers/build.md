# Build from source

The workspace is Rust 2021 and pins Rust `1.96.0`. Native dependencies are
needed for MDBX, proof code and GUI packaging.

## Host requirements

All platforms need:

- the pinned Rust toolchain with `rustfmt`;
- a native C/C++ compiler;
- CMake;
- libclang;
- Git.

On Debian or Ubuntu:

```sh
sudo apt update
sudo apt install --no-install-recommends \
  build-essential clang libclang-dev cmake pkg-config
```

The Linux GUI package additionally needs `appstreamcli` and `dpkg-deb`.
Windows release packaging uses Inno Setup 6. macOS packaging uses the standard
`codesign`, `iconutil` and `hdiutil` tools.

The repository's `rust-toolchain.toml` selects the compiler automatically:

```sh
rustup show active-toolchain
rustc --version
cargo --version
```

## Check the workspace

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
```

Build ordinary development binaries:

```sh
cargo build --locked \
  -p noid_node \
  -p noid-extminer \
  -p noid_gui \
  --bins
```

Development binaries exercise parsing, UI and non-production test paths. A
block-producing release requires the authenticated HistoryStep matrix pack
described below.

## Reproduce the soundness certificate

The production calculations and proof documents are in
[`noid_soundness`](https://github.com/ignotusnemo/parano1d/tree/main/noid_soundness).

```sh
cargo run --release --locked -p noid_soundness
cargo run --release --locked -p noid_soundness -- --exact
cargo test --release --locked -p noid_soundness
```

## Legacy proof pack

The canonical pack contains:

```text
v1/history-step.runtime
v1/history-step-c00.field-r1cs.zst
v1/history-step-c01.field-r1cs.zst
pins.env
SHA256SUMS
```

The v2 transition keeps the published B25 and B255 matrices unchanged. Reuse
the authenticated legacy pack. The generator is retained for reproduction:

```sh
mkdir -p ../parano1d-artifacts
./scripts/generate_history_step_pack.sh \
  ../parano1d-artifacts/history-step-pack-v1
```

Generation is expensive and only needs to be performed once for an unchanged
relation. Keep the pack outside `target/`.

The script writes to a staging directory, derives semantic pins, authenticates
every artifact and publishes the completed directory atomically. It refuses to
overwrite an existing output path.

## Scheduled v2 material

The v2 branch also needs a frozen joint bank matching the source activation
height, and both independently authenticated legacy preprocessing keys. An H10
isolated-network pack cannot be embedded in a mainnet executable: the node's
build checks reject the schedule mismatch.

After choosing and qualifying the final capacities, the matrix tool can freeze
the mainnet relation before the real predecessor block exists:

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_capacity
target/release/noid_v2_capacity joint-freeze-mainnet \
  LEGACY_PACK LEGACY_METADATA_PIN NEW_OUTPUT \
  SMALL_PAGES SMALL_INPUTS SMALL_CALLS LARGE_INPUTS LARGE_CALLS
```

This mode requires the normal mainnet build profile. It reassembles both
matrices under the final pins and two hypothetical boundary witnesses, then
checks matrix identity and the transport bounds. It creates no verified fork
origin or accepted terminal. The resulting pack contains
`v2-runtime-metadata.bin`, `v2-small.field-r1cs.zst` and
`v2-large.field-r1cs.zst`; intermediate matrices and the generation report
remain beside them for reproducibility.

The preprocessing directory contains `class-0.key` and `class-1.key`. Its
reviewed key pins must come from recomputation against the canonical legacy
matrices. Prepare a separate pin file with exactly these three assignments,
each followed by its 64-character lowercase hexadecimal digest:

```text
NOID_V2_RELEASE_BANK=<frozen-bank-digest>
NOID_RETIREMENT_KEY_0_PIN=<authenticated-key-0-digest>
NOID_RETIREMENT_KEY_1_PIN=<authenticated-key-1-digest>
```

The release script parses this file as data. It never executes it as shell
code. The [v2 soundness tool](../../noid_soundness/docs/v2-retirement.md)
evaluates the exact bank and keys separately from artifact generation.

## Build native deliverables

```sh
./scripts/build_release.sh \
  --pack ../parano1d-artifacts/history-step-pack-v1 \
  --v2-pack PATH_TO_FROZEN_V2_PACK \
  --v2-pins PATH_TO_REVIEWED_PIN_FILE \
  --retirement-keys PATH_TO_AUTHENTICATED_KEYS
```

The script:

1. checks the supplied artifact layouts and reads explicit release pins;
2. embeds legacy material, authenticated v2 matrices and preprocessing keys;
3. rejects an incompatible bank or activation schedule during the node build;
4. builds Core, external miner and GUI;
5. smoke-tests every executable;
6. packages the Core archive and native GUI installer;
7. verifies archive membership and writes SHA-256 sums.

Source checks, tests and network qualification are separate pre-build steps.
The script records the proof pins and selected legacy-history profile in
`proof-pins.env` beside the build log.

Find the output:

```sh
cat target/release-builds/LAST_RELEASE
```

Use `--output PATH` to choose a fresh output directory.

For a later release, after an authenticated retirement certificate for the
selected fork origin is available over P2P, `--retired-history` omits legacy
matrix bytes from the node. That build accepts a legacy pack containing only
`v1/history-step.runtime` and its existing `pins.env`; neither old matrix file
is required. It still embeds both new matrices and independently checks the
certificate before accepting the old ancestry. The transition release keeps
the old matrices and is built without this flag.

## Portable binaries

x86-64 releases are compiled against a portable process baseline. After
checking the host, runtime dispatch selects `pclmul`, `avx2+vpclmul` or
`avx512bw+vpclmul`. ARM64 selects `neon+pmull`.

Do not compile official artifacts with `target-cpu=native`. That would make the
binary depend on the build machine before runtime hardware checks can run.

## Reproducible archive details

On GNU tar hosts, `SOURCE_DATE_EPOCH` controls member timestamps and defaults
to zero. The Core archive has a fixed member set:

```text
README.txt
LICENSE
NOTICE
parano1d
parano1d-cli
parano1d-miner
```

The GUI package contains only the application and its private node. It does not
include operator CLI or external-mining tools.

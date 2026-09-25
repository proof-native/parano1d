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

## Production proof material

A production v2 build needs the authenticated joint bank and old-ancestry
verification material. The source schedule is mainnet H210537, with Small
63/504/63 and Large 206/504/63. An H10 test-network bank fails the mainnet build
checks. Keep expensive generated artifacts outside the disposable `target/` tree.

The transition release reuses the unchanged historical pack. Its layout and
reproduction command are:

```text
v1/history-step.runtime
v1/history-step-c00.field-r1cs.zst
v1/history-step-c01.field-r1cs.zst
pins.env
SHA256SUMS
```

```sh
mkdir -p ../parano1d-artifacts
./scripts/generate_history_step_pack.sh \
  ../parano1d-artifacts/history-step-pack-v1
```

Generation writes a fresh staging directory, derives pins, authenticates the
artifacts and publishes atomically. Existing output directories are not overwritten.

Freeze the v2 relation from source under the mainnet schedule:

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_capacity
target/release/noid_v2_capacity joint-freeze-mainnet \
  LEGACY_PACK LEGACY_METADATA_PIN NEW_OUTPUT \
  63 504 63 504 63 --large-pages=206
```

Replace uppercase placeholders with actual paths and independently checked
pins. This mode assembles both matrices under hypothetical boundary witnesses
and checks their identity and transport bounds. The real fork origin is obtained
at the boundary. The pack includes `v2-runtime-metadata.bin`,
`v2-small.field-r1cs.zst` and `v2-large.field-r1cs.zst`.

The preprocessing directory contains `class-0.key` and `class-1.key`, derived
and authenticated against the canonical historical matrices. A separate pin
file contains exactly three assignments:

```text
NOID_V2_RELEASE_BANK=c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e
NOID_RETIREMENT_KEY_0_PIN=<authenticated-key-0-digest>
NOID_RETIREMENT_KEY_1_PIN=<authenticated-key-1-digest>
```

Use the independently recomputed key digests. The script parses this file as
data; it does not execute shell code. The [final-bank record](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/v2_feasibility/results/2026-09-25-common-input-budget/REPORT.md)
links generation, authentication and qualification evidence.

## Soundness reproduction

The v2 tool evaluates the actual bank, both keys and historical ancestry:

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_soundness
target/release/noid_v2_soundness \
  LEGACY_METADATA LEGACY_METADATA_PIN V2_METADATA V2_BANK_PIN \
  CLASS_0_KEY CLASS_0_KEY_PIN CLASS_1_KEY CLASS_1_KEY_PIN
```

The [derivation](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/noid_soundness/docs/v2-retirement.md)
explains its assumptions. The default `noid_soundness` executable retains the
archived profile's calculation; use the v2 tool for this bank.

## Native deliverables

Run source checks and [qualification](testing.md) before packaging:

```sh
./scripts/build_release.sh \
  --pack ../parano1d-artifacts/history-step-pack-v1 \
  --v2-pack PATH_TO_FROZEN_V2_PACK \
  --v2-pins PATH_TO_REVIEWED_PIN_FILE \
  --retirement-keys PATH_TO_AUTHENTICATED_KEYS

cat target/release-builds/LAST_RELEASE
```

The release script validates input layouts and pins, embeds authenticated
material, rejects a mismatched schedule, builds Core and GUI, smoke-tests the
executables and verifies package membership and SHA-256 sums. It does not run
the entire protocol test suite. `proof-pins.env` records the embedded identities.
Use `--output PATH` for a fresh output directory.

A later `--retired-history` build omits old matrix bytes once the selected
origin's authenticated certificate is available over P2P. It still requires the
two new matrices, old runtime metadata, independently pinned preprocessing keys
and full certificate verification. Its historical pack may contain only
`v1/history-step.runtime` and `pins.env`. Build the transition release without
that flag so it can cross the boundary using the original matrices.

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
CONTRACTS.md
LICENSE
NOTICE
parano1d
parano1d-cli
parano1d-miner
```

The GUI package contains only the application and its private node. It does not
include operator CLI or external-mining tools.

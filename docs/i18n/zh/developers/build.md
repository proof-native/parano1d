# 从源码构建

工作区使用 Rust 2021，并固定 Rust `1.96.0`。MDBX、证明代码和 GUI
打包需要原生依赖。

## 主机要求

所有平台都需要：

- 固定版本的 Rust 工具链，以及 `rustfmt`；
- 原生 C/C++ 编译器；
- CMake；
- libclang；
- Git。

Debian 或 Ubuntu：

```sh
sudo apt update
sudo apt install --no-install-recommends \
  build-essential clang libclang-dev cmake pkg-config
```

Linux GUI 包还需要 `appstreamcli` 和 `dpkg-deb`。Windows 发布打包使用
Inno Setup 6，macOS 使用标准的 `codesign`、`iconutil` 和 `hdiutil`。

仓库中的 `rust-toolchain.toml` 会自动选择编译器：

```sh
rustup show active-toolchain
rustc --version
cargo --version
```

## 检查工作区

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
```

构建普通开发二进制：

```sh
cargo build --locked \
  -p noid_node \
  -p noid-extminer \
  -p noid_gui \
  --bins
```

开发版二进制文件可测试解析、UI 和非生产用证明路径。能够生产区块的发布版需要
下文所述经过认证的 HistoryStep 矩阵包。

## 生产证明材料

v2 生产构建需要认证联合矩阵库及旧祖先链验证材料。源码计划为主网 H210537，
Small 63/504/63，Large 206/504/63。H10 测试库无法通过主网构建检查。
请将昂贵生成的工件保存在可清理的 `target/` 目录之外。

过渡版本复用不变的历史包，其结构和复现命令如下：

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

生成写入新的暂存目录，推导固定摘要、认证工件，再原子发布，不覆盖已有目录。

从主网计划源码固定 v2 关系：

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_capacity
target/release/noid_v2_capacity joint-freeze-mainnet \
  LEGACY_PACK LEGACY_METADATA_PIN NEW_OUTPUT \
  63 504 63 504 63 --large-pages=206
```

请将大写占位符替换为真实路径及独立检查的摘要。该模式使用假设边界见证构建
两套矩阵并检查身份及传输上限，真实分叉来源在边界获得。包包含
`v2-runtime-metadata.bin`、`v2-small.field-r1cs.zst` 和 `v2-large.field-r1cs.zst`。

预处理目录包含从认证规范历史矩阵推导并认证的 `class-0.key` 和 `class-1.key`。
单独固定值文件准确包含三项赋值：

```text
NOID_V2_RELEASE_BANK=c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e
NOID_RETIREMENT_KEY_0_PIN=<authenticated-key-0-digest>
NOID_RETIREMENT_KEY_1_PIN=<authenticated-key-1-digest>
```

使用独立重算的密钥摘要。脚本将文件作为数据读取，不执行 shell 代码。
[最终库记录](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/v2_feasibility/results/2026-09-25-common-input-budget/REPORT.md)
链接生成、认证及验证证据。

## 复现安全计算

v2 工具评估实际矩阵库、两个密钥及历史祖先链：

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_soundness
target/release/noid_v2_soundness \
  LEGACY_METADATA LEGACY_METADATA_PIN V2_METADATA V2_BANK_PIN \
  CLASS_0_KEY CLASS_0_KEY_PIN CLASS_1_KEY CLASS_1_KEY_PIN
```

[推导](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/noid_soundness/docs/v2-retirement.md)
解释其假设。默认 `noid_soundness` 可执行程序保留归档配置计算，此矩阵库应使用 v2 工具。

## 原生发布物

打包前执行源码检查及[验证](testing.md)：

```sh
./scripts/build_release.sh \
  --pack ../parano1d-artifacts/history-step-pack-v1 \
  --v2-pack PATH_TO_FROZEN_V2_PACK \
  --v2-pins PATH_TO_REVIEWED_PIN_FILE \
  --retirement-keys PATH_TO_AUTHENTICATED_KEYS

cat target/release-builds/LAST_RELEASE
```

发布脚本验证输入布局和摘要，嵌入认证材料，拒绝不匹配的计划，构建 Core 和 GUI，
执行二进制冒烟检查，验证包成员和 SHA-256。完整协议测试套件单独运行。
`proof-pins.env` 记录嵌入身份，`--output PATH` 选择新输出目录。

后续 `--retired-history` 构建在所选来源认证证书可通过 P2P 获取后移除旧矩阵
字节，仍需两套新矩阵、旧运行时元数据、独立固定的预处理密钥及完整证书验证。
历史包可只包含 `v1/history-step.runtime` 和 `pins.env`。过渡版本不使用此标志，
以便通过原矩阵跨越边界。

## 可移植二进制

x86-64 版本针对可移植的指令集基线编译。检查主机后，运行时分派机制
会选择 `pclmul`、`avx2+vpclmul` 或 `avx512bw+vpclmul`。ARM64 选择
`neon+pmull`。

发布产物不得使用 `target-cpu=native` 构建，否则二进制会在运行时硬件
检查之前就依赖构建机器。

## 可复现归档细节

在 GNU tar 主机上，`SOURCE_DATE_EPOCH` 控制成员时间戳，默认值为零。
Core 归档成员固定为：

```text
README.txt
CONTRACTS.md
LICENSE
NOTICE
parano1d
parano1d-cli
parano1d-miner
```

GUI 包只包含应用及其私有节点，不包含运营 CLI 或外部挖矿工具。

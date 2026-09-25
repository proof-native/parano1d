# 历史性能 — v1 / v1.1

本页保留旧配置的测量结果。当前 v2 测量见[性能](../reference/performance.md)。

性能数据只对特定源码修订、证明配置、经过认证的矩阵包、构建配置和主机成立。它不是共识常量，也不能仅由核心数量推导。

下表是历史构造耗时测量，使用 Parano1d 修订版 `39626b22d53cf2f2c480a7e28446c197dca68043`、实际部署的 C1 配置以及经过认证的 B25/B255 矩阵包，早于 v1.1 共享路径编码。这些数据可作为后续容量实验（包括 v2）的硬件基线；新的证明形状和完整出块流程仍需单独测量。

| 主机 | 类别 | `HistoryStep` 构造 | 统计量 |
|---|---|---:|---|
| 低成本 AVX2 笔记本电脑，12 线程 | B25 / `m=22` | **10.734 秒** | 3 次测量的 p50 |
| 低成本 AVX2 笔记本电脑，12 线程 | B255 / `m=24` | **34.938 秒** | 1 次隔离式测量 |
| AVX-512 PC，24 线程 | B25 / `m=22` | **6.905 秒** | 3 次测量的 p50 |
| AVX-512 PC，24 线程 | B255 / `m=24` | **21.053 秒** | 3 次测量的 p50 |

[原始测量记录](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/two_class/results/2026-08-06-history-step-b25-b255.md)保留了该修订版展开后的终端大小。这些大小不等于 v1.1 网络载荷；上述耗时不包含共享路径编解码开销。

表中不包含 PoW nonce 搜索。ASERT 的目标是已接受区块之间的完整时间间隔，而不是为 nonce 搜索单独分配 20 秒。证明准备、nonce 搜索和网络传播共同占用同一个观测到的区块间隔，ASERT 根据这一完整节奏调整 nonce 目标。

## v1.1 终端大小

共享路径编码只存储和传输一次公共认证节点。2026-09-24 的编解码审计使用实际部署的 C1 配置和经过认证的 B25/B255 矩阵包，测量同一已验证证明的两种表示：

| 类别 | 展开路径 | v1.1 共享路径 | 减少 |
|---|---:|---:|---:|
| B25 / `m=22` | 971,732 字节 | **874,516 字节** | 10.00% |
| B255 / `m=24` | 1,081,108 字节 | **982,100 字节** | 9.16% |

两个样本都小于 1 MB（1,000,000 字节）。它们是单个证明的示例，不是固定大小或上界；共享路径大小取决于查询打开位置。另一个已验证的主网 B25 终端（高度 137191）为 872,500 字节。编码后与展开后的共识上限仍为 1,100,000 字节。上述大小不含区块体、传输帧或 RPC 十六进制文本。

[编解码测量记录](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/two_class/results/2026-09-24-terminal-shared-paths.md)包含复现命令和验证结果。这次审计测量表示方式节省的字节数，没有重新测试历史 AVX-512 主机。

## 节点状态处理

节点保留已认证的分段列及其精确 Merkle 树的有界缓存。每个状态视图的数据预算为 64 MiB，包括列数据，可容纳九个实际部署尺寸的分段。副本在修改前共享不可变数据。处理当前区块的临时内存不计入此保留缓存预算。

缓存中的槽更新只重新计算发生变化的路径。加载未缓存的分段时，节点先根据精确根验证整个分段。提交后释放缓存外的列；重启和安装快照后按需重建缓存。这一策略适用于整个槽空间。频繁访问不同分段的负载仍需支付完整认证成本，应与反复更新同一分段的负载分开测量。

手动基准覆盖密集分段、八个和十六个被访问分段，以及跨三十二个分段的重复缓存未命中。每个场景的最终根都与流式参考实现比较。计时包括加载认证和本地根更新，不包括磁盘提交、HistoryStep 验证或 PoW。第一轮从空缓存开始。

```sh
RAYON_NUM_THREADS=4 cargo test --locked --release -p noid_chain --lib \
  bench_exact_state_cache_cycles -- --ignored --nocapture --test-threads=1
```

比较两个二进制时应固定 CPU 亲和性、计算后端和构建配置。分别记录不同负载，以及冷启动和缓存预热后的结果。

## 钱包授权

钱包基准程序测量页面构建、逻辑哈希、一个授权胶囊、完整交易意图编解码以及本地胶囊接纳。不包含网络延迟和区块 `HistoryStep` 证明。

```sh
NOID_WALLET_BENCH_SAMPLES=20 cargo run --release --locked \
  --manifest-path research/two_class/Cargo.toml \
  --bin two-class-wallet-bench
```

实际部署的 C1 钱包使用 65 个 Fiat–Shamir 查询。一个 `PagedSpend` 无论占用一页还是完整的 128 页，都只包含一个授权胶囊。规范序列化授权的最坏情况上界为 92,696 字节。

## HistoryStep

隔离式实际部署基准测试需要完整且经过认证的矩阵包。分别运行两个类别，以便输出明确标识父类别和子类别。

```sh
NOID_PACK_ROOT=../parano1d-artifacts/history-step-pack-v1
source "$NOID_PACK_ROOT/pins.env"
export NOID_HISTORY_STEP_RUNTIME_METADATA_RELEASE_DIGEST
export NOID_HISTORY_STEP_PACK_LEAF_DIGESTS
export NOID_HISTORY_STEP_PACK_DIR="$NOID_PACK_ROOT"

NOID_HISTORY_STEP_BENCH_FILTER=B25 \
NOID_HISTORY_STEP_BENCH_SAMPLES=20 \
cargo bench --locked -p bench_prover --bench history_step_proof

NOID_HISTORY_STEP_BENCH_FILTER=B255 \
NOID_HISTORY_STEP_BENCH_SAMPLES=20 \
cargo bench --locked -p bench_prover --bench history_step_proof
```

`cargo bench` 使用优化的 `bench` 配置。交易构建、钱包证明、区块模板构建和矩阵认证都属于测量前的准备工作。`history_step_ms` 包含父 terminal 解码、有界输入和授权准备、递归组装、nonce 封存、证明构建和 terminal 编码。`verify_ms` 包含有界线格式解码和完整 terminal 验证。

## 端到端区块生产

隔离式证明测量不等于完整挖矿延迟。容量决策必须测量：

```text
选择交易意图
  + 组装当前区块轨迹
  + 重放并绑定父 terminal
  + 证明 HistoryStep
  + 搜索 nonce
  + 提交并接受区块
```

nonce 搜索和网络传播与证明构建独立变化。端到端比较必须在最终主机上测量完整生产路径。自动 B255 许可仅使用第一次完成的 B25 准备时间及四倍预测，详见[挖矿架构](../architecture/mining.md)。官方二进制保留可移植基线，并在运行时选择 `pclmul`、`avx2+vpclmul`、`avx512bw+vpclmul` 或 `neon+pmull` 后端。

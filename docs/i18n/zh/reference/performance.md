# 性能测量

性能数据只对特定源码修订、证明配置、经过认证的矩阵包、构建配置和主机成立。它不是共识常量，也不能仅由核心数量推导。

下表使用 Parano1d 修订版 `39626b22d53cf2f2c480a7e28446c197dca68043`、实际部署的 C1 配置以及经过认证的 B25/B255 矩阵包。表中只包含隔离式实际部署基准测试。

| 主机 | 类别 | `HistoryStep` 构造 | 统计量 | 展开后的终端 |
|---|---|---:|---|---:|
| 低成本 AVX2 笔记本电脑，12 线程 | B25 | **10.734 秒** | 3 次测量的 p50 | 971,732 字节 |
| 低成本 AVX2 笔记本电脑，12 线程 | B255 | **34.938 秒** | 1 次隔离式测量 | 1,081,108 字节 |
| AVX-512 PC，24 线程 | B25 | **6.905 秒** | 3 次测量的 p50 | 971,732 字节 |
| AVX-512 PC，24 线程 | B255 | **21.053 秒** | 3 次测量的 p50 | 1,081,108 字节 |

表中终端大小对应完整认证路径。共享路径编码可减少存储与传输字节，精确大小取决于打开位置。这些测量不包含共享路径编解码开销。

表中不包含 PoW nonce 搜索。ASERT 的目标是已接受区块之间的完整时间间隔，而不是为 nonce 搜索单独分配 20 秒。证明准备、nonce 搜索和网络传播共同占用同一个观测到的区块间隔，ASERT 根据这一完整节奏调整 nonce 目标。

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

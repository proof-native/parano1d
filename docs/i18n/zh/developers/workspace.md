# 工作区

仓库按证明边界与信任边界拆分，而不是按二进制文件拆分。

## 算术与证明基础

| Crate | 职责 |
|---|---|
| `noid_core` | 二进制塔域、向量化内核与 CPU 运行时分派 |
| `noid_poseidon2b` | Poseidon2b 置换、域、哈希与批量执行 |
| `noid_fri` | FRI 原语 |
| `noid_fri_binius` | 二进制域 FRI-Binius/BaseFold 集成 |
| `noid_ivc_core` | 递归公共 I/O 与验证者基础组件 |
| `noid_ivc_prover` | 递归证明者实现 |
| `noid_gkr` | [FROST-GKR](../research/frost-gkr.md) 关系与钱包授权 |
| `noid_recursive` | `HistoryStep`、精确 State 关系与递归接受 |
| `noid_soundness` | 与源码绑定的 Block–Tiwari、QROM 与 Category 1 证书 |
| `bench_prover` | 矩阵生成、固定值工具与证明基准测试 |

## 协议对象

| Crate | 职责 |
|---|---|
| `noid_tx` | 固定 `Tx8x2`、逻辑 `PagedSpend`、ID 与授权绑定 |
| `noid_block` | 区块级证明组合 |
| `noid_chain` | 区块头、共识、State、费用、收据、MDBX 与快照 |

`noid_tx` 包含无需链上下文即可检查的表示层规则。`noid_chain` 再加入当前
周期、Live State、发行、分配和分叉上下文。

## 运行时

| Crate | 职责 |
|---|---|
| `noid_mempool` | 交易意图准入、CPU 许可、冲突与选择元数据 |
| `noid_miner` | 共享 CPU 计划、模板构建、证明和 PoW |
| `noid_p2p` | libp2p 发现、Gossip 转发、同步与资源限制 |
| `noid_rpc` | 类型化 JSON-RPC API 与钱包操作 |
| `noid_node` | 守护进程、CLI、钱包状态与子系统协调 |
| `noid_gui` | 原生多语言钱包及私有节点管理 |
| `noid_extminer` | 外部 Poseidon2b nonce 搜索进程 |

## 依赖方向

预期方向为：

```text
field / hashes / proof primitives
            ↓
transaction and block relations
            ↓
chain consensus and storage
            ↓
mempool / miner / P2P / RPC
            ↓
node and GUI applications
```

底层 crate 不调用 GUI、RPC 或网络策略。共识类型不依赖钱包标签或应用
展示。

## 共识敏感修改

修改以下任意内容都属于共识敏感：

- 规范字节编码；
- Poseidon2b 字段顺序或域标签；
- 交易或区块有效性；
- State 根派生；
- 发行、费用销毁或分配；
- 难度、时间戳、最终性或扩展规则；
- `HistoryStep` 关系或认证矩阵。

此类修改需要更新测试向量、关系测试、矩阵包和嵌入的固定值。只有新矩阵
包而没有对应源码语义，不能构成升级机制。

## 仅应用层的修改

翻译、布局、钱包标签、日志展示和普通 RPC 响应格式都在共识之外，前提是
不改变提交给节点的序列化对象。

钱包选币同样属于策略。最终 `PagedSpend` 仍必须满足与其他实现构建交易
完全相同的共识规则。

## 合约组件

`noid_tx::experimental_object` 定义规范公开条款、程序、策略及调用编码；
`noid_tx::experimental_object::applications` 构造模板。`noid_recursive` 在区块关系内证明其执行。
`noid_chain` 处理当前 State 检查、回执和分叉上下文；`noid_rpc` 提供类型化
构造与钱包调用。`noid_gui` 管理表单、本地名称及文件导入合并。

使用现有 ABI 的新程序可复用同一矩阵。更改操作码、操作数含义、权限规则或
执行预算会改变证明关系，需要审查共识与矩阵。条款文件及回执日志属于应用
数据，不能覆盖已承诺程序或当前 State。
